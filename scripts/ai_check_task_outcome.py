"""Fail-closed validation for Task Outcome JSON and derived Markdown."""

from __future__ import annotations

import re
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from typing import Any

from ai_check_summary import validate_implementation_approach

STATUSES = {
    "completed",
    "completed_with_warnings",
    "needs_human_confirmation",
    "blocked",
    "cancelled",
}
SEVERITIES = {"informational", "low", "medium", "high", "critical"}
SECTIONS = {
    "outcomeSummary",
    "taskOverview",
    "deliveredChanges",
    "findings",
    "risks",
    "warnings",
    "limitations",
    "nonRiskExplanations",
    "forbiddenClaims",
    "interventions",
    "forcedStops",
    "resolutions",
    "recurrencePrevention",
    "avoidedImpact",
    "residualRisks",
    "humanDecisions",
    "evidence",
    "implementationApproach",
}
LEGACY_SECTIONS = SECTIONS - {"implementationApproach"}
LEGACY_MINIMAL_SECTIONS = LEGACY_SECTIONS - {
    "limitations",
    "nonRiskExplanations",
    "forbiddenClaims",
}
SECRET_KEY = re.compile(
    r"(password|passwd|secret|token|api[_-]?key|private[_-]?key)", re.IGNORECASE
)
UNSUPPORTED_KEY = re.compile(
    r"(score|hours?|money|percentage|percent|productivity|savings)", re.IGNORECASE
)
CONDITIONAL = ("if not detected", "could have", "如果未被发现", "可能导致")
NOT_RUN = re.compile(r"\bnot[_ -]?run\b", re.IGNORECASE)
INCOMPATIBLE_VERIFIED_CLAIM = re.compile(
    r"\b(?:enterprise[-_ ]ready|platform[-_ ]verified)\b", re.IGNORECASE
)
SELF_PRAISE = re.compile(
    r"(?:system(?:表现非常优秀|performed exceptionally)|成功保护了项目|极大提升了质量|大幅节省了时间|dramatically improved project quality|greatly improved quality|saved a lot of time)",
    re.IGNORECASE,
)
# Work Item Contracts historically use both hyphenated and underscore task IDs
# (for example, the installed first-adoption Contract is adopt_ai_cockpit).
# Outcome validation must bind that canonical Contract ID rather than reject a
# valid lifecycle purely because its separator differs.
TASK_ID = re.compile(r"^[a-z0-9][a-z0-9_-]{2,127}$")
SHA256 = re.compile(r"^[a-f0-9]{64}$")
COMMIT = re.compile(r"^[0-9a-f]{40}$")
HUMAN_STATUS_COLORS = {"green", "yellow", "red", "unknown"}
STATUS_COLORS = {
    "completed": "green",
    "completed_with_warnings": "yellow",
    "needs_human_confirmation": "yellow",
    "blocked": "red",
    "cancelled": "red",
}
SUPPORTED_LOCALES = {"en", "ja", "zh-CN"}
HANDOFF_QUESTION_LISTS = (
    "blockedProblems",
    "resolvedProblems",
    "resolutionApproach",
    "avoidedRisks",
    "remainingRisks",
    "agentUnknowns",
    "humanConfirmations",
)


def _requires_human_status_projection(outcome: Mapping[str, Any]) -> bool:
    bindings = outcome.get("bindings")
    if not isinstance(bindings, Mapping):
        return False
    version = bindings.get("generatorVersion")
    if not isinstance(version, str):
        return False
    try:
        major, minor = (int(part) for part in version.split(".", maxsplit=1))
    except ValueError:
        return False
    return (major, minor) >= (1, 1)


def _validate_human_status_projection(
    outcome: Mapping[str, Any], errors: list[ValidationError]
) -> None:
    if not _requires_human_status_projection(outcome):
        return
    status = outcome.get("status")
    color = outcome.get("humanStatusColor")
    expected_color = STATUS_COLORS.get(status) if isinstance(status, str) else None
    if color not in HUMAN_STATUS_COLORS or color != expected_color:
        _error(errors, "human_status", "humanStatusColor contradicts Outcome status")
    failed_gate = outcome.get("failedGate")
    recovery = outcome.get("recoveryCondition")
    if not isinstance(failed_gate, str) or not isinstance(recovery, str):
        _error(errors, "human_status", "diagnostic fields must be text")
    elif status == "blocked" and (not failed_gate.strip() or not recovery.strip()):
        _error(errors, "human_status", "blocked Outcome requires failedGate and recoveryCondition")


