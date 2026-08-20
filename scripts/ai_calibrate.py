#!/usr/bin/env python3
"""Generate and validate project boundary calibration Profiles."""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
import os
import sys
import tempfile
from collections.abc import Callable
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

from ai_calibration_profiles import load_policy
from ai_project_profile import BOUNDARY_KEYS, FACT_KEYS, load_profile

CALIBRATION_STAGES = (
    "repository_role",
    "language_and_stack",
    "source_boundaries",
    "test_boundaries",
    "generated_artifacts",
    "critical_paths",
    "quality_commands",
    "review_requirements",
    "risk_and_unknowns",
    "adoption_readiness",
)
ANSWER_TYPES = ("yes_no", "alternative_input", "unknown", "not_applicable")
CONFIRMATION_PHASES = ("reviewer", "owner")
SESSION_SCHEMA_VERSION = 2
CHECKLIST_DECISIONS = ("PASS", "STOP")


def _now() -> str:
    return datetime.now(UTC).isoformat()


def _evidence(kind: str, detail: str, *, status: str = "passed") -> dict[str, str]:
    return {"kind": kind, "status": status, "detail": detail, "recordedAt": _now()}


def _empty_checklist_evidence() -> dict[str, Any]:
    return {
        "observedEvidence": [],
        "candidateChange": None,
        "owner": None,
        "reviewer": None,
        "decision": None,
        "decisionReason": None,
        "retryStep": None,
        "recordedAt": None,
    }


def _canonical_json_bytes(value: Any) -> bytes:
    return json.dumps(
        value,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
    ).encode("utf-8")


def _json_document_bytes(value: Any) -> bytes:
    return json.dumps(value, ensure_ascii=False, indent=2).encode("utf-8") + b"\n"


class CalibrationError(ValueError):
    """Raised when a calibration session transition is invalid."""


