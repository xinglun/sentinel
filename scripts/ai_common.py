#!/usr/bin/env python3
"""Shared helpers for AI Cockpit scripts."""

from __future__ import annotations

import hashlib
import json
import math
import os
import re
import shlex
import subprocess
from collections.abc import Callable
from datetime import datetime
from functools import lru_cache
from pathlib import Path
from typing import Any

PROJECT_ROOT = Path(__file__).resolve().parents[1]


class InvalidDataShapeError(TypeError, ValueError):
    """Reject malformed structured data while retaining the legacy ValueError contract."""


class InvalidProviderPayloadError(TypeError, RuntimeError):
    """Reject malformed provider payloads while retaining the legacy RuntimeError contract."""


CHECKS_PATH = PROJECT_ROOT / ".ai" / "cockpit" / "checks.yaml"
SCENARIO_COVERAGE_STATUSES = {"verified", "unverified", "not_applicable"}
GIT_ENV_PREFIX = "GIT_"
MAKE_OVERRIDE_BLOCKLIST = {
    "AI_BASE_COMMIT",
    "CONTRACT",
    "SUMMARY",
    "TASK",
    "TITLE",
    "MODE",
    "REPORT_LANGUAGE",
    "ARCHIVE",
}
MAKE_ENTRYPOINT_ENV = "AI_COCKPIT_MAKE_ENTRYPOINT"
SUPPORTED_MAKEFILE_NAMES = {"GNUmakefile", "makefile", "Makefile", "Makefile.ai"}


def _clean_make_overrides(value: str) -> str:
    """Keep project command overrides while dropping nested Work Item context."""
    kept: list[str] = []
    for token in shlex.split(value):
        name = token.split("=", 1)[0]
        if name not in MAKE_OVERRIDE_BLOCKLIST:
            kept.append(token)
    return " ".join(kept)


def clean_git_environment() -> dict[str, str]:
    """Return a subprocess environment without ambient Git or coverage overrides."""
    environment = {
        key: value
        for key, value in os.environ.items()
        if not key.startswith(GIT_ENV_PREFIX)
        and not key.startswith("COV_CORE_")
        and not key.startswith("COVERAGE_")
        and key not in {"GNUMAKEFLAGS", "MFLAGS", "MAKELEVEL"}
        and key not in MAKE_OVERRIDE_BLOCKLIST
    }
    if "MAKEFLAGS" in os.environ:
        flags = _clean_make_overrides(os.environ["MAKEFLAGS"])
        if flags:
            environment["MAKEFLAGS"] = flags
    if "MAKEOVERRIDES" in os.environ:
        overrides = _clean_make_overrides(os.environ["MAKEOVERRIDES"])
        if overrides:
            environment["MAKEOVERRIDES"] = overrides
    environment["PYTHONDONTWRITEBYTECODE"] = "1"
    return environment


def _validated_make_entrypoint(value: str, *, root: Path) -> str:
    """Return one normalized repository-local Make entrypoint or fail closed."""
    raw = value.strip()
    path = Path(raw)
    if not raw or path.is_absolute() or ".." in path.parts:
        raise ValueError("Make entrypoint must be a repository-relative path")
    if path.name not in SUPPORTED_MAKEFILE_NAMES:
        raise ValueError("Make entrypoint must name a supported Makefile or Makefile.ai")
    root_resolved = root.resolve()
    candidate = (root_resolved / path).resolve()
    try:
        relative = candidate.relative_to(root_resolved)
    except ValueError as exc:
        raise ValueError("Make entrypoint must remain inside the repository") from exc
    if not candidate.is_file():
        raise ValueError(f"Make entrypoint does not exist: {relative.as_posix()}")
    return relative.as_posix()


