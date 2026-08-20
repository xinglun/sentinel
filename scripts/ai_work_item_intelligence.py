"""Fact-derived, repository-local Work Item Intelligence snapshots.

This module deliberately has no network client and no scheduler.  Commands that
change a Work Item may append a fact; queries only read and validate snapshots.
"""

from __future__ import annotations

import hashlib
import json
import os
import re
import tempfile
import time
from contextlib import contextmanager
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
RUNTIME = ROOT / ".ai" / "work-items" / "runtime"
IDENTIFIER = re.compile(r"^[a-z0-9][a-z0-9_-]{2,127}$")
SECRET = re.compile(r"(password|passwd|secret|token|api[_-]?key|private[_-]?key)", re.IGNORECASE)
LIFECYCLE = (
    "intake",
    "preflight",
    "implementation",
    "verification",
    "review",
    "finish",
    "closure",
    "closed",
)
GOVERNANCE = (
    "draft",
    "not_ready",
    "ready",
    "active",
    "waiting_for_dependency",
    "needs_human_confirmation",
    "blocked",
    "verification_failed",
    "ready_for_review",
    "completed_with_limitations",
    "completed",
    "closing",
    "closed",
    "failed",
    "cancelled",
)
HEALTH = ("not_observed", "active", "idle", "stale", "ended", "unknown")
EXIT = {
    "not_found": 10,
    "unavailable": 11,
    "inconsistent": 12,
    "stale": 13,
    "invalid_query": 20,
    "invalid_data": 30,
    "internal": 40,
}
TERMINAL_CLAIMS = {"completed", "release_ready", "distribution_verified"}


class IntelligenceError(ValueError):
    def __init__(self, code: str, message: str):
        super().__init__(message)
        self.code, self.message = code, message


def _now() -> str:
    return datetime.now(UTC).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def _digest(value: object) -> str:
    return (
        "sha256:"
        + hashlib.sha256(
            json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode()
        ).hexdigest()
    )


def _safe(value: Any, path: str = "fact") -> None:
    if isinstance(value, dict):
        for key, child in value.items():
            if SECRET.search(str(key)):
                raise IntelligenceError(
                    "invalid_data", f"secret-like field is forbidden: {path}.{key}"
                )
            _safe(child, f"{path}.{key}")
    elif isinstance(value, list):
        for index, child in enumerate(value):
            _safe(child, f"{path}[{index}]")


def paths(work_item: str, *, root: Path = ROOT) -> dict[str, Path]:
    if not IDENTIFIER.fullmatch(work_item):
        raise IntelligenceError("invalid_query", "invalid work item identifier")
    base = root / ".ai" / "work-items" / "runtime" / work_item
    return {
        "base": base,
        "facts": base / "facts.jsonl",
        "reducer": base / "reducer-state.json",
        "status": base / "status.json",
        "activity": base / "activity.json",
        "lock": base / "status.lock",
        "indexEntry": base / "index-entry.json",
        "index": base.parent / "index.json",
        "indexCache": base.parent / "index-cache.json",
        "indexLock": base.parent / "index.lock",
    }


@contextmanager
def _exclusive_lock(path: Path, *, timeout_seconds: float = 5.0):
    """Use an owner lease and exclusive creation; never remove a live writer lock."""
    path.parent.mkdir(parents=True, exist_ok=True)
    deadline = time.monotonic() + timeout_seconds
    descriptor: int | None = None
    while descriptor is None:
        try:
            descriptor = os.open(path, os.O_CREAT | os.O_EXCL | os.O_WRONLY)
            os.write(
                descriptor,
                json.dumps(
                    {"pid": os.getpid(), "leaseExpiresAt": time.time() + timeout_seconds}
                ).encode(),
            )
        except FileExistsError:
            try:
                owner = json.loads(path.read_text(encoding="utf-8"))
            except (OSError, json.JSONDecodeError):
                owner = {}
            if (
                isinstance(owner, dict)
                and isinstance(owner.get("leaseExpiresAt"), (int, float))
                and owner["leaseExpiresAt"] < time.time()
                and not _process_is_alive(owner.get("pid"))
            ):
                path.unlink(missing_ok=True)
                continue
            if time.monotonic() >= deadline:
                raise IntelligenceError(
                    "unavailable", f"runtime write lock is unavailable: {path.name}"
                )
            time.sleep(0.01)
    try:
        yield
    finally:
        os.close(descriptor)
        path.unlink(missing_ok=True)


