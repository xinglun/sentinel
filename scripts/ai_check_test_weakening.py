#!/usr/bin/env python3
"""Emit evidence-backed signals when a Git diff weakens test verification."""

from __future__ import annotations

import argparse
import difflib
import json
import re
import subprocess  # nosec B404 - fixed list-form Git interrogation only
import sys
from pathlib import Path, PurePosixPath
from typing import Any

from ai_common import parse_yaml

EXIT = {"continue": 0, "warning": 0, "review": 2, "needs_human_confirmation": 2, "block": 3}
TEST_PATH = re.compile(r"(^|/)(tests?|specs?)(/|$)|(^|/).*(test|spec).*\.[^.]+$", re.IGNORECASE)
ASSERTION = re.compile(
    r"\bassert\b|\bexpect\s*\(|\bassert(?:Equals?|True|False|That|Throws?)\s*\(|"
    r"\bXCTAssert\w*\s*\(|\bshould\s*(?:\.|\()",
    re.IGNORECASE,
)
SKIP = re.compile(
    r"pytest\.mark\.(?:skip|skipif)|unittest\.skip|\b(?:xit|xdescribe|xtest)\s*\(|"
    r"\b(?:test|it|describe)\.skip\s*\(|@(?:Ignore|Disabled)\b|\bDISABLED_",
    re.IGNORECASE,
)
TEST_CASE = re.compile(
    r"(?m)^\s*(?:async\s+)?def\s+(test_[A-Za-z0-9_]+)|"
    r"\b(?:test|it)\s*\(\s*['\"]([^'\"]+)",
)
EXCEPTION_ASSERTION = re.compile(
    r"pytest\.raises\s*\(|assertRaises\s*\(|toThrow\s*\(|XCTAssertThrowsError\s*\(|"
    r"assertThrows?\s*\(",
    re.IGNORECASE,
)
BYPASS_REQUEST = re.compile(
    r"(?:delete|remove|disable|skip|ignore|turn\s+off).{0,40}(?:failing\s+)?tests?.{0,40}"
    r"(?:pass|green|ci)|(?:pass|green|ci).{0,40}(?:delete|remove|disable|skip).{0,40}tests?",
    re.IGNORECASE | re.DOTALL,
)
COVERAGE_TO_PASS = re.compile(
    r"(?:lower|reduce|decrease).{0,30}coverage.{0,40}(?:pass|green)|"
    r"(?:pass|green).{0,40}(?:lower|reduce|decrease).{0,30}coverage",
    re.IGNORECASE | re.DOTALL,
)
COVERAGE_PATH = re.compile(
    r"(^|/)(?:\.coveragerc|coverage\.(?:json|ya?ml)|pyproject\.toml|tox\.ini|"
    r"jest\.config\.[^/]+|package\.json)$",
    re.IGNORECASE,
)
SNAPSHOT_PATH = re.compile(
    r"(?:^|/)(?:__snapshots__|snapshots?)/|\.(?:snap|snapshot)$", re.IGNORECASE
)
WORKFLOW_PATH = re.compile(
    r"(^|/)(?:\.github/workflows/.*\.ya?ml|\.gitlab-ci\.ya?ml|Makefile|"
    r"pyproject\.toml|tox\.ini|package\.json)$",
    re.IGNORECASE,
)
DEFAULT_THRESHOLDS = {
    "materialAssertionMinimumBefore": 4,
    "materialAssertionRatio": 0.6,
    "snapshotReviewChangedLines": 20,
}
RETIREMENT_EVIDENCE_DIR = ".ai/evidence/test-weakening"


class InputError(ValueError):
    """The requested repository evidence cannot be inspected safely."""


def _git(root: Path, *args: str, check: bool = True) -> subprocess.CompletedProcess[str]:
    return subprocess.run(  # nosec B603 B607 - executable and argument list are fixed
        ["git", *args], cwd=root, capture_output=True, text=True, check=check
    )


def _git_bytes(root: Path, *args: str, check: bool = True) -> subprocess.CompletedProcess[bytes]:
    return subprocess.run(  # nosec B603 B607 - executable and argument list are fixed
        ["git", *args], cwd=root, capture_output=True, check=check
    )


def _text_content(data: bytes) -> str | None:
    if b"\x00" in data:
        return None
    try:
        return data.decode("utf-8")
    except UnicodeDecodeError:
        return None


def _safe_relative(value: str) -> str:
    normalized = value.replace("\\", "/")
    path = PurePosixPath(normalized)
    if path.is_absolute() or not normalized or ".." in path.parts:
        raise InputError(f"unsafe repository path: {value}")
    return path.as_posix()