def _requires_human_handoff_projection(outcome: Mapping[str, Any]) -> bool:
    bindings = outcome.get("bindings")
    if not isinstance(bindings, Mapping):
        return False
    version = bindings.get("generatorVersion")
    if not isinstance(version, str):
        return False
    try:
        major, minor = (int(part) for part in version.split(".", maxsplit=1))
    except ValueError:
        return False
    return (major, minor) >= (1, 2)


def _validate_claim_items(
    value: Any, errors: list[ValidationError], path: str, *, required: bool = False
) -> None:
    if not isinstance(value, list):
        _error(errors, "human_handoff", f"{path} must be an array")
        return
    if required and not value:
        _error(errors, "human_handoff", f"{path} must contain at least one evidence-backed item")
    for index, item in enumerate(value):
        if not isinstance(item, Mapping):
            _error(errors, "human_handoff", f"{path}[{index}] must be an object")
            continue
        claim = item.get("claim")
        refs = item.get("evidenceRefs")
        if not isinstance(claim, str) or not claim.strip():
            _error(errors, "human_handoff", f"{path}[{index}].claim must be non-empty text")
        if not isinstance(refs, list):
            _error(errors, "human_handoff", f"{path}[{index}].evidenceRefs must be an array")
        if refs == [] and item.get("inference") is not True:
            _error(
                errors,
                "human_handoff",
                f"{path}[{index}] without evidence must be marked inference",
            )
        if refs and item.get("inference") is True:
            _error(
                errors, "human_handoff", f"{path}[{index}] with evidence cannot be marked inference"
            )


def _validate_human_handoff_projection(
    outcome: Mapping[str, Any], errors: list[ValidationError]
) -> None:
    if not _requires_human_handoff_projection(outcome):
        return
    handoff = outcome.get("humanHandoff")
    if not isinstance(handoff, Mapping):
        _error(errors, "human_handoff", "generator >= 1.2 requires humanHandoff")
        return
    locale = handoff.get("locale")
    if locale not in SUPPORTED_LOCALES:
        _error(errors, "human_handoff", "humanHandoff.locale is unsupported or missing")
    status = outcome.get("status")
    for key in ("completed", "passed", "retained", "risks", "redReasons"):
        _validate_claim_items(
            handoff.get(key),
            errors,
            f"humanHandoff.{key}",
            required=key in {"completed", "passed"},
        )
    if status == "blocked" and not handoff.get("redReasons"):
        _error(errors, "human_handoff", "blocked Outcome requires humanHandoff.redReasons")
    questions = handoff.get("questions")
    if not isinstance(questions, Mapping):
        _error(errors, "human_handoff", "humanHandoff.questions is required")
        return
    problem_count = questions.get("problemCount")
    if not isinstance(problem_count, int) or problem_count < 0:
        _error(
            errors,
            "human_handoff",
            "humanHandoff.questions.problemCount must be a non-negative integer",
        )
    refs = questions.get("problemCountEvidenceRefs")
    if not isinstance(refs, list):
        _error(errors, "human_handoff", "problemCountEvidenceRefs must be an array")
    for key in HANDOFF_QUESTION_LISTS:
        _validate_claim_items(questions.get(key), errors, f"humanHandoff.questions.{key}")
    for key in ("recurrenceLikelihood", "nextTime"):
        item = questions.get(key)
        if (
            not isinstance(item, Mapping)
            or not isinstance(item.get("claim"), str)
            or not item.get("claim", "").strip()
        ):
            _error(errors, "human_handoff", f"humanHandoff.questions.{key} must contain a claim")
        elif not isinstance(item.get("evidenceRefs"), list):
            _error(
                errors,
                "human_handoff",
                f"humanHandoff.questions.{key}.evidenceRefs must be an array",
            )
        elif not item.get("evidenceRefs") and item.get("inference") is not True:
            _error(
                errors,
                "human_handoff",
                f"humanHandoff.questions.{key} without evidence must be marked inference",
            )


@dataclass(frozen=True)
class ValidationError:
    code: str
    message: str


@dataclass(frozen=True)
class ValidationReport:
    valid: bool
    errors: tuple[ValidationError, ...]


def _error(errors: list[ValidationError], code: str, message: str) -> None:
    errors.append(ValidationError(code, message))


