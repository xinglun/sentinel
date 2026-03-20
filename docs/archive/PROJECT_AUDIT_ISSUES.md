# Project Audit Issues

This document converts the audit findings into executable issue tickets.

Each issue is written to be directly usable for task tracking.

## P0

### P0-1: Add End-to-End Pipeline Integration Tests

**Goal**

Protect the full decision pipeline from regression.

**Why**

The project has unit tests, but no integration test that proves `Engine::run_daily_pipeline()` still produces a coherent `DecisionPacket`.

**Scope**

1. Add integration-style tests around:
   - feature extraction
   - market regime transition
   - portfolio policy derivation
   - asset-state computation
   - action-matrix output
2. Use deterministic fixture histories instead of live providers.
3. Verify final `DecisionPacket` fields, not just intermediate states.

**Suggested Files**

1. New test module under `src/core/engine.rs` or `tests/pipeline_integration.rs`
2. Reusable fixtures under `tests/fixtures/` if needed

**Acceptance Criteria**

1. At least one bullish-path integration test exists
2. At least one defensive-path integration test exists
3. Tests validate final `DecisionPacket.market_regime`, `portfolio_policy`, and selected `assets[*].action`

**Dependencies**

None

### P0-2: Add Archival Package Integration Tests

**Goal**

Guarantee that a successful run produces the expected daily archival package.

**Why**

Daily archival is now a product requirement, not a side effect.

**Scope**

1. Add tests covering dry-run archival output in `save_to`
2. Verify required files are created:
   - `decision_history.jsonl`
   - `decision_packet_[DATE].json`
   - `state_transitions.csv`
   - `state_transitions.jsonl`
   - `execution_gate_log.jsonl`
   - `portfolio_snapshot_[DATE].json`
   - `account_snapshot_[DATE].json`
   - `data_quality_log.jsonl`
   - `[DATE].md`
3. Verify write failure propagates as an error for required assets

**Suggested Files**

1. New test module near [cli.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/cli.rs)
2. Or `tests/archival_integration.rs`

**Acceptance Criteria**

1. A dry-run path creates the full daily archive package
2. Missing write permissions or invalid output path causes testable failure
3. No required archival asset is silently skipped

**Dependencies**

P0-1 is helpful but not required

### P0-3: Add ExecutionGate Boundary Matrix Tests

**Goal**

Make the risk gate trustworthy under edge conditions.

**Why**

`ExecutionGate` is now a hard control point for:

1. daily budget
2. total exposure
3. buying power
4. risk overlay

This must be explicitly tested.

**Scope**

1. Add tests for:
   - passes under neutral conditions
   - blocks on `max_daily_budget`
   - blocks on `global_budget`
   - blocks on `buying_power`
   - blocks buy-side trades under defensive/broken regimes
   - handles `config_multiplier` and policy multiplier correctly
2. Validate both:
   - `ExecutionResult.trades`
   - `ExecutionResult.audits`

**Suggested Files**

1. New tests in [execution_gate.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/execution_gate.rs)

**Acceptance Criteria**

1. All blocking reasons are individually tested
2. At least one test validates multiple candidate trades competing for budget
3. Audit payloads are asserted, not just trade count

**Dependencies**

None

### P0-4: Remove Unsafe Runtime Assumptions in Main Pipeline

**Goal**

Replace hidden runtime assumptions with explicit validation.

**Why**

The current pipeline still hard-unwraps trading config:

[cli.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/cli.rs):233

That is acceptable only if validation is enforced before runtime. Today that assumption is implicit.

**Scope**

1. Remove `unwrap()` from the runtime path
2. Add explicit config validation at startup
3. Return typed or at least contextual errors for:
   - missing `[trading]`
   - malformed provider/trading combinations
   - execution mode without required config

**Suggested Files**

1. [config.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/config.rs)
2. [cli.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/cli.rs)

**Acceptance Criteria**

