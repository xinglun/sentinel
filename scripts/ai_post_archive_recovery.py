"""Open and validate a narrow same-Work-Item recovery after a failed PR audit."""

from __future__ import annotations

import argparse
import hashlib
import io
import json
import re
import subprocess  # nosec B404 - all process calls below use fixed list-form commands
import zipfile
from collections.abc import Callable
from datetime import UTC, datetime
from pathlib import Path
from xml.etree import (
    ElementTree,  # nosec B405 - JUnit DTD/entity declarations are rejected before parsing.
)

from ai_common import PROJECT_ROOT, clean_git_environment

RECEIPT_DIRECTORY = Path(".ai/work-items/recovery-receipts")
ARCHIVE_SUFFIXES = ("contract", "summary", "outcome", "archive-manifest")
ALLOWED_GATES = {
    "changedCriticalCoverage",
    "archiveEvidence",
    "hostedAggregateCoverage",
    "hostedFunctionalFailure",
    "hostedGovernanceFailure",
}
RECOVERABLE_OUTCOME_STATUSES = {"completed", "completed_with_warnings"}
HOSTED_RECOVERABLE_OUTCOME_STATUSES = RECOVERABLE_OUTCOME_STATUSES | {"needs_human_confirmation"}
HOSTED_RECEIPT_VERSION = 2
HOSTED_FUNCTIONAL_RECEIPT_VERSION = 3
HOSTED_GOVERNANCE_RECEIPT_VERSION = 4
GITHUB_REPOSITORY = re.compile(r"^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$")
COVERAGE_FAILURE = re.compile(
    r"Required test coverage of (?P<required>\d+(?:\.\d+)?)% not reached\.\s*"
    r"Total coverage:\s*(?P<actual>\d+(?:\.\d+)?)%",
    re.IGNORECASE,
)
FUNCTIONAL_FAILURE = re.compile(r"\bBLOCKED:.*\bRecovery:", re.IGNORECASE | re.DOTALL)
GOVERNANCE_FAILURE = re.compile(
    r"quality-full\s+blocked.*?Failed\s+gate:\s+\S+.*?Recovery:\s+\S+",
    re.IGNORECASE | re.DOTALL,
)
PYTEST_FAILURE_NODE = re.compile(r"(?m)^FAILED\s+\S+::\S+")
PYTEST_FAILURE_SUMMARY = re.compile(r"(?mi)^[=\s]*\d+\s+failed(?:,|\s)")
SHA256 = re.compile(r"^[0-9a-f]{64}$")


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def functional_failure_marker(text: str) -> str | None:
    """Return the supported, concrete functional-failure signal in a job log."""
    if governance_failure_marker(text) is not None:
        return "quality_gate_blocked"
    if FUNCTIONAL_FAILURE.search(text) is not None:
        return "blocked_recovery"
    if PYTEST_FAILURE_NODE.search(text) and PYTEST_FAILURE_SUMMARY.search(text):
        return "pytest_failure"
    return None


def governance_failure_marker(text: str) -> str | None:
    """Return the explicit quality-gate blocked signal from a hosted log."""
    return "quality_gate_blocked" if GOVERNANCE_FAILURE.search(text) is not None else None


def junit_artifact_failure(
    fetch_provider: Callable[[str], bytes], *, repository: str, run_id: int, artifact_name: str
) -> dict:
    """Bind one named, non-expired run artifact containing failed JUnit testcases."""
    if not artifact_name or artifact_name.strip() != artifact_name:
        raise ValueError("GitHub JUnit artifact name is invalid")
    listing = _provider_json(fetch_provider, f"/repos/{repository}/actions/runs/{run_id}/artifacts")
    values = listing.get("artifacts")
    matches = (
        [item for item in values if isinstance(item, dict) and item.get("name") == artifact_name]
        if isinstance(values, list)
        else []
    )
    if len(matches) != 1 or matches[0].get("expired") is not False:
        raise ValueError("GitHub JUnit artifact is missing, ambiguous, or expired")
    artifact_id = _positive_int(matches[0].get("id"), "GitHub JUnit artifact ID")
    payload = fetch_provider(f"/repos/{repository}/actions/artifacts/{artifact_id}/zip")
    if not isinstance(payload, bytes):
        raise TypeError("GitHub JUnit artifact download is unavailable")
    try:
        with zipfile.ZipFile(io.BytesIO(payload)) as bundle:
            xml_files = [name for name in bundle.namelist() if name.endswith(".xml")]
            xml_payloads = [bundle.read(name) for name in xml_files]
            if any(
                b"<!DOCTYPE" in content.upper() or b"<!ENTITY" in content.upper()
                for content in xml_payloads
            ):
                raise ValueError("DTD and entity declarations are not accepted in JUnit XML")
            roots = [
                ElementTree.fromstring(content)  # nosec B314 - declarations are rejected above.
                for content in xml_payloads
            ]
    except (OSError, ValueError, zipfile.BadZipFile, ElementTree.ParseError) as exc:
        raise ValueError(f"GitHub JUnit artifact is invalid: {exc}") from exc
    failures = sum(
        1
        for root in roots
        for testcase in root.iter("testcase")
        if any(child.tag in {"failure", "error"} for child in testcase)
    )
    if not xml_files or failures <= 0:
        raise ValueError("GitHub JUnit artifact has no failed testcase")
    return {
        "id": artifact_id,
        "name": artifact_name,
        "sha256": hashlib.sha256(payload).hexdigest(),
        "failedTestcases": failures,
    }


