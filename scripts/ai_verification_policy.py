"""Pure, deterministic verification selection, caching, and escalation policies."""

from __future__ import annotations

import difflib
import fnmatch
import hashlib
import json
import re
from typing import Any

from ai_impact_classifier import classify_path

POLICY_LEVELS = ("light", "standard", "strict")
VERIFICATION_SCOPES = ("focused", "full")
ESCALATION_DOMAINS = frozenset(
    {"release", "workflow", "trust", "installer", "dependency", "unknown"}
)
DOMAIN_LEVELS = {
    "docs": "light",
    "project_code": "standard",
    "tests": "standard",
    "unknown": "standard",
    "dependency": "strict",
    "workflow": "strict",
    "trust": "strict",
    "installer": "strict",
    "lifecycle": "strict",
    "release": "strict",
}

STRICT_TARGETED_PATTERNS = (
    ".ai/quality/**",
    "scripts/ai_*.py",
    "templates/**",
    "Makefile",
    "Makefile.ai",
    "AGENTS.md",
    "GEMINI.md",
    "CLAUDE.md",
)
STRICT_FULL_PATTERNS = (
    ".ai/guards/**",
    ".ai/policies/**",
    ".ai/work-items/**",
    ".github/workflows/**",
    ".cursor/**",
    "scripts/install*.py",
    "scripts/uninstall*.py",
    "install.sh",
    "uninstall.sh",
    "requirements*",
    "pyproject.toml",
    "poetry.lock",
    "package.json",
    "package-lock.json",
    "pnpm-lock.yaml",
    "yarn.lock",
    "Cargo.toml",
    "Cargo.lock",
    "go.mod",
    "go.sum",
    "migrations/**",
    "db/migrations/**",
    "release*",
    "dist/**",
    "signing/**",
)
PROJECT_TEST_PATTERNS = (
    "scripts/**",
    "templates/**",
    "Makefile",
    "Makefile.ai",
    "tests/**",
    "test/**",
    "src/**",
    "lib/**",
    "app/**",
)
PROJECT_CONSISTENCY_PATTERNS = (
    ".ai/quality/**",
    "scripts/**",
    "templates/**",
    "Makefile",
    "Makefile.ai",
    "AGENTS.md",
    "GEMINI.md",
    "CLAUDE.md",
)

_IMMUTABLE_USES_LINE = re.compile(
    r"^(?P<prefix>\s*-\s*uses:\s+)(?P<action>[^\s@]+)@"
    r"(?P<sha>[0-9a-fA-F]{40})(?P<suffix>\s*(?:#.*)?)$"
)
_RELEASE_OR_SIGNING_WORKFLOW = re.compile(
    r"(?:^|[-_.])(release|publish|sign(?:ing)?|provenance|sbom)(?:[-_.]|$)",
    re.IGNORECASE,
)


def classify_immutable_workflow_pin_change(path: str, before: str, after: str) -> dict[str, Any]:
    """Prove whether a workflow diff changes exactly one immutable action SHA.

    The classifier deliberately accepts only a one-line replacement in a
    non-release workflow.  It returns facts suitable for an audit receipt and
    never returns file contents.
    """
    normalized = path.replace("\\", "/").removeprefix("./")
    facts: dict[str, Any] = {
        "path": normalized,
        "kind": "immutable_workflow_pin",
        "eligible": False,
        "reason": "not evaluated",
        "replacementCount": 0,
    }
    workflow_name = normalized.removeprefix(".github/workflows/")
    if not (
        normalized.startswith(".github/workflows/")
        and workflow_name
        and "/" not in workflow_name
        and workflow_name.endswith((".yml", ".yaml"))
    ):
        facts["reason"] = "path is not a single GitHub workflow file"
        return facts
    if _RELEASE_OR_SIGNING_WORKFLOW.search(workflow_name):
        facts["reason"] = "release or signing workflow remains on full quality"
        return facts

    before_lines = before.splitlines()
    after_lines = after.splitlines()
    opcodes = difflib.SequenceMatcher(a=before_lines, b=after_lines, autojunk=False).get_opcodes()
    changed = [opcode for opcode in opcodes if opcode[0] != "equal"]
    if len(changed) != 1 or changed[0][0] != "replace":
        facts["reason"] = "diff is not exactly one line replacement"
        facts["replacementCount"] = len(changed)
        return facts
    _, before_start, before_end, after_start, after_end = changed[0]
    if before_end - before_start != 1 or after_end - after_start != 1:
        facts["reason"] = "replacement does not contain exactly one line on each side"
        facts["replacementCount"] = 1
        return facts

    before_match = _IMMUTABLE_USES_LINE.fullmatch(before_lines[before_start])
    after_match = _IMMUTABLE_USES_LINE.fullmatch(after_lines[after_start])
    if before_match is None or after_match is None:
        facts["reason"] = "replaced lines are not immutable uses SHA pins"
        facts["replacementCount"] = 1
        return facts
    if before_match.group("sha") == after_match.group("sha"):
        facts["reason"] = "action SHA did not change"
        facts["replacementCount"] = 1
        return facts
    if any(
        before_match.group(name) != after_match.group(name)
        for name in ("prefix", "action", "suffix")
    ):
        facts["reason"] = "workflow uses prefix, action identity, or suffix changed"
        facts["replacementCount"] = 1
        return facts

    facts.update(
        {
            "eligible": True,
            "reason": "exactly one immutable action SHA replacement",
            "replacementCount": 1,
        }
    )
    return facts


