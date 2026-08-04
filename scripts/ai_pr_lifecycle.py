#!/usr/bin/env python3
"""PR の merge と branch cleanup を fail-closed で実行する。"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys


def run(args: list[str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(args, text=True, capture_output=True, check=False)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--pr", required=True, type=int)
    parser.add_argument("--dry-run", action="store_true")
    parser.add_argument("--cleanup-local", action="store_true")
    args = parser.parse_args()
    view = run(["gh", "pr", "view", str(args.pr), "--json", "state,mergeStateStatus,headRefName"])
    if view.returncode != 0:
        print(f"❌ GitHub PR state cannot be verified: {view.stderr.strip()}", file=sys.stderr)
        return 1
    try:
        details = json.loads(view.stdout)
    except json.JSONDecodeError:
        print("❌ GitHub PR response is not valid JSON", file=sys.stderr)
        return 1
    if details.get("state") != "OPEN":
        print("❌ PR is not open; refusing merge/cleanup", file=sys.stderr)
        return 1
    checks = run(["gh", "pr", "checks", str(args.pr), "--required"])
    if checks.returncode != 0:
        print(
            f"❌ Required PR checks are unavailable or unsuccessful: {checks.stderr.strip()}",
            file=sys.stderr,
        )
        return 1
    if args.dry_run:
        print("✅ dry-run: verified GitHub PR query; merge and cleanup were not run")
        return 0
    merge = run(["gh", "pr", "merge", str(args.pr), "--merge", "--delete-branch"])
    if merge.returncode != 0:
        print(f"❌ PR merge/remote cleanup failed: {merge.stderr.strip()}", file=sys.stderr)
        return 1
    if args.cleanup_local:
        branch = details.get("headRefName")
        local = run(["git", "branch", "-d", branch])
        if local.returncode != 0:
            print(f"❌ PR merged, but local branch cleanup failed: {local.stderr.strip()}", file=sys.stderr)
            return 1
        prune = run(["git", "worktree", "prune"])
        if prune.returncode != 0:
            print(f"❌ PR merged, but worktree prune failed: {prune.stderr.strip()}", file=sys.stderr)
            return 1
        print("✅ PR merged and remote/local branches were cleaned")
    else:
        print("✅ PR merged and remote branch deleted; local cleanup requires --cleanup-local")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