def _read_before(root: Path, base: str, path: str) -> str | None:
    result = _git_bytes(root, "show", f"{base}:{path}", check=False)
    return _text_content(result.stdout) if result.returncode == 0 else ""


def _read_after(root: Path, path: str) -> str | None:
    candidate = root / path
    if not candidate.exists():
        return ""
    resolved = candidate.resolve()
    try:
        resolved.relative_to(root.resolve())
    except ValueError as exc:
        raise InputError(f"path escapes repository through symlink: {path}") from exc
    if not resolved.is_file():
        raise InputError(f"changed path is not a regular file: {path}")
    return _text_content(resolved.read_bytes())


def _changed_paths(root: Path, base: str) -> list[tuple[str, str, str]]:
    if _git(root, "rev-parse", "--verify", f"{base}^{{commit}}", check=False).returncode:
        raise InputError(f"invalid Git base revision: {base}")
    result = _git(root, "diff", "--name-status", "--find-renames", base, "--")
    changes: list[tuple[str, str, str]] = []
    for line in result.stdout.splitlines():
        fields = line.split("\t")
        status = fields[0]
        if status.startswith("R") and len(fields) == 3:
            old_path = _safe_relative(fields[1])
            changes.append(("R", _safe_relative(fields[2]), old_path))
        elif len(fields) == 2:
            path = _safe_relative(fields[1])
            changes.append((status[:1], path, path))
    return changes


def _signal(kind: str, path: str, severity: str, **evidence: Any) -> dict[str, Any]:
    return {"type": kind, "path": path, **evidence, "severity": severity}


def _case_names(content: str) -> set[str]:
    return {left or right for left, right in TEST_CASE.findall(content)}


def _negative_case(name: str) -> bool:
    return bool(
        re.search(
            r"invalid|reject|den(?:y|ied)|error|fail|negative|unauthori[sz]ed|forbid|missing",
            name,
            re.IGNORECASE,
        )
    )


def _assignment_values(content: str, names: tuple[str, ...]) -> list[str]:
    name_pattern = "|".join(re.escape(name) for name in names)
    pattern = re.compile(rf"(?im)^\s*(?:{name_pattern})\s*[:=]\s*([^#\n]+)")
    return [value.strip().strip("'\"") for value in pattern.findall(content)]


def _number(content: str, names: tuple[str, ...]) -> float | None:
    values = _assignment_values(content, names)
    if not values:
        return None
    match = re.search(r"\d+(?:\.\d+)?", values[-1])
    return float(match.group()) if match else None


def _items(values: list[str]) -> set[str]:
    return {item.strip() for value in values for item in re.split(r"[,\s]+", value) if item.strip()}


def _diff_lines(before: str, after: str) -> tuple[list[str], list[str]]:
    removed: list[str] = []
    added: list[str] = []
    for line in difflib.ndiff(before.splitlines(), after.splitlines()):
        if line.startswith("- "):
            removed.append(line[2:])
        elif line.startswith("+ "):
            added.append(line[2:])
    return removed, added


def _coverage_signals(path: str, before: str, after: str) -> list[dict[str, Any]]:
    signals: list[dict[str, Any]] = []
    before_exclusions = _items(
        _assignment_values(
            before, ("omit", "exclude", "exclude_lines", "coveragePathIgnorePatterns")
        )
    )
    after_exclusions = _items(
        _assignment_values(
            after, ("omit", "exclude", "exclude_lines", "coveragePathIgnorePatterns")
        )
    )
    added_exclusions = sorted(after_exclusions - before_exclusions)
    if added_exclusions:
        signals.append(
            _signal(
                "coverage_exclusion_added",
                path,
                "high",
                before=sorted(before_exclusions),
                after=sorted(after_exclusions),
                added=added_exclusions,
            )
        )
    before_sources = _items(_assignment_values(before, ("source", "source_pkgs")))
    after_sources = _items(_assignment_values(after, ("source", "source_pkgs")))
    removed_sources = sorted(before_sources - after_sources)
    if removed_sources:
        signals.append(
            _signal(
                "coverage_source_reduced",
                path,
                "high",
                before=sorted(before_sources),
                after=sorted(after_sources),
                removed=removed_sources,
            )
        )
    before_threshold = _number(before, ("fail_under", "threshold", "minimum"))
    after_threshold = _number(after, ("fail_under", "threshold", "minimum"))
    if (
        before_threshold is not None
        and after_threshold is not None
        and after_threshold < before_threshold
    ):
        signals.append(
            _signal(
                "coverage_threshold_lowered",
                path,
                "high",
                before=before_threshold,
                after=after_threshold,
            )
        )
    return signals


