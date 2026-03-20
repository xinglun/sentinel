# Project Audit

## Scope
This audit evaluates the current Sentinel project across five dimensions:

1. Technical framework rationality
2. Code quality
3. Test coverage
4. Performance
5. Functional completeness

The goal is not to restate the roadmap. The goal is to identify what is already structurally sound, what is still fragile, and what should be fixed next.

## Executive Summary

### Overall Assessment

| Dimension | Assessment | Notes |
|---|---|---|
| Technical framework | Good | Four-layer architecture is now recognizable and mostly coherent |
| Code quality | Fair to good | `cargo check` and `cargo test` are green, but `clippy` still shows avoidable debt |
| Test coverage | Moderate at best | Unit tests exist, but integration and workflow coverage are thin |
| Performance | Adequate for daily use | Daily radar is likely IO-bound; backtest is the clear compute bottleneck |
| Functional completeness | Strong for decision support | Daily Telegram + archival + backtest loop is in place |

### Current Position

Sentinel has already crossed the line from "analysis script" into "decision system prototype".

It is suitable now for:

1. Daily market observation
2. Telegram delivery
3. Persistent archival to `reports/` and the `data` branch
4. Small to medium strategy iteration loops

It is not yet at the standard of a high-reliability automated trading production system.

## Detailed Evaluation

### 1. Technical Framework Rationality

The framework is directionally correct.

Strengths:

1. `DecisionPacket` is the system boundary and source of truth, which is the right center of gravity.
2. The application is logically split into:
   - Shell / orchestration
   - Decision kernel
   - Delivery and audit
   - Execution adapters
3. The dual-provider model is pragmatic:
   - Yahoo for CI and stateless scheduled runs
   - Futu for local daemon and execution contexts
4. Daily archival and GitHub Actions are now aligned with the runtime outputs.

Weaknesses:

1. `src/cli.rs` still carries too much orchestration, execution-context assembly, archival, and reporting flow.
2. Some rule semantics still live in implementation-heavy modules instead of cleaner domain contracts.
3. Backtest shares the engine, which is correct, but the evaluation layer is still simplistic.

Assessment: `8/10`

### 2. Code Quality

The codebase is materially better structured than a typical prototype, but not yet "strictly clean".

Strengths:

1. Modules have clear intent.
2. Runtime state, regime state, policy, action mapping, and persistence are separated.
3. The code compiles and tests pass.

Weaknesses:

1. `cargo clippy --all-targets --all-features` still reports several warnings.
2. There are still signs of tactical patching and uneven flow structure.
3. Some invariants are enforced by assumption instead of explicit type or validation boundaries.

Examples:

1. `trading` is still hard-unwrapped in the main pipeline:
   [cli.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/cli.rs):233
2. `ActionMatrix::decide()` still has too many raw arguments:
   [action_matrix.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/action_matrix.rs):39
3. Persistence still uses `filter_map(Result::ok).last()` on lines input:
   [persistence.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/persistence.rs):45

Assessment: `7/10`

### 3. Test Coverage

Coverage is enough to protect local refactors, but not enough to certify production behavior.

Covered:

1. Config parsing
2. Basic regime transitions
3. Selected action-matrix behavior
4. Asset-state rules
5. Persistence roundtrip
6. Basic trader dispatch

Gaps:

1. No end-to-end test for `Engine::run_daily_pipeline()`
2. No integration test for the full archival package
3. No systematic budget/gate boundary testing
4. No workflow-equivalent output validation test
5. No adapter contract test for Futu execution

Assessment: `5.5/10`

### 4. Performance

There is no formal benchmark suite yet, so only structural performance assessment is possible.

Current judgment:

1. Daily radar is likely acceptable because the critical path is dominated by external market-data IO.
2. Backtest is the main performance concern.
3. Feature extraction has repeated calculations that will compound under replay workloads.

Notable hotspots:

1. Repeated trend recomputation for `trend_age`:
   [features.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/features.rs):194
2. Historical percentile computation over long windows:
   [features.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/features.rs):212
3. Daily history slicing and repeated forward-window scans in backtest:
   [backtest.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/backtest.rs):72

Assessment: `6/10`

### 5. Functional Evaluation

From a decision-support perspective, functionality is already strong.

Working capabilities:

1. Multi-provider market data
2. Market regime state machine
3. Asset execution state machine
4. Action matrix
5. Portfolio risk gating
6. Telegram delivery
7. Daily archival package
8. Weekly backtest calibration

