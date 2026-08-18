# Observation Semantic Reconciliation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Unify Observation facts and report semantics for Breadth, Leader absence, Relative Strength conflicts, Interpretation, and Markdown output without changing Decision behavior.

**Architecture:** Add one canonical Breadth fact derivation at the observation boundary, preserve unavailable values as `Option`, and project the result through persistence, presentation, interpretation, and report layers. Extend Leader Persistence with explicit snapshot/confirmed/absence fields; compute RS conflict and tactical leaderless structure only in read models.

**Tech Stack:** Rust, Serde JSON, Chrono, existing radar presentation/report tests, Make-based AI Cockpit gates.

**Spec:** `docs/superpowers/specs/2026-08-18-observation-semantic-reconciliation-design.md`

## Global Constraints

- Decision、Action Matrix、Gate、Execution、Trader、READY / EXECUTE、Position Sizing、Trade Signal は変更しない。
- `total_count == 0` は `UNAVAILABLE` とし、Breadth の欠損を `0.0` として新規保存しない。
- Breadth Raw、Label、Classification Score は同じ up/flat/down/total fact source を使用する。
- Leader absence 5 trading days 以上は tactical `LEADERLESS / FRAGMENTED` とする。
- Current Relative Strength block は Markdown/archival/Telegram の全 delivery body で純 Markdown とし、他の Telegram section の HTML channel contract は維持する。
- すべての code comment/documentation comment は日本語、repository Markdown 本文は日本語で記述する。
- Required verification は Contract に記録された `make` target を実行し、結果を Summary に同期する。

---

### Task 1: Canonical Breadth facts and unavailable persistence

**Files:**
- Modify: `src/features/radar/domain/observation_timeline.rs`
- Modify: `src/features/radar/infrastructure/persistence/model.rs`
- Modify: `src/features/radar/infrastructure/persistence/migration.rs`
- Modify: `src/features/radar/infrastructure/persistence.rs`
- Modify: `src/features/radar/interface/presentation_assembler.rs`
- Modify: `src/features/radar/interface/radar_pipeline_runner.rs`
- Test: `src/features/radar/domain/observation_timeline.rs`
- Test: `src/features/radar/infrastructure/persistence.rs`

**Interfaces:**
- Produces a canonical observation containing `raw_percent: Option<f64>`, `label: String`, and `classification_score: Option<f64>` from the four market counts.
- Persists `Option<f64>` for snapshot/timeline breadth values and renders missing history as `UNAVAILABLE`.

- [ ] **Step 1: Write failing domain tests**

Add tests for `total_count == 0` returning unavailable facts and for `up=5, flat=2, down=3, total=10` returning raw/score `50.0` and label `Narrow`. Add a timeline test asserting a zero-total entry serializes/deserializes as `None` and renders an unavailable sequence value.

- [ ] **Step 2: Run the focused test to verify RED**

Run: `make test`

Expected: the new assertions fail because current code maps zero-total breadth to `0.0` and derives the label from `TrendBreadthMode`.

- [ ] **Step 3: Implement the smallest canonical fact helper**

Add the helper beside the observation timeline domain types. Use `Option<f64>` for raw and score, explicit `UNAVAILABLE` label for zero total, and thresholds `<30`, `<60`, `>=60`. Make `has_structural_change` compare options rather than subtracting unavailable values.

- [ ] **Step 4: Propagate the helper through writes and legacy projection**

Change `TradingDaySnapshot.breadth` and timeline breadth fields to optional values, update migration/pipeline/data-quality JSON to write `null`/omitted optional values for unavailable data, and normalize old zero-total timeline records to `None` when counts prove the universe is empty. Update test fixtures and weekly projections only where compilation requires it.

- [ ] **Step 5: Make the report summary consume the same facts**

Replace mode-derived Breadth Label/Score in the report presentation with the canonical helper while retaining `TrendBreadthMode` as a separate market-background observation. Render `UNAVAILABLE` for missing raw/score/universe values.

- [ ] **Step 6: Run the focused test to verify GREEN**

Run: `make test`

Expected: Breadth domain and persistence regressions pass; unrelated failures are recorded before moving to the next task.

### Task 2: Explicit Leader Persistence semantics and tactical structure

**Files:**
- Modify: `src/features/radar/domain/leader_persistence/mod.rs`
- Modify: `src/features/radar/domain/leader_persistence/persistence.rs`
- Modify: `src/features/radar/domain/leader_persistence/snapshot.rs`
- Modify: `src/features/radar/domain/leader_persistence/tests.rs`
- Modify: `src/features/radar/interface/presentation.rs`
- Modify: `src/features/radar/interface/market_interpretation_read_model.rs`
- Modify: `src/features/radar/interface/report.rs`
- Test: `src/features/radar/domain/leader_persistence/tests.rs`
- Test: `src/features/radar/interface/market_interpretation_read_model.rs`

**Interfaces:**
- `LeaderPersistenceResult` and `LeaderPersistenceViewModel` expose current, previous snapshot, last confirmed, absence start, and duration independently.
- Market interpretation receives an absence context and emits `LEADERLESS / FRAGMENTED` after 5 trading days.

- [ ] **Step 1: Write failing Leader tests**

Add a sequence with `TSLA` followed by nine `none` observations. Assert current `none`, previous snapshot `none`, last confirmed `TSLA`, absence duration `9`, absence start at the first `none` date, and tactical structure `LEADERLESS / FRAGMENTED`. Assert a one-day absence does not trigger the threshold.

- [ ] **Step 2: Run the focused test to verify RED**

Run: `make test`

Expected: current code exposes only `previous_leader`, lacks the last-confirmed/absence-start fields, and keeps the old leadership semantics.

- [ ] **Step 3: Implement explicit domain fields**