def archive_files(root: Path, task: str) -> dict[str, Path]:
    found: dict[str, list[Path]] = {suffix: [] for suffix in ARCHIVE_SUFFIXES}
    for year in root.joinpath(".ai/work-items/archive").glob("*"):
        if not year.is_dir():
            continue
        for suffix in ARCHIVE_SUFFIXES:
            candidate = year / f"{task}.{suffix}.json"
            if candidate.is_file():
                found[suffix].append(candidate)
    missing_or_ambiguous = [name for name, paths in found.items() if len(paths) != 1]
    if missing_or_ambiguous:
        raise ValueError(
            "expected exactly one immutable archive artifact for "
            f"{task}: {', '.join(missing_or_ambiguous)}"
        )
    return {name: paths[0] for name, paths in found.items()}


def _require_recoverable_outcome(
    artifacts: dict[str, Path], task: str, *, allow_human_confirmation: bool = False
) -> None:
    outcome = json.loads(artifacts["outcome"].read_text(encoding="utf-8"))
    allowed_statuses = (
        HOSTED_RECOVERABLE_OUTCOME_STATUSES
        if allow_human_confirmation
        else RECOVERABLE_OUTCOME_STATUSES
    )
    if outcome.get("workItemId") != task or outcome.get("status") not in allowed_statuses:
        if allow_human_confirmation:
            raise ValueError(
                "same-Work-Item hosted recovery requires a completed or explicitly human-confirmed archived Outcome"
            )
        raise ValueError("same-Work-Item recovery requires a completed archived Outcome")


def classify_failure(output: str) -> str:
    lowered = output.lower()
    if "changed-critical coverage" in lowered or "below" in lowered and "coverage" in lowered:
        return "changedCriticalCoverage"
    if "archive" in lowered or "paired ownership" in lowered or "human benefit report" in lowered:
        return "archiveEvidence"
    raise ValueError(
        "PR audit failure is not an allowed coverage or archive-evidence recovery gate"
    )


def normalized_paths(paths: list[str]) -> list[str]:
    if not paths:
        raise ValueError("at least one recovery path is required")
    normalized: list[str] = []
    for raw in paths:
        value = raw.strip().replace("\\", "/")
        if not value or value.startswith("/") or ".." in Path(value).parts:
            raise ValueError(f"invalid recovery path: {raw!r}")
        if value.startswith((".ai/work-items/archive/", ".ai/work-items/active/")):
            raise ValueError("recovery paths must not rewrite archive or active Work Item evidence")
        if value not in normalized:
            normalized.append(value)
    return normalized


def receipt_target(directory: Path, task: str, provider: dict | None = None) -> Path:
    """Allocate an append-only receipt path, preserving the original task filename."""
    primary = directory / f"{task}.json"
    if not primary.exists():
        return primary
    if isinstance(provider, dict):
        run_id = provider.get("runId")
        job_id = provider.get("jobId")
        if isinstance(run_id, int) and isinstance(job_id, int):
            target = directory / f"{task}-{run_id}-{job_id}.json"
            if not target.exists():
                return target
            raise ValueError(f"recovery receipt already exists: {target}")
    index = 2
    while (target := directory / f"{task}-{index}.json").exists():
        index += 1
    return target


def _github_api(endpoint: str) -> bytes:
    """Read provider evidence across GitHub CLI versions used by hosted runners."""
    result = subprocess.run(  # nosec B603 B607 - fixed executable and repository-validated endpoint
        ["gh", "api", "--allow-escape-sequences", endpoint],
        cwd=PROJECT_ROOT,
        env=clean_git_environment(),
        capture_output=True,
        check=False,
    )
    if result.returncode != 0:
        detail = result.stderr.decode("utf-8", errors="replace").strip()
        if "unknown flag: --allow-escape-sequences" not in detail:
            raise ValueError(
                f"GitHub provider evidence is unavailable: {detail or 'gh api failed'}"
            )
        result = subprocess.run(  # nosec B603 B607 - fixed executable and repository-validated endpoint
            ["gh", "api", endpoint],
            cwd=PROJECT_ROOT,
            env=clean_git_environment(),
            capture_output=True,
            check=False,
        )
        if result.returncode != 0:
            detail = result.stderr.decode("utf-8", errors="replace").strip()
            raise ValueError(
                f"GitHub provider evidence is unavailable: {detail or 'gh api failed'}"
            )
    return result.stdout