def _workflow_signals(path: str, before: str, after: str) -> list[dict[str, Any]]:
    signals: list[dict[str, Any]] = []
    removed, added = _diff_lines(before, after)
    if any(
        re.search(
            r"\bcontinue-on-error\s*:\s*true\b|\ballow_failure\s*:\s*true\b", line, re.IGNORECASE
        )
        for line in added
    ):
        signals.append(_signal("ci_continue_on_error_added", path, "critical"))
    if any("|| true" in line or re.search(r";\s*exit\s+0\b", line) for line in added):
        signals.append(_signal("test_command_success_bypass_added", path, "critical"))
    optional_input_required_lines = _optional_workflow_input_required_lines(after)
    if any(
        re.search(r"\brequired\s*:\s*false\b", line, re.IGNORECASE)
        and line.strip() not in optional_input_required_lines
        for line in added
    ):
        signals.append(_signal("required_check_made_nonblocking", path, "high"))
    before_test_paths = {
        token.rstrip("'\";,)")
        for line in removed
        if re.search(r"\b(?:pytest|test|spec)\b", line, re.IGNORECASE)
        for token in re.findall(r"(?:tests?|specs?)/[^\s]+", line, re.IGNORECASE)
    }
    after_text = "\n".join(added)
    removed_test_paths = sorted(path for path in before_test_paths if path not in after_text)
    if removed_test_paths:
        signals.append(
            _signal(
                "test_command_scope_reduced",
                path,
                "high",
                removed=removed_test_paths,
            )
        )
    return signals


def _optional_workflow_input_required_lines(workflow: str) -> set[str]:
    """Return optional-input declarations, which are not CI check semantics."""
    optional: set[str] = set()
    inputs_indent: int | None = None
    for line in workflow.splitlines():
        stripped = line.strip()
        if not stripped or stripped.startswith("#"):
            continue
        indent = len(line) - len(line.lstrip())
        if stripped == "inputs:":
            inputs_indent = indent
            continue
        if inputs_indent is not None and indent <= inputs_indent:
            inputs_indent = None
        if (
            inputs_indent is not None
            and indent > inputs_indent
            and re.fullmatch(r"required\s*:\s*false", stripped, re.IGNORECASE)
        ):
            optional.add(stripped)
    return optional


def _load_policy(path: Path | None) -> dict[str, float]:
    thresholds = dict(DEFAULT_THRESHOLDS)
    if path is None:
        return thresholds
    try:
        raw = parse_yaml(path)
    except (OSError, ValueError) as exc:
        raise InputError(f"invalid test weakening policy: {exc}") from exc
    if not isinstance(raw, dict) or str(raw.get("version")) != "1":
        raise InputError("invalid test weakening policy: version must be 1")
    configured = raw.get("thresholds")
    if not isinstance(configured, dict):
        raise InputError("invalid test weakening policy: thresholds must be an object")
    for name, default in DEFAULT_THRESHOLDS.items():
        value = configured.get(name, default)
        try:
            numeric = float(value)
        except (TypeError, ValueError) as exc:
            raise InputError(f"invalid test weakening policy: {name} must be positive") from exc
        if isinstance(value, bool) or numeric <= 0:
            raise InputError(f"invalid test weakening policy: {name} must be positive")
        thresholds[name] = numeric
    if thresholds["materialAssertionRatio"] >= 1:
        raise InputError("invalid test weakening policy: materialAssertionRatio must be below 1")
    return thresholds


def _approved_retirement_evidence(
    root: Path, base: str, signals: list[dict[str, Any]]
) -> dict[str, Any] | None:
    """Return a narrow, source-bound approval for review-only test retirement.

    Evidence can never clear critical signals and must cover exactly the current
    base, affected paths, and reported signal types.  Missing or malformed
    records intentionally leave the normal review decision intact.
    """
    directory = root / RETIREMENT_EVIDENCE_DIR
    if (
        not directory.is_dir()
        or not signals
        or any(item["severity"] == "critical" for item in signals)
    ):
        return None
    paths = {str(item["path"]) for item in signals}
    types = {str(item["type"]) for item in signals}
    for candidate in sorted(directory.glob("*.json")):
        try:
            record = json.loads(candidate.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError):
            continue
        if not isinstance(record, dict) or record.get("version") != 1:
            continue
        if (
            record.get("baseRef") != base
            or record.get("decision") != "retire_cancelled_requirement"
        ):
            continue
        source = record.get("humanAuthorization")
        if not isinstance(source, dict) or not all(
            isinstance(source.get(key), str) and source[key].strip()
            for key in ("reference", "digest", "approvedBy")
        ):
            continue
        if (
            set(record.get("retiredPaths", [])) != paths
            or set(record.get("allowedSignals", [])) != types
        ):
            continue
        return {
            "status": "accepted",
            "path": candidate.relative_to(root).as_posix(),
            "source": source["reference"],
        }
    return None


