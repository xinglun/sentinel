"""Build a confirmation-gated, preserve-evidence uninstall proposal."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
from typing import Any

MODES = ("disable", "preserve-evidence", "purge")
PROPOSAL_SCHEMA_VERSION = 1


def _canonical(value: Any) -> bytes:
    return (
        json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")) + "\n"
    ).encode()


def proposal_digest(proposal: dict[str, Any]) -> str:
    payload = {key: value for key, value in proposal.items() if key != "proposalDigest"}
    return "sha256:" + hashlib.sha256(_canonical(payload)).hexdigest()


def validate_proposal(proposal: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    if not isinstance(proposal, dict) or proposal.get("schemaVersion") != PROPOSAL_SCHEMA_VERSION:
        return ["unsupported_proposal_schema"]
    if proposal.get("proposalDigest") != proposal_digest(proposal):
        errors.append("proposal_digest_mismatch")
    for field in ("repositoryIdentity", "installationId", "sessionId"):
        if not isinstance(proposal.get(field), str) or not proposal[field]:
            errors.append(f"missing_{field}")
    for field in ("deletionList", "preservePaths"):
        value = proposal.get(field)
        if not isinstance(value, list) or any(not isinstance(item, str) for item in value):
            errors.append(f"invalid_{field}")
    receipt_path = proposal.get("receiptPath")
    if not isinstance(receipt_path, str) or not receipt_path or receipt_path.startswith("/"):
        errors.append("invalid_receipt_path")
    if proposal.get("mode") not in MODES:
        errors.append("invalid_mode")
    return errors


def build_proposal(
    facts: dict[str, Any], mode: str = "preserve-evidence", confirmed: bool = False
) -> dict[str, Any]:
    """Return Phase A evidence without mutating repository state."""
    if mode not in MODES:
        return {"state": "blocked", "reason": "invalid_mode", "writes": []}
    if mode == "disable":
        return {
            "state": "blocked",
            "reason": "use_disable_entrypoint",
            "writes": [],
        }
    if facts.get("drift") or facts.get("unknownOwnership"):
        return {
            "state": "blocked",
            "reason": "drift_or_unknown_ownership",
            "writes": [],
            "resumeCondition": "reconcile facts and ownership",
        }
    evidence = [
        "bootstrap",
        "archive",
        "human_decisions",
        "project_policy",
        "complexity_baseline",
        "audit",
    ]
    deletion = sorted(
        item["path"] if isinstance(item, dict) else item
        for item in facts.get("runtimeFiles", [])
        if (item["path"] if isinstance(item, dict) else item) not in facts.get("projectOwned", [])
    )
    preserve_paths = sorted(set(facts.get("preservePaths", [])) | set(facts.get("preserve", [])))
    session_id = facts.get("sessionId", "pending")
    proposal = {
        "schemaVersion": PROPOSAL_SCHEMA_VERSION,
        "state": "confirmed" if confirmed else "needs_human_confirmation",
        "mode": mode,
        "phase": "proposal",
        "writes": [],
        "deletionList": deletion,
        "preservePaths": preserve_paths,
        "repositoryIdentity": facts.get("repositoryIdentity", "unknown"),
        "installationId": facts.get("installationId", "unknown"),
        "sessionId": session_id,
        "receiptPath": f".ai/upgrade/uninstall-evidence/{session_id}.receipt.json",
        "preserveEvidence": evidence,
        "evidenceExport": {
            "required": True,
            "bundle": f".ai/upgrade/uninstall-evidence/{session_id}.json",
        },
        "detachedUninstaller": {"required": True, "sessionId": session_id},
        "receipt": {"required": True, "state": "pending"},
    }
    if mode == "purge":
        proposal["state"] = "blocked"
        proposal["reason"] = "purge_not_supported_by_uninstall_executor"
        proposal["resumeCondition"] = "use a separately approved purge workflow"
    proposal["proposalDigest"] = proposal_digest(proposal)
    return proposal


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--facts", type=Path, required=True)
    parser.add_argument("--mode", choices=MODES, default="preserve-evidence")
    parser.add_argument("--confirmed", action="store_true")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    facts = json.loads(args.facts.read_text(encoding="utf-8"))
    proposal = build_proposal(facts, args.mode, args.confirmed)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(
        json.dumps(proposal, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )
    print(json.dumps(proposal, ensure_ascii=False, sort_keys=True, indent=2))
    return 0 if proposal.get("state") != "blocked" else 2


if __name__ == "__main__":
    raise SystemExit(main())
