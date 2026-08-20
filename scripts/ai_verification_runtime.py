"""Fail-closed planning and execution of reusable verification evidence."""

from __future__ import annotations

import hashlib
import json
import time
from collections.abc import Callable, Mapping, Sequence
from dataclasses import asdict, dataclass
from datetime import UTC, datetime
from typing import Any, Literal

PROTECTED_GATE_CLASSES = frozenset({"security", "scope", "governance", "coverage", "source_bound"})
REUSE_CLASSES = frozenset({"content-bound", "diff-bound", "environment-bound", "none"})
REQUIRED_RECEIPT_DIGESTS = (
    "commandDigest",
    "scopeDigest",
    "governanceDigest",
    "environmentDigest",
    "toolchainDigest",
    "policyDigest",
    "outputDigest",
)


@dataclass(frozen=True)
class VerificationNode:
    """One check candidate and the explicit policy needed to reuse it."""

    node_id: str
    command: tuple[str, ...]
    gate_class: str
    required: bool
    scope: tuple[str, ...]
    depends_on: tuple[str, ...] = ()
    reuse_class: str = "none"
    binding_classes: tuple[str, ...] = ()
    reuse_allowed: bool = False
    protected: bool = False


@dataclass(frozen=True)
class PlannedCheck:
    """One fail-closed execution decision."""

    node_id: str
    action: Literal["execute", "skip_reused"]
    decision_state: Literal["fresh", "stale", "unknown", "not_applicable"]
    status: Literal["planned", "skipped"]
    reason_code: str
    receipt_id: str | None
    satisfied_by: Literal["execution", "reused_receipt", "none"]
    required: bool
    protected: bool


@dataclass(frozen=True)
class VerificationPlan:
    """Deterministic plan plus auditable node-count metrics."""

    checks: tuple[PlannedCheck, ...]
    metrics: dict[str, int]

    def to_dict(self) -> dict[str, Any]:
        return {
            "schemaVersion": 1,
            "checks": [asdict(check) for check in self.checks],
            "metrics": dict(self.metrics),
        }


def _digest(value: object) -> str:
    payload = json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
    return hashlib.sha256(payload.encode("utf-8")).hexdigest()


def _parse_time(value: object) -> datetime:
    if not isinstance(value, str) or not value:
        raise ValueError("receipt timestamp is missing")
    timestamp = value.removesuffix("Z") + "+00:00" if value.endswith("Z") else value
    parsed = datetime.fromisoformat(timestamp)
    return parsed if parsed.tzinfo is not None else parsed.replace(tzinfo=UTC)


def _now(value: datetime | str) -> datetime:
    if isinstance(value, datetime):
        return value if value.tzinfo is not None else value.replace(tzinfo=UTC)
    return _parse_time(value)


def _valid_digest(value: object) -> bool:
    return (
        isinstance(value, str)
        and len(value) == 64
        and all(character in "0123456789abcdef" for character in value)
    )


def _binding_digest(reuse_class: str, current_inputs: Mapping[str, object]) -> tuple[str, str]:
    if reuse_class == "content-bound":
        return "contentDigest", _digest(current_inputs["content"])
    if reuse_class == "diff-bound":
        return "diffDigest", _digest(current_inputs["diff"])
    if reuse_class == "environment-bound":
        return "environmentDigest", _digest(current_inputs["environment"])
    raise ValueError(f"unsupported reusable class: {reuse_class}")


def _binding_classes(node: VerificationNode) -> tuple[str, ...]:
    """Return the unique binding dimensions for one concrete checker node."""
    return tuple(
        dict.fromkeys(
            reuse_class
            for reuse_class in (node.reuse_class, *node.binding_classes)
            if reuse_class != "none"
        )
    )


def _binding_digests(
    node: VerificationNode, current_inputs: Mapping[str, object]
) -> dict[str, str]:
    """Compute every identity dimension that must remain equal to reuse a node."""
    classes = _binding_classes(node)
    if not classes:
        raise ValueError(f"unsupported reusable class: {node.reuse_class}")
    return dict(_binding_digest(reuse_class, current_inputs) for reuse_class in classes)


