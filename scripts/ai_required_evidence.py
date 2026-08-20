"""Derive operation-required evidence without relying on agent-declared unknowns."""

from __future__ import annotations

from dataclasses import dataclass
from fnmatch import fnmatchcase


@dataclass(frozen=True)
class EvidenceContext:
    """Normalized operation facts used by the declarative rule registry."""

    requested_operation: str
    changed_paths: tuple[str, ...]
    risk_types: tuple[str, ...]
    capability_claims: tuple[str, ...]
    environment: str
    external_system: str
    destructive_level: str
    governance_profile: str
    available_evidence: tuple[str, ...]


@dataclass(frozen=True)
class EvidenceRule:
    """One context predicate and its evidence/decision consequence."""

    identifier: str
    required_evidence: tuple[str, ...]
    owner: str
    blocking_level: str
    human_decision_required: bool
    forbidden_claim: str
    operations: tuple[str, ...] = ()
    destructive_levels: tuple[str, ...] = ()
    risk_types: tuple[str, ...] = ()
    environments: tuple[str, ...] = ()
    path_patterns: tuple[str, ...] = ()
    capability_claims: tuple[str, ...] = ()
    path_or_capability: bool = False

    def matches(self, context: EvidenceContext) -> bool:
        path_match = any(
            fnmatchcase(path, pattern)
            for path in context.changed_paths
            for pattern in self.path_patterns
        )
        claim_match = bool(set(self.capability_claims) & set(context.capability_claims))
        context_match = (
            (path_match or claim_match)
            if self.path_or_capability
            else (
                (not self.path_patterns or path_match)
                and (not self.capability_claims or claim_match)
            )
        )
        return (
            (not self.operations or context.requested_operation in self.operations)
            and (
                not self.destructive_levels or context.destructive_level in self.destructive_levels
            )
            and (not self.risk_types or bool(set(self.risk_types) & set(context.risk_types)))
            and (not self.environments or context.environment in self.environments)
            and context_match
        )


@dataclass(frozen=True)
class RequiredEvidenceResult:
    """Explainable evidence requirement projection for one operation context."""

    required_evidence: tuple[str, ...]
    missing_evidence: tuple[str, ...]
    owner_by_evidence: dict[str, str]
    blocking_level: str
    human_decision_required: bool
    forbidden_claims: tuple[str, ...]
    matched_rules: tuple[str, ...]


RULES = (
    EvidenceRule(
        identifier="deletion",
        destructive_levels=("delete",),
        required_evidence=(
            "usage_analysis",
            "reference_search",
            "public_api_impact",
            "test_impact",
            "migration_impact",
            "rollback_plan",
        ),
        owner="repository_maintainer",
        blocking_level="block",
        human_decision_required=False,
        forbidden_claim="Do not claim deletion safety or compatibility preservation.",
    ),
    EvidenceRule(
        identifier="publication",
        operations=("publish",),
        required_evidence=(
            "tag",
            "commit",
            "digest",
            "sbom",
            "provenance",
            "provider_release_receipt",
            "asset_availability",
        ),
        owner="release_manager",
        blocking_level="block",
        human_decision_required=True,
        forbidden_claim="Do not claim a published or downloadable release.",
    ),
    EvidenceRule(
        identifier="release_context",
        path_patterns=(
            "release.json",
            "next-release.json",
            "release-state.json",
            "VERSION",
            "version.txt",
            "version.json",
            "package-publish*.json",
            ".npmrc",
            ".github/workflows/release*.yml",
            ".github/workflows/release*.yaml",
            ".github/workflows/sign*.yml",
            ".github/workflows/sign*.yaml",
            "signing/**",
            "dist/**",
            "release-assets/**",
            ".ai/cockpit/release-digests.json",
            ".ai/cockpit/sbom.json",
            ".ai/cockpit/provenance/**",
        ),
        capability_claims=("release_ready", "distribution_verified"),
        path_or_capability=True,
        required_evidence=(
            "tag",
            "commit",
            "digest",
            "sbom",
            "provenance",
            "provider_release_receipt",
            "asset_availability",
        ),
        owner="release_manager",
        blocking_level="block",
        human_decision_required=True,
        forbidden_claim="Do not claim release readiness or distribution verification.",
    ),
    EvidenceRule(
        identifier="permission",
        risk_types=("permission_operation",),
        required_evidence=(
            "provider_identity",
            "authorization_scope",
            "resource_id",
            "approval_evidence",
            "audit_receipt",
        ),
        owner="repository_administrator",
        blocking_level="block",
        human_decision_required=True,
        forbidden_claim="Do not claim authorization or permission execution.",
    ),
    EvidenceRule(
        identifier="mobile_validation",
        operations=("mobile_validate",),
        required_evidence=(
            "source_compiles",
            "unit_tests",
            "simulator",
            "device",
            "signing",
            "store_submission",
        ),
        owner="mobile_release_owner",
        blocking_level="review",
        human_decision_required=False,
        forbidden_claim="Do not claim unobserved mobile lifecycle stages.",
    ),
)

_BLOCKING_ORDER = {"none": 0, "review": 1, "block": 2}


def derive_required_evidence(context: EvidenceContext) -> RequiredEvidenceResult:
    """Return deterministic requirements and prohibitions for matching rules."""
    matched_rules = tuple(rule for rule in RULES if rule.matches(context))
    required: list[str] = []
    owners: dict[str, str] = {}
    forbidden: list[str] = []
    for rule in matched_rules:
        for evidence in rule.required_evidence:
            if evidence not in owners:
                required.append(evidence)
                owners[evidence] = rule.owner
        if rule.forbidden_claim not in forbidden:
            forbidden.append(rule.forbidden_claim)
    available = set(context.available_evidence)
    missing = tuple(item for item in required if item not in available)
    blocking_level = max(
        (rule.blocking_level for rule in matched_rules),
        key=lambda level: _BLOCKING_ORDER[level],
        default="none",
    )
    return RequiredEvidenceResult(
        required_evidence=tuple(required),
        missing_evidence=missing,
        owner_by_evidence=owners,
        blocking_level=blocking_level,
        human_decision_required=any(rule.human_decision_required for rule in matched_rules),
        forbidden_claims=tuple(forbidden),
        matched_rules=tuple(rule.identifier for rule in matched_rules),
    )