1. No panic path remains for missing trading config
2. Invalid config fails early with a readable error
3. Tests cover invalid config cases

**Dependencies**

None

### P0-5: Enforce a Clean Clippy Baseline

**Goal**

Move from "compiles" to "maintainably clean".

**Why**

`cargo check` is green, but `cargo clippy --all-targets --all-features` still reports avoidable issues.

**Scope**

1. Fix current `clippy` findings that are real quality signals, including:
   - identity map
   - `clone_on_copy`
   - manual clamp
   - `lines().filter_map(Result::ok)`
   - bool assert comparison
   - formatting-related cleanup
2. Do not rename business enums purely to satisfy acronym lint if that hurts the domain language
3. Add targeted `#[allow(...)]` only where the domain naming is intentional

**Suggested Files**

1. [cli.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/cli.rs)
2. [features.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/features.rs)
3. [persistence.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/persistence.rs)
4. [portfolio_policy.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/portfolio_policy.rs)
5. [action_matrix.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/action_matrix.rs)

**Acceptance Criteria**

1. `cargo clippy --all-targets --all-features` passes
2. Any remaining lint allow-list is intentional and documented

**Dependencies**

None

## P1

### P1-1: Slim Down cli.rs

**Goal**

Reduce the orchestration hotspot.

**Why**

`cli.rs` still owns too much:

1. config routing
2. provider selection
3. data fetch fan-out
4. archival orchestration
5. execution context assembly
6. report delivery

**Scope**

1. Extract archival orchestration into a dedicated service/module
2. Extract execution context assembly into a dedicated helper or module
3. Keep `cli` focused on:
   - command routing
   - top-level run mode dispatch

**Suggested Files**

1. [cli.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/cli.rs)
2. New module such as `src/core/runtime_pipeline.rs` or `src/core/archive_service.rs`

**Acceptance Criteria**

1. `cli.rs` becomes materially smaller
2. Main pipeline responsibilities are split into named units
3. Behavior remains unchanged

**Dependencies**

P0-1 and P0-2 should land first

### P1-2: Refactor ActionMatrix API

**Goal**

Make action derivation easier to extend and safer to maintain.

**Why**

`ActionMatrix::decide()` currently takes too many primitive arguments.

**Scope**

1. Replace the current argument list with a typed input struct
2. Group market context, asset context, and execution config separately
3. Preserve current behavior

**Suggested Files**

1. [action_matrix.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/action_matrix.rs)
2. [engine.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/engine.rs)

**Acceptance Criteria**

1. `ActionMatrix::decide()` no longer takes a long primitive argument list
2. Existing behavior remains test-covered
3. New API makes future matrix extension easier

**Dependencies**

P0-1 and P0-3

### P1-3: Optimize Feature Extraction Hotspots

**Goal**

Reduce repeated computation cost in replay-heavy workloads.

**Why**

Feature computation currently performs repeated work for:

1. trend-age derivation
2. long-window percentile estimation

**Scope**

1. Reduce repeated trend recomputation
2. Reuse rolling MA or cached series where practical
3. Revisit percentile computation strategy to avoid repeated full-window scanning

**Suggested Files**

1. [features.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/features.rs)

**Acceptance Criteria**

1. Feature extraction remains behaviorally equivalent
2. Replay runtime improves measurably on the same sample dataset
3. New implementation is covered by tests

**Dependencies**

P0-1

### P1-4: Optimize Backtest Replay Loop

**Goal**

Make backtest scale better with more assets and longer windows.

**Why**

Current backtest repeatedly slices and scans history in ways that will not scale.

**Scope**

1. Reduce repeated cloning of daily slices
2. Pre-index bars by date where useful
3. Avoid repeated linear scans for forward return lookup and drawdown windows

**Suggested Files**

1. [backtest.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/backtest.rs)

**Acceptance Criteria**

1. Backtest runtime is measurably improved on the same dataset
2. Output remains equivalent or differences are explicitly explained
3. No regression in summary generation