def _process_is_alive(pid: object) -> bool:
    if not isinstance(pid, int) or pid <= 0:
        return False
    try:
        os.kill(pid, 0)
    except ProcessLookupError:
        return False
    except PermissionError:
        return True
    return True


def read_facts(work_item: str, *, root: Path = ROOT) -> list[dict[str, Any]]:
    source = paths(work_item, root=root)["facts"]
    if not source.exists():
        raise IntelligenceError("not_found", f"runtime facts not found for {work_item}")
    facts: list[dict[str, Any]] = []
    fact_ids: set[str] = set()
    for number, line in enumerate(source.read_text(encoding="utf-8").splitlines(), 1):
        try:
            fact = json.loads(line)
        except json.JSONDecodeError as exc:
            raise IntelligenceError("invalid_data", f"invalid fact JSON at line {number}") from exc
        if not isinstance(fact, dict):
            raise IntelligenceError("invalid_data", f"fact {number} must be an object")
        if fact.get("workItemId") != work_item or not isinstance(fact.get("factId"), str):
            raise IntelligenceError("invalid_data", f"invalid fact identity at line {number}")
        if fact["factId"] in fact_ids:
            raise IntelligenceError("invalid_data", f"duplicate factId: {fact['factId']}")
        if fact.get("sequence") != number:
            raise IntelligenceError(
                "invalid_data", f"non-contiguous fact sequence at line {number}"
            )
        claimed = fact.pop("digest", None)
        if claimed != _digest(fact):
            raise IntelligenceError("invalid_data", f"fact digest mismatch at line {number}")
        fact["digest"] = claimed
        _safe(fact)
        fact_ids.add(fact["factId"])
        facts.append(fact)
    return facts


def append_fact(
    work_item: str, fact_type: str, payload: dict[str, Any], *, root: Path = ROOT
) -> dict[str, Any]:
    if not fact_type or not isinstance(payload, dict):
        raise IntelligenceError("invalid_data", "fact type and object payload are required")
    _safe(payload)
    target = paths(work_item, root=root)
    with _exclusive_lock(target["lock"]):
        return _append_unlocked(work_item, fact_type, payload, root=root)


def _append_unlocked(
    work_item: str, fact_type: str, payload: dict[str, Any], *, root: Path
) -> dict[str, Any]:
    """Append while the owning Work Item lock is held."""
    target = paths(work_item, root=root)
    existing = _read_reducer_facts(target)
    if existing is None:
        existing = read_facts(work_item, root=root) if target["facts"].exists() else []
    sequence = len(existing) + 1
    fact = {
        "factId": f"{work_item}:{sequence}",
        "workItemId": work_item,
        "sequence": sequence,
        "factType": fact_type,
        "occurredAt": _now(),
        "source": "ai-cockpit",
        "payload": payload,
    }
    fact["digest"] = _digest(fact)
    with target["facts"].open("a", encoding="utf-8") as handle:
        handle.write(json.dumps(fact, ensure_ascii=False, sort_keys=True) + "\n")
    _rebuild_unlocked(work_item, root=root, facts=[*existing, fact])
    return fact


def record_fact_once(
    work_item: str, fact_type: str, payload: dict[str, Any], *, root: Path = ROOT
) -> dict[str, Any] | None:
    """Append an authoritative lifecycle fact once without making it an agent claim."""
    target = paths(work_item, root=root)
    _safe(payload)
    with _exclusive_lock(target["lock"]):
        if target["facts"].exists():
            for fact in read_facts(work_item, root=root):
                if fact["factType"] == fact_type and fact.get("payload") == payload:
                    return None
        return _append_unlocked(work_item, fact_type, payload, root=root)


