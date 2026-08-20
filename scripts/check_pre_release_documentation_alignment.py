#!/usr/bin/env python3
"""Derive and verify current pre-release documentation alignment evidence."""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path, PurePosixPath
from typing import Any

from check_trust_layer_docs import check_repository as check_trust_layer

ROOT = Path(__file__).resolve().parents[1]
JSON_REPORT = Path("docs/reference/pre-release-documentation-alignment.json")
MARKDOWN_REPORT = Path("docs/reference/pre-release-documentation-alignment.md")
WORK_ITEM_ID = "documentation-alignment-ruff016-binary-corrective-20260730"
UPDATED_SURFACES = {"docs/superpowers/plans/2026-07-25-ai-cockpit-comprehensive-remediation.md"}

SURFACES: dict[str, tuple[str, tuple[str, ...]]] = {
    "README.md": ("language_entry", ("Human-Agent Trust Layer", "Repository Governance Layer")),
    "README.zh-CN.md": (
        "language_entry",
        ("Human-Agent Trust Layer", "Repository Governance Layer"),
    ),
    "README.ja.md": ("language_entry", ("Human-Agent Trust Layer", "Repository Governance Layer")),
    "docs/trust-layer.md": (
        "trust_authority",
        ("Repository Governance Layer", "Evidence over Self-Declaration"),
    ),
    "docs/trust-layer.zh-CN.md": (
        "trust_authority",
        ("Repository Governance Layer", "Evidence over Self-Declaration"),
    ),
    "docs/trust-layer.ja.md": (
        "trust_authority",
        ("Repository Governance Layer", "Evidence over Self-Declaration"),
    ),
    "docs/reference/documentation-architecture.md": (
        "documentation_map",
        (
            "Trust Layer: why AI Cockpit exists",
            "Capability Truth Matrix: current implementation status",
        ),
    ),
    "docs/reference/documentation-architecture.ja.md": (
        "documentation_map",
        ("Trust Layer", "Capability Truth Matrix"),
    ),
    "docs/reference/capability-truth-matrix.md": (
        "capability_authority",
        ("source of truth", "Repository Governance Layer"),
    ),
    "docs/reference/capability-truth-matrix.json": (
        "capability_authority",
        ("implemented", "limitations"),
    ),
    "docs/getting-started/security-release-verification.md": (
        "release_evidence_boundary",
        ("Capability Truth Matrix", "SBOM"),
    ),
    "docs/getting-started/security-release-verification.zh-CN.md": (
        "release_evidence_boundary",
        ("能力事实矩阵", "SBOM"),
    ),
    "docs/getting-started/security-release-verification.ja.md": (
        "release_evidence_boundary",
        ("Capability Truth Matrix", "SBOM"),
    ),
    "docs/reference/japanese-capability-assessment.json": (
        "japanese_release_gate",
        ("final_reassessment", "blockingFindings"),
    ),
    "docs/reference/japanese-capability-assessment.md": (
        "japanese_release_gate",
        ("final_reassessment", "blocking"),
    ),
    "docs/superpowers/plans/2026-07-25-ai-cockpit-comprehensive-remediation.md": (
        "serial_execution_plan",
        ("文档对齐", "WI-18", "WI-19"),
    ),
}
FORBIDDEN_CLAIMS = (
    "independently guarantees enterprise compliance",
    "candidate evidence proves this version is publicly released",
    "repository tests prove provider identity and runtime isolation",
)


class AlignmentError(ValueError):
    """Raised for invalid alignment evidence."""


def digest(value: Any) -> str:
    payload = dict(value) if isinstance(value, dict) else value
    if isinstance(payload, dict):
        payload.pop("digest", None)
    encoded = json.dumps(
        payload, ensure_ascii=False, sort_keys=True, separators=(",", ":")
    ).encode()
    return f"sha256:{hashlib.sha256(encoded).hexdigest()}"


def file_digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def normalized_path(raw: str) -> str:
    path = PurePosixPath(raw)
    if path.is_absolute() or ".." in path.parts or path.as_posix() in {"", "."}:
        raise AlignmentError(f"path escapes repository: {raw}")
    return path.as_posix()