def normalize_report(value: dict[str, Any]) -> dict[str, Any]:
    """Read the pre-version legacy report shape without inventing evidence."""
    if value.get("version") == 1:
        return dict(value)
    if "version" in value:
        raise InputError(f"unsupported test weakening report version: {value.get('version')}")
    decision = value.get("decision")
    signals = value.get("signals")
    if decision not in EXIT or not isinstance(signals, list):
        raise InputError("legacy test weakening report is malformed")
    return {
        "version": 1,
        "legacySourceVersion": 0,
        "decision": decision,
        "signals": signals,
        "requiredExplanation": bool(value.get("requiredExplanation", signals)),
        "recoveryCondition": "Legacy report requires renewed analysis.",
        "limitations": ["Legacy input does not contain Git base or analysis-mode evidence."],
    }


def _discover_base(root: Path, supplied: str | None) -> str:
    if supplied:
        return supplied
    active = sorted((root / ".ai/work-items/active").glob("*.contract.json"))
    if len(active) == 1:
        try:
            contract = json.loads(active[0].read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError) as exc:
            raise InputError(f"cannot read active Contract baseCommit: {exc}") from exc
        base = contract.get("baseCommit")
        if not isinstance(base, str) or not base:
            raise InputError("active Contract has no valid baseCommit")
        return base
    result = _git(root, "rev-parse", "HEAD", check=False)
    if result.returncode or not result.stdout.strip():
        raise InputError("cannot discover a Git base; pass --base-ref")
    return result.stdout.strip()


