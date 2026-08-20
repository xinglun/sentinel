#!/usr/bin/env python3
"""新しい Work Item Contract / Summary の骨格を作成する。"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from ai_observability import AiRunContext, create_observability


PROJECT_ROOT = Path(__file__).resolve().parents[1]
ACTIVE_DIR = PROJECT_ROOT / ".ai" / "work-items" / "active"
PREFLIGHT_REVIEW_PATH = PROJECT_ROOT / "target" / "ai_preflight_review.json"


def slug(value: str) -> str:
    normalized = re.sub(r"[^a-zA-Z0-9_-]+", "_", value.strip().lower()).strip("_")
    if not normalized:
        raise ValueError("TASK は空にできません。")
    return normalized


def write_json(path: Path, data: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(data, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def contract_hash(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()[:16]


def file_fingerprint(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def current_head() -> str:
    result = subprocess.run(["git", "rev-parse", "HEAD"], cwd=PROJECT_ROOT, text=True, capture_output=True, check=False)
    return result.stdout.strip() if result.returncode == 0 else ""


def baseline_dirty_paths() -> list[dict[str, Any]]:
    result = subprocess.run(
        ["git", "status", "--porcelain=v1", "--untracked-files=all"],
        cwd=PROJECT_ROOT,
        text=True,
        capture_output=True,
        check=False,
    )
    if result.returncode != 0:
        return []
    paths: list[dict[str, Any]] = []
    for line in result.stdout.splitlines():
        if len(line) < 4:
            continue
        status = line[:2].strip() or "M"
        path = line[3:].strip()
        if " -> " in path:
            path = path.split(" -> ", 1)[1]
        if path:
            file_path = PROJECT_ROOT / path
            if file_path.exists() and file_path.is_file():
                paths.append({"path": path, "status": status, "fingerprint": file_fingerprint(file_path)})
            else:
                paths.append({"path": path, "status": status, "fingerprint": "deleted"})
    unique: dict[str, dict[str, Any]] = {}
    for item in paths:
        path = item.get("path")
        if isinstance(path, str) and path:
            unique[path] = item
    return [unique[path] for path in sorted(unique)]


def required_verification_count(contract: dict) -> int:
    values = contract.get("verification", [])
    if not isinstance(values, list):
        return 0
    return sum(
        1
        for item in values
        if isinstance(item, dict) and item.get("required") is True and isinstance(item.get("command"), str) and item["command"].strip()
    )


def run_preflight_checks() -> int:
    result = subprocess.run(["make", "ai-preflight"], cwd=PROJECT_ROOT, check=False)
    return result.returncode


def generate_active_status(contract_path: Path, summary_path: Path) -> int:
    contract_rel = contract_path.relative_to(PROJECT_ROOT).as_posix()
    summary_rel = summary_path.relative_to(PROJECT_ROOT).as_posix()
    result = subprocess.run(
        [
            sys.executable,
            "scripts/ai_generate_status.py",
            contract_rel,
            "--summary",
            summary_rel,
        ],
        cwd=PROJECT_ROOT,
        check=False,
    )
    return result.returncode


def preflight_review_status() -> str | None:
    if not PREFLIGHT_REVIEW_PATH.exists():
        return None
    try:
        report = json.loads(PREFLIGHT_REVIEW_PATH.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError, ValueError):
        return None
    status = report.get("status") if isinstance(report, dict) else None
    return status if isinstance(status, str) else None


def linked_worktree_records(*, root: Path = PROJECT_ROOT) -> list[tuple[Path, str | None]]:
    """リンクされた worktree と checkout branch を列挙する。"""
    result = subprocess.run(
        ["git", "worktree", "list", "--porcelain"],
        cwd=root,
        text=True,
        capture_output=True,
        check=False,
    )
    if result.returncode != 0:
        if not (root / ".git").exists():
            return [(root, None)]
        raise RuntimeError("リンクされた worktree を列挙できません。")
    records: list[tuple[Path, str | None]] = []
    for block in result.stdout.split("\n\n"):
        lines = block.splitlines()
        location = next(
            (line.removeprefix("worktree ") for line in lines if line.startswith("worktree ")),
            None,
        )
        if not location:
            continue
        branch = next(
            (
                line.removeprefix("branch refs/heads/")
                for line in lines
                if line.startswith("branch refs/heads/")
            ),
            None,
        )
        records.append((Path(location), branch))
    return records


def has_complete_archived_work_item(worktree: Path, task: str) -> bool:
    """active record が archive の完全な pair に移動済みか判定する。"""
    archive_root = worktree / ".ai" / "work-items" / "archive"
    required = ("contract.json", "summary.json", "outcome.json", "archive-manifest.json")
    return (
        any(
            all((year / f"{task}.{suffix}").is_file() for suffix in required)
            for year in archive_root.iterdir()
            if year.is_dir()
        )
        if archive_root.is_dir()
        else False
    )


@dataclass(frozen=True)
class LinkedWorktreeIdentity:
    """リンク先 checkout に存在する active Work Item の identity。"""

    worktree: Path
    branch: str
    task: str


def linked_worktree_identity_report(
    *, root: Path = PROJECT_ROOT
) -> tuple[list[LinkedWorktreeIdentity], list[str]]:
    """active Work Item を持つ linked worktree を変更なしで検査する。"""
    try:
        records = linked_worktree_records(root=root)
    except (OSError, RuntimeError):
        return [], ["ERROR: linked worktree を列挙できません。"]
    identities: list[LinkedWorktreeIdentity] = []
    errors: list[str] = []
    for worktree, branch in records:
        try:
            if worktree.resolve() == root.resolve() or branch is None:
                continue
        except OSError:
            errors.append(f"ERROR: linked worktree を解決できません: {worktree}")
            continue
        active_dir = worktree / ".ai" / "work-items" / "active"
        contracts = {
            path.name.removesuffix(".contract.json")
            for path in active_dir.glob("*.contract.json")
        }
        summaries = {
            path.name.removesuffix(".summary.json")
            for path in active_dir.glob("*.summary.json")
        }
        summaries -= {
            task for task in summaries - contracts if has_complete_archived_work_item(worktree, task)
        }
        if not contracts and not summaries:
            continue
        if contracts != summaries:
            errors.append(f"ERROR: linked worktree の active Work Item pair が不完全です: {worktree}")
            continue
        if len(contracts) != 1:
            errors.append(f"ERROR: linked worktree に複数の active Work Item があります: {worktree}")
            continue
        task = next(iter(contracts))
        try:
            contract = json.loads(
                (active_dir / f"{task}.contract.json").read_text(encoding="utf-8")
            )
            summary = json.loads(
                (active_dir / f"{task}.summary.json").read_text(encoding="utf-8")
            )
        except (OSError, json.JSONDecodeError):
            errors.append(f"ERROR: linked worktree の active Work Item を読めません: {worktree}")
            continue
        if contract.get("workItemId") != task or summary.get("workItemId") != task:
            errors.append(f"ERROR: linked worktree の Work Item ID が path と一致しません: {worktree}")
            continue
        identities.append(LinkedWorktreeIdentity(worktree, branch, task))
    return identities, errors


def recoverable_foreign_duplicate_identities(
    identities: list[LinkedWorktreeIdentity],
) -> set[LinkedWorktreeIdentity]:
    """canonical branch が一つある foreign duplicate だけを復旧候補にする。"""
    recoverable: set[LinkedWorktreeIdentity] = set()
    for identity in identities:
        canonical = [
            candidate
            for candidate in identities
            if candidate.task == identity.task and candidate.branch == f"codex/{identity.task}"
        ]
        if identity.branch != f"codex/{identity.task}" and len(canonical) == 1:
            recoverable.add(identity)
    return recoverable


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="AI Work Item の skeleton を作成します。")
    parser.add_argument("--task", required=True, help="task id。例: risk_taxonomy_refine")
    parser.add_argument("--title", help="Work Item title。未指定時は task id を使う。")
    parser.add_argument("--mode", default="investigate", choices=["investigate", "author_todo", "code", "review", "cleanup"])
    parser.add_argument("--force", action="store_true", help="既存 skeleton を上書きする。")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        task = slug(args.task)
    except ValueError as exc:
        print(f"❌ {exc}", file=sys.stderr)
        return 2

    preflight_code = run_preflight_checks()
    if preflight_code != 0:
        return preflight_code

    contract_path = ACTIVE_DIR / f"{task}.contract.json"
    summary_path = ACTIVE_DIR / f"{task}.summary.json"
    if not args.force and (contract_path.exists() or summary_path.exists()):
        print(f"❌ Work Item は既に存在します: {task}", file=sys.stderr)
        return 1

    title = args.title or task.replace("_", " ")
    contract_rel = contract_path.relative_to(PROJECT_ROOT).as_posix()
    summary_rel = summary_path.relative_to(PROJECT_ROOT).as_posix()
    contract = {
        "contractVersion": 2,
        "workItemId": task,
        "mode": args.mode,
        "title": title,
        "baseCommit": current_head(),
        "baselineDirtyPaths": baseline_dirty_paths(),
        "problemStatement": "このタスクが解決する問題を記述する。製品文脈がない場合は機械的変更であることを明記する。",
        "intent": {},
        "scope": [contract_rel, summary_rel],
        "outOfScope": [],
        "sources": [{"path": contract_rel, "reason": "Work Item の初期 skeleton。"}],
        "unknowns": ["scope / sources / acceptance を task に合わせて確定する。"],
        "notCodable": args.mode == "code",
        "riskAssessment": {
            "level": "medium",
            "riskTypes": ["scope_unclear", "review_debt"],
            "reason": "初期 skeleton のため、実装前に scope / sources / acceptance / verification を確定する必要がある。",
        },
        "agentCapability": {
            "canImplement": args.mode != "code",
            "canVerify": False,
            "needsHumanDecision": args.mode == "code",
            "blockedReason": "code mode の skeleton は Contract 確定まで実装不可。",
        },
        "executionDecision": {
            "status": "contract_update_required",
            "reason": "実装前に Contract を task に合わせて更新する。",
        },
        "preReviewWarnings": ["初期 skeleton のため、このまま ready_for_review にしない。"],
        "checkpointPolicy": {
            "requiredCheckpoints": [
                "contract_start",
                "before_edit",
                "before_ready",
                "after_verification",
            ],
            "reminder": "scope / acceptance / verification / agentCapability が変わった場合は Contract と Summary を更新してから進める。",
        },
        "acceptance": ["Work Item Contract が task に合わせて更新されている。"],
        "verification": [
            {"command": f"make check-ai-contract CONTRACT={contract_rel}", "required": True},
            {"command": f"make check-ai-scope CONTRACT={contract_rel}", "required": True},
            {"command": "make fmt-check", "required": True},
            {"command": f"make check-ai-guards CONTRACT={contract_rel}", "required": True},
            {"command": f"make check-ai-backtrack CONTRACT={contract_rel} SUMMARY={summary_rel}", "required": True},
            {"command": "make check-ai-coverage-guard", "required": True},
            {"command": "make check-ai-scenario-coverage", "required": True},
            {"command": f"make check-ai-change-summary SUMMARY={summary_rel} CONTRACT={contract_rel}", "required": True},
            {"command": f"make generate-cockpit-status CONTRACT={contract_rel} SUMMARY={summary_rel}", "required": True},
            {"command": f"make check-ai-status CONTRACT={contract_rel} SUMMARY={summary_rel}", "required": True},
        ],
        "destructiveChangePolicy": {"allowed": False, "requiresHumanApproval": True, "allowPatterns": []},
        "rollbackNote": "この Work Item の diff を revert する。",
    }
    write_json(contract_path, contract)
    contract_digest = contract_hash(contract_path)
    summary = {
        "workItemId": task,
        "contractPath": contract_rel,
        "changedFiles": [
            {"path": contract_rel, "reason": "Work Item Contract skeleton を作成した。"},
            {"path": summary_rel, "reason": "AI Change Summary skeleton を作成した。"},
        ],
        "sourcesUsed": [contract_rel],
        "verification": [{"command": item["command"], "result": "not_run"} for item in contract["verification"]],
        "unknownsRemaining": ["scope / sources / acceptance を task に合わせて確定する。"],
        "risk": {"level": "medium", "detail": "初期 skeleton のため、実装前に Contract を確定する必要がある。"},
        "generatedFiles": [],
        "destructiveChanges": [],
        "observedIssues": [],
        "checkpointEvidence": [
            {
                "stage": stage,
                "recorded": False,
                "detail": "initial skeleton",
                "contractHash": contract_digest,
                "acceptanceCount": 0,
                "unknownCount": 0,
                "requiredChecks": required_verification_count(contract),
                "requiredChecksPassed": 0,
            }
            for stage in [
                "contract_start",
                "before_edit",
                "before_ready",
                "after_verification",
            ]
        ],
        "checkpointReview": [
            {
                "checkpoint": "contract_start",
                "status": "blocked",
                "note": "初期 skeleton のため Contract 確定が必要。",
            },
            {
                "checkpoint": "before_edit",
                "status": "blocked",
                "note": "scope / sources / acceptance 未確定のため編集不可。",
            },
            {
                "checkpoint": "before_ready",
                "status": "blocked",
                "note": "required checks 未実行のため ready 不可。",
            },
            {
                "checkpoint": "after_verification",
                "status": "blocked",
                "note": "verification 結果未記録。",
            },
        ],
        "residualRisks": [
            {
                "level": "medium",
                "area": "contract_readiness",
                "detail": "初期 skeleton は scope / sources / acceptance 未確定のため review 不可。",
                "reviewRecommended": True,
                "followUpCandidate": False,
            }
        ],
        "reviewReadiness": {
            "status": "not_ready",
            "reason": "Contract 未確定で required checks も未実行。",
            "expectedReviewFocus": ["scope", "sources", "acceptance", "verification"],
        },
    }
    write_json(summary_path, summary)
    status_code = generate_active_status(contract_path, summary_path)
    if status_code != 0:
        return status_code
    if args.mode == "code":
        post_preflight_code = run_preflight_checks()
        status_code = generate_active_status(contract_path, summary_path)
        if status_code != 0:
            return status_code
        review_status = preflight_review_status()
        if review_status is None:
            print("❌ Preflight Review の結果を確認できません。", file=sys.stderr)
            return 1
        if review_status != "ready":
            print(f"⏸️ Preflight Review pause: status={review_status}", file=sys.stderr)
            return 1
        if post_preflight_code != 0:
            return post_preflight_code
    print(f"✅ Work Item skeleton created: {task}")
    print(f"contract: {contract_rel}")
    print(f"summary: {summary_rel}")

    # -- observability --
    obs = create_observability(work_item_id=task)
    obs.work_item_started(fields={"mode": args.mode, "title": title})

    return 0


if __name__ == "__main__":
    sys.exit(main())