def strict_quality_routing(
    changed_paths: list[str],
    *,
    release: bool = False,
    explicit_strict: bool = False,
    immutable_pin_facts: dict[str, Any] | None = None,
) -> dict[str, Any]:
    """Choose a strict target without lowering high-risk or explicit strict work."""
    normalized = sorted(path.replace("\\", "/").removeprefix("./") for path in changed_paths)
    if release:
        return {
            "target": "quality-full",
            "requiredGroups": ["quality-full"],
            "reason": "release escalation requires the complete quality graph",
        }
    if explicit_strict:
        return {
            "target": "quality-full",
            "requiredGroups": ["quality-full"],
            "reason": "explicit strict governance requires the complete quality graph",
        }
    if (
        len(normalized) == 1
        and isinstance(immutable_pin_facts, dict)
        and immutable_pin_facts.get("eligible") is True
        and immutable_pin_facts.get("path") == normalized[0]
    ):
        return {
            "target": "quality-strict-targeted",
            "requiredGroups": ["quality-fast"],
            "reason": "evidence-bound immutable workflow pin change uses targeted strict quality",
            "immutablePinChange": immutable_pin_facts,
        }
    high_risk = [
        path
        for path in normalized
        if any(fnmatch.fnmatchcase(path, pattern) for pattern in STRICT_FULL_PATTERNS)
    ]
    if high_risk:
        return {
            "target": "quality-full",
            "requiredGroups": ["quality-full"],
            "reason": f"high-risk strict paths require full quality: {', '.join(high_risk)}",
        }
    unclassified = [
        path
        for path in normalized
        if not any(fnmatch.fnmatchcase(path, pattern) for pattern in STRICT_TARGETED_PATTERNS)
    ]
    if unclassified:
        return {
            "target": "quality-full",
            "requiredGroups": ["quality-full"],
            "reason": f"strict paths without a targeted routing rule require full quality: {', '.join(unclassified)}",
        }
    groups = ["quality-fast", "check-ai-reference-impact"]
    if any(
        any(fnmatch.fnmatchcase(path, pattern) for pattern in PROJECT_TEST_PATTERNS)
        for path in normalized
    ):
        groups.append("project-test")
    if any(
        any(fnmatch.fnmatchcase(path, pattern) for pattern in PROJECT_CONSISTENCY_PATTERNS)
        for path in normalized
    ):
        groups.append("quality-project-consistency-group")
    return {
        "target": "quality-strict-targeted",
        "requiredGroups": groups,
        "reason": "automatic strict governance uses only groups matched by changed-path domains",
    }


def finish_quality_route(
    changed_paths: list[str],
    *,
    requested: str | None = None,
    immutable_pin_facts: dict[str, Any] | None = None,
) -> dict[str, Any]:
    """Return the auditable Finish route without lowering a Contract profile."""

    policy = select_policy(
        "task",
        changed_paths,
        requested=requested,
        immutable_pin_facts=immutable_pin_facts,
    )
    return {
        "policy": policy,
        "command": f"make ai-cockpit-quality GOVERNANCE_PROFILE={policy['level']}",
    }