def _provider_json(fetch_provider: Callable[[str], bytes], endpoint: str) -> dict:
    try:
        value = json.loads(fetch_provider(endpoint).decode("utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError, TypeError, ValueError) as exc:
        raise ValueError(f"GitHub provider response is invalid for {endpoint}: {exc}") from exc
    if not isinstance(value, dict):
        raise TypeError(f"GitHub provider response is not an object for {endpoint}")
    return value


def _sha(value: object, label: str) -> str:
    if (
        not isinstance(value, str)
        or len(value) != 40
        or any(character not in "0123456789abcdef" for character in value.lower())
    ):
        raise ValueError(f"{label} must be a 40-character SHA")
    return value.lower()


def _positive_int(value: object, label: str) -> int:
    if not isinstance(value, int) or isinstance(value, bool) or value <= 0:
        raise ValueError(f"{label} must be a positive integer")
    return value


def validate_recorded_provider_binding(provider: object, *, gate: str) -> list[str]:
    """Validate the immutable provider facts captured when a receipt was opened.

    Provider APIs are deliberately consulted only while creating a hosted recovery
    receipt.  PR validation checks the captured facts and the immutable archive
    binding offline, so a hosted runner's network policy cannot change a
    previously verified recovery decision.
    """
    if not isinstance(provider, dict):
        return ["recorded provider binding is required"]
    try:
        repository = provider.get("repository")
        if not isinstance(repository, str) or not GITHUB_REPOSITORY.fullmatch(repository):
            raise ValueError("recorded provider repository is invalid")
        pull_request = _positive_int(provider.get("pullRequest"), "recorded provider pull request")
        failed_head = _sha(
            provider.get("failedCandidateHead"), "recorded provider failed candidate Head SHA"
        )
        run_id = _positive_int(provider.get("runId"), "recorded provider run ID")
        job_id = _positive_int(provider.get("jobId"), "recorded provider job ID")
        run_attempt = provider.get("runAttempt")
        if run_attempt is not None:
            _positive_int(run_attempt, "recorded provider run attempt")
    except ValueError as exc:
        return [str(exc)]
    if provider.get("kind") != "github_actions":
        return ["recorded provider kind is not github_actions"]
    if provider.get("event") != "pull_request":
        return ["recorded provider event is not pull_request"]
    if provider.get("runStatus") != "completed" or provider.get("runConclusion") != "failure":
        return ["recorded provider run is not a completed failure"]
    if provider.get("jobConclusion") != "failure":
        return ["recorded provider job is not a failure"]
    if provider.get("runUrl") != f"https://github.com/{repository}/actions/runs/{run_id}":
        return ["recorded provider run URL does not match its repository and run ID"]
    workflow_path = provider.get("workflowPath")
    if not isinstance(workflow_path, str) or not workflow_path.startswith(".github/workflows/"):
        return ["recorded provider workflow path is invalid"]
    log_digest = provider.get("logSha256")
    if not isinstance(log_digest, str) or SHA256.fullmatch(log_digest) is None:
        return ["recorded provider log digest is invalid"]
    if gate == "hostedAggregateCoverage":
        if provider.get("jobName") != "template-smoke":
            return ["recorded coverage provider job is not template-smoke"]
        coverage = provider.get("observedCoverage")
        if not isinstance(coverage, dict) or coverage.get("parserVersion") != 1:
            return ["recorded provider coverage evidence is invalid"]
        actual, required = coverage.get("actual"), coverage.get("required")
        if (
            not isinstance(actual, (int, float))
            or isinstance(actual, bool)
            or not isinstance(required, (int, float))
            or isinstance(required, bool)
            or actual >= required
        ):
            return ["recorded provider coverage does not prove a below-floor failure"]
    elif gate == "hostedFunctionalFailure":
        if not isinstance(provider.get("jobName"), str) or not provider["jobName"].strip():
            return ["recorded functional provider job name is invalid"]
        marker = provider.get("failureMarker")
        if marker not in {"blocked_recovery", "pytest_failure", "junit_artifact_failure"}:
            return ["recorded functional provider failure marker is invalid"]
        if marker == "junit_artifact_failure":
            junit = provider.get("junitArtifact")
            if not isinstance(junit, dict) or not isinstance(junit.get("name"), str):
                return ["recorded functional JUnit artifact evidence is invalid"]
            try:
                artifact_id = _positive_int(junit.get("id"), "recorded JUnit artifact ID")
                failures = _positive_int(
                    junit.get("failedTestcases"), "recorded JUnit failed testcase count"
                )
            except ValueError:
                return ["recorded functional JUnit artifact evidence is invalid"]
            if (
                artifact_id <= 0
                or failures <= 0
                or not isinstance(junit.get("sha256"), str)
                or SHA256.fullmatch(junit["sha256"]) is None
            ):
                return ["recorded functional JUnit artifact evidence is invalid"]
    elif gate == "hostedGovernanceFailure":
        if not isinstance(provider.get("jobName"), str) or not provider["jobName"].strip():
            return ["recorded governance provider job name is invalid"]
        if provider.get("failureMarker") != "quality_gate_blocked":
            return ["recorded governance provider failure marker is invalid"]
    else:
        return ["recorded provider gate is unsupported"]
    # Keep explicit local variables above: each receipt fact is independently
    # type-checked before the PR gate grants restricted recovery paths.
    _ = (pull_request, failed_head, job_id, run_attempt)
    return []


def verified_hosted_coverage_failure(
    *,
    repository: object,
    pull_request: object,
    failed_candidate_head: object,
    run_id: object,
    job_id: object,
    fetch_provider: Callable[[str], bytes] | None = None,
) -> dict:
    """Return exact GitHub facts only for one failed hosted coverage job."""
    if not isinstance(repository, str) or not GITHUB_REPOSITORY.fullmatch(repository):
        raise ValueError("GitHub repository must be an owner/name identifier")
    fetch_provider = fetch_provider or _github_api
    pull_request = _positive_int(pull_request, "GitHub pull request")
    run_id = _positive_int(run_id, "GitHub workflow run")
    job_id = _positive_int(job_id, "GitHub workflow job")
    failed_candidate_head = _sha(failed_candidate_head, "failed candidate Head SHA")
    run_endpoint = f"/repos/{repository}/actions/runs/{run_id}"
    job_endpoint = f"/repos/{repository}/actions/jobs/{job_id}"
    run = _provider_json(fetch_provider, run_endpoint)
    job = _provider_json(fetch_provider, job_endpoint)
    if run.get("id") != run_id:
        raise ValueError("GitHub workflow run ID does not match the requested run")
    if run.get("event") != "pull_request":
        raise ValueError("GitHub workflow run is not a pull_request event")
    if _sha(run.get("head_sha"), "GitHub workflow run Head SHA") != failed_candidate_head:
        raise ValueError("GitHub workflow run Head SHA does not match the failed candidate")
    if run.get("status") != "completed" or run.get("conclusion") != "failure":
        raise ValueError("GitHub workflow run is not a completed failure")
    pull_requests = run.get("pull_requests")
    if not isinstance(pull_requests, list) or not any(
        isinstance(item, dict) and item.get("number") == pull_request for item in pull_requests
    ):
        raise ValueError("GitHub workflow run does not bind the requested pull request")
    if job.get("id") != job_id or job.get("run_id") != run_id:
        raise ValueError("GitHub workflow job does not belong to the requested run")
    if job.get("name") != "template-smoke":
        raise ValueError("GitHub workflow job is not template-smoke")
    if job.get("status") != "completed" or job.get("conclusion") != "failure":
        raise ValueError("GitHub workflow job is not a completed failure")
    log = fetch_provider(f"{job_endpoint}/logs")
    if not isinstance(log, bytes):
        raise TypeError("GitHub workflow job log is unavailable")
    text = log.decode("utf-8", errors="replace")
    match = COVERAGE_FAILURE.search(text)
    if not match:
        raise ValueError("GitHub workflow job log has no canonical coverage failure")
    actual = float(match.group("actual"))
    required = float(match.group("required"))
    if actual >= required:
        raise ValueError("GitHub workflow job coverage does not prove a below-floor failure")
    run_url = run.get("html_url")
    if not isinstance(run_url, str) or not run_url.startswith("https://github.com/"):
        raise ValueError("GitHub workflow run URL is invalid")
    workflow_path = run.get("path")
    if not isinstance(workflow_path, str) or not workflow_path.startswith(".github/workflows/"):
        raise ValueError("GitHub workflow path is invalid")
    return {
        "kind": "github_actions",
        "repository": repository,
        "pullRequest": pull_request,
        "failedCandidateHead": failed_candidate_head,
        "runId": run_id,
        "runUrl": run_url,
        "workflowPath": workflow_path,
        "event": "pull_request",
        "runAttempt": run.get("run_attempt"),
        "jobId": job_id,
        "jobName": "template-smoke",
        "runStatus": "completed",
        "runConclusion": "failure",
        "jobConclusion": "failure",
        "logSha256": hashlib.sha256(log).hexdigest(),
        "observedCoverage": {
            "actual": actual,
            "required": required,
            "parserVersion": 1,
        },
    }


def open_hosted_post_archive_recovery(
    *,
    root: Path,
    task: str,
    base_commit: str,
    issue: str,
    authority: str,
    recovery_paths: list[str],
    repository: str,
    pull_request: int,
    failed_candidate_head: str,
    run_id: int,
    job_id: int,
    worktree_clean: Callable[[], bool],
    fetch_provider: Callable[[str], bytes] | None = None,
) -> dict:
    """Open the hosted-only recovery route from independently verified provider facts."""
    if len(base_commit) != 40:
        raise ValueError("PR base commit must be a 40-character SHA")
    if not issue.startswith(f"https://github.com/{repository}/issues/"):
        raise ValueError("hosted recovery Issue must belong to the GitHub repository")
    if not authority.strip():
        raise ValueError("human authority is required")
    if not worktree_clean():
        raise ValueError("post-archive recovery must start from a clean committed worktree")
    artifacts = archive_files(root, task)
    _require_recoverable_outcome(artifacts, task, allow_human_confirmation=True)
    provider = verified_hosted_coverage_failure(
        repository=repository,
        pull_request=pull_request,
        failed_candidate_head=failed_candidate_head,
        run_id=run_id,
        job_id=job_id,
        fetch_provider=fetch_provider,
    )
    receipt = {
        "receiptVersion": HOSTED_RECEIPT_VERSION,
        "kind": "same_work_item_post_archive_recovery",
        "workItemId": task,
        "prBaseCommit": base_commit,
        "issue": issue,
        "humanAuthorization": {"type": "human", "reference": authority},
        "failure": {"gate": "hostedAggregateCoverage"},
        "provider": provider,
        "archive": {
            name: {
                "path": path.relative_to(root).as_posix(),
                "sha256": digest(path),
            }
            for name, path in artifacts.items()
        },
        "recoveryPaths": normalized_paths(recovery_paths),
        "openedAt": datetime.now(UTC).isoformat(),
    }
    directory = root / RECEIPT_DIRECTORY
    directory.mkdir(parents=True, exist_ok=True)
    target = receipt_target(directory, task, provider)
    target.write_text(json.dumps(receipt, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return receipt


def verified_hosted_functional_failure(
    *,
    repository: object,
    pull_request: object,
    failed_candidate_head: object,
    run_id: object,
    job_id: object,
    artifact_name: str | None = None,
    fetch_provider: Callable[[str], bytes] | None = None,
) -> dict:
    """Bind a completed fail-closed hosted functional failure to one PR candidate."""
    if not isinstance(repository, str) or not GITHUB_REPOSITORY.fullmatch(repository):
        raise ValueError("GitHub repository must be an owner/name identifier")
    fetch_provider = fetch_provider or _github_api
    pull_request = _positive_int(pull_request, "GitHub pull request")
    run_id = _positive_int(run_id, "GitHub workflow run")
    job_id = _positive_int(job_id, "GitHub workflow job")
    failed_candidate_head = _sha(failed_candidate_head, "failed candidate Head SHA")
    run_endpoint = f"/repos/{repository}/actions/runs/{run_id}"
    job_endpoint = f"/repos/{repository}/actions/jobs/{job_id}"
    run = _provider_json(fetch_provider, run_endpoint)
    job = _provider_json(fetch_provider, job_endpoint)
    if run.get("id") != run_id or run.get("event") != "pull_request":
        raise ValueError("GitHub workflow run is not the requested pull_request run")
    if _sha(run.get("head_sha"), "GitHub workflow run Head SHA") != failed_candidate_head:
        raise ValueError("GitHub workflow run Head SHA does not match the failed candidate")
    if run.get("status") != "completed" or run.get("conclusion") != "failure":
        raise ValueError("GitHub workflow run is not a completed failure")
    pull_requests = run.get("pull_requests")
    if not isinstance(pull_requests, list) or not any(
        isinstance(item, dict) and item.get("number") == pull_request for item in pull_requests
    ):
        raise ValueError("GitHub workflow run does not bind the requested pull request")
    job_name = job.get("name")
    if not isinstance(job_name, str) or not job_name.strip():
        raise ValueError("GitHub workflow job name is invalid")
    if job.get("id") != job_id or job.get("run_id") != run_id:
        raise ValueError("GitHub workflow job does not belong to the requested run")
    if job.get("status") != "completed" or job.get("conclusion") != "failure":
        raise ValueError("GitHub workflow job is not a completed failure")
    log = fetch_provider(f"{job_endpoint}/logs")
    if not isinstance(log, bytes):
        raise TypeError("GitHub workflow job log is unavailable")
    text = log.decode("utf-8", errors="replace")
    failure_marker = functional_failure_marker(text)
    junit_artifact = None
    if failure_marker is None and artifact_name is not None:
        junit_artifact = junit_artifact_failure(
            fetch_provider, repository=repository, run_id=run_id, artifact_name=artifact_name
        )
        failure_marker = "junit_artifact_failure"
    if failure_marker is None:
        raise ValueError("GitHub workflow job log has no canonical fail-closed functional failure")
    run_url = run.get("html_url")
    workflow_path = run.get("path")
    if not isinstance(run_url, str) or not run_url.startswith("https://github.com/"):
        raise ValueError("GitHub workflow run URL is invalid")
    if not isinstance(workflow_path, str) or not workflow_path.startswith(".github/workflows/"):
        raise ValueError("GitHub workflow path is invalid")
    provider = {
        "kind": "github_actions",
        "repository": repository,
        "pullRequest": pull_request,
        "failedCandidateHead": failed_candidate_head,
        "runId": run_id,
        "runUrl": run_url,
        "workflowPath": workflow_path,
        "event": "pull_request",
        "runAttempt": run.get("run_attempt"),
        "jobId": job_id,
        "jobName": job_name,
        "runStatus": "completed",
        "runConclusion": "failure",
        "jobConclusion": "failure",
        "logSha256": hashlib.sha256(log).hexdigest(),
        "failureMarker": failure_marker,
    }
    if junit_artifact is not None:
        provider["junitArtifact"] = junit_artifact
    return provider


def verified_hosted_governance_failure(
    *,
    repository: object,
    pull_request: object,
    failed_candidate_head: object,
    run_id: object,
    job_id: object,
    fetch_provider: Callable[[str], bytes] | None = None,
) -> dict:
    """Bind an explicit hosted quality-gate block to one PR candidate."""
    provider = verified_hosted_functional_failure(
        repository=repository,
        pull_request=pull_request,
        failed_candidate_head=failed_candidate_head,
        run_id=run_id,
        job_id=job_id,
        fetch_provider=fetch_provider,
    )
    log_marker = provider.get("failureMarker")
    if log_marker != "quality_gate_blocked":
        raise ValueError("GitHub workflow job log has no canonical quality-gate blocked failure")
    return provider


def open_hosted_functional_failure_recovery(
    *,
    root: Path,
    task: str,
    base_commit: str,
    issue: str,
    authority: str,
    recovery_paths: list[str],
    repository: str,
    pull_request: int,
    failed_candidate_head: str,
    run_id: int,
    job_id: int,
    artifact_name: str | None = None,
    worktree_clean: Callable[[], bool],
    fetch_provider: Callable[[str], bytes] | None = None,
) -> dict:
    """Open same-Work-Item recovery for a provider-bound fail-closed hosted defect."""
    if len(base_commit) != 40:
        raise ValueError("PR base commit must be a 40-character SHA")
    if not issue.startswith(f"https://github.com/{repository}/issues/"):
        raise ValueError("hosted recovery Issue must belong to the GitHub repository")
    if not authority.strip():
        raise ValueError("human authority is required")
    if not worktree_clean():
        raise ValueError("post-archive recovery must start from a clean committed worktree")
    artifacts = archive_files(root, task)
    _require_recoverable_outcome(artifacts, task, allow_human_confirmation=True)
    provider = verified_hosted_functional_failure(
        repository=repository,
        pull_request=pull_request,
        failed_candidate_head=failed_candidate_head,
        run_id=run_id,
        job_id=job_id,
        artifact_name=artifact_name,
        fetch_provider=fetch_provider,
    )
    receipt = {
        "receiptVersion": HOSTED_FUNCTIONAL_RECEIPT_VERSION,
        "kind": "same_work_item_post_archive_recovery",
        "workItemId": task,
        "prBaseCommit": base_commit,
        "issue": issue,
        "humanAuthorization": {"type": "human", "reference": authority},
        "failure": {"gate": "hostedFunctionalFailure"},
        "provider": provider,
        "archive": {
            name: {"path": path.relative_to(root).as_posix(), "sha256": digest(path)}
            for name, path in artifacts.items()
        },
        "recoveryPaths": normalized_paths(recovery_paths),
        "openedAt": datetime.now(UTC).isoformat(),
    }
    directory = root / RECEIPT_DIRECTORY
    directory.mkdir(parents=True, exist_ok=True)
    target = receipt_target(directory, task, provider)
    target.write_text(json.dumps(receipt, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return receipt


def open_hosted_governance_failure_recovery(
    *,
    root: Path,
    task: str,
    base_commit: str,
    issue: str,
    authority: str,
    recovery_paths: list[str],
    repository: str,
    pull_request: int,
    failed_candidate_head: str,
    run_id: int,
    job_id: int,
    worktree_clean: Callable[[], bool],
    fetch_provider: Callable[[str], bytes] | None = None,
) -> dict:
    """Open same-Work-Item recovery for a provider-bound quality-gate block."""
    if len(base_commit) != 40:
        raise ValueError("PR base commit must be a 40-character SHA")
    if not issue.startswith(f"https://github.com/{repository}/issues/"):
        raise ValueError("hosted recovery Issue must belong to the GitHub repository")
    if not authority.strip():
        raise ValueError("human authority is required")
    if not worktree_clean():
        raise ValueError("post-archive recovery must start from a clean committed worktree")
    artifacts = archive_files(root, task)
    _require_recoverable_outcome(artifacts, task, allow_human_confirmation=True)
    provider = verified_hosted_governance_failure(
        repository=repository,
        pull_request=pull_request,
        failed_candidate_head=failed_candidate_head,
        run_id=run_id,
        job_id=job_id,
        fetch_provider=fetch_provider,
    )
    receipt = {
        "receiptVersion": HOSTED_GOVERNANCE_RECEIPT_VERSION,
        "kind": "same_work_item_post_archive_recovery",
        "workItemId": task,
        "prBaseCommit": base_commit,
        "issue": issue,
        "humanAuthorization": {"type": "human", "reference": authority},
        "failure": {"gate": "hostedGovernanceFailure"},
        "provider": provider,
        "archive": {
            name: {"path": path.relative_to(root).as_posix(), "sha256": digest(path)}
            for name, path in artifacts.items()
        },
        "recoveryPaths": normalized_paths(recovery_paths),
        "openedAt": datetime.now(UTC).isoformat(),
    }
    directory = root / RECEIPT_DIRECTORY
    directory.mkdir(parents=True, exist_ok=True)
    target = receipt_target(directory, task, provider)
    target.write_text(json.dumps(receipt, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return receipt


def open_post_archive_recovery(
    *,
    root: Path,
    task: str,
    base_commit: str,
    issue: str,
    authority: str,
    recovery_paths: list[str],
    run_pr_audit: Callable[[list[str]], tuple[int, str]],
    worktree_clean: Callable[[], bool],
) -> dict:
    if len(base_commit) != 40:
        raise ValueError("PR base commit must be a 40-character SHA")
    if not issue.startswith("https://github.com/"):
        raise ValueError("recovery Issue must be a GitHub Issue URL")
    if not authority.strip():
        raise ValueError("human authority is required")
    if not worktree_clean():
        raise ValueError("post-archive recovery must start from a clean committed worktree")
    artifacts = archive_files(root, task)
    _require_recoverable_outcome(artifacts, task)
    code, output = run_pr_audit(["make", "check-ai-pr", f"AI_BASE_COMMIT={base_commit}"])
    if code == 0:
        raise ValueError("post-archive recovery may open only after check-ai-pr must fail")
    gate = classify_failure(output)
    receipt = {
        "receiptVersion": 1,
        "kind": "same_work_item_post_archive_recovery",
        "workItemId": task,
        "prBaseCommit": base_commit,
        "issue": issue,
        "humanAuthorization": {"type": "human", "reference": authority},
        "failure": {
            "gate": gate,
            "command": ["make", "check-ai-pr", f"AI_BASE_COMMIT={base_commit}"],
            "exitCode": code,
            "outputDigest": hashlib.sha256(output.encode("utf-8")).hexdigest(),
        },
        "archive": {
            name: {
                "path": path.relative_to(root).as_posix(),
                "sha256": digest(path),
            }
            for name, path in artifacts.items()
        },
        "recoveryPaths": normalized_paths(recovery_paths),
        "openedAt": datetime.now(UTC).isoformat(),
    }
    directory = root / RECEIPT_DIRECTORY
    directory.mkdir(parents=True, exist_ok=True)
    target = receipt_target(directory, task)
    target.write_text(json.dumps(receipt, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return receipt


def validate_recovery_receipt(
    root: Path,
    receipt: object,
    *,
    pr_base: str,
    fetch_provider: Callable[[str], bytes] | None = None,
) -> list[str]:
    if not isinstance(receipt, dict):
        return ["recovery receipt must be an object"]
    version = receipt.get("receiptVersion")
    if (
        version
        not in {
            1,
            HOSTED_RECEIPT_VERSION,
            HOSTED_FUNCTIONAL_RECEIPT_VERSION,
            HOSTED_GOVERNANCE_RECEIPT_VERSION,
        }
        or receipt.get("kind") != "same_work_item_post_archive_recovery"
    ):
        return ["recovery receipt has an unsupported schema"]
    task = receipt.get("workItemId")
    if not isinstance(task, str) or not task:
        return ["recovery receipt workItemId is required"]
    if receipt.get("prBaseCommit") != pr_base:
        return ["recovery receipt PR base does not match the checked PR base"]
    authorization = receipt.get("humanAuthorization")
    if (
        not isinstance(authorization, dict)
        or authorization.get("type") != "human"
        or not isinstance(authorization.get("reference"), str)
        or not authorization["reference"].strip()
    ):
        return ["recovery receipt requires human authorization"]
    failure = receipt.get("failure")
    if not isinstance(failure, dict) or failure.get("gate") not in ALLOWED_GATES:
        return ["recovery receipt failure gate is not allowed"]
    if failure.get("gate") in {
        "hostedAggregateCoverage",
        "hostedFunctionalFailure",
        "hostedGovernanceFailure",
    }:
        provider_issues = validate_recorded_provider_binding(
            receipt.get("provider"), gate=failure["gate"]
        )
        if provider_issues:
            return provider_issues
    elif version != 1:
        return ["hosted recovery receipt must declare hostedAggregateCoverage"]
    paths = receipt.get("recoveryPaths")
    try:
        if not isinstance(paths, list) or normalized_paths(paths) != paths:
            return ["recovery receipt paths are invalid or non-canonical"]
        artifacts = archive_files(root, task)
    except (TypeError, ValueError) as exc:
        return [str(exc)]
    archive = receipt.get("archive")
    if not isinstance(archive, dict):
        return ["recovery receipt archive binding is required"]
    issues: list[str] = []
    for name, path in artifacts.items():
        expected = archive.get(name)
        if (
            not isinstance(expected, dict)
            or expected.get("path") != path.relative_to(root).as_posix()
            or expected.get("sha256") != digest(path)
        ):
            issues.append(f"recovery receipt archive binding changed: {name}")
    return issues


def _clean_worktree() -> bool:
    result = subprocess.run(  # nosec B603 B607 - fixed list-form Git status inspection
        ["git", "status", "--porcelain", "--untracked-files=all"],
        cwd=PROJECT_ROOT,
        env=clean_git_environment(),
        text=True,
        capture_output=True,
        check=False,
    )
    return result.returncode == 0 and not result.stdout.strip()


def _run_pr_audit(command: list[str]) -> tuple[int, str]:
    result = subprocess.run(  # nosec B603 - caller constructs only the fixed PR-audit argv
        command,
        cwd=PROJECT_ROOT,
        env=clean_git_environment(),
        text=True,
        capture_output=True,
        check=False,
    )
    return result.returncode, result.stdout + result.stderr


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--task", required=True)
    parser.add_argument("--base", required=True)
    parser.add_argument("--issue", required=True)
    parser.add_argument("--authority", required=True)
    parser.add_argument("--recovery-path", action="append", default=[])
    parser.add_argument("--hosted-repository")
    parser.add_argument("--hosted-pull-request", type=int)
    parser.add_argument("--hosted-candidate-head")
    parser.add_argument("--hosted-run-id", type=int)
    parser.add_argument("--hosted-job-id", type=int)
    parser.add_argument("--hosted-artifact-name")
    parser.add_argument(
        "--hosted-failure-kind",
        choices=("coverage", "functional", "governance"),
        default="coverage",
    )
    args = parser.parse_args()
    try:
        hosted_values = (
            args.hosted_repository,
            args.hosted_pull_request,
            args.hosted_candidate_head,
            args.hosted_run_id,
            args.hosted_job_id,
        )
        if any(value is not None for value in hosted_values):
            if any(value is None for value in hosted_values):
                raise ValueError(
                    "hosted repository, pull request, candidate Head, run ID, and job ID are required together"
                )
            if args.hosted_failure_kind == "functional":
                receipt = open_hosted_functional_failure_recovery(
                    root=PROJECT_ROOT,
                    task=args.task,
                    base_commit=args.base,
                    issue=args.issue,
                    authority=args.authority,
                    recovery_paths=args.recovery_path,
                    repository=args.hosted_repository,
                    pull_request=args.hosted_pull_request,
                    failed_candidate_head=args.hosted_candidate_head,
                    run_id=args.hosted_run_id,
                    job_id=args.hosted_job_id,
                    artifact_name=args.hosted_artifact_name,
                    worktree_clean=_clean_worktree,
                )
            elif args.hosted_failure_kind == "governance":
                receipt = open_hosted_governance_failure_recovery(
                    root=PROJECT_ROOT,
                    task=args.task,
                    base_commit=args.base,
                    issue=args.issue,
                    authority=args.authority,
                    recovery_paths=args.recovery_path,
                    repository=args.hosted_repository,
                    pull_request=args.hosted_pull_request,
                    failed_candidate_head=args.hosted_candidate_head,
                    run_id=args.hosted_run_id,
                    job_id=args.hosted_job_id,
                    fetch_provider=None,
                    worktree_clean=_clean_worktree,
                )
            else:
                receipt = open_hosted_post_archive_recovery(
                    root=PROJECT_ROOT,
                    task=args.task,
                    base_commit=args.base,
                    issue=args.issue,
                    authority=args.authority,
                    recovery_paths=args.recovery_path,
                    repository=args.hosted_repository,
                    pull_request=args.hosted_pull_request,
                    failed_candidate_head=args.hosted_candidate_head,
                    run_id=args.hosted_run_id,
                    job_id=args.hosted_job_id,
                    worktree_clean=_clean_worktree,
                )
        else:
            receipt = open_post_archive_recovery(
                root=PROJECT_ROOT,
                task=args.task,
                base_commit=args.base,
                issue=args.issue,
                authority=args.authority,
                recovery_paths=args.recovery_path,
                run_pr_audit=_run_pr_audit,
                worktree_clean=_clean_worktree,
            )
    except ValueError as exc:
        print(f"ERROR: {exc}")
        return 1
    print(json.dumps(receipt, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