def _declared_makefiles(command: list[str]) -> list[str]:
    values: list[str] = []
    index = 1
    while index < len(command):
        token = command[index]
        if token in {"-f", "--file", "--makefile"}:
            if index + 1 >= len(command):
                raise ValueError("Make entrypoint option requires a path")
            values.append(command[index + 1])
            index += 2
            continue
        if token.startswith(("--file=", "--makefile=")):
            values.append(token.split("=", 1)[1])
        elif token.startswith("-f") and token != "-f":  # nosec B105 - Make option
            values.append(token[2:])
        index += 1
    return values


def nested_make_command(
    command: list[str],
    *,
    root: Path = PROJECT_ROOT,
    environment: dict[str, str] | None = None,
) -> list[str]:
    """Apply the selected Makefile to a nested Make argv without using a shell."""
    argv = list(command)
    if not argv or Path(argv[0]).name not in {"make", "gmake"}:
        return argv
    source_environment = os.environ if environment is None else environment
    selected_raw = source_environment.get(MAKE_ENTRYPOINT_ENV)
    if not selected_raw:
        return argv
    selected = _validated_make_entrypoint(selected_raw, root=root)
    declared = _declared_makefiles(argv)
    if declared:
        normalized = [_validated_make_entrypoint(value, root=root) for value in declared]
        if any(value != selected for value in normalized):
            raise ValueError(
                "Make entrypoint conflicts with an explicit nested -f/--file selection"
            )
        return argv
    return [argv[0], "-f", selected, *argv[1:]]