def _walk(value: Any, errors: list[ValidationError], path: str = "outcome") -> None:
    if isinstance(value, dict):
        for key, child in value.items():
            if SECRET_KEY.search(str(key)):
                _error(errors, "privacy", f"secret-like key at {path}.{key}")
            if UNSUPPORTED_KEY.search(str(key)):
                _error(
                    errors, "unsupported_quantification", f"unsupported metric key at {path}.{key}"
                )
            _walk(child, errors, f"{path}.{key}")
    elif isinstance(value, str) and SELF_PRAISE.search(value):
        _error(errors, "unsupported_self_praise", f"unsupported self-praise claim at {path}")
    elif isinstance(value, list):
        for index, child in enumerate(value):
            _walk(child, errors, f"{path}[{index}]")


def _required_mapping(
    value: Any, keys: set[str], errors: list[ValidationError], code: str, path: str
) -> None:
    if not isinstance(value, dict) or not keys.issubset(value):
        _error(
            errors,
            code,
            f"{path} is missing required fields: {sorted(keys - set(value) if isinstance(value, dict) else keys)}",
        )


def _validate_bindings(
    outcome: Mapping[str, Any], expected_task_id: str | None, errors: list[ValidationError]
) -> None:
    task_id = outcome.get("workItemId")
    if (
        not isinstance(task_id, str)
        or not TASK_ID.fullmatch(task_id)
        or (expected_task_id and task_id != expected_task_id)
    ):
        _error(errors, "task_binding", "workItemId does not match the expected Task ID")
    bindings = outcome.get("bindings")
    required = {
        "taskId",
        "contractDigest",
        "summaryDigest",
        "verificationDigest",
        "baseCommit",
        "headCommit",
        "lifecycleStage",
        "pullRequest",
        "aiCockpitVersion",
        "generatorVersion",
    }
    _required_mapping(bindings, required, errors, "binding", "bindings")
    if not isinstance(bindings, dict):
        return
    if bindings.get("taskId") != task_id:
        _error(errors, "binding", "bindings.taskId does not match workItemId")
    for key in ("contractDigest", "summaryDigest", "verificationDigest"):
        if not isinstance(bindings.get(key), str) or not SHA256.fullmatch(bindings[key]):
            _error(errors, "binding", f"{key} is not a SHA-256 digest")
    for key in ("baseCommit", "headCommit"):
        if not isinstance(bindings.get(key), str) or not COMMIT.fullmatch(bindings[key]):
            _error(errors, "binding", f"{key} is not a commit object ID")
    stage = bindings.get("lifecycleStage")
    pull = bindings.get("pullRequest")
    if stage == "pre_merge":
        if pull != {"state": "not_created"}:
            _error(errors, "provenance", "pre_merge pullRequest binding must be not_created")
    elif stage == "post_pr":
        if (
            not isinstance(pull, dict)
            or not isinstance(pull.get("number"), int)
            or pull.get("number", 0) < 1
            or not isinstance(pull.get("url"), str)
            or not pull["url"].startswith("https://")
        ):
            _error(errors, "provenance", "post_pr pullRequest binding is invalid")
    else:
        _error(errors, "provenance", "lifecycleStage is invalid")


def _validate_sections(sections: Any, errors: list[ValidationError]) -> None:
    if not isinstance(sections, dict) or set(sections) not in (
        SECTIONS,
        LEGACY_SECTIONS,
        LEGACY_MINIMAL_SECTIONS,
    ):
        _error(errors, "section_shape", "sections must contain the supported Outcome section set")
        return
    for key in sections:
        if key == "implementationApproach":
            if not isinstance(sections[key], Mapping):
                _error(errors, "section_shape", "sections.implementationApproach must be an object")
        elif key not in {"outcomeSummary", "taskOverview"} and not isinstance(sections[key], list):
            _error(errors, "section_shape", f"sections.{key} must be an array")
    if isinstance(sections.get("warnings"), list) and any(
        not isinstance(item, str) for item in sections["warnings"]
    ):
        _error(errors, "section_shape", "sections.warnings items must be text")
    for key in ("outcomeSummary", "taskOverview"):
        if not isinstance(sections[key], str) or not sections[key].strip():
            _error(errors, "section_shape", f"sections.{key} must be non-empty text")