Remaining limitations:

1. Telegram remains a compact summary, not a dense research-grade daily brief.
2. Backtest output is useful but still shallow for parameter experiments and attribution.
3. Execution reliability is not yet at the standard of production trade operations.
4. Data-quality logging exists, but there is no automatic quality scoring or escalation loop.

Assessment: `8/10`

## Risk Register

### Operational Risks

1. CI success still depends on provider availability, network stability, and secrets correctness.
2. Futu execution remains environment-sensitive and should not be treated as fully hardened trade infrastructure.

### Engineering Risks

1. `cli.rs` remains a concentration point for future complexity.
2. Backtest performance will degrade as the watchlist or history window grows.
3. Incomplete integration test coverage raises regression risk in archival and execution paths.

## Remediation Plan

The next wave of work should be managed in three bands: `P0`, `P1`, `P2`.

### P0

These are the highest-value items because they reduce production risk directly.

1. Add integration tests for the full decision pipeline
   - Target: `Engine::run_daily_pipeline()`
   - Validate: feature extraction, regime transition, policy derivation, action matrix output

2. Add integration tests for the archival package
   - Validate that a successful dry-run produces the expected daily files under `save_to`
   - Validate failure behavior when required archival writes fail

3. Add execution-gate boundary tests
   - Cases:
     - `max_daily_budget`
     - `global_budget`
     - `buying_power`
     - circuit-breaker / defensive overlay
   - Validate both accepted trades and blocked trades

4. Remove unsafe runtime assumptions from the main pipeline
   - Replace hard `unwrap()` on trading config with explicit validation and typed failure
   - Current risk point:
     [cli.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/cli.rs):233

5. Establish a strict lint baseline
   - `cargo clippy --all-targets --all-features` should be green
   - This is important because `cargo check` green is not enough for long-term maintainability

#### P0 Acceptance

1. `cargo check`, `cargo test`, and `cargo clippy --all-targets --all-features` all pass
2. Full pipeline integration tests exist
3. Archival package completeness is test-covered
4. Budget and exposure gates are boundary-tested

### P1

These items improve maintainability and replay scalability.

1. Slim down `src/cli.rs`
   - Extract archive orchestration
   - Extract execution-context assembly
   - Keep `cli` focused on argument routing and high-level pipeline assembly

2. Refactor `ActionMatrix::decide()`
   - Replace the current many-argument function with a single typed input struct
   - Reduce raw primitive coupling

3. Optimize feature extraction hotspots
   - Reduce repeated trend recomputation
   - Reuse rolling computations where possible
   - Revisit percentile computation strategy for replay workloads

4. Optimize backtest slicing and forward-return evaluation
   - Avoid repeated full-history cloning
   - Pre-index bars by date
   - Reduce repeated linear scans

5. Improve persistence robustness
   - Replace line iteration patterns that are fragile under IO errors
   - Add explicit schema expectations for long-lived JSONL assets

#### P1 Acceptance

1. `cli.rs` is materially smaller and narrower in responsibility
2. Backtest runtime improves measurably on the same input window
3. Feature extraction cost drops under replay load
4. Action-matrix API is easier to maintain and extend

### P2

These items improve operator usability and research depth.

1. Upgrade Telegram output
   - Preserve brevity
   - Increase information density
   - Include clearer state, policy, and top asset-action explanations

2. Strengthen backtest reporting
   - Add comparative experiment outputs
   - Add regime-duration summaries
   - Add action-level attribution views

3. Add quality scoring on data ingestion
   - Turn `data_quality_log.jsonl` from passive logging into active diagnostics

4. Add stronger execution observability
   - Structured order-status reconciliation
   - Failure taxonomy
   - Retry/abort reporting

#### P2 Acceptance

1. Telegram is more informative without becoming noisy
2. Backtest output supports real parameter iteration, not just narrative review
3. Data quality can be judged systematically
4. Execution failures are observable and classifiable

## Recommended Sequence

1. Finish `P0` before adding new strategy complexity
2. Move to `P1` before scaling backtest or watchlist breadth
3. Use `P2` to improve operator experience and research productivity

## Final Judgment

Sentinel is in a strong transitional state:

1. The architecture is largely correct
2. The core functionality is already useful
3. The archival and delivery loops are effectively in place
4. The next bottlenecks are not product gaps, but engineering hardening gaps

The project should now be managed as a hardening-and-scaling effort, not as a greenfield strategy rewrite.