Rename the ambiguous previous field to `previous_snapshot_leader`, scan backward for the last non-`none` leader, and calculate the start date of the trailing absence streak. Add the five-trading-day constant and keep persistence score/qualification rules unchanged.

- [ ] **Step 4: Project and render the new fields**

Populate the presentation view model and render the four explicit labels in the absent branch. Add tactical structure fields to the interpretation view model and keep strategic context separate.

- [ ] **Step 5: Run the focused test to verify GREEN**

Run: `make test`

Expected: leader persistence and interpretation regressions pass without changes to action/decision modules.

### Task 3: Relative Strength conflict and recovery watch

**Files:**
- Modify: `src/features/radar/interface/presentation.rs`
- Modify: `src/features/radar/interface/presentation_assembler.rs`
- Modify: `src/features/radar/interface/report.rs`
- Modify: `src/features/radar/interface/market_interpretation_read_model.rs`
- Test: `src/features/radar/interface/report_ui_tests.rs`

**Interfaces:**
- Current Relative Strength item view models carry optional action/exit state, conflict code, recovery-watch state, and localized explanation.
- Conflict detection maps existing `StrengthLoss`/`CohesionExit`/weak actions to `WEAKENING` without changing the source decision.

- [ ] **Step 1: Write failing report-level conflict tests**

Construct an `SPCX` asset with existing `StrengthLoss`/trim semantics and an `IMPROVING` current-relative-strength observation. Assert Markdown contains `SIGNAL_CONFLICT`, `RECOVERY_WATCH`, and the recovery explanation while the asset action remains unchanged.

- [ ] **Step 2: Run the focused test to verify RED**

Run: `make test`

Expected: current output contains only `IMPROVING` and no conflict/recovery-watch explanation.

- [ ] **Step 3: Implement read-model-only conflict detection**

Join observations to packet assets by symbol, derive the existing weak state, and add conflict metadata only when weakness and improving RS coexist. Do not modify `DecisionPacket.assets`, `AssetActionDecision.action`, or any gate input.

- [ ] **Step 4: Render localized conflict output**

Render the stable code and recovery-watch explanation under the affected symbol in Markdown syntax in every delivery body and in Japanese/English/Chinese localized text.

- [ ] **Step 5: Run the focused test to verify GREEN**

Run: `make test`

Expected: conflict output passes and existing action/NO TRADE assertions remain unchanged.

### Task 4: Interpretation reconciliation and renderer contract

**Files:**
- Modify: `src/features/radar/interface/market_interpretation_read_model.rs`
- Modify: `src/features/radar/interface/radar_pipeline_runner.rs`
- Modify: `src/features/radar/interface/report.rs`
- Modify: `src/features/radar/interface/report_ui_tests.rs`
- Test: `src/features/radar/interface/report_ui_tests.rs`

**Interfaces:**
- Narrative builder consumes absence duration, raw breadth availability/value, improving RS symbols, shrink/watch counts, and crowding/overheat state.
- Current Relative Strength block uses Markdown syntax only in every delivery body; other Telegram sections remain channel-specific HTML.

- [ ] **Step 1: Write failing interpretation and renderer tests**

Add a fragile no-leader scenario asserting the narrative says no new acute deterioration but explicitly names leader absence and insufficient diffusion, and that it does not emit the unconditional old sentence. Assert both delivery bodies contain no `<h3>`/`<li>` in the Current Relative Strength block and preserve Markdown heading/list syntax there.

- [ ] **Step 2: Run the focused test to verify RED**

Run: `make test`

Expected: current narrative always emits the old no-deterioration sentence and the HTML-mode renderer is the only path with the requested tags; the new consistency assertions fail before the implementation changes.

- [ ] **Step 3: Implement fact-aware narrative**

Pass the already-built observation facts into the narrative builder. Add the two-part fragile-structure explanation and a separate RS recovery sentence that explicitly says it is not yet new leadership. Include shrink/watch and crowding/overheat facts when present.

- [ ] **Step 4: Fix Current Relative Strength renderer without changing other Telegram sections**

Keep `###` and `-` output for the Current Relative Strength block in every delivery mode, and ensure no `<h3>`/`<li>` can be emitted there. Keep the existing HTML channel formatting covered for unrelated Telegram sections.

- [ ] **Step 5: Run the focused test to verify GREEN**

Run: `make test`

Expected: interpretation and renderer tests pass in all three languages.

### Task 5: Full verification and Work Item evidence

**Files:**
- Modify: `.ai/work-items/active/semantic-reconciliation.summary.json`
- Modify: `.ai/cockpit/current_status.md`

- [ ] **Step 1: Run required Rust gates**

Run: `make fmt-check`, `make test`, and `make clippy`.

Expected: all commands exit 0; record exact results in Summary.

- [ ] **Step 2: Run all Contract and architecture gates**

Run the Contract verification list through `make`, including `make check-ai-contract`, `make check-ai-scope`, `make check-ai-guards`, `make check-ai-backtrack`, `make check-ai-coverage-guard`, `make check-ai-scenario-coverage`, `make check-ai-change-summary`, `make generate-cockpit-status`, `make check-ai-status`, `make check-ai-status-consistency`, `make check-architecture-all`, and `make quality`.

Expected: no required check is left `not_run` or failed.

- [ ] **Step 3: Audit scope and boundaries**

Confirm the diff contains no changes under Action Matrix, Gate, Execution, Trader, or Position Sizing and that report tests still prove NO TRADE and observation-only behavior.

- [ ] **Step 4: Update Summary and finish the Work Item**

Record changed files, verified scenario evidence, residual risks, expected review focus, and user authorization. Run `make ai-finish TASK=semantic-reconciliation` only after all required checks pass.
