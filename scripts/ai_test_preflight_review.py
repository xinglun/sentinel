#!/usr/bin/env python3
"""Preflight Review と Cockpit 表示の回帰テスト。"""

from __future__ import annotations

import json
import shutil
import sys
import tempfile
from pathlib import Path
from unittest.mock import patch

import ai_generate_status
import ai_preflight
import ai_preflight_review
import ai_start


REPO_ROOT = Path(__file__).resolve().parents[1]


def assert_true(value: bool, message: str) -> None:
    if not value:
        raise AssertionError(message)


def assert_equal(actual: object, expected: object, message: str) -> None:
    if actual != expected:
        raise AssertionError(f"{message}: expected={expected!r}, actual={actual!r}")


def write_json(path: Path, value: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def write_preflight_review(root: Path, status: str) -> None:
    review_path = root / "target/ai_preflight_review.json"
    write_json(
        review_path,
        {
            "status": status,
        },
    )


def write_policy(path: Path, *, gate_enabled: bool) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        "\n".join(
            [
                "version: 1",
                f"gateEnabled: {'true' if gate_enabled else 'false'}",
                "blockedStatuses:",
                "  - needs_human_confirmation",
                "  - not_ready",
                "",
            ]
        ),
        encoding="utf-8",
    )


def base_contract(work_item_id: str, *, ready: bool) -> dict:
    contract = {
        "contractVersion": 2,
        "workItemId": work_item_id,
        "mode": "code",
        "title": "Preflight Review test",
        "baseCommit": "deadbeefdeadbeef",
        "baselineDirtyPaths": [],
        "problemStatement": "Preflight Review readiness を検証する。",
        "intent": {
            "problem": "Readiness derivation を検証する。",
            "constraints": [
                "Contract schema に新しい field を追加しない。",
                "review は make entrypoint で生成する。",
            ],
            "rationale": "Evidence over Self-Declaration を固定する。",
        },
        "scope": [
            "scripts/ai_preflight_review.py",
            "scripts/ai_preflight.py",
            "scripts/ai_generate_status.py",
            "scripts/ai_check_status.py",
            ".ai/guards/preflight_review_policy.yaml",
            ".ai/glossary.md",
        ],
        "outOfScope": [
            "src/**",
            "Cargo.toml",
        ],
        "sources": [
            {
                "path": "docs/specs/DEVELOPMENT_PROTOCOL.md",
                "reason": "Preflight Pause Rule の参照。",
            },
            {
                "path": "scripts/ai_preflight_review.py",
                "reason": "Preflight Review 実装の参照。",
            },
        ],
        "unknowns": [] if ready else ["open question about follow-up sequencing"],
        "notCodable": False if ready else True,
        "riskAssessment": {
            "level": "low",
            "riskTypes": ["governance_process"],
            "reason": "review-only path の検証。",
        },
        "agentCapability": {
            "canImplement": True,
            "canVerify": True,
            "needsHumanDecision": False if ready else True,
            "blockedReason": "" if ready else "manual clarification required for the test fixture",
        },
        "executionDecision": {
            "status": "continue" if ready else "contract_update_required",
            "reason": "fixture is ready for review" if ready else "fixture is intentionally blocked",
        },
        "preReviewWarnings": ["Preflight Review は reviewer visibility である。"],
        "checkpointPolicy": {
            "requiredCheckpoints": [
                "contract_start",
                "before_edit",
                "before_ready",
                "after_verification",
            ],
            "reminder": "fixture checkpoint reminder",
        },
        "acceptance": [
            "target/ai_preflight_review.json is generated from contract evidence.",
            "make ai-preflight shows the preflight review before coding continues.",
            "current_status.md renders Status, Recommendation, Decision Drivers, and Pause Rule.",
        ],
        "verification": [
            {"command": "make fmt-check", "required": True},
            {"command": "make test", "required": True},
            {"command": "make clippy", "required": True},
        ],
        "destructiveChangePolicy": {"allowed": False, "requiresHumanApproval": True, "allowPatterns": []},
        "rollbackNote": "fixture rollback",
    }
    return contract