class CalibrationSession:
    """Durable, side-effect-free calibration session state machine.

    The object is JSON serializable. Repository persistence is deliberately
    handled by :func:`save_session`, so callers can review a snapshot before
    writing it to an adopter's calibration directory.
    """

    def __init__(self, data: dict[str, Any]):
        self.data = data

    @classmethod
    def start(cls, session_id: str) -> CalibrationSession:
        if not session_id:
            raise CalibrationError("session_id must not be empty")
        stages: list[dict[str, Any]] = [
            {
                "id": stage,
                "position": index,
                "status": "current" if index == 0 else "pending",
                "checklist": {
                    "answerTypes": list(ANSWER_TYPES),
                    "answer": None,
                    "reason": None,
                },
                "checklistEvidence": _empty_checklist_evidence(),
                "evidence": [],
            }
            for index, stage in enumerate(CALIBRATION_STAGES)
        ]
        return cls(
            {
                "schemaVersion": SESSION_SCHEMA_VERSION,
                "sessionId": session_id,
                "language": "ja",
                "state": "in_progress",
                "currentStage": CALIBRATION_STAGES[0],
                "stages": stages,
                "events": [_evidence("session_started", "Calibration session created.")],
                "checks": {},
                "confirmations": {},
                "candidate": {
                    "status": "not_prepared",
                    "revision": 0,
                    "digestAlgorithm": "sha256",
                    "digest": None,
                    "configuration": None,
                },
                "active": {"status": "unchanged", "configuration": None},
                "staleStages": [],
                "legacyConfirmationHistory": [],
            }
        )

    def _stage(self, stage_id: str) -> dict[str, Any]:
        for stage in self.data["stages"]:
            if stage["id"] == stage_id:
                return stage
        raise CalibrationError(f"unknown calibration stage: {stage_id}")

    def _require_live(self) -> None:
        if self.data["state"] == "paused":
            raise CalibrationError("resume the paused session before continuing")
        if self.data["state"] in {"activated", "aborted"}:
            raise CalibrationError(f"session is already {self.data['state']}")

    def _invalidate_candidate(self, detail: str) -> None:
        candidate = self.data.get("candidate", {})
        if candidate.get("status") in {"prepared", "activated"}:
            candidate["status"] = "stale"
            candidate["staleReason"] = detail
            self.data["events"].append(_evidence("candidate_stale", detail, status="warning"))
        if self.data.get("confirmations"):
            self.data["events"].append(
                _evidence(
                    "confirmations_invalidated",
                    detail,
                    status="warning",
                )
            )
        self.data["confirmations"] = {}
        for check in ("full_self_check", "governance_simulation"):
            if check in self.data.get("checks", {}):
                self.data["checks"][check] = {
                    "status": "blocked",
                    "evidence": _evidence(
                        check,
                        f"Re-run after calibration changed: {detail}",
                        status="blocked",
                    ),
                }
        self.data.pop("review", None)

    def blocking_fields(self) -> dict[str, list[str]]:
        blockers: dict[str, list[str]] = {}
        for stage in self.data["stages"]:
            fields: list[str] = []
            checklist = stage.get("checklist", {})
            answer_type = checklist.get("answerType")
            if answer_type not in ANSWER_TYPES:
                fields.append("answerType")
            if not isinstance(checklist.get("answer"), str) or not checklist["answer"].strip():
                fields.append("answer")
            if answer_type == "unknown":
                fields.append("unknown")
            if answer_type == "not_applicable" and (
                not isinstance(checklist.get("reason"), str) or not checklist["reason"].strip()
            ):
                fields.append("reason")
            if stage.get("status") == "stale" or stage["id"] in self.data.get("staleStages", []):
                fields.append("stale")

            record = stage.get("checklistEvidence", {})
            observed = record.get("observedEvidence")
            if (
                not isinstance(observed, list)
                or not observed
                or any(not isinstance(item, str) or not item.strip() for item in observed)
            ):
                fields.append("observedEvidence")
            for field in (
                "candidateChange",
                "owner",
                "reviewer",
                "decisionReason",
            ):
                if not isinstance(record.get(field), str) or not record[field].strip():
                    fields.append(field)
            decision = record.get("decision")
            if decision not in CHECKLIST_DECISIONS:
                fields.append("decision")
            if decision == "STOP":
                fields.append("decision")
                if not isinstance(record.get("retryStep"), str) or not record["retryStep"].strip():
                    fields.append("retryStep")
            if fields:
                blockers[stage["id"]] = sorted(set(fields))
        return blockers

    def _require_no_blockers(self) -> None:
        blockers = self.blocking_fields()
        if blockers:
            details = "; ".join(
                f"{stage}: {', '.join(fields)}" for stage, fields in blockers.items()
            )
            raise CalibrationError(f"blocking calibration evidence: {details}")

    def record_checklist_evidence(
        self,
        stage_id: str,
        *,
        observed_evidence: list[str],
        candidate_change: str,
        owner: str,
        reviewer: str,
        decision: str,
        decision_reason: str,
        retry_step: str = "",
    ) -> None:
        self._require_live()
        stage = self._stage(stage_id)
        if (
            not isinstance(observed_evidence, list)
            or not observed_evidence
            or any(not isinstance(item, str) or not item.strip() for item in observed_evidence)
        ):
            raise CalibrationError("observed evidence must contain non-empty values")
        for name, value in (
            ("candidate change", candidate_change),
            ("owner", owner),
            ("reviewer", reviewer),
            ("decision reason", decision_reason),
        ):
            if not isinstance(value, str) or not value.strip():
                raise CalibrationError(f"{name} must not be empty")
        if decision not in CHECKLIST_DECISIONS:
            raise CalibrationError(f"decision must be one of {CHECKLIST_DECISIONS}")
        if decision == "STOP" and not retry_step.strip():
            raise CalibrationError("STOP requires a retry step")
        stage["checklistEvidence"] = {
            "observedEvidence": [item.strip() for item in observed_evidence],
            "candidateChange": candidate_change.strip(),
            "owner": owner.strip(),
            "reviewer": reviewer.strip(),
            "decision": decision,
            "decisionReason": decision_reason.strip(),
            "retryStep": retry_step.strip() or None,
            "recordedAt": _now(),
        }
        self.data["events"].append(_evidence("checklist_evidence_recorded", stage_id))
        self._invalidate_candidate(f"checklist evidence changed for {stage_id}")

    def answer(
        self,
        stage_id: str,
        answer: str,
        *,
        answer_type: str = "alternative_input",
        reason: str = "",
    ) -> None:
        self._require_live()
        stage = self._stage(stage_id)
        if answer_type not in ANSWER_TYPES:
            raise CalibrationError(f"unsupported answer type: {answer_type}")
        if not isinstance(answer, str) or not answer.strip():
            raise CalibrationError("answer must be a non-empty string")
        if answer_type == "yes_no" and answer not in {"Y", "N"}:
            raise CalibrationError("yes_no answers must be Y or N")
        if answer_type == "not_applicable" and not reason.strip():
            raise CalibrationError("Not Applicable requires a reason")
        previous = (
            stage["checklist"].get("answerType"),
            stage["checklist"].get("answer"),
            stage["checklist"].get("reason"),
        )
        stage["checklist"] = {
            "answerTypes": list(ANSWER_TYPES),
            "answer": answer,
            "reason": reason or None,
        }
        stage["checklist"]["answerType"] = answer_type
        stage["status"] = "blocked" if answer_type == "unknown" else "complete"
        stage["evidence"].append(_evidence("answer", f"{stage_id}: {answer_type}={answer}"))
        self.data["events"].append(_evidence("answer_recorded", stage_id))
        position = stage["position"]
        current = (answer_type, answer, reason or None)
        if previous[1] is not None and previous != current:
            for downstream in self.data["stages"][position + 1 :]:
                if downstream["status"] == "complete":
                    downstream["status"] = "stale"
                if downstream["id"] not in self.data["staleStages"]:
                    self.data["staleStages"].append(downstream["id"])
            self.data["events"].append(_evidence("dependency_stale", stage_id, status="warning"))
        if previous != current:
            self._invalidate_candidate(f"answer changed for {stage_id}")
        if position + 1 < len(self.data["stages"]):
            self.data["currentStage"] = self.data["stages"][position + 1]["id"]
            if self.data["stages"][position + 1]["status"] == "pending":
                self.data["stages"][position + 1]["status"] = "current"
        else:
            self.data["currentStage"] = None

    def back(self) -> None:
        self._require_live()
        current = self.data.get("currentStage")
        if current is None:
            index = len(self.data["stages"]) - 1
        else:
            index = next(
                stage["position"] for stage in self.data["stages"] if stage["id"] == current
            )
        if index == 0:
            raise CalibrationError("already at the first stage")
        self.data["stages"][index]["status"] = "pending"
        previous = self.data["stages"][index - 1]
        previous["status"] = "current"
        self.data["currentStage"] = previous["id"]
        self.data["events"].append(_evidence("back", previous["id"]))

    def review(self) -> dict[str, Any]:
        blocking_fields = self.blocking_fields()
        incomplete = [
            stage["id"]
            for stage in self.data["stages"]
            if stage["status"] != "complete" or stage["id"] in blocking_fields
        ]
        review = {
            "status": "blocked" if incomplete else "ready",
            "incompleteStages": incomplete,
            "blockingFields": blocking_fields,
            "evidence": _evidence("review", "Calibration review generated."),
        }
        self.data["review"] = review
        self.data["events"].append(review["evidence"])
        return review

    def pause(self) -> None:
        if self.data["state"] != "in_progress":
            raise CalibrationError("only an in-progress session can be paused")
        self.data["state"] = "paused"
        self.data["events"].append(_evidence("paused", "Session paused for later resume."))

    def resume(self) -> None:
        if self.data["state"] != "paused":
            raise CalibrationError("session is not paused")
        self.data["state"] = "in_progress"
        self.data["events"].append(_evidence("resumed", "Session resumed."))

    def revalidate(self) -> None:
        """Return stale downstream stages to the authoritative session queue."""
        self._require_live()
        stale = list(self.data.get("staleStages", []))
        if not stale:
            raise CalibrationError("session has no stale stages to revalidate")
        for stage in self.data["stages"]:
            if stage["id"] in stale:
                stage["status"] = "pending"
        first = next(stage for stage in self.data["stages"] if stage["id"] in stale)
        first["status"] = "current"
        self.data["currentStage"] = first["id"]
        self.data["staleStages"] = []
        self.data["events"].append(_evidence("revalidated", first["id"]))

    def _check(self, name: str, passed: bool, detail: str) -> dict[str, Any]:
        result = {
            "status": "passed" if passed else "blocked",
            "evidence": _evidence(name, detail, status="passed" if passed else "blocked"),
        }
        self.data["checks"][name] = result
        return result

    def stage_self_check(self) -> dict[str, Any]:
        current_stage = self.data.get("currentStage")
        stage = (
            self._stage(current_stage)
            if isinstance(current_stage, str)
            else self.data["stages"][-1]
        )
        return self._check(
            "stage_self_check",
            stage["status"] == "complete" and stage["id"] not in self.blocking_fields(),
            f"Stage {stage['id']} checklist state.",
        )

    def full_self_check(self) -> dict[str, Any]:
        complete = (
            all(stage["status"] == "complete" for stage in self.data["stages"])
            and not self.blocking_fields()
        )
        return self._check("full_self_check", complete, "All ten stages are complete.")

    def governance_simulation(self) -> dict[str, Any]:
        passed = (
            all(stage["status"] == "complete" for stage in self.data["stages"])
            and not self.data["staleStages"]
            and not self.blocking_fields()
        )
        return self._check(
            "governance_simulation",
            passed,
            "Candidate governance checks use recorded calibration answers.",
        )

    def prepare_candidate(self) -> dict[str, Any]:
        self._require_live()
        self._require_no_blockers()
        if self.data.get("review", {}).get("status") != "ready":
            raise CalibrationError("ready review is required before Candidate preparation")
        for check in ("full_self_check", "governance_simulation"):
            if self.data.get("checks", {}).get(check, {}).get("status") != "passed":
                raise CalibrationError(
                    f"{check.replace('_', ' ')} must pass before Candidate preparation"
                )
        previous_revision = self.data.get("candidate", {}).get("revision", 0)
        revision = previous_revision + 1 if isinstance(previous_revision, int) else 1
        configuration = {
            "sessionId": self.data["sessionId"],
            "language": self.data["language"],
            "stages": [
                {
                    "id": stage["id"],
                    "answer": copy.deepcopy(stage["checklist"]),
                    "evidence": copy.deepcopy(stage["checklistEvidence"]),
                }
                for stage in self.data["stages"]
            ],
        }
        digest = hashlib.sha256(_canonical_json_bytes(configuration)).hexdigest()
        candidate = {
            "status": "prepared",
            "revision": revision,
            "digestAlgorithm": "sha256",
            "digest": digest,
            "configuration": configuration,
            "preparedAt": _now(),
        }
        self.data["candidate"] = candidate
        self.data["confirmations"] = {}
        self.data["events"].append(
            _evidence("candidate_prepared", f"revision={revision}; sha256={digest}")
        )
        return copy.deepcopy(candidate)

    def confirm(
        self,
        phase: str,
        *,
        candidate_revision: int,
        candidate_digest: str,
    ) -> None:
        if phase not in CONFIRMATION_PHASES:
            raise CalibrationError(f"confirmation phase must be one of {CONFIRMATION_PHASES}")
        if self.data.get("checks", {}).get("full_self_check", {}).get("status") != "passed":
            raise CalibrationError("full self-check must pass before human confirmation")
        candidate = self.data.get("candidate", {})
        if candidate.get("status") != "prepared":
            raise CalibrationError("a prepared Candidate is required before human confirmation")
        if candidate_revision != candidate.get("revision"):
            raise CalibrationError("Candidate revision does not match the prepared Candidate")
        if candidate_digest != candidate.get("digest"):
            raise CalibrationError("Candidate digest does not match the prepared Candidate")
        self.data["confirmations"][phase] = {
            "status": "confirmed",
            "candidateRevision": candidate_revision,
            "candidateDigest": candidate_digest,
            "evidence": _evidence("human_confirmation", phase),
        }

    def activation_configuration(self) -> dict[str, Any]:
        self._require_live()
        self._require_no_blockers()
        if set(self.data.get("confirmations", {})) != set(CONFIRMATION_PHASES):
            raise CalibrationError("both human confirmation phases are required")
        if self.data.get("checks", {}).get("full_self_check", {}).get("status") != "passed":
            raise CalibrationError("full self-check must pass before activation")
        if self.data.get("checks", {}).get("governance_simulation", {}).get("status") != "passed":
            raise CalibrationError("governance simulation must pass before activation")
        candidate = self.data.get("candidate", {})
        if candidate.get("status") != "prepared":
            raise CalibrationError("a prepared Candidate is required before activation")
        configuration = candidate.get("configuration")
        if not isinstance(configuration, dict):
            raise CalibrationError("prepared Candidate configuration is missing")
        digest = hashlib.sha256(_canonical_json_bytes(configuration)).hexdigest()
        if digest != candidate.get("digest"):
            raise CalibrationError("prepared Candidate digest is stale or invalid")
        for phase in CONFIRMATION_PHASES:
            confirmation = self.data["confirmations"][phase]
            if (
                confirmation.get("candidateRevision") != candidate.get("revision")
                or confirmation.get("candidateDigest") != digest
            ):
                raise CalibrationError(
                    f"{phase} confirmation is not bound to the current Candidate"
                )
        return {
            **copy.deepcopy(configuration),
            "candidateRevision": candidate["revision"],
            "candidateDigest": digest,
        }