def finish_quality_route_for_contract(
    changed_paths: list[str],
    governance_profile: dict[str, Any] | None,
    *,
    immutable_pin_facts: dict[str, Any] | None = None,
) -> dict[str, Any]:
    """Route Finish from final scope without treating automatic defaults as overrides.

    An automatic profile is a prior classification, not a human instruction.  Finish
    must therefore reclassify it against the final Contract scope, while preserving
    a recorded higher automatic level.  A human override remains an explicit request
    and is validated fail-closed by ``select_policy``.
    """
    profile = governance_profile if isinstance(governance_profile, dict) else {}
    selected = profile.get("selected")
    source = profile.get("source")
    automatic_route = finish_quality_route(changed_paths, immutable_pin_facts=immutable_pin_facts)

    if source != "automatic":
        return finish_quality_route(
            changed_paths,
            requested=selected,
            immutable_pin_facts=immutable_pin_facts,
        )
    if selected not in POLICY_LEVELS:
        return automatic_route

    automatic_level = str(automatic_route["policy"]["level"])
    if POLICY_LEVELS.index(str(selected)) > POLICY_LEVELS.index(automatic_level):
        return finish_quality_route(
            changed_paths,
            requested=str(selected),
            immutable_pin_facts=immutable_pin_facts,
        )
    return automatic_route


def select_policy(
    stage: str,
    changed_paths: list[str],
    *,
    requested: str | None = None,
    immutable_pin_facts: dict[str, Any] | None = None,
) -> dict[str, Any]:
    """Select a policy without permitting a caller to downgrade risk."""
    if requested is not None and requested not in POLICY_LEVELS:
        raise ValueError(f"unsupported policy level: {requested}")
    domains = {classify_path(path) for path in changed_paths}
    levels = [DOMAIN_LEVELS.get(domain, "standard") for domain in domains]
    level = max(levels, key=POLICY_LEVELS.index) if levels else "standard"
    if any(
        any(
            fnmatch.fnmatchcase(path.replace("\\", "/"), pattern)
            for pattern in (*STRICT_TARGETED_PATTERNS, *STRICT_FULL_PATTERNS)
        )
        for path in changed_paths
    ):
        level = "strict"
    stage_floor = "strict" if stage == "release" else "standard" if stage == "pr" else "light"
    if POLICY_LEVELS.index(stage_floor) > POLICY_LEVELS.index(level):
        level = stage_floor
    if requested is not None:
        if POLICY_LEVELS.index(requested) < POLICY_LEVELS.index(level):
            raise ValueError(f"requested policy {requested} cannot lower selected policy {level}")
        level = requested
    scope = "focused" if level == "light" else "full"
    if level == "strict":
        quality_routing = strict_quality_routing(
            changed_paths,
            release=stage == "release",
            explicit_strict=requested == "strict",
            immutable_pin_facts=immutable_pin_facts,
        )
    else:
        target = "quality-fast" if level == "light" else "quality-standard"
        quality_routing = {
            "target": target,
            "requiredGroups": [target],
            "reason": f"{level} governance uses its profile target",
        }
    return {
        "level": level,
        "scope": scope,
        "stage": stage,
        "domains": sorted(domains),
        "qualityTarget": quality_routing["target"],
        "requiredGroups": quality_routing["requiredGroups"],
        "qualityRouting": quality_routing,
    }