def ready_report(root: Path) -> tuple[Path, Path, Path, dict]:
    contract_path = root / ".ai/work-items/active/preflight-ready.contract.json"
    policy_path = root / ".ai/guards/preflight_review_policy.yaml"
    report_path = root / "target/ai_preflight_review.json"
    contract = base_contract("preflight-ready", ready=True)
    write_json(contract_path, contract)
    write_policy(policy_path, gate_enabled=False)
    report = ai_preflight_review.derive_report(contract, contract_path=contract_path, policy_path=policy_path)
    write_json(report_path, report)
    return contract_path, policy_path, report_path, report


def blocked_contract(root: Path) -> tuple[Path, Path, dict, dict]:
    contract_path = root / ".ai/work-items/active/preflight-blocked.contract.json"
    policy_path = root / ".ai/guards/preflight_review_policy.yaml"
    contract = base_contract("preflight-blocked", ready=False)
    write_json(contract_path, contract)
    write_policy(policy_path, gate_enabled=True)
    report = ai_preflight_review.derive_report(contract, contract_path=contract_path, policy_path=policy_path)
    policy = ai_preflight_review.load_policy(policy_path)
    return contract_path, policy_path, report, policy


def test_derive_ready_report(root: Path) -> None:
    contract_path, policy_path, _, report = ready_report(root)
    policy = ai_preflight_review.load_policy(policy_path)
    assert_equal(report["status"], "ready", "ready fixture should derive ready")
    assert_equal(ai_preflight_review.validate_report_structure(report), [], "ready report should validate")
    assert_true(
        not ai_preflight_review.report_is_blocked(report, policy),
        "advisory policy should not block ready report",
    )
    markdown = ai_preflight_review.render_markdown(report)
    assert_true("Status:" in markdown, "markdown should include status")
    assert_true("Decision Drivers:" in markdown, "markdown should include decision drivers")
    assert_true("Pause Rule:" in markdown, "markdown should include pause rule")
    assert_true(contract_path.exists(), "contract fixture should exist")


def test_gate_blocks_not_ready(root: Path) -> None:
    contract_path, policy_path, report, policy = blocked_contract(root)
    assert_true(report["status"] != "ready", "blocked fixture should not be ready")
    assert_true(
        ai_preflight_review.report_is_blocked(report, policy),
        "gate-enabled policy should block not-ready report",
    )
    issues = ai_preflight_review.validate_report_structure(report)
    assert_equal(issues, [], "blocked report should still match the report schema")
    assert_true(contract_path.exists(), "blocked contract fixture should exist")
    assert_true(policy_path.exists(), "policy fixture should exist")


def test_readiness_blockers_are_not_ready(root: Path) -> None:
    contract_path = root / ".ai/work-items/active/preflight-variants.contract.json"
    policy_path = root / ".ai/guards/preflight_review_policy.yaml"
    write_policy(policy_path, gate_enabled=True)
    variants = [
        ("notCodable", True),
        ("executionDecision", {"status": "blocked", "reason": "fixture"}),
        ("agentCapability", {"canImplement": False, "canVerify": True, "needsHumanDecision": False, "blockedReason": "fixture"}),
        ("agentCapability", {"canImplement": True, "canVerify": False, "needsHumanDecision": False, "blockedReason": "fixture"}),
        ("agentCapability", {"canImplement": True, "canVerify": True, "needsHumanDecision": True, "blockedReason": "fixture"}),
    ]
    for key, value in variants:
        contract = base_contract("preflight-variants", ready=True)
        contract[key] = value
        write_json(contract_path, contract)
        report = ai_preflight_review.derive_report(contract, contract_path=contract_path, policy_path=policy_path)
        assert_equal(report["status"], "not_ready", f"{key} should block readiness")