def _state(
    facts: list[dict[str, Any]],
) -> tuple[
    str, str, list[dict[str, str]], list[dict[str, str]], list[dict[str, str]], list[dict[str, str]]
]:
    types = [str(row.get("factType")) for row in facts]
    blockers: list[dict[str, str]] = []
    missing: list[dict[str, str]] = []
    risks: list[dict[str, str]] = []
    open_entities: list[dict[str, str]] = []
    invalid_resolution = False
    opened = {
        "verification_failed": "verification",
        "human_decision_requested": "decision",
        "dependency_missing": "dependency",
    }
    resolved = {
        "verification_passed": "verification",
        "human_decision_recorded": "decision",
        "dependency_satisfied": "dependency",
    }
    keyed_facts = any(
        str(row.get("factType")) in {*opened, *resolved}
        and isinstance(row.get("payload", {}).get("subject"), dict)
        for row in facts
    )
    if keyed_facts:
        open_keys: dict[str, dict[str, str]] = {}
        for fact in facts:
            payload = fact.get("payload", {})
            if not isinstance(payload, dict):
                continue
            fact_type = str(fact.get("factType"))
            subject = payload.get("subject")
            if fact_type in opened and isinstance(subject, dict):
                entity_id = str(subject.get("id") or "")
                if entity_id:
                    open_keys[f"{opened[fact_type]}:{entity_id}"] = {
                        "kind": opened[fact_type],
                        "id": entity_id,
                    }
            if fact_type in resolved:
                target = payload.get("resolves")
                if (
                    not isinstance(target, str)
                    or target not in open_keys
                    or (
                        not isinstance(subject, dict)
                        or subject.get("id") != open_keys[target]["id"]
                        or subject.get("kind") != open_keys[target]["kind"]
                    )
                ):
                    invalid_resolution = True
                else:
                    del open_keys[target]
        open_entities = list(open_keys.values())
        if invalid_resolution:
            return "verification", "inconsistent", blockers, missing, risks, open_entities
        kinds = {row["kind"] for row in open_entities}
        if "verification" in kinds:
            return "verification", "verification_failed", blockers, missing, risks, open_entities
        if "decision" in kinds:
            blockers.append(
                {"code": "human_decision_pending", "detail": "a human decision is required"}
            )
            return "review", "needs_human_confirmation", blockers, missing, risks, open_entities
        if "dependency" in kinds:
            blockers.append(
                {"code": "dependency_missing", "detail": "a declared dependency is unavailable"}
            )
            return "preflight", "waiting_for_dependency", blockers, missing, risks, open_entities
        # Keyed facts supersede historical type-presence checks only for entity facts.
        types = [fact_type for fact_type in types if fact_type not in {*opened, *resolved}]
    claimed = {
        str(row.get("payload", {}).get("claim"))
        for row in facts
        if isinstance(row.get("payload"), dict)
    }
    evidence = {
        str(row.get("payload", {}).get("evidenceKind"))
        for row in facts
        if isinstance(row.get("payload"), dict)
    }
    for claim in claimed & TERMINAL_CLAIMS:
        required = "closure" if claim == "completed" else claim
        if required not in evidence:
            missing.append(
                {
                    "code": "required_evidence_missing",
                    "detail": f"{claim} requires {required} evidence",
                }
            )
    if "verification_failed" in types:
        return "verification", "verification_failed", blockers, missing, risks, open_entities
    if "human_decision_requested" in types and "human_decision_recorded" not in types:
        blockers.append(
            {"code": "human_decision_pending", "detail": "a human decision is required"}
        )
        return "review", "needs_human_confirmation", blockers, missing, risks, open_entities
    if "dependency_missing" in types:
        blockers.append(
            {"code": "dependency_missing", "detail": "a declared dependency is unavailable"}
        )
        return "preflight", "waiting_for_dependency", blockers, missing, risks, open_entities
    if missing:
        return "verification", "blocked", blockers, missing, risks, open_entities
    if "closed" in types:
        return "closed", "closed", blockers, missing, risks, open_entities
    if "closure_started" in types:
        return "closure", "closing", blockers, missing, risks, open_entities
    if "finish_passed" in types:
        return "finish", "ready_for_review", blockers, missing, risks, open_entities
    if "verification_passed" in types:
        return "review", "ready_for_review", blockers, missing, risks, open_entities
    if "implementation_started" in types:
        return "implementation", "active", blockers, missing, risks, open_entities
    if "preflight_ready" in types:
        return "preflight", "ready", blockers, missing, risks, open_entities
    return "intake", "draft", blockers, missing, risks, open_entities