def verification_cache_key(inputs: dict[str, Any]) -> str:
    """Return a content address over every input that can affect verification."""
    required = ("base", "diff", "command", "tool", "dependency", "environment", "config")
    missing = [name for name in required if name not in inputs]
    if missing:
        raise ValueError(f"cache key inputs missing: {', '.join(missing)}")
    canonical = json.dumps(inputs, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
    return hashlib.sha256(canonical.encode("utf-8")).hexdigest()


def order_checks(graph: dict[str, list[str]]) -> list[str]:
    """Topologically order a check DAG and reject unknown/cyclic dependencies."""
    nodes = set(graph)
    unknown = sorted({dependency for deps in graph.values() for dependency in deps} - nodes)
    if unknown:
        raise ValueError(f"unknown check dependencies: {', '.join(unknown)}")
    ordered: list[str] = []
    visiting: set[str] = set()
    visited: set[str] = set()

    def visit(node: str) -> None:
        if node in visiting:
            raise ValueError("verification check DAG contains a cycle")
        if node in visited:
            return
        visiting.add(node)
        for dependency in sorted(graph[node]):
            visit(dependency)
        visiting.remove(node)
        visited.add(node)
        ordered.append(node)

    for node in sorted(nodes):
        visit(node)
    return ordered


RECEIPT_BINDINGS = (
    "baseCommit",
    "headCommit",
    "changedPaths",
    "command",
    "environment",
    "toolchain",
    "policy",
)


def evaluate_impact_graph(
    graph: dict[str, Any], *, profile: str, receipt_bindings: dict[str, str]
) -> dict[str, Any]:
    """Describe a verification DAG without executing checks or scheduling work."""
    if profile not in (*POLICY_LEVELS, "release"):
        raise ValueError(f"unsupported graph profile: {profile}")
    raw_nodes = graph.get("nodes", {})
    nodes = raw_nodes if isinstance(raw_nodes, dict) else {}
    errors: list[str] = []
    required = sorted(name for name, node in nodes.items() if node.get("required") is True)
    final_proofs = [
        name
        for name, node in nodes.items()
        if node.get("required") is True and node.get("finalProof") is True
    ]
    if not final_proofs:
        errors.append("required final proof node is missing")
    dependencies = {name: list(node.get("dependsOn", [])) for name, node in nodes.items()}
    try:
        ordered = order_checks(dependencies)
    except ValueError as error:
        errors.append(str(error))
        ordered = []
    cached: list[str] = []
    invalidated: list[str] = []
    for name in sorted(nodes):
        expected = nodes[name].get("receiptBindings")
        if not isinstance(expected, dict):
            continue
        matches = all(expected.get(key) == receipt_bindings.get(key) for key in RECEIPT_BINDINGS)
        (cached if matches else invalidated).append(name)
    layers = {
        layer: sorted(name for name, node in nodes.items() if node.get("layer") == layer)
        for layer in ("Fast", "Finish", "Hosted")
    }
    parallelizable = sorted(name for name, dependencies in dependencies.items() if not dependencies)
    return {
        "valid": not errors,
        "errors": errors,
        "profile": profile,
        "requiredNodes": required,
        "orderedNodes": ordered,
        "dependencies": dependencies,
        "parallelizableGroups": [parallelizable] if parallelizable else [],
        "cachedNodes": cached,
        "invalidatedNodes": invalidated,
        "proofLayers": layers,
    }


def evaluate_current_impact_graph(
    *, profile: str, receipt_bindings: dict[str, str]
) -> dict[str, Any]:
    """Evaluate the repository's declared proof layers without running them."""
    return evaluate_impact_graph(
        {
            "nodes": {
                "fast": {"layer": "Fast", "required": True, "dependsOn": []},
                "finish": {
                    "layer": "Finish",
                    "required": True,
                    "dependsOn": ["fast"],
                },
                "hosted": {
                    "layer": "Hosted",
                    "required": True,
                    "finalProof": True,
                    "dependsOn": ["finish"],
                },
            }
        },
        profile=profile,
        receipt_bindings=receipt_bindings,
    )


def escalation_reasons(
    changed_paths: list[str],
    *,
    unknown: bool = False,
    injection: bool = False,
    prior_failure: bool = False,
) -> list[str]:
    """Return stable reasons; an empty result never lowers an already strict policy."""
    reasons = sorted({classify_path(path) for path in changed_paths} & ESCALATION_DOMAINS)
    if unknown:
        reasons.append("unknown_input")
    if injection:
        reasons.append("injection_signal")
    if prior_failure:
        reasons.append("test_changed_after_failure")
    return sorted(set(reasons))


def verification_signal(required: list[str], index: dict[str, str]) -> dict[str, Any]:
    missing = [x for x in required if x not in index]
    failed = [x for x in required if index.get(x) == "failed"]
    not_run = [x for x in required if index.get(x) == "not_run"]
    passed = [x for x in required if index.get(x) == "passed"]
    if failed:
        value, evidence = "failed", [f"required verification failed: {', '.join(failed)}"]
    elif missing or not_run:
        detail = []
        if missing:
            detail.append(f"missing: {', '.join(missing)}")
        if not_run:
            detail.append(f"not_run: {', '.join(not_run)}")
        value, evidence = "incomplete", [f"required verification incomplete ({'; '.join(detail)})"]
    else:
        value, evidence = "passed", [f"required verification passed: {len(passed)}/{len(required)}"]
    return {
        "value": value,
        "evidence": evidence,
        "sources": ["contract.verification", "summary.verification"],
        "required": required,
        "passed": passed,
        "failed": failed,
        "missing": missing,
        "not_run": not_run,
    }