def _write_temporary(path: Path, content: bytes, prefix: str) -> Path:
    path.parent.mkdir(parents=True, exist_ok=True)
    descriptor, temporary = tempfile.mkstemp(prefix=prefix, dir=str(path.parent))
    temporary_path = Path(temporary)
    try:
        with os.fdopen(descriptor, "wb") as handle:
            handle.write(content)
            handle.flush()
            os.fsync(handle.fileno())
    except Exception:
        temporary_path.unlink(missing_ok=True)
        raise
    return temporary_path


def _atomic_write(
    path: Path,
    content: bytes,
    *,
    replace_fn: Callable[[str | Path, str | Path], None] = os.replace,
) -> None:
    temporary = _write_temporary(path, content, "calibration-write-")
    try:
        replace_fn(temporary, path)
    except Exception:
        temporary.unlink(missing_ok=True)
        raise


def save_session(session: CalibrationSession, path: Path) -> None:
    _atomic_write(path, _json_document_bytes(session.data))


def _migrate_session(value: dict[str, Any]) -> dict[str, Any]:
    version = value.get("schemaVersion")
    if version == SESSION_SCHEMA_VERSION:
        return value
    if version != 1:
        raise CalibrationError("unsupported calibration session schema or language")
    migrated = copy.deepcopy(value)
    migrated["schemaVersion"] = SESSION_SCHEMA_VERSION
    legacy_confirmations = copy.deepcopy(migrated.get("confirmations", {}))
    migrated["legacyConfirmationHistory"] = (
        [{"migratedAt": _now(), "records": legacy_confirmations}] if legacy_confirmations else []
    )
    migrated["confirmations"] = {}
    for stage in migrated.get("stages", []):
        stage.setdefault("checklistEvidence", _empty_checklist_evidence())
        if stage.get("checklist", {}).get("answerType") == "unknown":
            stage["status"] = "blocked"
    old_candidate = migrated.get("candidate", {})
    migrated["candidate"] = {
        "status": "not_prepared",
        "revision": int(old_candidate.get("revision", 0)) if isinstance(old_candidate, dict) else 0,
        "digestAlgorithm": "sha256",
        "digest": None,
        "configuration": None,
    }
    if migrated.get("state") == "activated":
        migrated["state"] = "paused"
        legacy_active = migrated.get("active")
        migrated["active"] = {
            "status": "legacy_unverified",
            "configuration": copy.deepcopy(
                legacy_active.get("configuration") if isinstance(legacy_active, dict) else None
            ),
        }
    migrated.setdefault("events", []).append(
        _evidence(
            "session_schema_migrated",
            "schemaVersion 1 migrated fail closed to schemaVersion 2.",
            status="warning",
        )
    )
    return migrated


