#!/usr/bin/env python3
"""Gray Rhino evidence contract の SSOT 整合を検証する。"""
from __future__ import annotations

import re
import sys
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parents[1]
SCHEMA_PATH = PROJECT_ROOT / ".ai/architecture/gray_rhino_evidence_schema.yaml"
DOC_PATH = PROJECT_ROOT / "docs/specs/GRAY_RHINO_EVIDENCE_CONTRACT.md"
FRAMEWORK_PATH = PROJECT_ROOT / "docs/specs/GRAY_RHINO_ESCALATION_FRAMEWORK.md"
DOMAIN_PATH = PROJECT_ROOT / "src/features/research/domain/gray_rhino_evidence.rs"
GOVERNANCE_SOURCE_PATH = PROJECT_ROOT / "src/features/research/domain/governance_source.rs"
ASSESSMENT_PATH = PROJECT_ROOT / "src/features/research/application/gray_rhino_assessment.rs"
REPLAY_FIXTURE_DIR = PROJECT_ROOT / "tests/fixtures/governance_sec"

REQUIRED_CATEGORIES = {
    "GovernanceConcentration",
    "DependencyConcentration",
    "InstitutionalMaturity",
    "RiskNormalization",
    "Redundancy",
}
REQUIRED_SOURCE_TYPES = {
    "RegulatoryFiling",
    "GovernanceDocument",
    "CompanyDisclosure",
    "IndependentAudit",
    "InfrastructureStatus",
    "SupplierDisclosure",
    "MarketNarrativeCorpus",
    "OperatorCuratedSource",
}
REQUIRED_BOUNDARY_TERMS = {
    "narrative_only",
    "price_action_only",
    "trade_signal",
    "gate_signal",
    "execution_signal",
    "trend_cohesion_override",
}


