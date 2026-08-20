"""Lightweight Task/PR/Release verification primitives."""

from __future__ import annotations

import argparse
import json
import platform
import subprocess  # nosec B404 - git executable and arguments are fixed below
import sys
from collections.abc import Mapping
from dataclasses import asdict
from datetime import UTC, datetime
from hashlib import sha256
from pathlib import Path
from typing import cast

from ai_check_registry import CheckerRegistry, CheckResult
from ai_verification_context import build_context
from ai_verification_policy import evaluate_current_impact_graph, select_policy
from ai_verification_runtime import (
    PlannedCheck,
    VerificationNode,
    VerificationPlan,
    execute_verification_plan,
    plan_verification,
)

STAGES = ("task", "pr", "release")
MODES = ("legacy", "unified", "compare")


REUSE_CLASSES_BY_CHECKER: dict[str, tuple[str, ...]] = {
    "tests": ("content-bound", "diff-bound", "environment-bound"),
}


def _runtime_gate_class(check_id: str) -> str:
    if check_id == "scope":
        return "scope"
    if check_id in {"trust", "identity", "supply_chain"}:
        return "security"
    return "project"


def runtime_nodes(stage: str, changed_paths: list[str]) -> tuple[VerificationNode, ...]:
    """Build the bounded stage candidates consumed by the runtime planner."""
    requested: tuple[str, ...] = (
        ("scope", "tests") if stage == "task" else ("scope", "tests", "trust")
    )
    if stage == "release":
        requested = ("scope", "tests", "trust", "identity", "supply_chain")
    scope = tuple(sorted(set(changed_paths))) or ("<clean-worktree>",)
    nodes: list[VerificationNode] = []
    for check_id in requested:
        reuse_classes = REUSE_CLASSES_BY_CHECKER.get(check_id, ())
        nodes.append(
            VerificationNode(
                node_id=check_id,
                command=("make", f"check-{check_id.replace('_', '-')}"),
                gate_class=_runtime_gate_class(check_id),
                required=True,
                scope=("project-content",) if reuse_classes else scope,
                reuse_class=reuse_classes[0] if reuse_classes else "none",
                binding_classes=reuse_classes[1:],
                reuse_allowed=bool(reuse_classes),
                protected=not reuse_classes,
            )
        )
    return tuple(nodes)


def _source_content(root: Path, changed_paths: tuple[str, ...]) -> dict[str, str]:
    """Hash source/tooling content while excluding unrelated documentation edits."""
    try:
        listing = subprocess.run(  # nosec B603 B607 - fixed git command
            ["git", "-C", str(root), "ls-files", "-z"],
            check=True,
            capture_output=True,
        ).stdout.split(b"\0")
        paths = [item.decode("utf-8") for item in listing if item]
    except (OSError, subprocess.CalledProcessError, UnicodeDecodeError):
        paths = list(changed_paths)
    selected = [
        path
        for path in paths
        if path and not path.startswith(("docs/", ".ai/work-items/", "target/"))
    ]
    content: dict[str, str] = {}
    for path in sorted(set(selected)):
        candidate = root / path
        if candidate.is_file():
            content[path] = sha256(candidate.read_bytes()).hexdigest()
    return content


def runtime_inputs(context, stage: str, policy: Mapping[str, object]) -> dict[str, object]:
    """Bind reuse to immutable content and the current execution context."""
    content = _source_content(context.root, context.changed_paths)
    return {
        "scope": list(context.changed_paths),
        "governance": {"level": policy.get("level"), "stage": stage},
        "environment": {"platform": platform.system(), "release": platform.release()},
        "toolchain": {"python": sys.version.split()[0]},
        "policy": dict(policy),
        "stage": stage,
        "runner": "local",
        "content": content,
        "diff": {"changedPaths": list(context.changed_paths)},
    }