def load_session(path: Path) -> CalibrationSession:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise CalibrationError(f"failed to read session: {exc}") from exc
    if not isinstance(value, dict) or value.get("language") != "ja":
        raise CalibrationError("unsupported calibration session schema or language")
    value = _migrate_session(value)
    if [stage.get("id") for stage in value.get("stages", [])] != list(CALIBRATION_STAGES):
        raise CalibrationError("calibration session must contain exactly ten ordered stages")
    return CalibrationSession(value)


def _restore_snapshot(
    path: Path,
    existed: bool,
    content: bytes,
    *,
    replace_fn: Callable[[str | Path, str | Path], None],
) -> None:
    if existed:
        _atomic_write(path, content, replace_fn=replace_fn)
    else:
        path.unlink(missing_ok=True)


def persist_activation(
    session: CalibrationSession,
    *,
    session_path: Path,
    active_path: Path,
    replace_fn: Callable[[str | Path, str | Path], None] = os.replace,
) -> None:
    active_configuration = session.activation_configuration()
    before_data = copy.deepcopy(session.data)
    final_data = copy.deepcopy(before_data)
    final_data["candidate"]["status"] = "activated"
    final_data["candidate"]["activatedAt"] = _now()
    final_data["candidate"]["evidence"] = _evidence(
        "candidate_activation",
        "Candidate and Session persisted through the rollback transaction.",
    )
    final_data["active"] = {
        "status": "active",
        "configuration": copy.deepcopy(active_configuration),
    }
    final_data["state"] = "activated"
    final_data["events"].append(
        _evidence("candidate_activated", f"revision={active_configuration['candidateRevision']}")
    )

    snapshots = {
        active_path: (
            active_path.exists(),
            active_path.read_bytes() if active_path.exists() else b"",
        ),
        session_path: (
            session_path.exists(),
            session_path.read_bytes() if session_path.exists() else b"",
        ),
    }
    active_temporary: Path | None = None
    session_temporary: Path | None = None
    attempted_replacements: list[Path] = []
    try:
        active_temporary = _write_temporary(
            active_path,
            _json_document_bytes(active_configuration),
            "calibration-active-",
        )
        session_temporary = _write_temporary(
            session_path,
            _json_document_bytes(final_data),
            "calibration-session-",
        )
        attempted_replacements.append(active_path)
        replace_fn(active_temporary, active_path)
        attempted_replacements.append(session_path)
        replace_fn(session_temporary, session_path)
    except Exception as exc:
        if active_temporary is not None:
            active_temporary.unlink(missing_ok=True)
        if session_temporary is not None:
            session_temporary.unlink(missing_ok=True)
        rollback_errors: list[str] = []
        for path in attempted_replacements:
            existed, content = snapshots[path]
            try:
                _restore_snapshot(
                    path,
                    existed,
                    content,
                    replace_fn=replace_fn,
                )
            except Exception as rollback_exc:  # noqa: BLE001 - rollback must retain every recovery failure
                rollback_errors.append(f"{path}: {rollback_exc}")
        session.data = before_data
        if rollback_errors:
            raise CalibrationError(
                "activation transaction failed and rollback failed; consistency is unproved: "
                + "; ".join(rollback_errors)
            ) from exc
        raise CalibrationError(
            f"activation transaction failed; Active and Session restored: {exc}"
        ) from exc
    session.data = final_data