def main() -> int:
    errors: list[str] = []
    paths = [
        SCHEMA_PATH,
        DOC_PATH,
        FRAMEWORK_PATH,
        DOMAIN_PATH,
        GOVERNANCE_SOURCE_PATH,
        ASSESSMENT_PATH,
    ]
    for path in paths:
        if not path.exists():
            errors.append(f"missing required file: {path.relative_to(PROJECT_ROOT)}")
    if errors:
        report(errors)
        return 1

    schema = SCHEMA_PATH.read_text(encoding="utf-8")
    doc = DOC_PATH.read_text(encoding="utf-8")
    framework = FRAMEWORK_PATH.read_text(encoding="utf-8")
    domain = DOMAIN_PATH.read_text(encoding="utf-8")
    governance_source = GOVERNANCE_SOURCE_PATH.read_text(encoding="utf-8")
    assessment = ASSESSMENT_PATH.read_text(encoding="utf-8")

    required_schema_pairs = {
        "contractKey: gray-rhino-evidence-contract",
        "humanReadableSsot: docs/specs/GRAY_RHINO_EVIDENCE_CONTRACT.md",
        "currentObservationSource: ManualConfiguration",
        "automatedCollectionEnabled: false",
        "governanceEvidencePipelineEnabled: true",
        "governanceEvidenceStore: gray_rhino_evidence.jsonl",
        "governanceSourceAdapterEnabled: true",
        "governanceSourceCache: gray_rhino_sources/governance",
        "governanceSourceManifest: gray_rhino_governance_source_manifest.jsonl",
        "governanceExtractionAudit: gray_rhino_governance_extraction_audit.jsonl",
        "governanceReplayFixturePack: tests/fixtures/governance_sec",
        "governanceSecLiveDryRunDefault: true",
        "dependencyEvidencePipelineEnabled: true",
        "dependencyEvidenceStore: gray_rhino_evidence.jsonl",
        "dependencyReplayFixturePack: tests/fixtures/dependency_local",
        "dependencySourceManifest: gray_rhino_dependency_source_manifest.jsonl",
        "dependencyExtractionAudit: gray_rhino_dependency_extraction_audit.jsonl",
        "evidence_must_not_set_escalation_state: true",
    }
    for item in required_schema_pairs:
        if item not in schema:
            errors.append(f"schema missing `{item}`")

    categories = set(extract_yaml_list(schema, "categories"))
    missing_categories = REQUIRED_CATEGORIES - categories
    if missing_categories:
        errors.append(f"schema categories missing: {sorted(missing_categories)}")

    source_types = set(extract_yaml_list(schema, "sourceTypes"))
    missing_source_types = REQUIRED_SOURCE_TYPES - source_types
    if missing_source_types:
        errors.append(f"schema sourceTypes missing: {sorted(missing_source_types)}")

    boundaries = set(extract_yaml_list(schema, "  forbidden_as_evidence"))
    missing_boundaries = REQUIRED_BOUNDARY_TERMS - boundaries
    if missing_boundaries:
        errors.append(f"schema forbidden boundaries missing: {sorted(missing_boundaries)}")

    doc_required = [
        "Gray Rhino Evidence Contract",
        "Evidence と Narrative の境界",
        "Governance Concentration Evidence",
        "Dependency Concentration Evidence",
        "Institutional Maturity Evidence",
        "Risk Normalization Evidence",
        "Redundancy Evidence",
        "Source Contract",
        "Phase 2: Gray Rhino Evidence Schema",
        "Phase 3-A: Governance Concentration Evidence Pipeline",
        "Phase 3-A: Governance Source Adapter",
        "Phase 3-A: Governance Backfill And Extraction Audit",
        "Phase 3-A: Governance SEC Replay Pack",
        "Phase 3-A: Governance SEC Live Dry-Run",
        "Phase 3-A: Governance SEC Field Coverage Calibration",
        "Phase 3-A: Governance SEC Expanded Sample Dry-Run",
        "Phase 3-A: Governance SEC Voting Structure Calibration",
        "Phase 3-A: Governance SEC Board Independence Calibration",
        "Phase 3-A: Governance SEC Founder Voting Power Calibration",
        "Phase 3-B: Dependency Concentration Evidence Pipeline",
        "repository-local structured JSON",
        "deterministic extraction",
        "rejection taxonomy",
        "sensor health",
        "自動情報収集",
    ]
    for item in doc_required:
        if item not in doc:
            errors.append(f"doc missing `{item}`")

    if "GRAY_RHINO_EVIDENCE_CONTRACT.md" not in framework:
        errors.append("framework doc does not link evidence contract")

    for enum_name in ["GrayRhinoEvidenceCategory", "GrayRhinoEvidenceSourceType"]:
        if enum_name not in domain:
            errors.append(f"domain missing enum `{enum_name}`")

    for item in REQUIRED_CATEGORIES | REQUIRED_SOURCE_TYPES:
        if not re.search(rf"\b{re.escape(item)}\b", domain):
            errors.append(f"domain missing schema enum variant `{item}`")

    required_domain_terms = [
        "GovernanceConcentrationEvidence",
        "GovernanceConcentrationMetrics",
        "DependencyConcentrationEvidence",
        "DependencyConcentrationMetrics",
        "DependencyConcentrationKind",
        "MissingSourceReference",
        "MissingGovernanceMetric",
        "MissingDependencyMetric",
        "NarrativeOnly",
        "ForbiddenBoundaryTerm",
        "validate(&self)",
    ]
    for item in required_domain_terms:
        if item not in domain:
            errors.append(f"domain missing validation boundary `{item}`")
    if "GovernanceSourceDocument" not in governance_source:
        errors.append("governance source domain missing `GovernanceSourceDocument`")
    for item in [
        "GovernanceSourceManifest",
        "GovernanceExtractionAuditRecord",
        "GovernanceMetricAuditStatus",
        "GovernanceReplayRejectionKind",
    ]:
        if item not in governance_source:
            errors.append(f"governance source domain missing `{item}`")
    if not REPLAY_FIXTURE_DIR.exists():
        errors.append("governance SEC replay fixture pack is missing")
    elif len(list(REPLAY_FIXTURE_DIR.glob("*.txt"))) < 5:
        errors.append("governance SEC replay fixture pack must include at least 5 fixtures")

    if "validate_gray_rhino_evidence_contract" not in assessment:
        errors.append("application boundary does not expose evidence contract validation")
    if "evaluate_gray_rhino_escalation" in domain:
        errors.append("evidence domain must not evaluate escalation state")

    governance_metrics = set(extract_yaml_list(schema, "  requiredMetricAtLeastOneOf"))
    for item in governance_metrics:
        if item not in domain:
            errors.append(f"governance domain missing metric `{item}`")

    for item in [
        "phase_3a_governance_backfill_audit: active",
        "phase_3a_governance_sec_replay_pack: active",
        "phase_3a_governance_sec_live_dry_run: active",
        "phase_3a_governance_sec_field_coverage_calibration: active",
        "phase_3a_governance_sec_expanded_sample_dry_run: active",
        "phase_3a_governance_sec_voting_structure_calibration: active",
        "phase_3a_governance_sec_board_independence_calibration: active",
        "phase_3a_governance_sec_founder_voting_power_calibration: active",
        "noEscalationStateMutation: true",
        "rawSourceCache: gray_rhino_sources/governance",
        "sourceManifest: gray_rhino_governance_source_manifest.jsonl",
        "extractionAudit: gray_rhino_governance_extraction_audit.jsonl",
        "sensorHealthReportOnly: true",
        "deterministicExtractionOnly: true",
        "replayCoverageReport: true",
        "defaultPersistEvidence: false",
        "writesRawCache: true",
        "writesSourceManifest: true",
        "writesExtractionAudit: true",
        "expandedSampleSize: 5-10",
        "reportsFieldCoverage: true",
        "controlled X% of the voting power",
        "representing X% of the voting power",
        "entitled to X% of the voting power",
        "hold X% of the voting power",
        "succession framework",
        "ceo succession framework",
        "multi-class voting structure",
        "multi-class common stock",
        "class b stock has 10 times the voting rights",
        "class b common stock have ten votes per share",
        "class b common stock represents 15 votes",
        "Of the N Board nominees, M are independent",
        "M out of N director nominees are independent",
        "consists of N directors, M of whom are independent",
        "MetriclessSource",
        "SourceInvalid",
        "ExtractionInvalid",
    ]:
        if item not in schema:
            errors.append(f"schema missing governance pipeline contract `{item}`")

    for item in [
        "dependencyEvidencePipelineEnabled: true",
        "dependencyEvidenceStore: gray_rhino_evidence.jsonl",
        "phase_3b_dependency_concentration_evidence_pipeline: active",
        "dependencyConcentration:",
        "phase: phase_3b_dependency_concentration_evidence_pipeline",
        "inputMode: repository_local_structured_json",
        "concentration_ratio",
        "single_point_of_failure",
        "fallback_disclosed",
        "dependency_kind",
        "dependency_name",
        "localIngestionCli: ingest-gray-rhino-dependency",
        "ingest-gray-rhino-dependency",
        "dependencyReplayFixturePack: tests/fixtures/dependency_local",
        "dependencySourceManifest: gray_rhino_dependency_source_manifest.jsonl",
        "dependencyExtractionAudit: gray_rhino_dependency_extraction_audit.jsonl",
        "localSourceCollectionOnly: true",
        "replayCoverageReport: true",
        "Infrastructure",
        "Compute",
        "Cloud",
        "Launch",
        "Supplier",
        "Ecosystem",
        "CompanyDisclosure",
        "InfrastructureStatus",
        "SupplierDisclosure",
        "IndependentAudit",
        "OperatorCuratedSource",
        "outputCategory: DependencyConcentration",
        "collectorEnabled: true",
        "noEscalationStateMutation: true",
    ]:
        if item not in schema:
            errors.append(f"schema missing dependency pipeline contract `{item}`")

    if errors:
        report(errors)
        return 1
    print("✅ gray rhino evidence contract check passed")
    return 0


def extract_yaml_list(raw: str, key: str) -> list[str]:
    lines = raw.splitlines()
    values: list[str] = []
    in_section = False
    base_indent: int | None = None
    for line in lines:
        if line.rstrip() == f"{key}:":
            in_section = True
            base_indent = len(line) - len(line.lstrip())
            continue
        if not in_section:
            continue
        indent = len(line) - len(line.lstrip())
        stripped = line.strip()
        if stripped.startswith("- "):
            values.append(stripped[2:])
            continue
        if stripped and base_indent is not None and indent <= base_indent:
            break
    return values


def report(errors: list[str]) -> None:
    print("❌ gray rhino evidence contract violations:")
    for error in errors:
        print(f"  - {error}")


if __name__ == "__main__":
    sys.exit(main())
