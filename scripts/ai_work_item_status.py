"""Read-only CLI for Work Item Intelligence snapshots."""

from __future__ import annotations

import argparse
import json

from ai_work_item_intelligence import (
    EXIT,
    IntelligenceError,
    measure_query_baseline,
    query,
    rebuild,
)

WORKTREE_QUERY_CONTEXT = {
    "scope": "current_worktree",
    "aggregatesAcrossWorktrees": False,
    "schedulerOwnership": "external_agent",
}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--work-item")
    parser.add_argument("--list-active", action="store_true")
    parser.add_argument("--state")
    parser.add_argument("--pending-human-decisions", action="store_true")
    parser.add_argument("--eligible-action")
    parser.add_argument("--after-index-version", type=int)
    parser.add_argument("--schema-version", type=int, choices=[1, 2], default=1)
    parser.add_argument("--rebuild", action="store_true")
    parser.add_argument("--measure", action="store_true")
    parser.add_argument("--format", default="json", choices=["json"])
    args = parser.parse_args()
    if not args.work_item and not args.list_active:
        print(
            json.dumps(
                {
                    "ok": False,
                    "data": None,
                    "error": {
                        "code": "invalid_query",
                        "message": "--work-item or --list-active is required",
                    },
                }
            )
        )
        return EXIT["invalid_query"]
    if args.rebuild and not args.work_item:
        print(
            json.dumps(
                {
                    "ok": False,
                    "data": None,
                    "error": {"code": "invalid_query", "message": "--rebuild requires --work-item"},
                }
            )
        )
        return EXIT["invalid_query"]
    try:
        data = (
            rebuild(args.work_item, schema_version=args.schema_version)
            if args.rebuild
            else query(
                work_item=args.work_item,
                state=args.state,
                pending_human_decisions=args.pending_human_decisions,
                eligible_action=args.eligible_action,
                after_index_version=args.after_index_version,
                schema_version=args.schema_version,
            )
        )
        if args.measure:
            data = {"query": data, "measurement": measure_query_baseline()}
    except IntelligenceError as exc:
        print(
            json.dumps(
                {"ok": False, "data": None, "error": {"code": exc.code, "message": exc.message}}
            )
        )
        return EXIT[exc.code]
    print(
        json.dumps(
            {"ok": True, "data": data, "error": None, "context": WORKTREE_QUERY_CONTEXT},
            ensure_ascii=False,
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
