"""Execute one exact, detached, evidence-preserving uninstall proposal."""

from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
import tempfile
from collections.abc import Callable
from pathlib import Path
from typing import Any

from ai_uninstall_facts import UninstallFactsError, collect_uninstall_facts
from ai_uninstall_proposal import build_proposal, validate_proposal

DETACHED_MODULES = (
    "ai_detached_uninstaller.py",
    "ai_install_facts.py",
    "ai_uninstall_facts.py",
    "ai_uninstall_proposal.py",
)


def prepare(session_id: str, facts: dict[str, Any], confirm: bool = False) -> dict[str, Any]:
    """Retain the non-mutating legacy model for unit fixtures.

    Installed adopters use :func:`execute_proposal`; this helper never touches
    a filesystem and is kept only for compatibility with historical fixtures.
    """
    if facts.get("drift") or facts.get("unknownOwnership") or not facts.get("detached", True):
        return {"state": "blocked", "writes": [], "reason": "drift_unknown_or_not_detached"}
    if not confirm:
        return {"state": "needs_human_confirmation", "writes": [], "sessionId": session_id}
    preserved = [item for item in facts.get("files", []) if item in facts.get("preserve", [])]
    removed = [item for item in facts.get("files", []) if item not in preserved]
    return {
        "state": "completed",
        "writes": ["receipt"],
        "receipt": {
            "sessionId": session_id,
            "removed": removed,
            "preserved": preserved,
            "evidencePreserved": True,
            "runtimeRemovalVerified": True,
        },
    }


def _safe_relative(value: str) -> str:
    path = Path(value)
    if not value or path.is_absolute() or ".." in value.replace("\\", "/").split("/"):
        raise ValueError("unsafe proposal path")
    return "/".join(part for part in value.replace("\\", "/").split("/") if part not in ("", "."))


def _confined_receipt(root: Path, relative: str) -> Path:
    receipt = root.joinpath(*_safe_relative(relative).split("/"))
    current = root
    for component in receipt.relative_to(root).parts:
        current /= component
        if current.is_symlink():
            raise ValueError("symlink in receipt path")
    return receipt


def _write_receipt(root: Path, relative: str, result: dict[str, Any]) -> None:
    receipt = _confined_receipt(root, relative)
    receipt.parent.mkdir(parents=True, exist_ok=True)
    if _confined_receipt(root, relative) != receipt:
        raise ValueError("receipt path changed")
    temporary = receipt.with_name(f".{receipt.name}.tmp")
    if temporary.is_symlink():
        raise ValueError("symlink in receipt temporary path")
    temporary.write_text(
        json.dumps(result, ensure_ascii=False, sort_keys=True, indent=2) + "\n",
        encoding="utf-8",
    )
    temporary.replace(receipt)