def _validate_events(events: Sequence[Mapping[str, Any]], errors: list[ValidationError]) -> None:
    ids: set[Any] = set()
    for event in events:
        if not isinstance(event, Mapping):
            _error(errors, "event_relationship", "event must be an object")
            continue
        event_id = event.get("eventId")
        if event_id in ids:
            _error(errors, "event_identity", f"duplicate eventId: {event_id}")
        ids.add(event_id)
        for relation in ("correctsEventId", "supersedesEventId"):
            if relation in event and event[relation] not in ids:
                _error(errors, "event_relationship", f"{relation} references missing event")


def _validate_severities(sections: Mapping[str, Any], errors: list[ValidationError]) -> None:
    for section in ("findings", "risks", "residualRisks"):
        values = sections.get(section, [])
        if not isinstance(values, list):
            continue
        for index, item in enumerate(values):
            if isinstance(item, Mapping) and item.get("severity") not in SEVERITIES:
                _error(errors, "severity", f"{section}[{index}].severity is invalid")


def _validate_claims(
    sections: Mapping[str, Any],
    errors: list[ValidationError],
    contract: Mapping[str, Any] | None = None,
) -> None:
    approach = sections.get("implementationApproach")
    if isinstance(approach, Mapping):
        approach_contract = dict(contract) if contract else None
        for issue in validate_implementation_approach(approach, approach_contract):
            _error(errors, "implementation_approach_evidence", issue)
    warnings = sections.get("warnings", [])
    limitations = sections.get("limitations", [])
    non_risks = sections.get("nonRiskExplanations", [])
    residual = sections.get("residualRisks", [])
    forbidden_claims = sections.get("forbiddenClaims", [])
    limitation_sources = {
        item.get("sourceWarning") for item in limitations if isinstance(item, Mapping)
    }
    non_risk_sources = {
        item.get("sourceWarning") for item in non_risks if isinstance(item, Mapping)
    }
    residual_sources = {item.get("sourceWarning") for item in residual if isinstance(item, Mapping)}
    for warning in (item for item in warnings if isinstance(item, str)):
        if warning not in limitation_sources or not (
            warning in non_risk_sources or warning in residual_sources
        ):
            _error(
                errors,
                "warning_binding",
                "warnings require structured limitation and residual-risk or non-risk bindings",
            )
    not_run_warnings = [
        warning for warning in warnings if isinstance(warning, str) and NOT_RUN.search(warning)
    ]
    if not_run_warnings:
        rendered_text = " ".join(
            str(sections.get(key, ""))
            for key in ("outcomeSummary", "taskOverview", "deliveredChanges", "findings")
        )
        if INCOMPATIBLE_VERIFIED_CLAIM.search(rendered_text):
            _error(
                errors,
                "not_run_claim",
                "not_run evidence is incompatible with enterprise-ready or platform-verified claims",
            )
    if sections.get("warnings") and not forbidden_claims:
        _error(errors, "forbidden_claim", "warnings require explicit forbidden claims")
    if sections.get("warnings") and any(
        not isinstance(item, str) or not item.strip() for item in forbidden_claims
    ):
        _error(errors, "forbidden_claim", "forbidden claims must be non-empty text")
    if sections.get("warnings") and not all(
        isinstance(item, Mapping) and isinstance(item.get("sourceWarning"), str)
        for item in limitations
    ):
        _error(errors, "warning_binding", "limitations must identify their source warning")
    for risk in residual:
        if not isinstance(risk, Mapping) or risk.get("severity") not in {"high", "critical"}:
            continue
        for key in ("decisionOwner", "requiredEvidence", "mitigation", "acceptanceStatus"):
            if not risk.get(key):
                _error(errors, "residual_risk", f"high residual risk requires {key}")
    for claim in sections.get("avoidedImpact", []):
        if not isinstance(claim, str) or not claim.strip().lower().startswith(CONDITIONAL):
            _error(errors, "conditional_claim", "Avoided Impact must use conditional language")
    rendered_text = " ".join(
        str(sections.get(key, ""))
        for key in (
            "outcomeSummary",
            "taskOverview",
            "deliveredChanges",
            "warnings",
            "humanDecisions",
        )
    )
    if (
        re.search(r"\bscore\s*[:=]", rendered_text, re.IGNORECASE)
        or re.search(r"\b\d+(?:\.\d+)?\s*%", rendered_text)
        or re.search(r"\b\d+(?:\.\d+)?\s*(?:hours?|percent|money)\b", rendered_text, re.IGNORECASE)
    ):
        _error(
            errors, "unsupported_quantification", "unsupported quantitative claim in report text"
        )
    risks = sections.get("risks", [])
    residual_keys = {
        (item.get("title"), item.get("state")) for item in residual if isinstance(item, Mapping)
    }
    for risk in risks:
        if (
            isinstance(risk, Mapping)
            and risk.get("state") in {"accepted", "unresolved"}
            and (risk.get("title"), risk.get("state")) not in residual_keys
        ):
            _error(errors, "residual_risk", f"residual risk is hidden: {risk.get('title')}")