**Dependencies**

P1-3 is helpful but not required

### P1-5: Harden Persistence Read Patterns

**Goal**

Reduce fragility in long-lived archival assets.

**Why**

Some persistence patterns are functional but not ideal for long-term reliability.

**Scope**

1. Improve JSONL read behavior for latest-packet loading
2. Add clearer corruption handling for malformed last-line history
3. Define expectations for append-only archive files

**Suggested Files**

1. [persistence.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/persistence.rs)

**Acceptance Criteria**

1. Latest-packet loading handles malformed tails gracefully
2. JSONL append/read semantics are documented
3. Tests cover malformed-history cases

**Dependencies**

P0-2

## P2

### P2-1: Upgrade Telegram Output Template

**Goal**

Increase decision density without making Telegram noisy.

**Why**

Current Telegram output is concise but still closer to a lightweight summary than a high-signal operator brief.

**Scope**

1. Keep the message short
2. Improve information hierarchy:
   - market state
   - portfolio mode
   - top actionable assets
   - key warning if defensive
3. Keep archival markdown richer than Telegram

**Suggested Files**

1. [decision.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/decision.rs)
2. [report.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/report.rs)

**Acceptance Criteria**

1. Telegram remains readable on mobile
2. Message contains clear state + policy + top actions
3. Archival markdown remains the richer artifact

**Dependencies**

P0 complete

### P2-2: Deepen Backtest Reporting

**Goal**

Turn backtest output into a real iteration tool.

**Why**

The current summary is useful, but still too shallow for systematic strategy tuning.

**Scope**

1. Add regime-duration statistics
2. Add action-level attribution
3. Add clearer confidence-bucket reporting
4. Prepare summary format for comparing runs over time

**Suggested Files**

1. [backtest.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/backtest.rs)
2. `backtest/summary.md` generator logic

**Acceptance Criteria**

1. Backtest output supports parameter iteration
2. Summary is easier to compare across runs
3. Attribution is more actionable than simple hit rate

**Dependencies**

P1-4

### P2-3: Turn Data Quality Logs into Diagnostics

**Goal**

Move from passive logging to active data-quality assessment.

**Why**

`data_quality_log.jsonl` exists, but it is still mostly a raw audit trail.

**Scope**

1. Add quality scoring or severity levels
2. Flag stale, sparse, or partial histories
3. Surface data-quality issues in archival or Telegram summaries where appropriate

**Suggested Files**

1. [cli.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/cli.rs)
2. [report.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/report.rs)

**Acceptance Criteria**

1. Data-quality logs become interpretable without manual inspection
2. Severe data-quality problems are visible to the operator

**Dependencies**

P0-2

### P2-4: Add Stronger Execution Observability

**Goal**

Make the execution path auditable under real operational failures.

**Why**

The system now has gating and snapshots, but not a full execution observability loop.

**Scope**

1. Add structured execution-failure taxonomy
2. Add order-status reconciliation hooks
3. Improve reporting of execution success vs broker failure

**Suggested Files**

1. [trader_agent.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/trader_agent.rs)
2. [ledger.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/ledger.rs)
3. [trade/trader.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/trade/trader.rs)
4. Futu adapter modules under `src/adapters/futu/`

**Acceptance Criteria**

1. Order failures are structurally logged
2. Broker-side rejection and local gate rejection are distinguishable
3. Execution outcomes can be audited over time

**Dependencies**

P0-3

## Suggested Execution Order

1. P0-1
2. P0-3
3. P0-2
4. P0-4
5. P0-5
6. P1-1
7. P1-2
8. P1-3
9. P1-4
10. P1-5
11. P2-1
12. P2-2
13. P2-3
14. P2-4

## Definition of Done

This audit issue list should be considered complete only when:

1. `P0` is green in code, tests, and lint
2. `P1` removes the main maintainability and replay bottlenecks
3. `P2` improves operator quality without destabilizing the core pipeline