def bound_evidence_errors(root: Path, assessment: dict[str, Any]) -> list[str]:
    rows = assessment.get("evidenceSource", {}).get("files")
    if not isinstance(rows, list):
        return ["Japanese assessment evidenceSource.files is not a list"]
    errors: list[str] = []
    seen: set[str] = set()
    for row in rows:
        if (
            not isinstance(row, dict)
            or not isinstance(row.get("path"), str)
            or not isinstance(row.get("sha256"), str)
        ):
            errors.append("Japanese assessment has malformed bound evidence")
            continue
        try:
            path = normalized_path(row["path"])
        except AlignmentError:
            errors.append(f"Japanese bound path escapes repository: {row['path']}")
            continue
        if path in seen:
            errors.append(f"Japanese bound path is duplicated: {path}")
            continue
        seen.add(path)
        candidate = root / path
        if not candidate.is_file():
            errors.append(f"Japanese bound path is missing: {path}")
        elif file_digest(candidate) != row["sha256"]:
            errors.append(f"Japanese bound evidence drift: {path}")
    return errors


def marker_errors(path: str, text: str, markers: tuple[str, ...]) -> list[str]:
    normalized = " ".join(text.split())
    return [
        f"{path}: missing required marker: {marker}"
        for marker in markers
        if " ".join(marker.split()) not in normalized
    ]


def claim_errors(path: str, text: str) -> list[str]:
    normalized = " ".join(text.lower().split())
    return [
        f"{path}: forbidden external-control claim: {claim}"
        for claim in FORBIDDEN_CLAIMS
        if claim in normalized
    ]


def build_report(root: Path = ROOT) -> dict[str, Any]:
    errors: list[str] = []
    surface_rows: list[dict[str, Any]] = []
    for raw_path, (role, markers) in SURFACES.items():
        path = normalized_path(raw_path)
        candidate = root / path
        if not candidate.is_file():
            errors.append(f"required surface is missing: {path}")
            continue
        text = candidate.read_text(encoding="utf-8")
        errors.extend(marker_errors(path, text, markers))
        errors.extend(claim_errors(path, text))
        decision = "updated" if path in UPDATED_SURFACES else "no_change"
        rationale = (
            "Updated current execution state to identify this fresh, source-bound alignment Work Item."
            if decision == "updated"
            else "Current source preserves the assigned authority, reader route, and documented limitation."
        )
        surface_rows.append(
            {
                "path": path,
                "role": role,
                "decision": decision,
                "rationale": rationale,
                "sha256": file_digest(candidate),
            }
        )
    assessment_path = root / "docs/reference/japanese-capability-assessment.json"
    assessment = json.loads(assessment_path.read_text(encoding="utf-8"))
    errors.extend(check_trust_layer(root))
    errors.extend(bound_evidence_errors(root, assessment))
    if assessment.get("workItemRole") != "final_reassessment":
        errors.append("Japanese assessment is not a final reassessment")
    if assessment.get("blockingFindings") != []:
        errors.append("Japanese final reassessment has blocking findings")
    unique_errors = list(dict.fromkeys(errors))
    plan_path = "docs/superpowers/plans/2026-07-25-ai-cockpit-comprehensive-remediation.md"
    report: dict[str, Any] = {
        "schemaVersion": 1,
        "workItemId": WORK_ITEM_ID,
        "status": "aligned" if not unique_errors else "blocked",
        "surfaces": surface_rows,
        "checks": [
            {
                "checkId": "surface-inventory-and-markers",
                "status": "pass"
                if not [
                    error
                    for error in unique_errors
                    if "missing required marker" in error or "required surface" in error
                ]
                else "fail",
            },
            {
                "checkId": "trust-layer-contract",
                "status": "pass" if not check_trust_layer(root) else "fail",
            },
            {
                "checkId": "japanese-source-binding",
                "status": "pass" if not bound_evidence_errors(root, assessment) else "fail",
            },
            {
                "checkId": "capability-and-release-boundary",
                "status": "pass"
                if not [
                    error for error in unique_errors if "forbidden external-control claim" in error
                ]
                else "fail",
            },
            {
                "checkId": "serial-plan-stage",
                "status": "pass"
                if not marker_errors(
                    plan_path,
                    (root / plan_path).read_text(encoding="utf-8"),
                    SURFACES[plan_path][1],
                )
                else "fail",
            },
        ],
        "blockingFindings": [
            {"findingId": f"DOC-ALIGN-{index:03d}", "severity": "blocking", "detail": detail}
            for index, detail in enumerate(unique_errors, start=1)
        ],
        "limitations": [
            "This deterministic audit does not prove native-human translation quality.",
            "Repository documentation does not prove provider identity, runtime isolation, immutable external audit, enterprise compliance, or publication.",
        ],
        "nextStages": [
            "pre-release-deprecated-assets-cleanup",
            "pre-release-real-absurd-injection-assessment",
            "final Japanese reassessment",
            "WI-18 publish-new-version",
            "WI-19 clean-execution-plan-documents",
        ],
    }
    report["surfaceCount"] = len(surface_rows)
    report["digest"] = digest(report)
    return report