def execute_proposal(
    root: Path,
    proposal: dict[str, Any],
    confirmed_digest: str,
    *,
    remove_file: Callable[[Path], None] | None = None,
    detached_execution: bool = False,
) -> dict[str, Any]:
    """Execute only a digest-matching preserve-evidence proposal."""
    root = root.resolve()
    remove_file = remove_file or (lambda path: path.unlink())
    if not detached_execution:
        return {
            "state": "blocked",
            "reason": "detached_execution_required",
            "writes": [],
        }
    errors = validate_proposal(proposal)
    if errors:
        return {"state": "blocked", "reason": errors[0], "writes": []}
    if proposal.get("mode") != "preserve-evidence":
        return {"state": "blocked", "reason": "unsupported_execution_mode", "writes": []}
    if confirmed_digest != proposal["proposalDigest"]:
        return {
            "state": "blocked",
            "reason": "confirmation_digest_mismatch",
            "writes": [],
        }
    try:
        receipt_path = _confined_receipt(root, proposal["receiptPath"])
    except ValueError:
        return {"state": "blocked", "reason": "unsafe_receipt_path", "writes": []}
    deletion = [_safe_relative(item) for item in proposal["deletionList"]]
    if receipt_path.exists():
        return {"state": "blocked", "reason": "receipt_already_exists", "writes": []}
    if any(receipt_path == root.joinpath(*item.split("/")) for item in deletion):
        return {"state": "blocked", "reason": "receipt_inside_deletion_set", "writes": []}

    try:
        current_facts = collect_uninstall_facts(root, proposal["sessionId"])
        current = build_proposal(current_facts, "preserve-evidence")
    except (UninstallFactsError, ValueError) as exc:
        return {
            "state": "blocked",
            "reason": "current_facts_mismatch",
            "detail": str(exc),
            "writes": [],
        }
    if current.get("proposalDigest") != proposal["proposalDigest"]:
        return {"state": "blocked", "reason": "current_facts_mismatch", "writes": []}

    removed: list[str] = []
    preserved = sorted(proposal["preservePaths"])
    failed: list[dict[str, str]] = []
    progress = {
        "state": "executing",
        "sessionId": proposal["sessionId"],
        "removed": removed,
        "preserved": preserved,
        "missing": [],
        "failed": failed,
        "detachedExecution": True,
        "runtimeRemovalVerified": False,
        "writes": [proposal["receiptPath"]],
    }
    try:
        _write_receipt(root, proposal["receiptPath"], progress)
    except (OSError, ValueError):
        return {"state": "blocked", "reason": "receipt_not_writable", "writes": []}
    for relative in deletion:
        path = root.joinpath(*relative.split("/"))
        if path.is_symlink() or not path.is_file():
            failed.append({"path": relative, "error": "path_missing_or_symlink"})
            break
        try:
            remove_file(path)
        except Exception as exc:  # noqa: BLE001 - receipt must report exact failure class
            failed.append({"path": relative, "error": type(exc).__name__})
            break
        removed.append(relative)
        _write_receipt(root, proposal["receiptPath"], progress)

    if failed:
        result = {
            "state": "partial_failure",
            "sessionId": proposal["sessionId"],
            "removed": removed,
            "preserved": preserved,
            "missing": [
                item
                for item in deletion
                if not (root.joinpath(*item.split("/")).exists()) and item not in removed
            ],
            "failed": failed,
            "detachedExecution": True,
            "runtimeRemovalVerified": False,
            "recovery": "reconcile the partial removal receipt before retrying",
            "writes": [proposal["receiptPath"]],
        }
        _write_receipt(root, proposal["receiptPath"], result)
        return result

    preserved_present = all(root.joinpath(*item.split("/")).exists() for item in preserved)
    removed_absent = all(not root.joinpath(*item.split("/")).exists() for item in deletion)
    result = {
        "state": "completed" if preserved_present and removed_absent else "blocked",
        "sessionId": proposal["sessionId"],
        "removed": removed,
        "preserved": preserved,
        "missing": [],
        "failed": [],
        "detachedExecution": True,
        "runtimeRemovalVerified": preserved_present and removed_absent,
        "writes": [proposal["receiptPath"]],
    }
    if result["state"] != "completed":
        result["reason"] = "post_state_verification_failed"
    _write_receipt(root, proposal["receiptPath"], result)
    return result


def _launch_detached(root: Path, proposal: Path, confirmed_digest: str) -> int:
    source_dir = Path(__file__).resolve().parent
    with tempfile.TemporaryDirectory(prefix="ai-cockpit-uninstall-") as temporary:
        detached_dir = Path(temporary)
        for name in DETACHED_MODULES:
            shutil.copy2(source_dir / name, detached_dir / name)
        environment = dict(os.environ)
        environment["PYTHONPATH"] = str(detached_dir)
        result = subprocess.run(
            [
                sys.executable,
                str(detached_dir / "ai_detached_uninstaller.py"),
                "--root",
                str(root.resolve()),
                "--proposal",
                str(proposal.resolve()),
                "--confirm-digest",
                confirmed_digest,
                "--detached-runtime",
            ],
            cwd=root.resolve(),
            text=True,
            capture_output=True,
            check=False,
            env=environment,
        )
        if result.stdout:
            print(result.stdout, end="")
        if result.stderr:
            print(result.stderr, end="", file=sys.stderr)
        return result.returncode


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, required=True)
    parser.add_argument("--proposal", type=Path, required=True)
    parser.add_argument("--confirm-digest", default="")
    parser.add_argument("--detached-runtime", action="store_true", help=argparse.SUPPRESS)
    args = parser.parse_args()
    if not args.detached_runtime:
        return _launch_detached(args.root, args.proposal, args.confirm_digest)
    proposal = json.loads(args.proposal.read_text(encoding="utf-8"))
    result = execute_proposal(
        args.root,
        proposal,
        args.confirm_digest,
        detached_execution=True,
    )
    print(json.dumps(result, ensure_ascii=False, sort_keys=True, indent=2))
    return 0 if result.get("state") == "completed" else 2


if __name__ == "__main__":
    raise SystemExit(main())