def quote(value: str) -> str:
    return json.dumps(value, ensure_ascii=False)


def values(items: Any, key: str) -> list[str]:
    if not isinstance(items, list):
        return []
    return sorted({str(item[key]) for item in items if isinstance(item, dict) and item.get(key)})


def render_key_list(lines: list[str], indent: str, key: str, items: list[str]) -> None:
    if not items:
        lines.append(f"{indent}{key}: []")
        return
    lines.append(f"{indent}{key}:")
    lines.extend(f"{indent}  - {quote(item)}" for item in items)


def proposed_profile(report: dict[str, Any]) -> str:
    facts = report.get("detectedFacts", {})
    suggestions = report.get("suggestedBoundaries", {})
    calibration_policy = load_policy()
    lines = [
        "# Generated proposal. Review facts and suggestions; do not treat this file as approved.",
        "version: 1",
        "repositoryRole: template",
        "",
        "calibrationProfile:",
        "  level: lite",
        "  selectedBy: pending_human",
        "  selectedAt: pending",
        "  reasons: []",
        "  requiredControls:",
    ]
    lines.extend(f"    - {control}" for control in calibration_policy.required_controls("lite"))
    lines.append("  deferredControls:")
    lines.extend(f"    - {control}" for control in calibration_policy.deferred_controls("lite"))
    lines.extend(
        [
            "",
            "detectedFacts:",
        ]
    )
    for key in FACT_KEYS:
        render_key_list(
            lines, "  ", key, values(facts.get(key, []) if isinstance(facts, dict) else [], "value")
        )
    lines.extend(["", "suggestedBoundaries:"])
    for key in BOUNDARY_KEYS:
        render_key_list(
            lines,
            "  ",
            key,
            values(suggestions.get(key, []) if isinstance(suggestions, dict) else [], "path"),
        )
    project_signals = report.get("projectSignals", {})
    lines.extend(["", "projectSignals:"])
    for key in ("qualityCommands", "criticalDomains"):
        items = project_signals.get(key, []) if isinstance(project_signals, dict) else []
        values_list = [
            str(item.get("value")) for item in items if isinstance(item, dict) and item.get("value")
        ]
        render_key_list(lines, "  ", key, sorted(set(values_list)))
    lines.extend(["", "approvedBoundaries:"])
    for key in BOUNDARY_KEYS:
        render_key_list(lines, "  ", key, [])
    lines.extend(["", "reviewRequirements: []", ""])
    render_key_list(
        lines,
        "",
        "unknowns",
        [str(item) for item in report.get("unknowns", []) if isinstance(item, str)],
    )
    evidence = []
    if isinstance(facts, dict):
        for category in FACT_KEYS:
            for item in facts.get(category, []):
                if isinstance(item, dict):
                    evidence.append(
                        f"{category}:{item.get('value', '')}|confidence:{item.get('confidence', '')}|evidence:{item.get('evidence', '')}"
                    )
    lines.append("")
    render_key_list(lines, "", "evidence", evidence)
    lines.extend(["", "approval:", "  reviewed: false", '  reviewedBy: ""', '  reason: ""'])
    return "\n".join(lines) + "\n"