def _source_validation(facts: list[dict[str, Any]], *, root: Path = ROOT) -> dict[str, Any]:
    """Validate declared local provenance without treating absent V1 provenance as a V2 fact."""
    records: list[dict[str, Any]] = []
    for fact in facts:
        if _is_runtime_observation(fact):
            continue
        payload = fact.get("payload")
        if not isinstance(payload, dict) or "sourceRef" not in payload:
            continue
        source_ref = payload.get("sourceRef")
        subject = payload.get("subject")
        record: dict[str, Any] = {"factId": fact["factId"], "valid": False}
        if not (
            isinstance(subject, dict)
            and isinstance(subject.get("kind"), str)
            and subject["kind"]
            and isinstance(subject.get("id"), str)
            and subject["id"]
            and isinstance(source_ref, dict)
            and isinstance(source_ref.get("kind"), str)
            and source_ref["kind"]
            and isinstance(source_ref.get("path"), str)
            and source_ref["path"]
            and isinstance(source_ref.get("digest"), str)
            and re.fullmatch(r"sha256:[0-9a-f]{64}", source_ref["digest"])
        ):
            record["reason"] = "invalid_source_ref"
        else:
            root = root.resolve()
            relative_path = Path(source_ref["path"])
            if relative_path.is_absolute() or ".." in relative_path.parts:
                record["reason"] = "source_path_outside_repository"
            else:
                path = (root / relative_path).resolve()
                try:
                    path.relative_to(root)
                except ValueError:
                    record["reason"] = "source_path_outside_repository"
                else:
                    try:
                        actual = "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()
                    except OSError:
                        record["reason"] = "source_unavailable"
                    else:
                        if actual == source_ref["digest"]:
                            record["valid"] = True
                        else:
                            record["reason"] = "source_digest_mismatch"
        records.append(record)
    return {"valid": all(row["valid"] for row in records), "records": records}


def _is_runtime_observation(fact: dict[str, Any]) -> bool:
    payload = fact.get("payload")
    subject = payload.get("subject") if isinstance(payload, dict) else None
    return fact.get("factType") == "observation" or (
        isinstance(subject, dict) and subject.get("kind") == "runtime"
    )