def load_runtime_receipts(path: str | Path) -> dict[str, Mapping[str, object]]:
    """Load only an explicit receipt map; absent or malformed evidence is unknown."""
    receipt_path = Path(path)
    if not receipt_path.is_file():
        return {}
    try:
        payload = json.loads(receipt_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return {}
    values = payload.get("receipts") if isinstance(payload, dict) else None
    if not isinstance(values, dict):
        return {}
    return {
        key: value
        for key, value in values.items()
        if isinstance(key, str) and isinstance(value, Mapping)
    }


def verification_scope(
    stage: str, changed_paths: list[str], *, full: bool = False
) -> dict[str, object]:
    """Describe whether a run is focused on the change or a complete stage gate."""
    if stage not in STAGES:
        raise ValueError(f"unsupported verification stage: {stage}")
    return {
        "mode": "full" if full or stage in {"pr", "release"} else "focused",
        "stage": stage,
        "paths": [] if full or stage in {"pr", "release"} else sorted(set(changed_paths)),
    }


def risk_and_authority(contract: Mapping[str, object]) -> dict[str, object]:
    """Keep evidence-derived risk classification separate from user authorization."""
    risk = contract.get("riskAssessment")
    risk_map = risk if isinstance(risk, Mapping) else {}
    approval = contract.get("restrictedWriteApproval")
    approval_map = approval if isinstance(approval, Mapping) else {}
    risk_types = risk_map.get("riskTypes", [])
    requested_operation = contract.get("requestedOperation")
    requested_operation_map = (
        requested_operation if isinstance(requested_operation, Mapping) else {}
    )
    return {
        "riskLevel": risk_map.get("level", "unknown"),
        "riskTypes": list(risk_types) if isinstance(risk_types, (list, tuple)) else [],
        "authority": {
            "required": bool(requested_operation_map.get("authorityRequired") is True),
            "approved": approval_map.get("approved") is True,
            "approvedBy": approval_map.get("approvedBy", ""),
        },
    }


def evaluate_trend(
    metric: str, samples: list[float], *, threshold: float, minimum_samples: int = 3
) -> CheckResult:
    """Turn a bounded trend into a soft result without hiding missing history."""
    if len(samples) < minimum_samples:
        return CheckResult.warning(
            metric,
            reason_code="insufficient_samples",
            detail=f"{len(samples)} sample(s); {minimum_samples} required before escalation",
        )
    spread = max(samples) - min(samples)
    if spread > threshold:
        return CheckResult(
            metric,
            "needs_human_confirmation",
            "soft",
            "threshold_exceeded",
            f"trend spread {spread:g} exceeds threshold {threshold:g}",
        )
    return CheckResult.passed(
        metric, gate="soft", detail=f"trend spread {spread:g} within threshold {threshold:g}"
    )


def verify_stage(
    context,
    stage: str,
    registry: CheckerRegistry,
    *,
    runtime_results: list[CheckResult] | None = None,
) -> list[CheckResult]:
    if stage not in STAGES:
        raise ValueError(f"unsupported verification stage: {stage}")
    requested: tuple[str, ...] = (
        ("scope", "tests") if stage == "task" else ("scope", "tests", "trust")
    )
    if stage == "release":
        requested = ("scope", "tests", "trust", "identity", "supply_chain")
    if runtime_results is not None:
        return runtime_results
    return registry.run(requested, available=set(registry.checker_ids))


def consume_runtime_plan(
    plan: VerificationPlan, registry: CheckerRegistry
) -> tuple[list[CheckResult], dict[str, object]]:
    """Consume the plan through the registry without reopening the full stage graph."""

    def execute_one(decision: PlannedCheck) -> Mapping[str, object]:
        result = registry.run([decision.node_id], available={decision.node_id})[0]
        return {
            "status": result.status,
            "checkerResult": asdict(result),
        }

    execution = execute_verification_plan(plan, executor=execute_one)
    results: list[CheckResult] = []
    for item in cast(list[Mapping[str, object]], execution["results"]):
        if item.get("action") == "skip_reused":
            results.append(
                CheckResult.passed(
                    str(item["node_id"]),
                    detail=f"satisfied by validated receipt ({item.get('reason_code', '')})",
                )
            )
            continue
        checker_result = item.get("checkerResult")
        if isinstance(checker_result, Mapping):
            results.append(
                CheckResult(
                    str(checker_result.get("checker_id", item["node_id"])),
                    str(checker_result.get("status", item.get("status", "failed"))),
                    str(checker_result.get("gate", "hard")),
                    str(checker_result.get("reason_code", "")),
                    str(checker_result.get("detail", "")),
                )
            )
        else:
            results.append(
                CheckResult.failed(
                    str(item["node_id"]), detail="runtime executor returned no checker result"
                )
            )
    return results, execution


def run_verification(
    context,
    registry: CheckerRegistry,
    *,
    mode: str = "unified",
    runtime_stage: str | None = None,
    runtime_results: list[CheckResult] | None = None,
):
    """Preserve the legacy surface while exposing unified and compare modes."""
    if mode not in MODES:
        raise ValueError(f"unsupported verification mode: {mode}")

    def stage_results(stage: str) -> list[CheckResult]:
        if runtime_stage == stage and runtime_results is not None:
            return runtime_results
        return verify_stage(context, stage, registry)

    legacy = {"mode": "legacy", "results": stage_results("task")}
    unified = {
        "mode": "unified",
        "results": {stage: stage_results(stage) for stage in STAGES},
    }
    if mode == "legacy":
        return legacy
    if mode == "compare":
        return {"mode": "compare", "legacy": legacy, "unified": unified}
    return unified


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", default=".")
    parser.add_argument("--contract", required=True)
    parser.add_argument("--summary", required=True)
    parser.add_argument("--stage", choices=STAGES, default="task")
    parser.add_argument("--mode", choices=MODES, default="unified")
    parser.add_argument("--scope", choices=("focused", "full"), default=None)
    parser.add_argument(
        "--receipts",
        default="target/quality/verification-receipts.json",
        help="Optional explicit passed-receipt map; missing or malformed evidence reruns.",
    )
    args = parser.parse_args()
    context = build_context(args.root, args.contract, args.summary)
    registry = CheckerRegistry()
    scope = verification_scope(
        args.stage,
        list(context.changed_paths),
        full=args.scope == "full",
    )
    risk_authority = risk_and_authority(context.contract)
    policy = select_policy(
        args.stage,
        list(context.changed_paths),
        requested=context.contract.get("verificationPolicy")
        if isinstance(context.contract.get("verificationPolicy"), str)
        else None,
    )
    impact_graph = evaluate_current_impact_graph(
        profile="release" if args.stage == "release" else policy["level"],
        receipt_bindings={},
    )
    runtime_plan_object = plan_verification(
        runtime_nodes(args.stage, list(context.changed_paths)),
        current_inputs=runtime_inputs(context, args.stage, policy),
        receipts=load_runtime_receipts(args.receipts),
        now=datetime.now(UTC),
    )
    runtime_results, runtime_execution = consume_runtime_plan(runtime_plan_object, registry)
    results = run_verification(
        context,
        registry,
        mode=args.mode,
        runtime_stage=args.stage,
        runtime_results=runtime_results,
    )
    runtime_plan = runtime_plan_object.to_dict()
    runtime_plan["execution"] = runtime_execution
    if args.mode == "unified":
        results = {
            args.stage: [asdict(result) for result in results["results"][args.stage]],
            "mode": args.mode,
            "verificationScope": scope,
            "riskAndAuthority": risk_authority,
            "policy": policy,
            "impactGraph": impact_graph,
            "runtimePlan": runtime_plan,
        }
    elif args.mode == "legacy":
        results["results"] = [asdict(result) for result in results["results"]]
        results["verificationScope"] = scope
        results["riskAndAuthority"] = risk_authority
        results["policy"] = policy
        results["impactGraph"] = impact_graph
        results["runtimePlan"] = runtime_plan
    else:
        results["legacy"]["results"] = [asdict(result) for result in results["legacy"]["results"]]
        results["unified"]["results"] = {
            stage: [asdict(result) for result in values]
            for stage, values in results["unified"]["results"].items()
        }
        results["verificationScope"] = scope
        results["riskAndAuthority"] = risk_authority
        results["policy"] = policy
        results["impactGraph"] = impact_graph
        results["runtimePlan"] = runtime_plan
    print(json.dumps(results, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