def _validate_markdown(
    markdown: str | None,
    outcome: Mapping[str, Any],
    sections: Mapping[str, Any],
    errors: list[ValidationError],
) -> None:
    if markdown is None:
        return
    titles = (
        "Findings",
        "Risks",
        "Warnings",
        "Limitations",
        "Non-Risk Explanations",
        "Forbidden Claims",
        "Interventions",
        "Forced Stops",
        "Resolutions",
        "Recurrence Prevention",
        "Avoided Impact",
        "Residual Risks",
        "Human Decisions",
        "Evidence",
    )
    if any(f"## {title}" not in markdown for title in titles):
        _error(errors, "markdown_parity", "Markdown is missing a required section")
    if "implementationApproach" in sections and "## Implementation Approach" not in markdown:
        _error(errors, "markdown_parity", "Markdown is missing the Implementation Approach section")
    for key, title in (("findings", "Findings"), ("residualRisks", "Residual Risks")):
        if not sections[key] and f"## {title}\nNone" not in markdown:
            _error(errors, "markdown_parity", f"empty {title} section must say None")
    if _requires_human_status_projection(outcome):
        color = outcome.get("humanStatusColor")
        if f"Human Status: `{color}`" not in markdown:
            _error(errors, "markdown_parity", "Markdown is missing the human status diagnostic")
        if outcome.get("status") == "blocked":
            if f"Failed Gate: `{outcome.get('failedGate')}`" not in markdown:
                _error(errors, "markdown_parity", "Markdown is missing the failed gate diagnostic")
            recovery = outcome.get("recoveryCondition")
            if not isinstance(recovery, str) or f"Recovery Condition: {recovery}" not in markdown:
                _error(
                    errors,
                    "markdown_parity",
                    "Markdown is missing the recovery condition diagnostic",
                )
    if _requires_human_handoff_projection(outcome):
        required_titles = (
            "Human Handoff",
            "What was completed",
            "What passed",
            "What was retained",
            "Risks",
            "Red reasons",
            "Human questions",
        )
        if any(
            f"## {title}" not in markdown and f"### {title}" not in markdown
            for title in required_titles
        ):
            _error(
                errors, "markdown_parity", "Markdown is missing a required human handoff section"
            )


def validate_outcome(
    outcome: Any,
    markdown: str | None = None,
    *,
    events: Sequence[Mapping[str, Any]] = (),
    expected_task_id: str | None = None,
    contract: Mapping[str, Any] | None = None,
) -> ValidationReport:
    """Validate one Outcome and return structured errors without mutating input."""

    errors: list[ValidationError] = []
    if not isinstance(outcome, dict):
        return ValidationReport(False, (ValidationError("schema", "outcome must be an object"),))
    required = {"format", "schemaVersion", "workItemId", "status", "bindings", "sections"}
    _required_mapping(outcome, required, errors, "schema", "outcome")
    if outcome.get("format") != "ai-cockpit-task-outcome" or outcome.get("schemaVersion") != 1:
        _error(errors, "schema", "format or schemaVersion is invalid")
    if outcome.get("status") not in STATUSES:
        _error(errors, "schema", "status is invalid")
    _validate_bindings(outcome, expected_task_id, errors)
    _validate_human_status_projection(outcome, errors)
    _validate_human_handoff_projection(outcome, errors)
    _validate_sections(outcome.get("sections"), errors)
    if isinstance(outcome.get("sections"), dict):
        _validate_claims(outcome["sections"], errors, contract)
        _validate_severities(outcome["sections"], errors)
        _validate_markdown(markdown, outcome, outcome["sections"], errors)
    _validate_events(events, errors)
    _walk(outcome, errors)
    return ValidationReport(not errors, tuple(errors))
