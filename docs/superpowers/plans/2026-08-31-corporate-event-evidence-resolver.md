# Corporate Event Evidence Resolver Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 複数の企業イベント source を look-ahead-safe な canonical evidence に統合し、既存の Signal Context と日報 appendix の観測層へ投影する。

**Architecture:** Application の pure resolver は provider read model と外部 enrichment を受け取り、時間 cutoff、公式確認境界、conflict、provenance、health を決定する。Research/Radar ACL が各 concrete provider を resolver input に束ね、Signal Context は canonical event の表示 projection だけを担当する。

**Tech Stack:** Rust、chrono、serde、既存の Research provider ports、既存の Signal Context read model、Cargo test、Make quality gates。

**Spec:** `docs/superpowers/specs/2026-08-31-corporate-event-evidence-resolver-design.md`

## Global Constraints

- `Scheduled`、`PendingConfirmation`、`Confirmed`、`Historical`、`Unavailable` を混同しない。
- SEC `EarningsRelated` 以外の filing と Finnhub aggregator actual は official earnings confirmation に昇格させない。
- `observed_at`、`accepted_at`、`report_run_at` で look-ahead を拒否する。
- Provider failure は no event ではなく health/diagnostic として保持する。
- 外部 enrichment は provenance 付きで generic provider に上書きさせない。symbol から theme を推定しない。
- Corporate Event は Observation/Interpretation 専用で、Decision、Gate、Leader、Action Matrix、Position Sizing、Execution を変更しない。
- Secret は source URL、logs、diagnostic、fixture、archive evidence に出さない。
- repository 内の手書き document/comment は日本語、identifier は英語、commit subject は日本語。

---

### Task 1: Application canonical evidence contract

**Files:**
- Create: `src/features/research/application/corporate_event_evidence_resolver.rs`
- Modify: `src/features/research/application/mod.rs`
- Test: `src/features/research/application/corporate_event_evidence_resolver.rs` 内の unit tests

**Interfaces:**
- Consumes: `ExpectedCorporateEventProviderReadModel`、`OfficialDisclosureProviderReadModel`、`CorporateEventProviderReadModel`。
- Produces: `CorporateEventEvidenceResolverInput`、`CorporateEventEvidence`、`CorporateEventEvidenceResolution`、`CorporateEventEvidenceResolver::resolve`。

- [ ] **Step 1: Write failing reconciliation tests**

  Add tests for Alpha-only Scheduled, SEC EarningsRelated Confirmed, non-earnings filing rejection, Finnhub-only PendingConfirmation, conflict retention, external enrichment retention, and no evidence Unavailable.

- [ ] **Step 2: Run the focused tests and verify the expected missing-contract failure**

  Run `cargo test --lib corporate_event_evidence_resolver`. Expected: compile failure because the new application module and resolver types do not yet exist.

- [ ] **Step 3: Implement the minimal pure resolver**

  Define provider-neutral lifecycle, confidence, evidence ref, diagnostic, enrichment and health types. Filter future/untraceable evidence, group by normalized symbol and earnings event type, apply official-only confirmation, retain all refs, and emit deterministic sorted output.

- [ ] **Step 4: Run the focused tests and refactor only after green**

  Run `cargo test --lib corporate_event_evidence_resolver`. Expected: all lifecycle, conflict, enrichment and provenance tests pass.

- [ ] **Step 5: Commit the application unit**

  Run `git add src/features/research/application/corporate_event_evidence_resolver.rs src/features/research/application/mod.rs` and `git commit -m "feat: 企業イベント証拠 Resolver を追加"`.

### Task 2: Research/Radar ACL source assembly

**Files:**
- Modify: `src/features/research/acl/corporate_event_provider_factory.rs`
- Modify: `src/features/research/acl/official_disclosure_provider_factory.rs`
- Modify: `src/features/radar/acl/corporate_event_provider_factory.rs`
- Modify: `src/features/radar/interface/radar_pipeline_runner.rs`
- Modify: `src/features/radar/interface/signal_context_coverage.rs`
- Modify: `src/features/radar/interface/signal_context_event_read_model.rs`

**Interfaces:**
- Consumes: Existing Finnhub, SEC and Alpha provider factories and `SENTINEL_SIGNAL_CONTEXT_JSON_PATH`.
- Produces: Radar-visible `CorporateEventEvidenceResolution` without exposing infrastructure payloads.