def _receipt_state(
    node: VerificationNode,
    receipt: Mapping[str, object] | None,
    current_inputs: Mapping[str, object],
    now: datetime,
) -> tuple[Literal["fresh", "stale", "unknown"], str, str | None]:
    if receipt is None:
        return "unknown", "evidence_reuse_unknown", None
    if receipt.get("checkerId") != node.node_id or receipt.get("result") != "passed":
        return "unknown", "evidence_receipt_invalid", None
    receipt_id = receipt.get("receiptId")
    if (
        not isinstance(receipt_id, str)
        or not receipt_id.startswith("sha256:")
        or not _valid_digest(receipt_id.removeprefix("sha256:"))
    ):
        return "unknown", "evidence_receipt_invalid", None
    receipt_body = dict(receipt)
    receipt_body.pop("receiptId", None)
    if receipt_id.removeprefix("sha256:") != _digest(receipt_body):
        return "unknown", "evidence_receipt_invalid", receipt_id
    if any(not _valid_digest(receipt.get(key)) for key in REQUIRED_RECEIPT_DIGESTS):
        return "unknown", "evidence_receipt_invalid", receipt_id
    try:
        if _parse_time(receipt.get("expiresAt")) <= now:
            return "stale", "evidence_expired", receipt_id
        if _parse_time(receipt.get("createdAt")) > now:
            return "unknown", "evidence_receipt_invalid", receipt_id
    except (TypeError, ValueError):
        return "unknown", "evidence_receipt_invalid", receipt_id
    try:
        digest_map = {
            "commandDigest": _digest(node.command),
            "scopeDigest": _digest(node.scope),
            "governanceDigest": _digest(current_inputs["governance"]),
            "environmentDigest": _digest(current_inputs["environment"]),
            "toolchainDigest": _digest(current_inputs["toolchain"]),
            "policyDigest": _digest(current_inputs["policy"]),
        }
    except KeyError:
        return "unknown", "evidence_reuse_unknown", receipt_id
    if any(receipt.get(key) != expected for key, expected in digest_map.items()):
        return "stale", "evidence_binding_mismatch", receipt_id
    if receipt.get("stage") != current_inputs.get("stage") or receipt.get(
        "runner"
    ) != current_inputs.get("runner"):
        return "stale", "evidence_execution_context_mismatch", receipt_id
    try:
        expected_bindings = _binding_digests(node, current_inputs)
    except (KeyError, ValueError):
        return "unknown", "evidence_reuse_unknown", receipt_id
    binding = receipt.get("binding")
    if not isinstance(binding, Mapping) or any(
        binding.get(key) != value for key, value in expected_bindings.items()
    ):
        return "stale", "evidence_binding_mismatch", receipt_id
    return "fresh", "evidence_reuse_fresh", receipt_id


def create_receipt(
    node: VerificationNode,
    current_inputs: Mapping[str, object],
    *,
    output_digest: str,
    created_at: datetime,
    expires_at: datetime,
    result: str = "passed",
    binding: Mapping[str, object] | None = None,
) -> dict[str, object]:
    """Create the receipt shape emitted after a successful real execution."""

    if not _valid_digest(output_digest):
        raise ValueError("output_digest must be a lowercase SHA-256 digest")
    computed_bindings = _binding_digests(node, current_inputs)
    receipt: dict[str, object] = {
        "checkerId": node.node_id,
        "result": result,
        "commandDigest": _digest(node.command),
        "scopeDigest": _digest(node.scope),
        "governanceDigest": _digest(current_inputs["governance"]),
        "environmentDigest": _digest(current_inputs["environment"]),
        "toolchainDigest": _digest(current_inputs["toolchain"]),
        "policyDigest": _digest(current_inputs["policy"]),
        "outputDigest": output_digest,
        "binding": {**dict(binding or {}), **computed_bindings},
        "stage": current_inputs.get("stage"),
        "runner": current_inputs.get("runner"),
        "createdAt": created_at.isoformat(),
        "expiresAt": expires_at.isoformat(),
    }
    receipt["receiptId"] = "sha256:" + _digest(receipt)
    return receipt


def _ordered_nodes(nodes: Sequence[VerificationNode]) -> list[VerificationNode]:
    by_id = {node.node_id: node for node in nodes}
    if len(by_id) != len(nodes):
        raise ValueError("duplicate verification node")
    visiting: set[str] = set()
    visited: set[str] = set()
    ordered: list[VerificationNode] = []

    def visit(node_id: str) -> None:
        if node_id in visiting:
            raise ValueError("verification dependency cycle")
        if node_id in visited:
            return
        node = by_id.get(node_id)
        if node is None:
            raise ValueError(f"missing dependency: {node_id}")
        visiting.add(node_id)
        for dependency in node.depends_on:
            visit(dependency)
        visiting.remove(node_id)
        visited.add(node_id)
        ordered.append(node)

    for node in nodes:
        visit(node.node_id)
    return ordered