def test_generate_does_not_write_markdown(root: Path) -> None:
    contract_path = root / ".ai/work-items/active/preflight-ready.contract.json"
    policy_path = root / ".ai/guards/preflight_review_policy.yaml"
    output_path = root / "target/ai_preflight_review.json"
    contract = base_contract("preflight-ready", ready=True)
    write_json(contract_path, contract)
    write_policy(policy_path, gate_enabled=False)
    with patch.object(
        sys,
        "argv",
        [
            "ai_preflight_review.py",
            "--contract",
            str(contract_path),
            "--output",
            str(output_path),
            "--policy",
            str(policy_path),
        ],
    ):
        code = ai_preflight_review.main()
    assert_equal(code, 0, "generate should succeed")
    assert_true(output_path.exists(), "json report should be generated")
    assert_true(not output_path.with_suffix(".md").exists(), "markdown report must not be generated")


def test_check_blocks_when_gate_enabled(root: Path) -> None:
    contract_path = root / ".ai/work-items/active/preflight-blocked.contract.json"
    policy_path = root / ".ai/guards/preflight_review_policy.yaml"
    output_path = root / "target/ai_preflight_review.json"
    contract = base_contract("preflight-blocked", ready=False)
    write_json(contract_path, contract)
    write_policy(policy_path, gate_enabled=True)
    report = ai_preflight_review.derive_report(contract, contract_path=contract_path, policy_path=policy_path)
    write_json(output_path, report)
    with patch.object(
        sys,
        "argv",
        [
            "ai_preflight_review.py",
            "--check",
            "--contract",
            str(contract_path),
            "--output",
            str(output_path),
            "--policy",
            str(policy_path),
        ],
    ):
        code = ai_preflight_review.main()
    assert_equal(code, 1, "gate-enabled check should fail for not-ready report")


def test_confirmed_human_review_allows_explicit_risk_acceptance(root: Path) -> None:
    contract_path = root / ".ai/work-items/active/preflight-confirmed.contract.json"
    policy_path = root / ".ai/guards/preflight_review_policy.yaml"
    contract = base_contract("preflight-confirmed", ready=True)
    contract["riskAssessment"]["level"] = "high"
    contract["scenarioCoverage"] = [
        {
            "scenario": "hosted external verification",
            "required": True,
            "status": "unverified",
            "evidence": [],
            "reason": "requires a hosted runner",
        }
    ]
    contract["humanReview"] = {
        "status": "confirmed",
        "decision": "continue_with_unverified_external_scenario",
        "openQuestions": ["hosted runner restore remains unverified"],
    }
    write_json(contract_path, contract)
    write_policy(policy_path, gate_enabled=True)
    report = ai_preflight_review.derive_report(
        contract,
        contract_path=contract_path,
        policy_path=policy_path,
    )
    assert_equal(report["status"], "ready", "confirmed human review should clear advisory pause")
    assert_true(
        any(signal["name"] == "Human Review" and signal["value"] == "Ready" for signal in report["signals"]),
        "report should expose the human review decision",
    )


