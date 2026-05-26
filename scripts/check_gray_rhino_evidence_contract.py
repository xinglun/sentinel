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
DISCOVERY_DOMAIN_PATH = PROJECT_ROOT / "src/features/research/domain/gray_rhino_candidate.rs"
DISCOVERY_POLICY_PATH = PROJECT_ROOT / "src/features/research/domain/gray_rhino_discovery_policy.rs"
ASSESSMENT_POLICY_PATH = PROJECT_ROOT / "src/features/research/domain/gray_rhino_assessment_policy.rs"
EVIDENCE_PROJECTION_POLICY_PATH = PROJECT_ROOT / "src/features/research/domain/gray_rhino_evidence_projection_policy.rs"
DISCOVERY_APP_PATH = PROJECT_ROOT / "src/features/research/application/gray_rhino_discovery.rs"
DAILY_REPORT_APP_PATH = PROJECT_ROOT / "src/features/research/application/gray_rhino_daily_report.rs"
DAILY_REPORT_REPOSITORY_PATH = PROJECT_ROOT / "src/features/research/infrastructure/gray_rhino_daily_report_repository.rs"
EVIDENCE_STORE_PATH = PROJECT_ROOT / "src/features/research/infrastructure/gray_rhino_evidence_store.rs"
REPORT_INTERFACE_PATH = PROJECT_ROOT / "src/features/research/interface/gray_rhino_report.rs"
MONITORING_STATE_PATH = PROJECT_ROOT / "src/features/research/application/gray_rhino_monitoring_state.rs"
MONITORING_POLICY_PATH = PROJECT_ROOT / "src/features/research/domain/gray_rhino_monitoring_policy.rs"
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
        DISCOVERY_DOMAIN_PATH,
        DISCOVERY_POLICY_PATH,
        ASSESSMENT_POLICY_PATH,
        EVIDENCE_PROJECTION_POLICY_PATH,
        DISCOVERY_APP_PATH,
        DAILY_REPORT_APP_PATH,
        DAILY_REPORT_REPOSITORY_PATH,
        EVIDENCE_STORE_PATH,
        REPORT_INTERFACE_PATH,
        MONITORING_STATE_PATH,
        MONITORING_POLICY_PATH,
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
    discovery_domain = DISCOVERY_DOMAIN_PATH.read_text(encoding="utf-8")
    discovery_policy = DISCOVERY_POLICY_PATH.read_text(encoding="utf-8")
    assessment_policy = ASSESSMENT_POLICY_PATH.read_text(encoding="utf-8")
    evidence_projection_policy = EVIDENCE_PROJECTION_POLICY_PATH.read_text(encoding="utf-8")
    discovery_app = DISCOVERY_APP_PATH.read_text(encoding="utf-8")
    daily_report_app = DAILY_REPORT_APP_PATH.read_text(encoding="utf-8")
    daily_report_repository = DAILY_REPORT_REPOSITORY_PATH.read_text(encoding="utf-8")
    evidence_store = EVIDENCE_STORE_PATH.read_text(encoding="utf-8")
    report_interface = REPORT_INTERFACE_PATH.read_text(encoding="utf-8")
    monitoring_state = MONITORING_STATE_PATH.read_text(encoding="utf-8")
    monitoring_policy = MONITORING_POLICY_PATH.read_text(encoding="utf-8")
    governance_source = GOVERNANCE_SOURCE_PATH.read_text(encoding="utf-8")
    assessment = ASSESSMENT_PATH.read_text(encoding="utf-8")

    required_schema_pairs = {
        "contractKey: gray-rhino-evidence-contract",
        "humanReadableSsot: docs/specs/GRAY_RHINO_EVIDENCE_CONTRACT.md",
        "currentObservationSource: AutoDiscovery",
        "automatedCollectionEnabled: true",
        "grayRhinoAutoDiscoveryEnabled: true",
        "candidateDomain: src/features/research/domain/gray_rhino_candidate.rs",
        "candidateDiscoveryScanner: src/features/research/application/gray_rhino_discovery.rs",
        "candidatePersistenceEnabled: true",
        "candidateStore: gray_rhino_candidates.jsonl",
        "monitoringStateMachineEnabled: true",
        "monitoringStateMachine: src/features/research/domain/gray_rhino_monitoring_policy.rs",
        "grayRhinoEvidenceProjectionPolicy: src/features/research/domain/gray_rhino_evidence_projection_policy.rs",
        "grayRhinoEvidenceProjectionRulesInApplicationAllowed: false",
        "grayRhinoSummaryUsesMonitoringStatuses: true",
        "grayRhinoSummaryActiveExcludesCoolingResolved: true",
        "grayRhinoSensorHealthScoreableReadinessEnabled: true",
        "grayRhinoEvidenceReadValidationEnabled: true",
        "grayRhinoEvidenceReadCategorySourceValidationEnabled: true",
        "grayRhinoEvidenceReadRejectedViewEnabled: true",
        "grayRhinoEvidenceReadBatchEnabled: true",
        "grayRhinoRejectedReasonI18nCoverageEnabled: true",
        "grayRhinoInterfaceEvidenceEligibilityAllowed: false",
        "grayRhinoMissingSubjectRejection: MissingSubject",
        "fredThresholdCalibrationEnabled: true",
        "deterministicThresholdStatesEnabled: true",
        "grayRhinoRefreshMakeTarget: gray-rhino-refresh",
        "grayRhinoRefreshRunsDailyCalibration: false",
        "grayRhinoRefreshReportMakeTarget: gray-rhino-refresh-report",
        "grayRhinoRefreshProviderIsolationEnabled: true",
        "grayRhinoRefreshStatusDisplayedInReport: true",
        "grayRhinoProviderFailureExitsNonZero: true",
        "grayRhinoProviderPartialFailureCoverageEnabled: true",
        "inlineWatchlistReferenceEnabled: true",
        "watchlistInlineDisplayEnabled: true",
        "marketReferenceDisplayEnabled: true",
        "semanticIsolationDisplayBoundary: true",
        "compactSummaryEnabled: true",
        "noiseCalibrationEnabled: true",
        "dailyGithubActionsRefreshEnabled: true",
        "dailyGithubActionsRefreshBeforeRadarEnabled: true",
        "dailyGithubActionsRefreshNotifyEnabled: false",
        "riskEffectMissingDefault: Unclassified",
        "unclassifiedEvidenceScoring: excluded_and_reported",
        "dailyReportUseCaseEnabled: true",
        "dailyReportRepositoryPort: GrayRhinoDailyReportRepository",
        "dailyReportQueryReadsPersistedCandidatesOnly: true",
        "dailyReportQueryRediscoveryEnabled: false",
        "dailyReportDisplayLatestCandidateEnabled: true",
        "persistentRiskNoAgeOnlyAutoResolve: true",
        "sourceCollectionUseCase: src/features/research/application/gray_rhino_source_collection.rs",
        "dailyReportOpsViewDtoEnabled: true",
        "dailyGithubActionsWorkflow: .github/workflows/daily_radar.yml",
        "manualRegistryPrimaryMechanism: false",
        "autoSourceCollectionCli: collect-gray-rhino-sources",
        "autoDiscoveryRunStore: gray_rhino_discovery_runs.jsonl",
        "fredConfigEnabled: true",
        "fredConfigField: fred_api_key",
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
        "Phase 4: Auto Discovery And Inline Reference",
        "GrayRhinoCandidate",
        "Gray Rhino Inline Reference",
        "collect-gray-rhino-sources",
        "gray_rhino_discovery_runs.jsonl",
        "gray_rhino_candidates.jsonl",
        "monitoring state machine",
        "Gray Rhino Monitoring State",
        "FRED threshold calibration",
        "make gray-rhino-refresh",
        "watchlist inline display",
        "Watchlist Inline Reference",
        "Market Reference",
        "Other Company Reference",
        "noise calibration",
        "Gray Rhino Summary",
        "Daily GitHub Actions refresh",
        "gray_rhino_refresh_status_latest.json",
        "Unclassified",
        "不可评分",
        "[fred] fred_api_key",
    ]
    for item in doc_required:
        if item not in doc:
            errors.append(f"doc missing `{item}`")

    if "GRAY_RHINO_EVIDENCE_CONTRACT.md" not in framework:
        errors.append("framework doc does not link evidence contract")

    forbidden_doc_assertions = [
        "current source scan",
        "当日の source scan",
        "source scan と persisted candidates を merge",
    ]
    for phrase in forbidden_doc_assertions:
        if phrase in doc:
            errors.append(f"evidence contract retains obsolete report-query assertion `{phrase}`")

    forbidden_framework_assertions = [
        "設定入力は `gray_rhino_escalation` に限定する",
        "現在の入力由来は `ManualConfiguration`",
        "専用の外部リスク evidence source は未接続",
        "source`: 現在は `ManualConfiguration` のみ",
    ]
    for phrase in forbidden_framework_assertions:
        if phrase in framework:
            errors.append(f"framework retains obsolete manual-only assertion `{phrase}`")

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
        "InstitutionalMaturityEvidence",
        "InstitutionalMaturityMetrics",
        "RedundancyEvidence",
        "RedundancyMetrics",
        "GrayRhinoRiskEffect",
        "risk_effect",
        "Amplifying",
        "Mitigating",
        "Neutral",
        "Unclassified",
        "MissingSourceReference",
        "MissingGovernanceMetric",
        "MissingDependencyMetric",
        "MissingInstitutionalMetric",
        "MissingRedundancyMetric",
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
    for item in [
        "GrayRhinoCandidate",
        "GrayRhinoCandidateScope",
        "GrayRhinoCandidateKind",
        "GrayRhinoCandidateState",
        "Company",
        "Market",
        "Critical",
    ]:
        if item not in discovery_domain:
            errors.append(f"discovery domain missing `{item}`")
    for item in [
        "discover_gray_rhino_candidates",
        "GovernanceConcentration",
        "MarketConcentration",
        "InstitutionalMaturityGap",
        "RedundancyGap",
        "NarrativeCrowding",
        "LiquidityFragility",
        "CapexPaybackFragility",
    ]:
        if item not in discovery_policy:
            errors.append(f"discovery domain policy missing `{item}`")
    for item in [
        "dual class",
        "fallback unavailable",
        "capex payback risk",
    ]:
        if item in discovery_app:
            errors.append(f"discovery application must not contain classification rule `{item}`")
    for item in [
        "latest_effective_subject_category_records",
        "GrayRhinoEvidenceCategory",
        "GrayRhinoRiskEffect",
        "amplifying categories",
    ]:
        if item not in assessment_policy:
            errors.append(f"assessment domain policy missing `{item}`")
    for item in [
        "evidence_resolved_candidates",
        "latest_effective_evidence",
        "has_prior_resolvable_candidate",
        "has_prior_amplifying_evidence",
    ]:
        if item not in evidence_projection_policy:
            errors.append(f"evidence projection domain policy missing `{item}`")
    for item in [
        "fn evidence_resolved_candidates",
        "fn latest_effective_evidence",
        "fn has_prior_resolvable_candidate",
        "fn has_prior_amplifying_evidence",
    ]:
        if item in daily_report_app:
            errors.append(f"daily report application must not contain evidence projection policy `{item}`")
    for item in [
        "fn is_scoreable_evidence_record",
        "GrayRhinoRiskEffect::Amplifying | GrayRhinoRiskEffect::Mitigating",
    ]:
        if item in report_interface:
            errors.append(f"interface must not contain evidence eligibility policy `{item}`")
    for item in [
        "source_type_allowed_for_category",
        "UnsupportedSourceType",
    ]:
        if item not in domain:
            errors.append(f"evidence domain missing category source validation `{item}`")
    for item in [
        "GrayRhinoEvidenceReadBatch",
        "load_evidence_read_batch",
    ]:
        if item not in evidence_store:
            errors.append(f"evidence store missing single read batch `{item}`")
    if "load_evidence_read_batch()?" not in daily_report_repository:
        errors.append("daily report repository must consume one evidence read batch")
    for item in [
        "MissingSourceReference",
        "MissingSourceTitle",
        "MissingPublisher",
        "MissingExtractionNote",
        "MissingStructuralFact",
        "UnsupportedSourceType",
        "MissingGovernanceMetric",
        "InvalidGovernanceMetric",
        "MissingDependencyMetric",
        "InvalidDependencyMetric",
        "MissingInstitutionalMetric",
        "InvalidInstitutionalMetric",
        "MissingRedundancyMetric",
        "InvalidRedundancyMetric",
    ]:
        if item not in report_interface:
            errors.append(f"report interface missing rejected reason label `{item}`")
    for item in [
        "render_gray_rhino_inline_reference",
        "reference only",
    ]:
        if item not in report_interface:
            errors.append(f"research interface missing `{item}`")
    for item in [
        "render_gray_rhino_inline_reference",
        "Boundary:",
        "reference only",
    ]:
        if item in discovery_app:
            errors.append(f"discovery application must not contain user-facing output template `{item}`")

    for item in [
        "evaluate_gray_rhino_monitoring_states",
        "GrayRhinoMonitoringDirection",
        "Intensifying",
        "Cooling",
        "Resolved",
    ]:
        if item not in monitoring_policy:
            errors.append(f"monitoring domain policy missing `{item}`")
    for item in [
        "classify_state",
        "stale_state_for_kind",
        "lifecycle_rank",
        "state_rank",
    ]:
        if item in monitoring_state:
            errors.append(f"monitoring application must not contain lifecycle policy `{item}`")

    for item in [
        "DGS10",
        "T10Y2Y",
        "FEDFUNDS",
        "BAMLH0A0HYM2",
        "WALCL",
        "RRPONTSYD",
    ]:
        if item not in schema:
            errors.append(f"schema missing FRED threshold series `{item}`")

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
        "dependencyLiveBackfillEnabled: true",
        "dependencyRealAdapterEnabled: true",
        "dependencyRawSourceCache: gray_rhino_sources/dependency",
        "dependencyHttpRetryCount: 3",
        "dependencyHttpTimeoutSeconds: 20",
        "backfillRunStore: gray_rhino_backfill_runs.jsonl",
        "providerSourceRegistryEnabled: false",
        "providerRegistryConfigRequired: false",
        "providerRegistryFixture: tests/fixtures/gray_rhino_historical/provider_registry.json",
        "scheduledBackfillEnabled: true",
        "backfillDriftDetectionEnabled: true",
        "fetch_failure",
        "timeout",
        "unsupported_format",
        "metricless_source",
        "stale_source",
        "freshnessPolicyEnabled: true",
        "reportOpsViewEnabled: true",
        "sourceCollectionInputMode: repository_local_or_url_dependency_disclosure",
        "localSourceCollectionOnly: true",
        "urlSourceCollectionEnabled: true",
        "liveBackfillDryRunDefault: true",
        "supplier concentration",
        "workloads hosted by",
        "single cloud provider",
        "backup provider",
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

    for item in [
        "institutionalMaturity:",
        "phase_3c_institutional_maturity_evidence_pipeline: active",
        "localIngestionCli: ingest-gray-rhino-institutional",
        "localSourceCollectionEnabled: true",
        "succession planning",
        "independent auditor",
        "comprehensive disclosure",
        "board oversight expanded",
        "developing compliance",
        "succession_structure_disclosed",
        "external_audit_present",
        "disclosure_quality_score",
        "oversight_evolution_disclosed",
        "compliance_maturity_level",
        "outputCategory: InstitutionalMaturity",
        "collectorEnabled: false",
        "noEscalationStateMutation: true",
    ]:
        if item not in schema:
            errors.append(f"schema missing institutional pipeline contract `{item}`")

    for item in [
        "redundancy:",
        "phase_3d_redundancy_evidence_pipeline: active",
        "multiCategorySensorHealthEnabled: true",
        "multiCategoryBackfillDryRunEnabled: true",
        "multiCategoryBackfillManifest: tests/fixtures/gray_rhino_backfill/multi_category_manifest.json",
        "evidenceQualityScoringEnabled: true",
        "evidenceQualityModelVersion: v2",
        "source_diversity",
        "rejection_ratio",
        "sensorReadinessScoreEnabled: true",
        "evidenceExplanationGraphEnabled: true",
        "evidenceDrivenEscalationEngineEnabled: true",
        "evidenceDrivenEscalationUsesCategoryCompleteness: true",
        "evidenceDrivenEscalationOutputBoundary: no_trade_gate_execution",
        "phase_3e_multi_category_sensor_health_dashboard: active",
        "phase_4_escalation_detection_engine: active",
        "localIngestionCli: ingest-gray-rhino-redundancy",
        "localSourceCollectionEnabled: true",
        "backup provider",
        "two alternative suppliers",
        "recovery plan",
        "failover test",
        "fallbackClaimedIsNotFailoverTested: true",
        "fallback_available",
        "alternative_supplier_count",
        "redundancy_ratio",
        "recovery_path_disclosed",
        "failover_tested",
        "outputCategory: Redundancy",
        "collectorEnabled: false",
        "noEscalationStateMutation: true",
    ]:
        if item not in schema:
            errors.append(f"schema missing redundancy pipeline contract `{item}`")

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