def render_markdown(report: dict[str, Any]) -> str:
    lines = [
        "---",
        "author: Ray",
        'title: "Pre-release Documentation Alignment Report"',
        'description: "Derived view of current, source-bound documentation alignment evidence."',
        "generated: true",
        "---",
        "",
        "# Pre-release Documentation Alignment Report",
        "",
        f"- Work Item: `{report['workItemId']}`",
        f"- Status: `{report['status']}`",
        f"- Surfaces: `{report['surfaceCount']}`",
        f"- Digest: `{report['digest']}`",
        "",
        "## Surface decisions",
        "",
        "| Path | Role | Decision | Rationale |",
        "| --- | --- | --- | --- |",
    ]
    lines.extend(
        f"| `{row['path']}` | `{row['role']}` | `{row['decision']}` | {row['rationale']} |"
        for row in report["surfaces"]
    )
    lines.extend(["", "## Checks", ""])
    lines.extend(f"- `{row['checkId']}`: **{row['status']}**" for row in report["checks"])
    lines.extend(["", "## Limitations", ""])
    lines.extend(f"- {item}" for item in report["limitations"])
    if report["blockingFindings"]:
        lines.extend(["", "## Blocking findings", ""])
        lines.extend(
            f"- `{item['findingId']}`: {item['detail']}" for item in report["blockingFindings"]
        )
    return "\n".join(lines) + "\n"


def generated_artifact_errors(root: Path, report: dict[str, Any]) -> list[str]:
    """Reject checked-in generated views that no longer represent current sources."""
    expected_json = json.dumps(report, ensure_ascii=False, indent=2) + "\n"
    expected_markdown = render_markdown(report)
    refresh_command = "python3 scripts/check_pre_release_documentation_alignment.py --write"
    errors: list[str] = []
    for path, expected, label in (
        (JSON_REPORT, expected_json, "JSON"),
        (MARKDOWN_REPORT, expected_markdown, "Markdown"),
    ):
        candidate = root / path
        if not candidate.is_file() or candidate.read_text(encoding="utf-8") != expected:
            errors.append(
                f"generated documentation-alignment {label} is stale: {path}; run {refresh_command}"
            )
    return errors


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--write", action="store_true")
    args = parser.parse_args()
    report = build_report()
    if args.write:
        (ROOT / JSON_REPORT).write_text(
            json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
        )
        (ROOT / MARKDOWN_REPORT).write_text(render_markdown(report), encoding="utf-8")
    artifact_errors = [] if args.write else generated_artifact_errors(ROOT, report)
    if report["status"] != "aligned" or artifact_errors:
        print("[ERROR] documentation alignment is blocked", file=sys.stderr)
        for finding in report["blockingFindings"]:
            print(f"[ERROR] {finding['detail']}", file=sys.stderr)
        for error in artifact_errors:
            print(f"[ERROR] {error}", file=sys.stderr)
        return 2
    print("pre-release documentation alignment passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