def snapshot(work_item: str, facts: list[dict[str, Any]], *, root: Path = ROOT) -> dict[str, Any]:
    """Build the legacy V1 projection; V2 wraps this without changing it."""
    phase, governance, blockers, missing, risks, open_entities = _state(facts)
    activity_path = paths(work_item, root=root)["activity"]
    health = "not_observed"
    activity: dict[str, Any] = {"health": health}
    if activity_path.exists():
        activity = json.loads(activity_path.read_text(encoding="utf-8"))
        health = activity.get("health", "unknown")
        if health not in HEALTH:
            health = "unknown"
        activity["health"] = health
    completed = sum(
        1 for fact in facts if fact["factType"] in {"verification_passed", "finish_passed"}
    )
    dependencies = [
        fact["payload"]
        for fact in facts
        if fact["factType"] in {"dependency_declared", "dependency_missing"}
    ]
    decisions = [
        fact["payload"]
        for fact in facts
        if fact["factType"] in {"human_decision_requested", "human_decision_recorded"}
    ]
    verification = [
        fact["payload"]
        for fact in facts
        if fact["factType"]
        in {"verification_started", "verification_passed", "verification_failed"}
    ]
    actions = {
        name: {"eligible": False, "reasonCodes": ["governance_state"]}
        for name in (
            "start",
            "continue",
            "run_verification",
            "retry",
            "request_human_decision",
            "finish",
            "close",
            "cancel",
        )
    }
    if governance == "draft":
        actions["start"] = {"eligible": True, "reasonCodes": []}
    if governance in {"ready", "active"}:
        actions["continue"] = {"eligible": True, "reasonCodes": []}
    if governance == "active":
        actions["run_verification"] = {"eligible": True, "reasonCodes": []}
    if governance == "ready_for_review":
        actions["finish"] = {"eligible": True, "reasonCodes": []}
    if governance == "closing":
        actions["close"] = {"eligible": True, "reasonCodes": []}
    if governance == "needs_human_confirmation":
        actions["request_human_decision"] = {"eligible": True, "reasonCodes": []}
    result: dict[str, Any] = {
        "schemaVersion": 1,
        "identity": {"workItemId": work_item},
        "status": {
            "lifecyclePhase": phase,
            "governanceState": governance,
            "activityHealth": health,
        },
        "progressFacts": {"factCount": len(facts), "verificationPassCount": completed},
        "blockingReasons": blockers,
        "missingEvidence": missing,
        "dependencies": dependencies,
        "humanDecisions": decisions,
        "risks": risks,
        "openEntities": open_entities,
        "verification": {
            "state": "not_run" if not verification else verification[-1].get("result", "observed"),
            "records": verification,
        },
        "actionEligibility": actions,
        "currentActivity": activity,
        "statusVersion": 1,
        "factSequence": len(facts),
        "lastFactId": facts[-1]["factId"] if facts else None,
    }
    result["snapshotDigest"] = _digest(result)
    return result


def _snapshot_v2(
    work_item: str, facts: list[dict[str, Any]], *, root: Path = ROOT
) -> dict[str, Any]:
    legacy = snapshot(work_item, facts, root=root)
    validation = _source_validation(facts, root=root)
    source_facts = [
        fact
        for fact in facts
        if isinstance(fact.get("payload"), dict)
        and "sourceRef" in fact["payload"]
        and not _is_runtime_observation(fact)
    ]
    runtime_facts = [fact for fact in facts if _is_runtime_observation(fact)]
    versions = {
        "governance": len(source_facts),
        "sourceSequence": len(source_facts),
        "runtimeObservation": len(runtime_facts),
    }
    result = dict(legacy)
    result["schemaVersion"] = 2
    result["statusVersion"] = max(1, versions["governance"])
    result["versions"] = versions
    result["sourceValidation"] = validation
    result["governance"] = {
        "lifecyclePhase": result["status"]["lifecyclePhase"],
        "state": result["status"]["governanceState"],
        "version": versions["governance"],
    }
    result["runtimeObservation"] = {
        "activityHealth": result["status"]["activityHealth"],
        "version": versions["runtimeObservation"],
    }
    result["completion"] = _completion(facts)
    result["governancePermissions"] = _governance_permissions(result)
    result["subjects"] = [
        fact["payload"]["subject"]
        for fact in source_facts + runtime_facts
        if isinstance(fact.get("payload", {}).get("subject"), dict)
    ]
    if not validation["valid"]:
        result["status"] = dict(result["status"])
        result["status"]["governanceState"] = "inconsistent"
        result["blockingReasons"] = [
            *result["blockingReasons"],
            {
                "code": "source_validation_failed",
                "detail": "source-bound fact evidence is inconsistent",
            },
        ]
    result.pop("snapshotDigest", None)
    result["snapshotDigest"] = _digest(result)
    return result


def _last_completion_fact(
    facts: list[dict[str, Any]], fact_types: set[str]
) -> tuple[int, dict[str, Any]] | None:
    """Return the last fact relevant to one completion phase."""
    for index in range(len(facts) - 1, -1, -1):
        fact = facts[index]
        if str(fact.get("factType")) in fact_types:
            return index, fact
    return None


