"""Validate the bounded Work Item exception for a live calibration Session."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
from typing import Any

CALIBRATION_SESSION = Path(".ai/calibration/session.json")
LIVE_SESSION_STATES = frozenset({"in_progress", "paused"})
CORRECTIVE_FIELDS = {
    "schemaVersion",
    "sessionPath",
    "sessionId",
    "sessionState",
    "sessionDigest",
    "findingId",
    "findingSummary",
    "authority",
    "repairPaths",
    "resumeCondition",
}


def _non_empty_string(value: object) -> bool:
    return isinstance(value, str) and bool(value.strip())


def _safe_repository_path(value: object) -> bool:
    if not _non_empty_string(value):
        return False
    path = Path(str(value))
    return not path.is_absolute() and ".." not in path.parts and "*" not in str(value)


def validate_calibration_corrective_shape(corrective: object) -> str | None:
    """Validate the declaration independently of a particular Session instance."""
    if not isinstance(corrective, dict):
        return "calibrationCorrective must be a JSON object"
    unexpected = sorted(set(corrective) - CORRECTIVE_FIELDS)
    missing = sorted(CORRECTIVE_FIELDS - set(corrective))
    if unexpected or missing:
        details = []
        if missing:
            details.append("missing " + ", ".join(missing))
        if unexpected:
            details.append("unexpected " + ", ".join(unexpected))
        return "calibration corrective declaration has " + "; ".join(details)
    if corrective.get("schemaVersion") != 1:
        return "calibrationCorrective.schemaVersion must be 1"
    if corrective.get("sessionPath") != CALIBRATION_SESSION.as_posix():
        return "calibrationCorrective.sessionPath must be .ai/calibration/session.json"
    if not _non_empty_string(corrective.get("sessionId")):
        return "calibrationCorrective.sessionId must be a non-empty string"
    if not _non_empty_string(corrective.get("sessionState")):
        return "calibrationCorrective.sessionState must be a non-empty string"
    digest = corrective.get("sessionDigest")
    if (
        not isinstance(digest, str)
        or len(digest) != 64
        or any(c not in "0123456789abcdef" for c in digest)
    ):
        return "calibrationCorrective.sessionDigest must be a SHA-256 digest"
    for key in ("findingId", "findingSummary", "authority", "resumeCondition"):
        if not _non_empty_string(corrective.get(key)):
            return f"calibrationCorrective.{key} must be a non-empty string"
    repair_paths = corrective.get("repairPaths")
    if (
        not isinstance(repair_paths, list)
        or not repair_paths
        or len(set(repair_paths)) != len(repair_paths)
        or any(not _safe_repository_path(path) for path in repair_paths)
    ):
        return "calibrationCorrective.repairPaths must be unique repository-relative paths"
    forbidden = {CALIBRATION_SESSION.as_posix(), ".ai/calibration/active.json"}
    if any(path in forbidden for path in repair_paths):
        return "calibrationCorrective.repairPaths cannot modify calibration Session state"
    return None


def _corrective_issue(
    corrective: object, *, session: dict[str, Any], session_bytes: bytes
) -> str | None:
    shape_issue = validate_calibration_corrective_shape(corrective)
    if shape_issue:
        return "ERROR: " + shape_issue
    if not isinstance(corrective, dict):
        return "ERROR: calibrationCorrective must be a JSON object"
    if corrective.get("sessionId") != session["sessionId"]:
        return "ERROR: calibrationCorrective.sessionId does not match live calibration Session"
    if corrective.get("sessionState") != session["state"]:
        return "ERROR: calibrationCorrective.sessionState does not match live calibration Session"
    if corrective.get("sessionDigest") != hashlib.sha256(session_bytes).hexdigest():
        return "ERROR: calibrationCorrective.sessionDigest does not match live calibration Session"
    return None


def calibration_start_issue(corrective: object = None, *, root: Path) -> str | None:
    """Reject an ordinary Work Item start while calibration remains live.

    A corrective exception must be complete and bound to the current Session
    bytes. The function is side-effect free so callers can run it before any
    Work Item evidence is written.
    """

    session_path = root / CALIBRATION_SESSION
    if not session_path.is_file():
        return None
    try:
        value: Any = json.loads(session_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        return f"ERROR: calibration Session is unreadable: {exc}"
    if not isinstance(value, dict):
        return "ERROR: calibration Session must be a JSON object"
    session_id = value.get("sessionId")
    state = value.get("state")
    if not isinstance(session_id, str) or not session_id.strip():
        return "ERROR: calibration Session sessionId must be a non-empty string"
    if not isinstance(state, str) or not state.strip():
        return "ERROR: calibration Session state must be a non-empty string"
    if state not in LIVE_SESSION_STATES:
        if corrective is not None:
            return "ERROR: calibration corrective requires a live in_progress or paused Session"
        return None
    if corrective is not None:
        return _corrective_issue(corrective, session=value, session_bytes=session_path.read_bytes())
    return (
        f"ERROR: live calibration Session {session_id} is {state}; "
        "start requires a valid --calibration-corrective declaration before lifecycle writes."
    )


def calibration_corrective_binding_issue(corrective: object, *, root: Path) -> str | None:
    """Require an active Contract's exception to remain bound to a live Session."""
    if not (root / CALIBRATION_SESSION).is_file():
        return "ERROR: calibrationCorrective requires its bound live calibration Session"
    return calibration_start_issue(corrective, root=root)