def test_status_renderer_includes_preflight_section(root: Path) -> None:
    contract_path, policy_path, report_path, report = ready_report(root)
    contract = base_contract("preflight-ready", ready=True)
    status_path = root / ".ai/cockpit/current_status.md"
    summary_path = root / ".ai/work-items/active/preflight-ready.summary.json"
    summary = {
        "workItemId": "preflight-ready",
        "contractPath": contract_path.relative_to(root).as_posix(),
        "changedFiles": [
            {"path": contract_path.relative_to(root).as_posix(), "reason": "fixture"},
            {"path": summary_path.relative_to(root).as_posix(), "reason": "fixture"},
        ],
        "sourcesUsed": [contract_path.relative_to(root).as_posix()],
        "verification": [
            {"command": "make fmt-check", "result": "passed"},
            {"command": "make test", "result": "passed"},
            {"command": "make clippy", "result": "passed"},
        ],
        "unknownsRemaining": [],
        "risk": {"level": "low", "detail": "ready fixture"},
        "generatedFiles": [],
        "destructiveChanges": [],
        "observedIssues": [],
        "checkpointEvidence": [
            {
                "stage": stage,
                "recorded": True,
                "detail": "fixture checkpoint",
                "contractHash": ai_generate_status.contract_hash(contract_path),
                "acceptanceCount": len(contract["acceptance"]),
                "unknownCount": 0,
                "requiredChecks": len(contract["verification"]),
                "requiredChecksPassed": 3,
            }
            for stage in [
                "contract_start",
                "before_edit",
                "before_ready",
                "after_verification",
            ]
        ],
        "checkpointReview": [
            {"checkpoint": stage, "status": "confirmed", "note": "fixture"}
            for stage in [
                "contract_start",
                "before_edit",
                "before_ready",
                "after_verification",
            ]
        ],
        "residualRisks": [],
        "reviewReadiness": {
            "status": "ready_with_risks",
            "reason": "fixture",
            "expectedReviewFocus": ["scope boundary"],
        },
    }
    write_json(summary_path, summary)
    write_json(report_path, report)
    class FakeObservability:
        def status_generated(self, **_: object) -> None:
            return None

    with (
        patch.object(ai_generate_status, "PROJECT_ROOT", root),
        patch.object(ai_generate_status, "DEFAULT_OUTPUT", status_path),
        patch.object(ai_generate_status, "BACKTRACK_REPORT", root / "target/ai_backtrack_report.json"),
        patch.object(ai_generate_status, "DEFAULT_PREFLIGHT_REVIEW", report_path),
        patch.object(ai_generate_status, "create_observability", return_value=FakeObservability()),
        patch.object(
            sys,
            "argv",
            [
                "ai_generate_status.py",
                str(contract_path),
                "--summary",
                str(summary_path),
                "--output",
                str(status_path),
                "--observability-log",
                str(root / "target/ai_observability.jsonl"),
            ],
        ),
    ):
        code = ai_generate_status.main()
    assert_equal(code, 0, "ai_generate_status should succeed")
    text = status_path.read_text(encoding="utf-8")
    assert_true("## Preflight Review" in text, "status should include preflight review section")
    assert_true("- Status: `ready`" in text, "status should include preflight status")
    assert_true("- Recommendation:" in text, "status should include preflight recommendation")
    assert_true("- Decision Drivers:" in text, "status should include preflight drivers")
    assert_true("- Pause Rule:" in text, "status should include pause rule")


def test_preflight_main_runs_review_targets(root: Path) -> None:
    contract_path = root / ".ai/work-items/active/preflight-ready.contract.json"
    write_json(contract_path, base_contract("preflight-ready", ready=True))
    calls: list[tuple[str, str, str | None]] = []

    def fake_run_check(label: str, script: Path) -> int:
        calls.append(("check", label, script.as_posix()))
        return 0

    def fake_run_make(label: str, target: str, *, contract: Path | None = None) -> int:
        calls.append(("make", label, target if contract is None else contract.relative_to(root).as_posix()))
        return 0

    with (
        patch.object(ai_preflight, "PROJECT_ROOT", root),
        patch.object(ai_preflight, "ACTIVE_DIR", root / ".ai/work-items/active"),
        patch.object(ai_preflight, "CURRENT_STATUS", root / ".ai/cockpit/current_status.md"),
        patch.object(ai_preflight, "ARCHIVE_DIR", root / ".ai/work-items/archive"),
        patch.object(ai_preflight, "run_check", fake_run_check),
        patch.object(ai_preflight, "run_make", fake_run_make),
        patch.object(ai_preflight, "active_contracts", return_value=[contract_path]),
    ):
        code = ai_preflight.main()
    assert_equal(code, 0, "ai-preflight should succeed")
    assert_true(
        any(item[1] == "preflight review generation" for item in calls if item[0] == "make"),
        "ai-preflight should run generate-ai-preflight-review",
    )
    assert_true(
        any(item[1] == "preflight review validation" for item in calls if item[0] == "make"),
        "ai-preflight should run check-ai-preflight-review",
    )