def _reject_duplicate_keys(path: Path) -> Any:
    def hook(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
        data: dict[str, Any] = {}
        for key, value in pairs:
            if key in data:
                raise ValueError(f"duplicate key in {path.as_posix()}: {key}")
            data[key] = value
        return data

    return hook


def load_json(path: Path) -> dict[str, Any]:
    data = json.loads(
        path.read_text(encoding="utf-8"), object_pairs_hook=_reject_duplicate_keys(path)
    )
    if not isinstance(data, dict):
        raise InvalidDataShapeError("root must be a JSON object")
    return data


def save_json(path: Path, data: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(data, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def _aware_iso_datetime(value: Any, *, error: str) -> datetime:
    if not isinstance(value, str) or not value.strip():
        raise ValueError(error)
    try:
        parsed = datetime.fromisoformat(value)
    except ValueError as exc:
        raise ValueError(error) from exc
    if parsed.utcoffset() is None:
        raise ValueError(error)
    return parsed


def verification_status_for_generation(
    summary: dict[str, Any] | None, contract: dict[str, Any]
) -> dict[str, str]:
    """Return verification statuses eligible for the active resume generation."""
    if not isinstance(summary, dict):
        return {}

    cutoff: datetime | None = None
    for field in ("resumeHistory", "synchronizationHistory"):
        history = contract.get(field)
        if history is None:
            continue
        if not isinstance(history, list) or not history or not isinstance(history[-1], dict):
            raise ValueError(f"latest {field}.recordedAt is invalid")
        transition_cutoff = _aware_iso_datetime(
            history[-1].get("recordedAt"),
            error=f"latest {field}.recordedAt is invalid",
        )
        if cutoff is None or transition_cutoff > cutoff:
            cutoff = transition_cutoff

    statuses: dict[str, str] = {}
    verification = summary.get("verification", [])
    if not isinstance(verification, list):
        return statuses
    for item in verification:
        if (
            not isinstance(item, dict)
            or not verification_key(item)
            or not isinstance(item.get("result"), str)
        ):
            continue
        if cutoff is not None:
            if item["result"] == "not_run" and not item.get("executedAt"):
                continue
            executed_at = _aware_iso_datetime(
                item.get("executedAt"),
                error="verification record executedAt is required after baseline transition",
            )
            if executed_at < cutoff:
                continue
        statuses[verification_key(item)] = str(item["result"])
    return statuses


def canonical_json_hash(value: Any, *, length: int = 16) -> str:
    """Hash JSON evidence deterministically for cross-step identity checks."""
    payload = json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
    digest = hashlib.sha256(payload.encode("utf-8")).hexdigest()
    return digest[:length] if length else digest


def run_git(args: list[str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["git", *args],
        cwd=PROJECT_ROOT,
        env=clean_git_environment(),
        text=True,
        capture_output=True,
        check=False,
    )


def discover_remote_default_candidates(
    run_command: Callable[[list[str]], Any],
) -> list[tuple[str, str]]:
    """Return remote default branches as explicit ``(remote, branch)`` identities."""
    remotes = run_command(["remote"])
    if remotes.returncode != 0:
        raise RuntimeError("cannot enumerate Git remotes")
    candidates: list[tuple[str, str]] = []
    for remote in remotes.stdout.splitlines():
        remote = remote.strip()
        if not remote:
            continue
        head = run_command(["symbolic-ref", "--quiet", "--short", f"refs/remotes/{remote}/HEAD"])
        if head.returncode != 0:
            continue
        branch = remote_default_branch_from_symref(head.stdout, remote)
        if branch:
            candidates.append((remote, branch))
    return candidates


def remote_default_branch_from_symref(output: str, remote: str) -> str | None:
    """Parse one remote's symbolic HEAD, returning None for missing/invalid output."""
    refs = [
        match.group(1)
        for line in output.splitlines()
        if (match := re.fullmatch(r"ref:\s+refs/heads/(.+)\s+HEAD", line.strip()))
    ]
    if not refs:
        refs = [
            match.group(1)
            for line in output.splitlines()
            if (match := re.fullmatch(rf"{re.escape(remote)}/(.+)", line.strip()))
        ]
    if len(refs) != 1:
        return None
    branch = refs[0].strip()
    return branch if branch and "/" not in remote and remote else None


def current_head() -> str:
    result = run_git(["rev-parse", "--verify", "HEAD"])
    if result.returncode != 0:
        return ""
    return result.stdout.strip()


def path_fingerprint(path: str) -> str:
    candidate = PROJECT_ROOT / path
    try:
        stat_result = candidate.lstat()
    except FileNotFoundError:
        return "deleted"
    if candidate.is_symlink():
        target = os.readlink(candidate)
        payload = f"symlink:{stat_result.st_mode}:{target}".encode()
        return hashlib.sha256(payload).hexdigest()
    if not candidate.is_file():
        return f"non_file:{stat_result.st_mode}"
    payload = f"mode={stat_result.st_mode}\n".encode() + candidate.read_bytes()
    return hashlib.sha256(payload).hexdigest()


def _git_records(output: str) -> list[str]:
    if "\0" in output:
        return [item for item in output.split("\0") if item]
    return [line for line in output.splitlines() if line]


def _raw_worktree_changes() -> dict[str, str]:
    changes: dict[str, str] = {}
    if current_head():
        result = run_git(["diff", "--name-status", "-z", "HEAD"])
        if result.returncode != 0:
            raise RuntimeError(result.stderr.strip())
        _merge_name_status(changes, result.stdout)
    untracked = run_git(["ls-files", "--others", "--exclude-standard", "-z"])
    if untracked.returncode != 0:
        raise RuntimeError(untracked.stderr.strip())
    for path in _git_records(untracked.stdout):
        changes[path.strip()] = "A"
    return changes


def _merge_name_status(changes: dict[str, str], output: str) -> None:
    records = _git_records(output)
    if records and "\t" in records[0]:
        for line in records:
            parts = line.split("\t")
            if len(parts) < 2:
                continue
            status = parts[0]
            if status.startswith(("R", "C")) and len(parts) >= 3:
                changes[parts[1]] = "D" if status.startswith("R") else status
                changes[parts[2]] = "A"
            else:
                changes[parts[-1]] = status
        return

    i = 0
    while i < len(records):
        status = records[i]
        i += 1
        if status.startswith(("R", "C")):
            if i + 1 >= len(records):
                break
            old_path = records[i]
            new_path = records[i + 1]
            i += 2
            changes[old_path] = "D" if status.startswith("R") else status
            changes[new_path] = "A"
            continue
        if i >= len(records):
            break
        changes[records[i]] = status
        i += 1


def capture_dirty_baseline() -> list[dict[str, str]]:
    return [
        {"path": path, "status": status, "fingerprint": path_fingerprint(path)}
        for path, status in sorted(_raw_worktree_changes().items())
    ]


def active_contract() -> dict[str, Any] | None:
    contracts = sorted((PROJECT_ROOT / ".ai" / "work-items" / "active").glob("*.contract.json"))
    if len(contracts) != 1:
        return None
    try:
        return load_json(contracts[0])
    except (OSError, json.JSONDecodeError, ValueError):
        return None


def _baseline(contract: dict[str, Any] | None = None) -> tuple[str, list[dict[str, str]]]:
    data = contract if contract is not None else active_contract() or {}
    base = os.environ.get("AI_BASE_COMMIT", "").strip() or str(data.get("baseCommit", "")).strip()
    dirty = data.get("baselineDirtyPaths", [])
    return base, [item for item in dirty if isinstance(item, dict)] if isinstance(
        dirty, list
    ) else []


def changed_name_status(
    contract: dict[str, Any] | None = None, *, ignore_baseline_dirty: bool = False
) -> list[tuple[str, str]]:
    changes: dict[str, str] = {}
    head = current_head()
    base, baseline_dirty = _baseline(contract)
    if base:
        valid = run_git(["rev-parse", "--verify", f"{base}^{{commit}}"])
        if valid.returncode != 0:
            raise RuntimeError(f"baseCommit is not a valid commit: {base}")
        if head:
            result = run_git(["diff", "--name-status", "-z", f"{base}...HEAD"])
            if result.returncode != 0:
                raise RuntimeError(result.stderr.strip())
            _merge_name_status(changes, result.stdout)
    elif head:
        result = run_git(["diff", "--name-status", "-z", "HEAD"])
        if result.returncode != 0:
            raise RuntimeError(result.stderr.strip())
        _merge_name_status(changes, result.stdout)

    changes.update(_raw_worktree_changes())
    for item in [] if ignore_baseline_dirty else baseline_dirty:
        path = item.get("path")
        fingerprint = item.get("fingerprint")
        if isinstance(path, str) and isinstance(fingerprint, str):
            if path_fingerprint(path) == fingerprint:
                changes.pop(path, None)
            elif path not in changes:
                changes[path] = (
                    "D" if not (PROJECT_ROOT / path).exists() else str(item.get("status", "M"))
                )
    return sorted((status, path) for path, status in changes.items())


def changed_paths(
    contract: dict[str, Any] | None = None, *, ignore_baseline_dirty: bool = False
) -> list[str]:
    return [
        path
        for _, path in changed_name_status(contract, ignore_baseline_dirty=ignore_baseline_dirty)
    ]


def matches(pattern: str, path: str) -> bool:
    normalized = pattern.rstrip("/")
    if not normalized:
        return not path
    pattern_parts = normalized.split("/")
    path_parts = path.split("/")

    def segment_match(pattern_index: int, path_index: int) -> bool:
        while pattern_index < len(pattern_parts):
            token = pattern_parts[pattern_index]
            if token == "**":
                if pattern_index == len(pattern_parts) - 1:
                    return True
                for candidate in range(path_index, len(path_parts) + 1):
                    if segment_match(pattern_index + 1, candidate):
                        return True
                return False
            if path_index >= len(path_parts):
                return False
            if not _compiled_segment_pattern(token).fullmatch(path_parts[path_index]):
                return False
            pattern_index += 1
            path_index += 1
        return path_index == len(path_parts)

    return segment_match(0, 0)


def fnmatch_translate(pattern: str) -> str:
    """Translate a single path segment glob into a regex string."""
    translated: list[str] = []
    i = 0
    while i < len(pattern):
        char = pattern[i]
        if char == "*":
            translated.append("[^/]*")
        elif char == "?":
            translated.append("[^/]")
        else:
            translated.append(re.escape(char))
        i += 1
    return "".join(translated)


@lru_cache(maxsize=8192)
def _compiled_segment_pattern(pattern: str) -> re.Pattern[str]:
    """Compile each segment glob once while retaining exact path-boundary semantics."""
    return re.compile(fnmatch_translate(pattern))


def included(path: str, patterns: list[str]) -> bool:
    return any(matches(pattern, path) for pattern in patterns)


def parse_simple_manifest(path: Path) -> dict[str, dict[str, str]]:
    if not path.exists():
        raise ValueError(f"required guard manifest is missing: {path.as_posix()}")
    parsed = parse_yaml(path)
    manifest: dict[str, dict[str, str]] = {}
    for k, v in parsed.items():
        if isinstance(v, dict):
            manifest[k] = {str(sub_k): str(sub_v) for sub_k, sub_v in v.items()}
    return manifest


def first_match(
    path: str, manifest: dict[str, dict[str, str]]
) -> tuple[str, dict[str, str]] | None:
    found = [(pattern, data) for pattern, data in manifest.items() if matches(pattern, path)]
    if not found:
        return None
    found.sort(key=lambda item: len(item[0]), reverse=True)
    return found[0]


def parse_yaml(path: Path) -> dict[str, Any]:
    """Parse a subset of YAML used by guard policies, raising ValueError on syntax errors."""
    if not path.exists():
        return {}
    content = path.read_text(encoding="utf-8")
    lines = content.splitlines()
    root: dict[str, Any] = {}
    # stack holds tuples of (indent, key, container)
    stack: list[tuple[int, str | None, Any]] = [(-2, None, root)]

    for line_idx, raw_line in enumerate(lines, 1):
        stripped = raw_line.strip()
        if not stripped or stripped.startswith("#"):
            continue

        indent = len(raw_line) - len(raw_line.lstrip(" "))
        if indent % 2 != 0:
            raise ValueError(
                f"Syntax Error in {path.name}:{line_idx}: Indentation must be a multiple of 2 spaces."
            )

        while len(stack) > 1 and stack[-1][0] >= indent:
            stack.pop()

        parent_indent, parent_key, parent_container = stack[-1]

        if stripped.startswith("-"):
            if stripped != "-" and not stripped.startswith("- "):
                raise ValueError(
                    f"Syntax Error in {path.name}:{line_idx}: Invalid list item format."
                )
            val = stripped[1:].strip().strip('"')

            if isinstance(parent_container, dict):
                if len(stack) < 2:
                    raise ValueError(
                        f"Syntax Error in {path.name}:{line_idx}: List item without a parent key."
                    )
                gp_container = stack[-2][2]
                new_list: list[Any] = []
                gp_container[parent_key] = new_list
                stack[-1] = (parent_indent, parent_key, new_list)
                parent_container = new_list

            if not isinstance(parent_container, list):
                raise ValueError(
                    f"Syntax Error in {path.name}:{line_idx}: List item at invalid indentation level."
                )
            parent_container.append(val)
        else:
            if ":" not in stripped:
                raise ValueError(
                    f"Syntax Error in {path.name}:{line_idx}: Expected key-value pair or key ending in ':'."
                )

            key, val = stripped.split(":", 1)
            key = key.strip().strip('"')
            val = val.strip().strip('"')

            if not isinstance(parent_container, dict):
                raise ValueError(
                    f"Syntax Error in {path.name}:{line_idx}: Cannot define key-value pair under a list."
                )

            if key in parent_container:
                raise ValueError(f"Duplicate key in {path.name}:{line_idx}: {key}")

            if val:
                parent_container[key] = [] if val == "[]" else val
            else:
                new_container: dict[str, Any] = {}
                parent_container[key] = new_container
                stack.append((indent, key, new_container))

    return root


def numeric_value(value: Any) -> int | float | None:
    """Return a finite numeric policy value, including numeric YAML strings."""
    if isinstance(value, bool):
        return None
    if isinstance(value, (int, float)):
        return value
    if not isinstance(value, str):
        return None
    text = value.strip()
    if not text:
        return None
    try:
        number: int | float = int(text)
    except ValueError:
        try:
            number = float(text)
        except ValueError:
            return None
    return number if math.isfinite(number) else None


def _flatten_lists(
    obj: Any, prefix: str = "", result: dict[str, list[str]] | None = None
) -> dict[str, list[str]]:
    if result is None:
        result = {}
    if isinstance(obj, dict):
        for k, v in obj.items():
            new_prefix = f"{prefix}.{k}" if prefix else k
            if isinstance(v, list):
                result[new_prefix] = v
            else:
                _flatten_lists(v, new_prefix, result)
    return result


def _flatten_scalars(
    obj: Any, prefix: str = "", result: dict[str, str] | None = None
) -> dict[str, str]:
    if result is None:
        result = {}
    if isinstance(obj, dict):
        for k, v in obj.items():
            new_prefix = f"{prefix}.{k}" if prefix else k
            if isinstance(v, str):
                result[new_prefix] = v
            elif not isinstance(v, list):
                _flatten_scalars(v, new_prefix, result)
    return result


def simple_yaml_lists(path: Path) -> dict[str, list[str]]:
    """Read list values from a tiny YAML subset used by guard policies."""
    if not path.exists():
        return {}
    parsed = parse_yaml(path)
    return _flatten_lists(parsed)


def simple_yaml_scalars(path: Path) -> dict[str, str]:
    """Read scalar values from the same intentionally small YAML subset."""
    if not path.exists():
        return {}
    parsed = parse_yaml(path)
    return _flatten_scalars(parsed)


def non_empty_string(value: Any) -> bool:
    return isinstance(value, str) and bool(value.strip())


def validate_scenario_coverage(values: Any, *, field_name: str = "scenarioCoverage") -> list[str]:
    issues: list[str] = []
    if values is None:
        return issues
    if not isinstance(values, list):
        return [f"{field_name} must be a list"]
    for index, item in enumerate(values):
        prefix = f"{field_name}[{index}]"
        if not isinstance(item, dict):
            issues.append(f"{prefix} must be an object")
            continue
        if not non_empty_string(item.get("scenario")):
            issues.append(f"{prefix}.scenario is required")
        if not isinstance(item.get("required"), bool):
            issues.append(f"{prefix}.required must be boolean")
        status = item.get("status")
        if status not in SCENARIO_COVERAGE_STATUSES:
            issues.append(f"{prefix}.status must be one of {sorted(SCENARIO_COVERAGE_STATUSES)}")
        evidence = item.get("evidence")
        if not isinstance(evidence, list):
            issues.append(f"{prefix}.evidence must be a list")
        elif any(not non_empty_string(entry) for entry in evidence):
            issues.append(f"{prefix}.evidence must be a list of non-empty strings")
        if status == "verified" and isinstance(evidence, list) and not evidence:
            issues.append(
                f"{prefix}.evidence must contain at least one item when status is verified"
            )
        if status == "not_applicable":
            if not non_empty_string(item.get("reason")):
                issues.append(f"{prefix}.reason is required when status is not_applicable")
        elif (
            "reason" in item
            and item.get("reason") is not None
            and not isinstance(item.get("reason"), str)
        ):
            issues.append(f"{prefix}.reason must be a string when provided")
        if "verificationPlan" in item and not non_empty_string(item.get("verificationPlan")):
            issues.append(f"{prefix}.verificationPlan must be a non-empty string when provided")
    return issues


def load_check_registry(path: Path = CHECKS_PATH) -> dict[str, dict[str, str]]:
    """Parse the checks section of the repository's small YAML check catalog."""
    registry: dict[str, dict[str, str]] = {}
    try:
        parsed = parse_yaml(path)
    except ValueError:
        return registry
    checks = parsed.get("checks", {})
    if isinstance(checks, dict):
        for check_id, val in checks.items():
            if isinstance(val, dict):
                registry[check_id] = {k: str(v) for k, v in val.items()}
    return registry


def render_check_command(
    check_id: str,
    *,
    contract_path: str,
    summary_path: str,
    registry_path: Path = CHECKS_PATH,
) -> tuple[str, list[str]]:
    registry = load_check_registry(registry_path)
    definition = registry.get(check_id)
    if not definition:
        raise ValueError(f"verification check is not registered: {check_id}")
    template = definition.get("commandTemplate") or definition.get("command")
    if not template:
        raise ValueError(f"registered check has no command: {check_id}")
    command = template.replace("{contractPath}", contract_path).replace(
        "{summaryPath}", summary_path
    )
    argv = shlex.split(command)
    if not argv or Path(argv[0]).name not in {"make", "gmake"}:
        raise ValueError(f"registered check must invoke an explicit Make target: {check_id}")
    if len(argv) < 2 or argv[1].startswith("-") or "=" in argv[1]:
        raise ValueError(f"registered check must name a Make target: {check_id}")
    return command, argv


def verification_key(item: dict[str, Any]) -> str:
    check_id = item.get("check")
    if non_empty_string(check_id):
        return str(check_id).strip()
    command = item.get("command")
    return str(command).strip() if non_empty_string(command) else ""


def redact_machine_paths(value: str) -> str:
    redacted = value.replace(str(PROJECT_ROOT), "<PROJECT_ROOT>")
    redacted = re.sub(r"/(?:Users|home)/[^/\s]+/(?:[^\s\"']+)", "<LOCAL_PATH>", redacted)
    redacted = re.sub(r"[A-Za-z]:\\Users\\[^\\\s]+\\(?:[^\s\"']+)", "<LOCAL_PATH>", redacted)
    return redacted


def redact_sensitive_output(value: str) -> str:
    """Remove common credential formats before output is persisted in a Summary."""
    redacted = value
    patterns = [
        (r"(?i)\bBearer\s+[A-Za-z0-9._~+/=-]+", "Bearer [REDACTED]"),
        (r"\bAKIA[0-9A-Z]{16}\b", "[AWS_KEY_REDACTED]"),
        (r"\bgh[pousr]_[A-Za-z0-9_]{20,}\b", "[GITHUB_TOKEN_REDACTED]"),
        (
            (
                r"-----BEGIN (?P<private_key_kind>(?:[A-Z0-9]+ )*PRIVATE KEY)-----"
                r".*?(?:-----END (?P=private_key_kind)-----|\Z)"
            ),
            "[PRIVATE_KEY_REDACTED]",
        ),
        (
            r"(?i)\b(api[_-]?key|token|password|secret|credential)\b(\s*[:=]\s*)[^\s,;]+",
            r"\1\2[REDACTED]",
        ),
    ]
    for pattern, replacement in patterns:
        redacted = re.sub(
            pattern, replacement, redacted, flags=re.DOTALL if "BEGIN" in pattern else 0
        )
    return redacted


def contains_machine_path(value: str) -> bool:
    return redact_machine_paths(value) != value


def redact_machine_paths_in_data(value: Any) -> Any:
    if isinstance(value, str):
        return redact_machine_paths(value)
    if isinstance(value, list):
        return [redact_machine_paths_in_data(item) for item in value]
    if isinstance(value, dict):
        return {key: redact_machine_paths_in_data(item) for key, item in value.items()}
    return value