- [ ] **Step 1: Write failing ACL and cutoff tests**

  Add tests proving external fixture enrichment maps to application fields, report cutoff excludes post-run official evidence, and independent provider unavailable states are preserved.

- [ ] **Step 2: Run focused ACL tests and verify failure**

  Run `cargo test --lib corporate_event_provider_factory signal_context_coverage`. Expected: missing resolver assembly/projection symbols or assertions fail.

- [ ] **Step 3: Implement source assembly through Research/Radar ACL**

  Build expected, official and Finnhub read models independently, convert the filtered external Signal Context corporate items to enrichment objects, invoke the application resolver with `report_run_at`, and store the resolution in the event read model.

- [ ] **Step 4: Run focused tests and confirm provider failure isolation**

  Run `cargo test --lib corporate_event_provider_factory signal_context_coverage signal_context_event_read_model`. Expected: source assembly and failure isolation pass without network-dependent tests.

### Task 3: Signal Context projection and daily appendix

**Files:**
- Modify: `src/features/radar/interface/signal_context_coverage.rs`
- Modify: `src/features/radar/interface/signal_context_read_model.rs`
- Modify: `src/features/radar/interface/interpretation_read_model.rs`
- Modify: `src/features/radar/interface/radar_pipeline_runner.rs`
- Modify: `tests/fixtures/signal_context/2026-08-27-nvidia-earnings.json`

**Interfaces:**
- Consumes: `CorporateEventEvidenceResolution` stored in `SignalContextEventReadModel`.
- Produces: Observation-only corporate context item and appendix lines such as provider health, Scheduled/Confirmed state, and source list.

- [ ] **Step 1: Write failing projection and decision-invariance tests**

  Add tests for NVIDIA pre-release/post-release/T+1 lifecycle display and compare the unchanged decision fields for no evidence, scheduled, confirmed and enriched inputs.

- [ ] **Step 2: Run tests and verify missing projection behavior**

  Run `cargo test --lib signal_context_coverage signal_context_read_model report_ui_tests`. Expected: new canonical lifecycle/appendix assertions fail before projection is implemented.

- [ ] **Step 3: Implement canonical projection**

  Map canonical lifecycle to existing Signal Context lifecycle, render expected and confirmed dates separately, merge evidence records without replacing external enrichment, and append resolver health/source diagnostics. Keep `decision_weight` zero and existing decision fields untouched.

- [ ] **Step 4: Run focused tests and inspect generated report text**

  Run `cargo test --lib signal_context_coverage signal_context_read_model report_ui_tests` and verify the appendix contains health/source lines while no credential value is present.

### Task 4: Documentation, fixtures and full verification

**Files:**
- Modify: `tests/fixtures/corporate_events/finnhub-2026-08-27-nvidia-earnings.json`
- Create: resolver-specific deterministic fixture files under `tests/fixtures/corporate_events/`
- Modify: `docs/superpowers/specs/2026-08-31-corporate-event-evidence-resolver-design.md`
- Modify: `.ai/work-items/active/corporate-event-evidence-resolver.summary.json`

- [ ] **Step 1: Add deterministic resolver/NVIDIA fixtures**

  Add only non-secret JSON fixture data for expected, official, conflict and report-cutoff cases. Keep fixture facts semantically equivalent to the existing NVIDIA context.

- [ ] **Step 2: Run all required repository gates**

  Run `make fmt-check`, `make test`, `make clippy`, `make quality`, and `git diff --check`. Expected: all pass; coverage and report tests include the resolver scenarios.

- [ ] **Step 3: Update Summary evidence**

  Record changed paths, focused tests, full gate results, scenario evidence, decision invariance, report output, data/fixture status, residual risks and any unverified benefit without inventing a product benefit.

- [ ] **Step 4: Run Cockpit verification and finish**

  Run `ai-cockpit verify --repo . --work-item corporate-event-evidence-resolver --command make --args quality`, then `ai-cockpit finish --repo . --id corporate-event-evidence-resolver` and `ai-cockpit archive --repo . --id corporate-event-evidence-resolver` only after every required check is green.

- [ ] **Step 5: Commit the verified archive bundle**

  Run `git add .ai docs src tests` for only scoped paths and `git commit -m "feat: 企業イベント証拠を統合する Resolver を実装"`.