def test_ai_start_runs_post_skeleton_preflight(root: Path) -> None:
    active = root / ".ai/work-items/active"
    calls: list[list[str]] = []
    preflight_calls: list[int] = []
    status_calls: list[tuple[Path, Path]] = []
    events: list[str] = []

    def fake_run(command: list[str], **_: object) -> object:
        calls.append(command)

        class Result:
            returncode = 0

        return Result()

    def fake_run_preflight_checks() -> int:
        preflight_calls.append(1)
        events.append("preflight")
        write_preflight_review(root, "ready")
        return 0

    def fake_generate_status(contract_path: Path, summary_path: Path) -> int:
        status_calls.append((contract_path, summary_path))
        events.append("status")
        return 0

    with (
        patch.object(ai_start, "PROJECT_ROOT", root),
        patch.object(ai_start, "ACTIVE_DIR", active),
        patch.object(ai_start, "PREFLIGHT_REVIEW_PATH", root / "target" / "ai_preflight_review.json"),
        patch.object(ai_start, "current_head", return_value="abc123"),
        patch.object(
            ai_start,
            "baseline_dirty_paths",
            return_value=[
                {
                    "path": "scripts/ai_start.py",
                    "status": "M",
                    "fingerprint": "deadbeef",
                }
            ],
        ),
        patch.object(ai_start, "run_preflight_checks", fake_run_preflight_checks),
        patch.object(ai_start, "generate_active_status", fake_generate_status),
        patch.object(ai_start.subprocess, "run", fake_run),
        patch.object(
            sys,
            "argv",
            [
                "ai_start.py",
                "--task",
                "sample-start-status",
                "--title",
                "Sample Start Status",
                "--mode",
                "code",
            ],
        ),
    ):
        code = ai_start.main()

    assert_equal(code, 0, "ai_start should succeed")
    assert_equal(len(preflight_calls), 2, "code mode should run preflight twice")
    assert_equal(len(status_calls), 2, "status generation should run twice")
    assert_equal(
        events,
        ["preflight", "status", "preflight", "status"],
        "ai_start should preflight before skeleton generation, then refresh status after code-mode preflight",
    )
    contract = json.loads((active / "sample-start-status.contract.json").read_text(encoding="utf-8"))
    summary = json.loads((active / "sample-start-status.summary.json").read_text(encoding="utf-8"))
    assert_true(contract["contractVersion"] == 2, "contract should be v2")
    assert_true(contract["baseCommit"] == "abc123", "contract should record baseCommit")
    assert_true(
        contract["baselineDirtyPaths"]
        == [
            {
                "path": "scripts/ai_start.py",
                "status": "M",
                "fingerprint": "deadbeef",
            }
        ],
        "contract should record baselineDirtyPaths",
    )
    assert_true("checkpointEvidence" in summary, "summary should contain checkpointEvidence")
    assert_true(
        [item["stage"] for item in summary["checkpointEvidence"]] == [
            "contract_start",
            "before_edit",
            "before_ready",
            "after_verification",
        ],
        "summary should prefill checkpointEvidence chain",
    )
    assert_true(status_calls[0] == status_calls[1], "status generation should target the same contract and summary")


def test_preflight_review_templates_exist(root: Path) -> None:
    assert_true((root / "templates/agents/AI_COCKPIT_RULES.md").exists(), "agent rules template should exist")
    assert_true((root / "templates/glossary.md").exists(), "glossary template should exist")
    assert_true((root / "templates/make/Makefile.ai").exists(), "Makefile.ai template should exist")


def main() -> int:
    root = REPO_ROOT / "target" / "ai_preflight_review_test"
    shutil.rmtree(root, ignore_errors=True)
    try:
        with tempfile.TemporaryDirectory(dir=REPO_ROOT / "target") as temp_dir:
            temp_root = Path(temp_dir)
            test_derive_ready_report(temp_root)
            test_gate_blocks_not_ready(temp_root)
            test_readiness_blockers_are_not_ready(temp_root)
            test_generate_does_not_write_markdown(temp_root)
            test_check_blocks_when_gate_enabled(temp_root)
        test_status_renderer_includes_preflight_section(temp_root)
        test_confirmed_human_review_allows_explicit_risk_acceptance(temp_root)
        test_preflight_main_runs_review_targets(temp_root)
        test_ai_start_runs_post_skeleton_preflight(temp_root)
        test_preflight_review_templates_exist(REPO_ROOT)
        print("✅ preflight_review tests passed")
        return 0
    finally:
        shutil.rmtree(root, ignore_errors=True)


if __name__ == "__main__":
    raise SystemExit(main())