def analyze(
    root: Path,
    base: str,
    *,
    mode: str,
    request: str = "",
    thresholds: dict[str, float] | None = None,
) -> dict[str, Any]:
    root = root.resolve()
    if not (root / ".git").exists():
        raise InputError(f"not a Git repository: {root}")

    configured = thresholds or {key: float(value) for key, value in DEFAULT_THRESHOLDS.items()}
    signals: list[dict[str, Any]] = []
    if BYPASS_REQUEST.search(request):
        signals.append(_signal("explicit_test_bypass_request", "<request>", "critical"))
    for status, path, before_path in _changed_paths(root, base):
        before = _read_before(root, base, before_path)
        after = "" if status == "D" else _read_after(root, path)
        if (status == "A" and after is None) or (status == "D" and before is None):
            continue
        if before is None and after is None:
            continue
        before = before or ""
        after = after or ""
        is_test = bool(TEST_PATH.search(path))
        if status == "R" and is_test and TEST_PATH.search(before_path):
            signals.append(
                _signal("test_renamed", path, "low", beforePath=before_path, afterPath=path)
            )
        if is_test and status == "D":
            signals.append(_signal("test_file_deleted", path, "high"))
            if re.search(
                r"security|vulnerab|auth|traversal|injection", path + before, re.IGNORECASE
            ):
                signals.append(_signal("security_test_deleted", path, "critical"))
            if re.search(r"regression", path + before, re.IGNORECASE):
                signals.append(_signal("regression_test_deleted", path, "critical"))
        if is_test:
            before_skips = len(SKIP.findall(before))
            after_skips = len(SKIP.findall(after))
            if status != "A" and after_skips > before_skips:
                signals.append(
                    _signal(
                        "skip_added",
                        path,
                        "high",
                        before=before_skips,
                        after=after_skips,
                    )
                )
            if mode == "full":
                before_assertions = len(ASSERTION.findall(before))
                after_assertions = len(ASSERTION.findall(after))
                if before_assertions > after_assertions:
                    material = (
                        before_assertions >= configured["materialAssertionMinimumBefore"]
                        and after_assertions
                        <= before_assertions * configured["materialAssertionRatio"]
                    )
                    signals.append(
                        _signal(
                            "assertion_reduction",
                            path,
                            "high" if material else "low",
                            before=before_assertions,
                            after=after_assertions,
                        )
                    )
                removed_lines, added_lines = _diff_lines(before, after)
                removed_conditions = [
                    line
                    for line in removed_lines
                    if "assert" in line and re.search(r"==|!=|<=|>=|\s<\s|\s>\s|\bin\b", line)
                ]
                added_plain = [
                    line
                    for line in added_lines
                    if re.search(r"\bassert\s+[A-Za-z_][A-Za-z0-9_.\[\]'\"]*\s*$", line)
                ]
                if removed_conditions and added_plain:
                    signals.append(
                        _signal(
                            "assertion_condition_relaxed",
                            path,
                            "low",
                            before=removed_conditions,
                            after=added_plain,
                        )
                    )
                before_cases = _case_names(before)
                after_cases = _case_names(after)
                removed_cases = sorted(before_cases - after_cases)
                protected_cases = [
                    name
                    for name in removed_cases
                    if _negative_case(name)
                    or re.search(r"security|vulnerab|auth|traversal|injection|regression", name)
                ]
                if removed_cases:
                    preserved_refactor = (
                        len(after_cases) >= len(before_cases)
                        and after_assertions >= before_assertions
                        and not protected_cases
                    )
                    signals.append(
                        _signal(
                            (
                                "test_case_renamed_or_refactored"
                                if preserved_refactor
                                else "test_case_removed"
                            ),
                            path,
                            "low" if preserved_refactor else "high",
                            before=len(before_cases),
                            after=len(after_cases),
                            removed=removed_cases,
                        )
                    )
                negative_cases = [name for name in removed_cases if _negative_case(name)]
                if negative_cases:
                    signals.append(
                        _signal(
                            "negative_test_removed",
                            path,
                            "high",
                            removed=negative_cases,
                        )
                    )
                before_exceptions = len(EXCEPTION_ASSERTION.findall(before))
                after_exceptions = len(EXCEPTION_ASSERTION.findall(after))
                if after_exceptions < before_exceptions:
                    signals.append(
                        _signal(
                            "exception_assertion_removed",
                            path,
                            "high",
                            before=before_exceptions,
                            after=after_exceptions,
                        )
                    )
        if mode == "full" and COVERAGE_PATH.search(path):
            signals.extend(_coverage_signals(path, before, after))
        if WORKFLOW_PATH.search(path):
            signals.extend(_workflow_signals(path, before, after))
        if mode == "full" and SNAPSHOT_PATH.search(path) and before != after:
            removed, added = _diff_lines(before, after)
            churn = len(removed) + len(added)
            signals.append(
                _signal(
                    "snapshot_churn"
                    if churn >= configured["snapshotReviewChangedLines"]
                    else "snapshot_changed",
                    path,
                    "high" if churn >= configured["snapshotReviewChangedLines"] else "low",
                    changedLines=churn,
                )
            )

    lowered = any(signal["type"] == "coverage_threshold_lowered" for signal in signals)
    if lowered and COVERAGE_TO_PASS.search(request):
        signals.append(_signal("coverage_threshold_lowered_to_pass", "<request>", "critical"))
    severities = {signal["severity"] for signal in signals}
    decision = (
        "block"
        if "critical" in severities
        else "review"
        if "high" in severities
        else "warning"
        if signals
        else "continue"
    )
    approval = _approved_retirement_evidence(root, base, signals)
    if decision == "review" and approval is not None:
        decision = "warning"
    report = {
        "version": 1,
        "mode": mode,
        "baseRef": base,
        "decision": decision,
        "signals": signals,
        "requiredExplanation": bool(signals),
        "recoveryCondition": (
            "Restore test strength or provide independently reviewable changed-requirement evidence."
            if signals
            else "No recovery action is required for the analyzed diff."
        ),
        "limitations": [
            "Static signals do not prove semantic equivalence or complete test coverage."
        ],
    }
    if approval is not None:
        report["requirementEvidence"] = approval
    return report


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path.cwd())
    parser.add_argument("--base-ref")
    parser.add_argument("--mode", choices=("fast", "full"), default="full")
    parser.add_argument("--request", default="")
    parser.add_argument("--policy", type=Path)
    args = parser.parse_args(argv)
    try:
        base = _discover_base(args.root.resolve(), args.base_ref)
        report = analyze(
            args.root,
            base,
            mode=args.mode,
            request=args.request,
            thresholds=_load_policy(args.policy),
        )
    except (InputError, OSError, subprocess.SubprocessError) as exc:
        print(f"test weakening analysis failed: {exc}", file=sys.stderr)
        return 4
    print(json.dumps(report, ensure_ascii=False, sort_keys=True))
    return EXIT[report["decision"]]


if __name__ == "__main__":
    raise SystemExit(main())
