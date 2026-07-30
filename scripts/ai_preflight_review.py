#!/usr/bin/env python3
"""Work Item Contract から Preflight Review を生成・検証する。"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

from ai_json import load_json as load_json_file


PROJECT_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_OUTPUT = PROJECT_ROOT / "target" / "ai_preflight_review.json"
DEFAULT_POLICY = PROJECT_ROOT / ".ai" / "guards" / "preflight_review_policy.yaml"
ALLOWED_STATUSES = {"ready", "needs_human_confirmation", "not_ready"}
ALLOWED_SIGNAL_VALUES = {
    "Ready",
    "Partial",
    "Missing",
    "Weak",
    "Broad",
    "Suspiciously Empty",
    "Inconsistent",
    "Not Applicable",
}


def non_empty_string(value: Any) -> bool:
    return isinstance(value, str) and bool(value.strip())


def string_list(values: Any) -> list[str]:
    if not isinstance(values, list):
        return []
    return [item.strip() for item in values if isinstance(item, str) and item.strip()]


def contract_hash(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()[:16]


def policy_hash(path: Path) -> str:
    if not path.exists():
        return "defaults"
    return hashlib.sha256(path.read_bytes()).hexdigest()[:16]


def _truthy(value: str | None, *, default: bool = False) -> bool:
    if value is None:
        return default
    normalized = value.strip().lower()
    if normalized in {"1", "true", "yes", "on"}:
        return True
    if normalized in {"0", "false", "no", "off"}:
        return False
    raise ValueError(f"invalid boolean value in policy: {value!r}")


def _strip_quotes(value: str) -> str:
    return value.strip().strip('"').strip("'")


def load_policy(path: Path) -> dict[str, Any]:
    scalars: dict[str, str] = {}
    blocked_statuses: list[str] = []
    current_list: str | None = None
    if path.exists():
        for raw in path.read_text(encoding="utf-8").splitlines():
            line = raw.rstrip()
            stripped = line.strip()
            if not stripped or stripped.startswith("#"):
                continue
            if not line.startswith(" ") and ":" in line:
                key, value = line.split(":", 1)
                key = key.strip()
                value = value.strip()
                current_list = None
                if value:
                    scalars[key] = _strip_quotes(value)
                else:
                    current_list = key
                    if current_list == "blockedStatuses":
                        blocked_statuses = []
                continue
            if current_list == "blockedStatuses" and line.startswith("  - "):
                blocked_statuses.append(_strip_quotes(line[4:].strip()))

    gate_enabled = _truthy(scalars.get("gateEnabled"), default=False)
    version = scalars.get("version", "1")
    return {
        "path": path.as_posix(),
        "version": version,
        "gateEnabled": gate_enabled,
        "blockedStatuses": [item for item in blocked_statuses if non_empty_string(item)],
        "raw": {"scalars": scalars},
    }


def source_value(source: Any) -> tuple[str, str]:
    if not isinstance(source, dict):
        return "", ""
    path = source.get("path")
    reason = source.get("reason")
    return (
        str(path).strip() if non_empty_string(path) else "",
        str(reason).strip() if non_empty_string(reason) else "",
    )


def is_placeholder_source(path: str) -> bool:
    lowered = path.lower()
    return any(token in lowered for token in ("replace-with", "example", "placeholder", "todo"))


def _acceptance_item_is_broad(item: str) -> bool:
    lowered = item.lower()
    vague_phrases = (
        "done",
        "implemented",
        "works",
        "properly",
        "correctly",
        "as needed",
        "if needed",
        "if necessary",
        "appropriate",
        "updated",
        "documented",
        "reviewed",
        "fixed",
        "improved",
        "requirements",
        "etc",
    )
    if any(phrase in lowered for phrase in vague_phrases):
        return True
    words = [word for word in item.replace("/", " ").replace("-", " ").split() if word]
    return len(words) <= 5


def scope_signal(contract: dict[str, Any]) -> dict[str, Any]:
    scope = contract.get("scope")
    out_of_scope = string_list(contract.get("outOfScope"))
    if not isinstance(scope, list):
        return {"name": "Scope", "value": "Missing", "evidence": ["contract.scope is missing"], "sources": ["contract.scope"]}

    scope_paths = string_list(scope)
    if len(scope_paths) != len(scope):
        return {"name": "Scope", "value": "Inconsistent", "evidence": ["contract.scope contains a non-string or empty entry"], "sources": ["contract.scope"]}
    if not scope_paths:
        return {"name": "Scope", "value": "Missing", "evidence": ["contract.scope is empty"], "sources": ["contract.scope"]}

    overlap = sorted(set(scope_paths).intersection(out_of_scope))
    if overlap:
        return {
            "name": "Scope",
            "value": "Inconsistent",
            "evidence": [f"scope overlaps outOfScope: {', '.join(overlap)}"],
            "sources": ["contract.scope", "contract.outOfScope"],
        }

    broad_patterns = [item for item in scope_paths if any(token in item for token in ("*", "?"))]
    if broad_patterns:
        return {
            "name": "Scope",
            "value": "Broad",
            "evidence": [f"scope contains broad pattern(s): {', '.join(broad_patterns)}"],
            "sources": ["contract.scope"],
        }

    return {
        "name": "Scope",
        "value": "Ready",
        "evidence": [f"scope declares {len(scope_paths)} path(s)"],
        "sources": ["contract.scope"],
    }


def out_of_scope_signal(contract: dict[str, Any]) -> dict[str, Any]:
    out_of_scope = contract.get("outOfScope")
    if not isinstance(out_of_scope, list):
        return {
            "name": "Out Of Scope",
            "value": "Missing",
            "evidence": ["contract.outOfScope is missing"],
            "sources": ["contract.outOfScope"],
        }
    entries = string_list(out_of_scope)
    if len(entries) != len(out_of_scope):
        return {
            "name": "Out Of Scope",
            "value": "Inconsistent",
            "evidence": ["contract.outOfScope contains a non-string or empty entry"],
            "sources": ["contract.outOfScope"],
        }
    if not entries:
        return {
            "name": "Out Of Scope",
            "value": "Not Applicable",
            "evidence": ["no exclusions were declared"],
            "sources": ["contract.outOfScope"],
        }
    return {
        "name": "Out Of Scope",
        "value": "Ready",
        "evidence": [f"outOfScope declares {len(entries)} exclusion(s)"],
        "sources": ["contract.outOfScope"],
    }


def intent_signal(contract: dict[str, Any]) -> dict[str, Any]:
    intent = contract.get("intent")
    if not isinstance(intent, dict):
        return {"name": "Intent", "value": "Missing", "evidence": ["contract.intent is missing"], "sources": ["contract.intent"]}

    problems: list[str] = []
    present: list[str] = []

    problem = intent.get("problem")
    if non_empty_string(problem):
        present.append("problem")
    elif problem is not None:
        problems.append("contract.intent.problem is empty")

    constraints = intent.get("constraints")
    if isinstance(constraints, list):
        constraint_values = string_list(constraints)
        if constraint_values:
            present.append("constraints")
        elif constraints:
            problems.append("contract.intent.constraints contains an empty entry")
    elif constraints is not None:
        problems.append("contract.intent.constraints must be a list")

    rationale = intent.get("rationale")
    if non_empty_string(rationale):
        present.append("rationale")
    elif rationale is not None:
        problems.append("contract.intent.rationale is empty")

    if problems:
        return {"name": "Intent", "value": "Inconsistent", "evidence": problems, "sources": ["contract.intent"]}
    if not present:
        return {"name": "Intent", "value": "Missing", "evidence": ["contract.intent has no meaningful content"], "sources": ["contract.intent"]}
    if len(present) == 3:
        return {
            "name": "Intent",
            "value": "Ready",
            "evidence": ["problem, constraints, and rationale are all present"],
            "sources": ["contract.intent.problem", "contract.intent.constraints", "contract.intent.rationale"],
        }
    return {
        "name": "Intent",
        "value": "Partial",
        "evidence": [f"intent has {len(present)} of 3 required evidence element(s): {', '.join(present)}"],
        "sources": ["contract.intent.problem", "contract.intent.constraints", "contract.intent.rationale"],
    }


def unknowns_signal(contract: dict[str, Any]) -> dict[str, Any]:
    risk = contract.get("riskAssessment") if isinstance(contract.get("riskAssessment"), dict) else {}
    level = risk.get("level")
    unknowns = contract.get("unknowns")
    if not isinstance(unknowns, list):
        return {
            "name": "Unknowns",
            "value": "Missing",
            "evidence": ["contract.unknowns is missing"],
            "sources": ["contract.unknowns", "contract.riskAssessment"],
        }
    values = string_list(unknowns)
    if len(values) != len(unknowns):
        return {
            "name": "Unknowns",
            "value": "Inconsistent",
            "evidence": ["contract.unknowns contains a non-string or empty entry"],
            "sources": ["contract.unknowns", "contract.riskAssessment"],
        }
    if not values:
        if level in {"medium", "high"}:
            return {
                "name": "Unknowns",
                "value": "Suspiciously Empty",
                "evidence": [f"riskAssessment.level is {level} but unknowns is empty"],
                "sources": ["contract.unknowns", "contract.riskAssessment"],
            }
        return {
            "name": "Unknowns",
            "value": "Ready",
            "evidence": ["no unknowns are declared for a low-risk Work Item"],
            "sources": ["contract.unknowns", "contract.riskAssessment"],
        }
    return {
        "name": "Unknowns",
        "value": "Partial",
        "evidence": [f"{len(values)} unknown(s) remain open"],
        "sources": ["contract.unknowns", "contract.riskAssessment"],
    }


def acceptance_signal(contract: dict[str, Any]) -> dict[str, Any]:
    acceptance = contract.get("acceptance")
    if not isinstance(acceptance, list):
        return {
            "name": "Acceptance",
            "value": "Missing",
            "evidence": ["contract.acceptance is missing"],
            "sources": ["contract.acceptance"],
        }
    values = string_list(acceptance)
    if len(values) != len(acceptance):
        return {
            "name": "Acceptance",
            "value": "Inconsistent",
            "evidence": ["contract.acceptance contains a non-string or empty entry"],
            "sources": ["contract.acceptance"],
        }
    if not values:
        return {
            "name": "Acceptance",
            "value": "Missing",
            "evidence": ["contract.acceptance is empty"],
            "sources": ["contract.acceptance"],
        }

    broad_items = [item for item in values if _acceptance_item_is_broad(item)]
    if len(broad_items) == len(values):
        return {
            "name": "Acceptance",
            "value": "Broad",
            "evidence": [f"acceptance is too broad: {', '.join(broad_items[:3])}"],
            "sources": ["contract.acceptance"],
        }
    if broad_items:
        return {
            "name": "Acceptance",
            "value": "Partial",
            "evidence": [f"{len(broad_items)} acceptance item(s) are broad or underspecified"],
            "sources": ["contract.acceptance"],
        }
    return {
        "name": "Acceptance",
        "value": "Ready",
        "evidence": [f"acceptance declares {len(values)} concrete item(s)"],
        "sources": ["contract.acceptance"],
    }


def sources_signal(contract: dict[str, Any]) -> dict[str, Any]:
    sources = contract.get("sources")
    if not isinstance(sources, list):
        return {
            "name": "Sources",
            "value": "Missing",
            "evidence": ["contract.sources is missing"],
            "sources": ["contract.sources"],
        }
    if not sources:
        return {
            "name": "Sources",
            "value": "Missing",
            "evidence": ["contract.sources is empty"],
            "sources": ["contract.sources"],
        }

    valid: list[tuple[str, str]] = []
    problems: list[str] = []
    for index, source in enumerate(sources):
        path, reason = source_value(source)
        if not path or not reason:
            problems.append(f"contract.sources[{index}] must include path and reason")
            continue
        valid.append((path, reason))

    if problems:
        return {"name": "Sources", "value": "Inconsistent", "evidence": problems, "sources": ["contract.sources"]}

    if any(is_placeholder_source(path) for path, _ in valid):
        return {
            "name": "Sources",
            "value": "Weak",
            "evidence": ["source evidence includes placeholder-style paths"],
            "sources": ["contract.sources"],
        }

    if len(valid) == 1:
        return {
            "name": "Sources",
            "value": "Weak",
            "evidence": ["only one source of evidence is declared"],
            "sources": ["contract.sources"],
        }

    internal_only = all(path.startswith(".ai/") or path.startswith("target/") for path, _ in valid)
    if internal_only:
        return {
            "name": "Sources",
            "value": "Weak",
            "evidence": ["sources only reference internal governance artifacts"],
            "sources": ["contract.sources"],
        }

    return {
        "name": "Sources",
        "value": "Ready",
        "evidence": [f"{len(valid)} source(s) of evidence are declared"],
        "sources": ["contract.sources"],
    }


def verification_signal(contract: dict[str, Any]) -> dict[str, Any]:
    verification = contract.get("verification")
    if not isinstance(verification, list):
        return {
            "name": "Verification",
            "value": "Missing",
            "evidence": ["contract.verification is missing"],
            "sources": ["contract.verification"],
        }
    if not verification:
        return {
            "name": "Verification",
            "value": "Missing",
            "evidence": ["contract.verification is empty"],
            "sources": ["contract.verification"],
        }

    required: list[str] = []
    problems: list[str] = []
    for index, item in enumerate(verification):
        if not isinstance(item, dict):
            problems.append(f"contract.verification[{index}] must be an object")
            continue
        command = item.get("command")
        if not non_empty_string(command):
            problems.append(f"contract.verification[{index}] requires a command")
            continue
        if item.get("required") is True:
            required.append(str(command).strip())

    if problems:
        return {
            "name": "Verification",
            "value": "Inconsistent",
            "evidence": problems,
            "sources": ["contract.verification"],
        }
    if not required:
        return {
            "name": "Verification",
            "value": "Broad",
            "evidence": ["verification does not declare any required checks"],
            "sources": ["contract.verification"],
        }
    return {
        "name": "Verification",
        "value": "Ready",
        "evidence": [f"verification declares {len(required)} required check(s)"],
        "sources": ["contract.verification"],
    }


def scenario_coverage_signal(contract: dict[str, Any]) -> dict[str, Any]:
    risk = contract.get("riskAssessment")
    level = risk.get("level") if isinstance(risk, dict) else "unknown"
    coverage = contract.get("scenarioCoverage")
    if not isinstance(coverage, list):
        if level == "low":
            return {
                "name": "Scenario Coverage",
                "value": "Not Applicable",
                "evidence": ["scenario coverage is not required for a low-risk Work Item"],
                "sources": ["contract.scenarioCoverage", "contract.riskAssessment"],
            }
        return {
            "name": "Scenario Coverage",
            "value": "Missing",
            "evidence": [f"scenario coverage is missing for {level or 'unknown'} risk"],
            "sources": ["contract.scenarioCoverage", "contract.riskAssessment"],
        }

    required_items = [item for item in coverage if isinstance(item, dict) and item.get("required") is True]
    if not required_items:
        if level == "low":
            return {
                "name": "Scenario Coverage",
                "value": "Not Applicable",
                "evidence": ["scenario coverage is optional for low-risk Work Items without required scenarios"],
                "sources": ["contract.scenarioCoverage", "contract.riskAssessment"],
            }
        return {
            "name": "Scenario Coverage",
            "value": "Missing",
            "evidence": ["no required scenario coverage is declared"],
            "sources": ["contract.scenarioCoverage", "contract.riskAssessment"],
        }

    statuses = {str(item.get("status")) for item in required_items}
    if "unverified" in statuses:
        return {
            "name": "Scenario Coverage",
            "value": "Partial",
            "evidence": [f"{len([item for item in required_items if item.get('status') == 'unverified'])} required scenario(s) remain unverified"],
            "sources": ["contract.scenarioCoverage", "contract.riskAssessment"],
        }
    if statuses <= {"verified", "not_applicable"}:
        return {
            "name": "Scenario Coverage",
            "value": "Ready",
            "evidence": [f"{len(required_items)} required scenario(s) are verified or not_applicable"],
            "sources": ["contract.scenarioCoverage", "contract.riskAssessment"],
        }
    return {
        "name": "Scenario Coverage",
        "value": "Inconsistent",
        "evidence": ["scenario coverage contains unsupported required statuses"],
        "sources": ["contract.scenarioCoverage", "contract.riskAssessment"],
    }


def not_codable_signal(contract: dict[str, Any]) -> dict[str, Any]:
    value = contract.get("notCodable")
    if value is False:
        return {
            "name": "Not Codable",
            "value": "Ready",
            "evidence": ["notCodable is false"],
            "sources": ["contract.notCodable"],
        }
    if value is True:
        return {
            "name": "Not Codable",
            "value": "Inconsistent",
            "evidence": ["notCodable is true"],
            "sources": ["contract.notCodable"],
        }
    return {
        "name": "Not Codable",
        "value": "Missing",
        "evidence": ["contract.notCodable is missing"],
        "sources": ["contract.notCodable"],
    }


def agent_capability_signal(contract: dict[str, Any]) -> dict[str, Any]:
    value = contract.get("agentCapability")
    if not isinstance(value, dict):
        return {
            "name": "Agent Capability",
            "value": "Missing",
            "evidence": ["contract.agentCapability is missing"],
            "sources": ["contract.agentCapability"],
        }
    can_implement = value.get("canImplement")
    can_verify = value.get("canVerify")
    needs_human = value.get("needsHumanDecision")
    if not isinstance(can_implement, bool) or not isinstance(can_verify, bool) or not isinstance(needs_human, bool):
        return {
            "name": "Agent Capability",
            "value": "Inconsistent",
            "evidence": ["contract.agentCapability must declare boolean canImplement, canVerify, needsHumanDecision"],
            "sources": ["contract.agentCapability"],
        }
    if can_implement and can_verify and not needs_human:
        return {
            "name": "Agent Capability",
            "value": "Ready",
            "evidence": ["agent can implement and verify without human decision"],
            "sources": ["contract.agentCapability.canImplement", "contract.agentCapability.canVerify", "contract.agentCapability.needsHumanDecision"],
        }
    return {
        "name": "Agent Capability",
        "value": "Inconsistent",
        "evidence": [
            f"canImplement={can_implement}",
            f"canVerify={can_verify}",
            f"needsHumanDecision={needs_human}",
        ],
        "sources": ["contract.agentCapability.canImplement", "contract.agentCapability.canVerify", "contract.agentCapability.needsHumanDecision"],
    }


def human_review_signal(contract: dict[str, Any]) -> dict[str, Any]:
    value = contract.get("humanReview")
    if value is None:
        return {
            "name": "Human Review",
            "value": "Not Applicable",
            "evidence": ["no human review override is recorded"],
            "sources": ["contract.humanReview"],
        }
    if not isinstance(value, dict):
        return {
            "name": "Human Review",
            "value": "Inconsistent",
            "evidence": ["contract.humanReview must be an object"],
            "sources": ["contract.humanReview"],
        }
    status = value.get("status")
    if status != "confirmed":
        return {
            "name": "Human Review",
            "value": "Partial",
            "evidence": [f"humanReview.status is {status!r}, expected 'confirmed'"],
            "sources": ["contract.humanReview.status"],
        }
    decision = value.get("decision")
    open_questions = string_list(value.get("openQuestions"))
    if not non_empty_string(decision) or not open_questions:
        return {
            "name": "Human Review",
            "value": "Inconsistent",
            "evidence": ["confirmed human review requires decision and openQuestions"],
            "sources": ["contract.humanReview"],
        }
    return {
        "name": "Human Review",
        "value": "Ready",
        "evidence": [
            f"human review confirmed: {decision.strip()}",
            f"open questions recorded: {len(open_questions)}",
        ],
        "sources": [
            "contract.humanReview.status",
            "contract.humanReview.decision",
            "contract.humanReview.openQuestions",
        ],
    }


def execution_decision_signal(contract: dict[str, Any]) -> dict[str, Any]:
    value = contract.get("executionDecision")
    if not isinstance(value, dict):
        return {
            "name": "Execution Decision",
            "value": "Missing",
            "evidence": ["contract.executionDecision is missing"],
            "sources": ["contract.executionDecision"],
        }
    status = value.get("status")
    reason = value.get("reason")
    if status == "continue":
        return {
            "name": "Execution Decision",
            "value": "Ready",
            "evidence": ["executionDecision.status is continue"],
            "sources": ["contract.executionDecision.status"],
        }
    if non_empty_string(status) and non_empty_string(reason):
        return {
            "name": "Execution Decision",
            "value": "Inconsistent",
            "evidence": [f"executionDecision.status is {status}"],
            "sources": ["contract.executionDecision.status", "contract.executionDecision.reason"],
        }
    return {
        "name": "Execution Decision",
        "value": "Missing",
        "evidence": ["contract.executionDecision is incomplete"],
        "sources": ["contract.executionDecision"],
    }


def risk_context(contract: dict[str, Any]) -> dict[str, Any]:
    risk = contract.get("riskAssessment")
    if not isinstance(risk, dict):
        return {
            "value": "unknown",
            "evidence": ["riskAssessment is missing"],
            "sources": ["contract.riskAssessment"],
        }
    level = risk.get("level") if risk.get("level") in {"low", "medium", "high"} else "unknown"
    risk_types = string_list(risk.get("riskTypes"))
    evidence = [
        f"riskAssessment.level is {level}",
        f"riskAssessment.riskTypes count: {len(risk_types)}",
    ]
    reason = risk.get("reason")
    if non_empty_string(reason):
        evidence.append(str(reason).strip())
    return {"value": level, "evidence": evidence, "sources": ["contract.riskAssessment"]}


def overall_status(signals: list[dict[str, Any]]) -> str:
    values = {str(signal.get("value")) for signal in signals}
    if "Inconsistent" in values or "Missing" in values:
        return "not_ready"
    if any(signal.get("name") == "Human Review" and signal.get("value") == "Ready" for signal in signals):
        return "ready"
    if values.intersection({"Partial", "Weak", "Broad", "Suspiciously Empty"}):
        return "needs_human_confirmation"
    return "ready"


def recommendation_for(status: str, signals: list[dict[str, Any]]) -> str:
    if status == "ready":
        return "Implementation may begin once the reviewer confirms the evidence is sufficient."
    if status == "not_ready":
        return "Resolve contradictory or missing contract evidence before implementation."

    priority = {signal["name"]: signal["value"] for signal in signals}
    if priority.get("Intent") in {"Missing", "Partial", "Inconsistent"}:
        return "Clarify intent before implementation."
    if priority.get("Unknowns") == "Suspiciously Empty":
        return "Document the open questions that are currently implicit in the risk assessment."
    if priority.get("Sources") in {"Missing", "Weak"}:
        return "Add stronger sources before implementation."
    if priority.get("Acceptance") in {"Missing", "Broad"}:
        return "Tighten the acceptance criteria before implementation."
    if priority.get("Not Codable") in {"Missing", "Inconsistent"}:
        return "Set notCodable to false before implementation."
    if priority.get("Agent Capability") in {"Missing", "Inconsistent"}:
        return "Confirm implementation and verification capability before implementation."
    if priority.get("Execution Decision") in {"Missing", "Inconsistent"}:
        return "Resolve the execution decision before implementation."
    if priority.get("Verification") in {"Missing", "Broad"}:
        return "Register concrete required checks before implementation."
    if priority.get("Scope") in {"Missing", "Broad"}:
        return "Narrow the implementation scope before implementation."
    if priority.get("Scenario Coverage") == "Partial":
        return "Clarify required scenarios before implementation."
    return "Clarify the remaining evidence before implementation."


def decision_drivers(signals: list[dict[str, Any]], context: dict[str, Any]) -> list[str]:
    drivers: list[str] = []
    for signal in signals:
        if signal["value"] not in {"Ready", "Not Applicable"}:
            drivers.extend([f"{signal['name']}: {item}" for item in signal["evidence"]])
    risk = context["risk"]
    drivers.append(risk["evidence"][0])
    return drivers


def pause_rule(report: dict[str, Any]) -> str:
    gate = report.get("gate") if isinstance(report.get("gate"), dict) else {}
    enabled = gate.get("enabled") is True
    if enabled:
        return "Policy gate is enabled: pause implementation when the review is needs_human_confirmation or not_ready."
    return "Policy gate is advisory by default: report the review and pause implementation in the agent workflow when the review is needs_human_confirmation or not_ready."


def derive_report(contract: dict[str, Any], *, contract_path: Path, policy_path: Path) -> dict[str, Any]:
    signals = [
        scope_signal(contract),
        out_of_scope_signal(contract),
        intent_signal(contract),
        unknowns_signal(contract),
        acceptance_signal(contract),
        sources_signal(contract),
        scenario_coverage_signal(contract),
        not_codable_signal(contract),
        agent_capability_signal(contract),
        human_review_signal(contract),
        execution_decision_signal(contract),
        verification_signal(contract),
    ]
    context = {"risk": risk_context(contract)}
    status = overall_status(signals)
    policy = load_policy(policy_path)
    report = {
        "generatedAt": datetime.now(timezone.utc).isoformat(),
        "workItemId": contract.get("workItemId", ""),
        "contractPath": contract_path.as_posix(),
        "contractHash": contract_hash(contract_path),
        "policyPath": policy["path"],
        "policyHash": policy_hash(policy_path),
        "policyVersion": policy["version"],
        "gate": {
            "enabled": policy["gateEnabled"],
            "blockedStatuses": policy["blockedStatuses"],
        },
        "status": status,
        "signals": signals,
        "context": context,
        "decisionDrivers": decision_drivers(signals, context),
        "recommendation": recommendation_for(status, signals),
    }
    report["pauseRule"] = pause_rule(report)
    return report


def validate_report_structure(report: dict[str, Any]) -> list[str]:
    issues: list[str] = []
    for field in (
        "generatedAt",
        "workItemId",
        "contractPath",
        "contractHash",
        "policyPath",
        "policyHash",
        "policyVersion",
        "gate",
        "status",
        "signals",
        "context",
        "decisionDrivers",
        "recommendation",
        "pauseRule",
    ):
        if field not in report:
            issues.append(f"missing field: {field}")
    if report.get("status") not in ALLOWED_STATUSES:
        issues.append(f"status must be one of {sorted(ALLOWED_STATUSES)}")
    gate = report.get("gate")
    if not isinstance(gate, dict):
        issues.append("gate must be an object")
    else:
        if not isinstance(gate.get("enabled"), bool):
            issues.append("gate.enabled must be boolean")
        blocked = gate.get("blockedStatuses")
        if not isinstance(blocked, list):
            issues.append("gate.blockedStatuses must be a list")
        elif any(item not in ALLOWED_STATUSES for item in blocked):
            issues.append(f"gate.blockedStatuses must only contain {sorted(ALLOWED_STATUSES)}")
    signals = report.get("signals")
    allowed_signal_names = {
        "Scope",
        "Out Of Scope",
        "Intent",
        "Unknowns",
        "Acceptance",
        "Sources",
        "Scenario Coverage",
        "Not Codable",
        "Agent Capability",
        "Human Review",
        "Execution Decision",
        "Verification",
    }
    if not isinstance(signals, list) or not signals:
        issues.append("signals must be a non-empty list")
    else:
        for index, signal in enumerate(signals):
            if not isinstance(signal, dict):
                issues.append(f"signals[{index}] must be an object")
                continue
            if signal.get("name") not in allowed_signal_names:
                issues.append(f"signals[{index}].name is invalid")
            if signal.get("value") not in ALLOWED_SIGNAL_VALUES:
                issues.append(f"signals[{index}].value must be one of {sorted(ALLOWED_SIGNAL_VALUES)}")
            if not isinstance(signal.get("evidence"), list) or not all(non_empty_string(item) for item in signal.get("evidence", [])):
                issues.append(f"signals[{index}].evidence must be a list of non-empty strings")
            if not isinstance(signal.get("sources"), list) or not all(non_empty_string(item) for item in signal.get("sources", [])):
                issues.append(f"signals[{index}].sources must be a list of non-empty strings")
    context = report.get("context")
    if not isinstance(context, dict):
        issues.append("context must be an object")
    else:
        risk = context.get("risk")
        if not isinstance(risk, dict):
            issues.append("context.risk must be an object")
        else:
            if risk.get("value") not in {"low", "medium", "high", "unknown"}:
                issues.append("context.risk.value is invalid")
            if not isinstance(risk.get("evidence"), list) or not all(non_empty_string(item) for item in risk.get("evidence", [])):
                issues.append("context.risk.evidence must be a list of non-empty strings")
            if not isinstance(risk.get("sources"), list) or not all(non_empty_string(item) for item in risk.get("sources", [])):
                issues.append("context.risk.sources must be a list of non-empty strings")
    if not isinstance(report.get("decisionDrivers"), list):
        issues.append("decisionDrivers must be a list")
    if not non_empty_string(report.get("recommendation")):
        issues.append("recommendation must be a non-empty string")
    if not non_empty_string(report.get("pauseRule")):
        issues.append("pauseRule must be a non-empty string")
    return issues


def policy_issues(report: dict[str, Any], policy: dict[str, Any]) -> list[str]:
    issues: list[str] = []
    gate_value = report.get("gate")
    gate: dict[str, Any] = gate_value if isinstance(gate_value, dict) else {}
    if gate.get("enabled") is True and not gate.get("blockedStatuses"):
        issues.append("gate.enabled is true but blockedStatuses is empty")
    if policy["gateEnabled"] != gate.get("enabled"):
        issues.append("report gate.enabled does not match policy")
    if policy["blockedStatuses"] != gate.get("blockedStatuses"):
        issues.append("report gate.blockedStatuses does not match policy")
    if report.get("policyHash") != policy_hash(Path(policy["path"])):
        issues.append("policyHash does not match the configured policy")
    return issues


def report_is_blocked(report: dict[str, Any], policy: dict[str, Any]) -> bool:
    if not policy["gateEnabled"]:
        return False
    return report.get("status") in set(policy["blockedStatuses"])


def resolve_contract_path(explicit: str | None) -> Path | None:
    if explicit:
        return Path(explicit)
    active_dir = PROJECT_ROOT / ".ai" / "work-items" / "active"
    contracts = sorted(active_dir.glob("*.contract.json"))
    if len(contracts) == 1:
        return contracts[0]
    return None


def render_markdown(report: dict[str, Any]) -> str:
    lines = [
        "# Preflight Review",
        "",
    ]
    if report.get("status") != "ready":
        lines.extend(
            [
                "Preflight Review requires attention before implementation.",
                "",
                f"Status: `{report['status']}`",
                "",
                f"Recommendation: {report['recommendation']}",
                "",
                "Advisory mode:",
                "The command does not block by default.",
                "The agent must report this review to the user before coding continues.",
                "",
            ]
        )
    lines.extend(
        [
            "Status:",
            f"`{report['status']}`",
            "",
            "Recommendation:",
            report["recommendation"],
            "",
            "Decision Drivers:",
            "",
        ]
    )
    if report["decisionDrivers"]:
        lines.extend(f"- {item}" for item in report["decisionDrivers"])
    else:
        lines.append("- none")
    lines.extend(
        [
            "",
            "Pause Rule:",
            "",
            report["pauseRule"],
            "",
        ]
    )
    return "\n".join(lines) + "\n"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Generate or validate a Preflight Review.")
    parser.add_argument("--contract")
    parser.add_argument("--output", default=str(DEFAULT_OUTPUT))
    parser.add_argument("--policy", default=str(DEFAULT_POLICY))
    parser.add_argument("--check", action="store_true", help="Validate an existing report instead of generating one.")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    contract_path = resolve_contract_path(args.contract)
    if contract_path is None:
        print("Skipping preflight review (no active contract provided)")
        return 0
    if not contract_path.exists():
        print(f"Failed to load preflight review contract: {contract_path}", file=sys.stderr)
        return 1

    output_path = Path(args.output)
    policy_path = Path(args.policy)
    try:
        contract = load_json_file(contract_path)
        policy = load_policy(policy_path)
    except (OSError, json.JSONDecodeError, ValueError) as exc:
        print(f"Failed to load preflight review inputs: {exc}", file=sys.stderr)
        return 1

    if args.check:
        if not output_path.exists():
            print(f"Preflight review report is missing: {output_path}", file=sys.stderr)
            return 1
        try:
            report = load_json_file(output_path)
        except (OSError, json.JSONDecodeError, ValueError) as exc:
            print(f"Failed to read preflight review report: {exc}", file=sys.stderr)
            return 1
        issues = validate_report_structure(report)
        issues.extend(policy_issues(report, policy))
        if report.get("contractHash") != contract_hash(contract_path):
            issues.append("contractHash does not match the active Contract")
        if report.get("workItemId") != contract.get("workItemId"):
            issues.append("workItemId does not match the active Contract")
        if report_is_blocked(report, policy):
            issues.append(f"preflight gate blocked status: {report.get('status')}")
        if issues:
            for issue in issues:
                print(f"[ERROR] {issue}", file=sys.stderr)
            return 1
        print(f"preflight review check passed: {output_path}")
        return 0

    report = derive_report(contract, contract_path=contract_path, policy_path=policy_path)
    issues = validate_report_structure(report)
    issues.extend(policy_issues(report, policy))
    if issues:
        for issue in issues:
            print(f"[ERROR] {issue}", file=sys.stderr)
        return 1

    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(render_markdown(report), end="")
    print(f"preflight review generated: {output_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