def generate(root: Path, report_path: Path, output: Path) -> int:
    if output.exists():
        print(f"ERROR: refusing to overwrite calibration proposal: {output}", file=sys.stderr)
        return 2
    try:
        report = json.loads(report_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        print(f"ERROR: failed to read Doctor report: {exc}", file=sys.stderr)
        return 2
    if report.get("reportVersion") != 1:
        print("ERROR: unsupported Doctor report version", file=sys.stderr)
        return 2
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(proposed_profile(report), encoding="utf-8")
    print(f"calibration proposal: {output.relative_to(root)}")
    print(
        "Review and copy approved values into .ai/project_profile.yaml; this command does not modify Guards."
    )
    return 0


def validate(path: Path, *, confirmed: bool) -> int:
    _, issues = load_profile(path, require_approval=confirmed)
    if issues:
        for issue in issues:
            print(f"[ERROR] {issue}", file=sys.stderr)
        return 1
    print(f"project Profile validation passed: {path}")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    generate_parser = subparsers.add_parser("generate")
    generate_parser.add_argument("--root", default=".")
    generate_parser.add_argument("--report", default="target/ai_project_doctor_report.json")
    generate_parser.add_argument("--output", default=".ai/project_profile.proposed.yaml")
    validate_parser = subparsers.add_parser("validate")
    validate_parser.add_argument("--profile", default=".ai/project_profile.proposed.yaml")
    validate_parser.add_argument("--confirmed", action="store_true")
    session_parser = subparsers.add_parser(
        "session", help="run the resumable ten-stage calibration session"
    )
    session_parser.add_argument(
        "action",
        choices=(
            "start",
            "answer",
            "record-evidence",
            "back",
            "review",
            "pause",
            "resume",
            "stage-self-check",
            "full-self-check",
            "simulate",
            "prepare-candidate",
            "confirm",
            "activate",
        ),
    )
    session_parser.add_argument("--session", default=".ai/calibration/session.json")
    session_parser.add_argument("--session-id", default="calibration-1")
    session_parser.add_argument("--stage")
    session_parser.add_argument("--answer")
    session_parser.add_argument("--answer-type", default="alternative_input", choices=ANSWER_TYPES)
    session_parser.add_argument("--reason", default="")
    session_parser.add_argument("--observed-evidence", action="append", default=[])
    session_parser.add_argument("--candidate-change")
    session_parser.add_argument("--owner")
    session_parser.add_argument("--reviewer")
    session_parser.add_argument("--decision", choices=CHECKLIST_DECISIONS)
    session_parser.add_argument("--decision-reason")
    session_parser.add_argument("--retry-step", default="")
    session_parser.add_argument("--phase", choices=CONFIRMATION_PHASES)
    session_parser.add_argument("--candidate-revision", type=int)
    session_parser.add_argument("--candidate-digest")
    session_parser.add_argument("--active", default=".ai/calibration/active.json")
    args = parser.parse_args()
    if args.command == "generate":
        root = Path(args.root).resolve()
        return generate(root, root / args.report, root / args.output)
    if args.command == "validate":
        return validate(Path(args.profile), confirmed=args.confirmed)
    try:
        session_path = Path(args.session)
        if args.action == "start":
            session = CalibrationSession.start(args.session_id)
        else:
            session = load_session(session_path)
            if args.action == "answer":
                if not args.stage or args.answer is None:
                    raise CalibrationError("answer requires --stage and --answer")
                session.answer(
                    args.stage, args.answer, answer_type=args.answer_type, reason=args.reason
                )
            elif args.action == "record-evidence":
                if not all(
                    (
                        args.stage,
                        args.observed_evidence,
                        args.candidate_change,
                        args.owner,
                        args.reviewer,
                        args.decision,
                        args.decision_reason,
                    )
                ):
                    raise CalibrationError(
                        "record-evidence requires stage, observed evidence, Candidate change, "
                        "owner, reviewer, decision, and decision reason"
                    )
                session.record_checklist_evidence(
                    args.stage,
                    observed_evidence=args.observed_evidence,
                    candidate_change=args.candidate_change,
                    owner=args.owner,
                    reviewer=args.reviewer,
                    decision=args.decision,
                    decision_reason=args.decision_reason,
                    retry_step=args.retry_step,
                )
            elif args.action == "back":
                session.back()
            elif args.action == "review":
                session.review()
            elif args.action == "pause":
                session.pause()
            elif args.action == "resume":
                session.resume()
            elif args.action == "stage-self-check":
                session.stage_self_check()
            elif args.action == "full-self-check":
                session.full_self_check()
            elif args.action == "simulate":
                session.governance_simulation()
            elif args.action == "prepare-candidate":
                session.prepare_candidate()
            elif args.action == "confirm":
                if not args.phase or args.candidate_revision is None or not args.candidate_digest:
                    raise CalibrationError(
                        "confirm requires phase, Candidate revision, and Candidate digest"
                    )
                session.confirm(
                    args.phase,
                    candidate_revision=args.candidate_revision,
                    candidate_digest=args.candidate_digest,
                )
            elif args.action == "activate":
                persist_activation(
                    session,
                    session_path=session_path,
                    active_path=Path(args.active),
                )
        if args.action != "activate":
            save_session(session, session_path)
        print(json.dumps(session.data, ensure_ascii=False, indent=2))
        return 0
    except CalibrationError as exc:
        print(f"ERROR: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