def plan_verification(
    nodes: Sequence[VerificationNode],
    *,
    current_inputs: Mapping[str, object],
    receipts: Mapping[str, Mapping[str, object]],
    now: datetime | str,
) -> VerificationPlan:
    """Plan reuse without executing commands; all ambiguity executes again."""

    ordered = _ordered_nodes(nodes)
    current_time = _now(now)
    decisions: dict[str, PlannedCheck] = {}
    metrics = {
        "nodesPlanned": len(ordered),
        "nodesExecuted": 0,
        "nodesSkippedReused": 0,
        "rerunStale": 0,
        "rerunUnknown": 0,
        "protectedNodesExecuted": 0,
        "protectedNodesSkipped": 0,
        "planningElapsedMs": 0,
    }
    started = time.monotonic()
    for node in ordered:
        if node.reuse_class not in REUSE_CLASSES or any(
            reuse_class not in REUSE_CLASSES for reuse_class in node.binding_classes
        ):
            raise ValueError(f"unsupported reuse class: {node.reuse_class}")
        action: Literal["execute", "skip_reused"]
        state: Literal["fresh", "stale", "unknown", "not_applicable"]
        reason: str
        receipt_id: str | None
        if node.gate_class in PROTECTED_GATE_CLASSES:
            action, state, reason, receipt_id = (
                "execute",
                "unknown",
                "protected_gate_execution_required",
                None,
            )
        elif not node.reuse_allowed or node.reuse_class == "none":
            action, state, reason, receipt_id = (
                "execute",
                "not_applicable",
                "reuse_not_allowed",
                None,
            )
        else:
            state, reason, receipt_id = _receipt_state(
                node, receipts.get(node.node_id), current_inputs, current_time
            )
            action = "skip_reused" if state == "fresh" else "execute"
        if action == "skip_reused" and any(
            decisions[dependency].action == "execute" for dependency in node.depends_on
        ):
            action, state, reason = "execute", "stale", "dependency_rerun_required"
        planned = PlannedCheck(
            node_id=node.node_id,
            action=action,
            decision_state=state,
            status="skipped" if action == "skip_reused" else "planned",
            reason_code=reason,
            receipt_id=receipt_id,
            satisfied_by="reused_receipt" if action == "skip_reused" else "execution",
            required=node.required,
            protected=node.protected or node.gate_class in PROTECTED_GATE_CLASSES,
        )
        decisions[node.node_id] = planned
        if action == "skip_reused":
            metrics["nodesSkippedReused"] += 1
            if planned.protected:
                metrics["protectedNodesSkipped"] += 1
        else:
            metrics["nodesExecuted"] += 1
            if planned.protected:
                metrics["protectedNodesExecuted"] += 1
            if state == "stale":
                metrics["rerunStale"] += 1
            elif state == "unknown":
                metrics["rerunUnknown"] += 1
    metrics["planningElapsedMs"] = int((time.monotonic() - started) * 1000)
    return VerificationPlan(tuple(decisions[node.node_id] for node in ordered), metrics)


def execute_verification_plan(
    plan: VerificationPlan,
    *,
    executor: Callable[[PlannedCheck], Mapping[str, object]],
) -> dict[str, object]:
    """Consume a plan and fail closed if any executed required node fails."""

    results: list[dict[str, object]] = []
    metrics = dict(plan.metrics)
    started = time.monotonic()
    passed = True
    for decision in plan.checks:
        if decision.action == "skip_reused":
            results.append({**asdict(decision), "result": "passed"})
            continue
        result = dict(executor(decision))
        status = result.get("status")
        if decision.required and status != "passed":
            passed = False
        results.append({**asdict(decision), **result})
    metrics["executionElapsedMs"] = int((time.monotonic() - started) * 1000)
    metrics["protectedNodesSkipped"] = sum(
        1
        for item in results
        if item.get("protected") is True and item.get("action") == "skip_reused"
    )
    return {"schemaVersion": 1, "passed": passed, "results": results, "metrics": metrics}