def _completion_phase(
    facts: list[dict[str, Any]],
    *,
    started: set[str],
    completed: set[str],
    failed: set[str],
    verification: bool = False,
) -> dict[str, str]:
    """Project one current completion state without treating old evidence as current."""
    latest = _last_completion_fact(facts, started | completed | failed)
    if latest is None:
        return {"state": "not_started"}
    index, fact = latest
    fact_type = str(fact.get("factType"))
    fact_id = str(fact["factId"])
    if fact_type in started:
        return {"state": "in_progress", "lastFactId": fact_id}
    if fact_type in completed:
        key = "lastPassedFactId" if verification else "lastFactId"
        return {"state": "completed", key: fact_id}
    prior_completed = _last_completion_fact(facts[:index], completed)
    if verification and prior_completed is not None:
        return {
            "state": "invalidated",
            "lastPassedFactId": str(prior_completed[1]["factId"]),
            "invalidatedBy": fact_id,
        }
    return {"state": "failed", "lastFactId": fact_id}


def _completion(facts: list[dict[str, Any]]) -> dict[str, dict[str, str]]:
    """Return the V2 current-state completion contract for every lifecycle phase."""
    return {
        "implementation": _completion_phase(
            facts,
            started={"implementation_started"},
            completed={"implementation_completed"},
            failed={"implementation_failed"},
        ),
        "verification": _completion_phase(
            facts,
            started={"verification_started"},
            completed={"verification_passed"},
            failed={"verification_failed"},
            verification=True,
        ),
        "review": _completion_phase(
            facts,
            started={"finish_started"},
            completed={"finish_passed"},
            failed={"finish_failed"},
        ),
        "integration": _completion_phase(
            facts,
            started={"integration_started"},
            completed={"integrated"},
            failed={"integration_failed"},
        ),
        "closure": _completion_phase(
            facts,
            started={"closure_started"},
            completed={"closed"},
            failed={"closure_failed"},
        ),
    }


def _governance_permissions(snapshot: dict[str, Any]) -> dict[str, Any]:
    """Project V2 local phase decisions from the legacy eligibility evidence.

    This is intentionally a read-only explanation layer.  It neither grants
    authority nor schedules work; callers still invoke only their bounded local
    Work Item operation after applying their own Agent policy.
    """
    phases = {
        "implementation": ("continue", ["ready", "active"]),
        "verification": ("run_verification", ["active"]),
        "finish": ("finish", ["ready_for_review"]),
        "closure": ("close", ["closing"]),
    }
    eligibility = snapshot["actionEligibility"]
    result: dict[str, Any] = {
        "statusVersion": snapshot["statusVersion"],
        "basis": {
            "governanceState": snapshot["status"]["governanceState"],
            "governanceVersion": snapshot["governance"]["version"],
        },
    }
    for phase, (action, states) in phases.items():
        allowed = eligibility[action]["eligible"]
        result[phase] = {
            "allowed": allowed,
            "reasonCodes": [] if allowed else ["governance_state_not_eligible"],
            "conditions": {"requiredGovernanceStates": states},
            "evidenceBasis": [
                "status.governanceState",
                f"actionEligibility.{action}",
            ],
        }
    return result


def _as_v1(snapshot_value: dict[str, Any]) -> dict[str, Any]:
    """Return a byte-compatible V1 view from a persisted V2 projection."""
    result = {
        key: value
        for key, value in snapshot_value.items()
        if key
        not in {
            "versions",
            "sourceValidation",
            "subjects",
            "openEntities",
            "governance",
            "runtimeObservation",
            "completion",
            "governancePermissions",
            "publicationId",
            "publicationCursor",
            "snapshotDigest",
        }
    }
    result["schemaVersion"] = 1
    result["statusVersion"] = 1
    result["snapshotDigest"] = _digest(result)
    return result


def _atomic_json(path: Path, value: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(
        "w", encoding="utf-8", dir=path.parent, delete=False
    ) as handle:
        json.dump(value, handle, ensure_ascii=False, indent=2)
        handle.write("\n")
        name = handle.name
    os.replace(name, path)


def _read_reducer_facts(target: dict[str, Path]) -> list[dict[str, Any]] | None:
    """Return verified incremental state, or force a full audit when it is absent."""
    source = target["reducer"]
    if not source.exists():
        return None
    try:
        state = json.loads(source.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return None
    claimed = state.pop("reducerDigest", None) if isinstance(state, dict) else None
    if claimed != _digest(state) or not isinstance(state.get("facts"), list):
        return None
    facts = state["facts"]
    if not all(isinstance(fact, dict) for fact in facts):
        return None
    return facts


def _entry_from_snapshot(work_item: str, snapshot: dict[str, Any]) -> dict[str, Any]:
    publication_id = _digest(
        {
            "workItemId": work_item,
            "factSequence": snapshot["factSequence"],
            "snapshotDigest": snapshot["snapshotDigest"],
        }
    )
    cursor = time.time_ns()
    entry = {
        "schemaVersion": 1,
        "workItemId": work_item,
        "publicationId": publication_id,
        "cursor": cursor,
        "governanceState": snapshot["status"]["governanceState"],
        "activityHealth": snapshot["status"]["activityHealth"],
        "factSequence": snapshot["factSequence"],
        "snapshotDigest": snapshot["snapshotDigest"],
    }
    entry["entryDigest"] = _digest(entry)
    return entry


def _read_item_entries(*, root: Path) -> list[dict[str, Any]]:
    runtime = root / ".ai" / "work-items" / "runtime"
    entries: list[dict[str, Any]] = []
    for path in sorted(runtime.glob("*/index-entry.json")):
        try:
            entry = json.loads(path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError):
            continue
        claimed = entry.pop("entryDigest", None) if isinstance(entry, dict) else None
        if claimed != _digest(entry) or not isinstance(entry.get("workItemId"), str):
            continue
        entry["entryDigest"] = claimed
        entries.append(entry)
    return entries


def _rebuild_cache(*, root: Path) -> dict[str, Any]:
    entries = _read_item_entries(root=root)
    cache = {
        "schemaVersion": 2,
        "indexVersion": max((int(entry["cursor"]) for entry in entries), default=0),
        "entries": sorted(entries, key=lambda entry: entry["workItemId"]),
    }
    cache["indexDigest"] = _digest(cache)
    target = root / ".ai" / "work-items" / "runtime"
    _atomic_json(target / "index-cache.json", cache)
    _atomic_json(target / "index.json", cache)
    return cache


def _rebuild_unlocked(
    work_item: str, *, root: Path = ROOT, facts: list[dict[str, Any]] | None = None
) -> dict[str, Any]:
    facts = read_facts(work_item, root=root) if facts is None else facts
    result = _snapshot_v2(work_item, facts, root=root)
    target = paths(work_item, root=root)
    entry = _entry_from_snapshot(work_item, result)
    if target["status"].exists():
        try:
            previous = json.loads(target["status"].read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError):
            previous = {}
        if previous.get("publicationId") == entry["publicationId"] and isinstance(
            previous.get("publicationCursor"), int
        ):
            entry["cursor"] = previous["publicationCursor"]
    result["publicationId"] = entry["publicationId"]
    result["publicationCursor"] = entry["cursor"]
    result.pop("snapshotDigest", None)
    result["snapshotDigest"] = _digest(result)
    entry["snapshotDigest"] = result["snapshotDigest"]
    entry["entryDigest"] = _digest(
        {key: value for key, value in entry.items() if key != "entryDigest"}
    )
    _atomic_json(target["status"], result)
    _atomic_json(target["indexEntry"], entry)
    reducer = {"schemaVersion": 1, "factSequence": len(facts), "facts": facts}
    reducer["reducerDigest"] = _digest(reducer)
    _atomic_json(target["reducer"], reducer)
    _rebuild_cache(root=root)
    return result


def rebuild(work_item: str, *, schema_version: int = 1, root: Path = ROOT) -> dict[str, Any]:
    if schema_version not in {1, 2}:
        raise IntelligenceError("invalid_query", "schema version must be 1 or 2")
    target = paths(work_item, root=root)
    with _exclusive_lock(target["lock"]):
        value = _rebuild_unlocked(work_item, root=root)
    return _as_v1(value) if schema_version == 1 else value


def read_snapshot(work_item: str, *, schema_version: int = 1, root: Path = ROOT) -> dict[str, Any]:
    if schema_version not in {1, 2}:
        raise IntelligenceError("invalid_query", "schema version must be 1 or 2")
    target = paths(work_item, root=root)["status"]
    if not target.exists():
        raise IntelligenceError("not_found", f"snapshot not found for {work_item}")
    try:
        value = json.loads(target.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise IntelligenceError("invalid_data", "snapshot JSON is invalid") from exc
    claimed = value.pop("snapshotDigest", None)
    if claimed != _digest(value):
        raise IntelligenceError("inconsistent", "snapshot digest mismatch; rebuild is required")
    value["snapshotDigest"] = claimed
    if schema_version == 2:
        if value.get("schemaVersion") != 2:
            raise IntelligenceError(
                "inconsistent", "V2 snapshot is unavailable; rebuild is required"
            )
        return value
    return _as_v1(value) if value.get("schemaVersion") == 2 else value


def query(
    *,
    work_item: str | None = None,
    state: str | None = None,
    pending_human_decisions: bool = False,
    eligible_action: str | None = None,
    after_index_version: int | None = None,
    schema_version: int = 1,
    root: Path = ROOT,
) -> dict[str, Any]:
    if work_item:
        return read_snapshot(work_item, schema_version=schema_version, root=root)
    all_entries = _read_item_entries(root=root)
    active_dir = root / ".ai" / "work-items" / "active"
    active_ids = {
        path.name.removesuffix(".contract.json") for path in active_dir.glob("*.contract.json")
    }
    published_ids = {str(entry["workItemId"]) for entry in all_entries}
    for active_id in active_ids:
        if paths(active_id, root=root)["status"].exists() and active_id not in published_ids:
            raise IntelligenceError(
                "inconsistent", f"item-local publication is missing for {active_id}"
            )
    if not all_entries:
        return {"schemaVersion": schema_version, "indexVersion": 0, "entries": []}
    index_version = max(int(entry["cursor"]) for entry in all_entries)
    entries = all_entries
    if after_index_version is not None and index_version <= after_index_version:
        entries = []
    selected = []
    for entry in entries:
        if entry.get("workItemId") not in active_ids:
            continue
        if state and entry.get("governanceState") != state:
            continue
        item = read_snapshot(entry["workItemId"], schema_version=schema_version, root=root)
        if (
            pending_human_decisions
            and item["status"]["governanceState"] != "needs_human_confirmation"
        ):
            continue
        if eligible_action and not item["actionEligibility"].get(eligible_action, {}).get(
            "eligible"
        ):
            continue
        selected.append(item)
    return {
        "schemaVersion": schema_version,
        "indexVersion": index_version,
        "entries": selected,
    }


def measure_query_baseline(*, root: Path = ROOT, rounds: int = 10) -> dict[str, Any]:
    """Measure read-only local query latency; no result is persisted."""
    if rounds < 1:
        raise IntelligenceError("invalid_query", "rounds must be positive")
    samples: list[float] = []
    for _ in range(rounds):
        start = time.perf_counter()
        query(root=root)
        samples.append((time.perf_counter() - start) * 1000)
    ordered = sorted(samples)
    return {
        "measurementVersion": 1,
        "rounds": rounds,
        "listActiveQueryMs": {
            "min": round(ordered[0], 3),
            "median": round(ordered[len(ordered) // 2], 3),
            "p95": round(ordered[min(len(ordered) - 1, int(len(ordered) * 0.95))], 3),
            "max": round(ordered[-1], 3),
        },
    }
