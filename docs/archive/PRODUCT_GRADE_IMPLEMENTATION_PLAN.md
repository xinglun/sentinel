# Product-Grade Implementation Plan

## Purpose

This document refines the product-grade audit tasks into implementation-level guidance.

It is intended to answer four questions for each priority band:

1. what to change
2. where to change it
3. how to structure the change
4. how to verify it

This is not a roadmap deck. It is a build plan for engineers.

## Guiding Principles

1. Prefer explicit runtime contracts over implicit assumptions
2. Prefer typed boundaries over primitive argument drift
3. Prefer one source of truth for archive contracts and config semantics
4. Prefer structured failure outcomes over log-only failure handling

## P0

### P0-1: Make `trading.enabled` a Real Hard Kill Switch

#### Target State

When `[trading].enabled = false`:

1. decision pipeline still runs
2. archive package still writes
3. Telegram may still send
4. no live order dispatch happens
5. audit trail explicitly records that execution was disabled

#### Current Problem

The runtime distinguishes `radar` vs `daemon`, but not strongly enough between:

1. live mode
2. dry-run mode
3. disabled-trading mode

#### Design Change

Introduce an explicit execution mode model.

Suggested enum:

```rust
pub enum ExecutionMode {
    Disabled,
    DryRun,
    Live,
}
```

#### Implementation

1. Add `ExecutionMode` in a runtime-facing module such as:
   - `src/core/runtime_mode.rs`
   - or inside `src/cli.rs` if kept local initially
2. Derive it from:
   - CLI command
   - `trading.enabled`
   - selected provider
3. Replace boolean `execute_trades` branching with `ExecutionMode`
4. Pass this mode through the main pipeline and include it in:
   - audit logs
   - run status
   - optional report metadata

#### Concrete Code Changes

1. [cli.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/cli.rs)
   - replace `execute_trades: bool`
   - resolve `ExecutionMode` once near startup
   - block `TraderAgent::execute_signals()` unless mode is `Live`
2. [execution_gate.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/execution_gate.rs)
   - add audit reason `TradingDisabled`
   - optionally include mode in `GatedAudit.details`
3. [trader_agent.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/trader_agent.rs)
   - assume caller has already authorized live execution
   - do not own policy decisions

#### Validation

1. Add one integration test for `daemon + trading.enabled = false`
2. Assert:
   - no placed orders
   - audits written
   - report still generated

---

### P0-2: Eliminate Config-Behavior Drift

#### Target State

Every config field must be one of:

1. used by runtime
2. explicitly rejected
3. documented as reserved and not user-settable

Nothing else.

#### Current Problem

The repo currently mixes:

1. active config fields
2. legacy fields still present in docs or sample config
3. fields silently ignored by `serde`

This is product-dangerous because operators will trust settings that do nothing.

#### Design Change

Adopt a strict schema boundary.

Recommended approach:

1. Add `#[serde(deny_unknown_fields)]` to top-level config structs that represent user-edited config
2. Remove legacy fields from `config.toml`
3. Update docs to only describe live fields

If a legacy field must be preserved temporarily, parse it explicitly and mark it deprecated.

#### Implementation

1. Audit runtime-consumed fields:
   - `AppConfig`
   - `OutputConfig`
   - `RulesConfig`
   - `WatchlistEntry`
2. Build a config-field matrix:
   - field name
   - declared in code
   - present in `config.toml`
   - referenced in runtime
   - documented in PRD
3. Decide field-by-field:
   - restore behavior
   - delete field
   - deprecate field with explicit startup error

#### Concrete Code Changes

1. [config.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/config.rs)
   - add strict serde handling where appropriate
   - add post-parse validation for mutually dependent fields
2. [config.toml](/Users/sei-rinn/dev/workspace_rust/sentinel/config.toml)
   - remove no-op fields
   - or align to real runtime support
3. [PRD.md](/Users/sei-rinn/dev/workspace_rust/sentinel/docs/specs/PRD.md)
4. [IMPLEMENTATION_WALKTHROUGH.md](/Users/sei-rinn/dev/workspace_rust/sentinel/docs/architecture/IMPLEMENTATION_WALKTHROUGH.md)

#### Suggested Output Artifact

Create a one-time internal matrix:

`docs/CONFIG_FIELD_MATRIX.md`

Columns:

1. field
2. config
3. runtime
4. docs
5. action

#### Validation

1. Unknown config field should fail parse if not supported
2. Docs and sample config should not mention dead fields
3. Add tests for:
   - unknown field rejection
   - missing required field rejection
   - deprecated field rejection if chosen

---

### P0-3: Upgrade `telemetry.csv` into a Trustworthy Research Contract

#### Target State

`telemetry.csv` must have:

1. a documented schema
2. stable semantics
3. enough context for longitudinal research

#### Current Problem

Today the file exists and is workflow-enforced, but its semantic contract is underdefined and narrower than the research promise around it.

#### Design Change

Promote telemetry from “convenience append” to “declared schema”.

Two options exist:

1. minimal telemetry, narrow docs
2. research telemetry, richer output

For product-grade operation, the second option is recommended.

#### Recommended Schema

Suggested columns:

1. `timestamp`
2. `date`
3. `provider`
4. `market_state`
5. `risk_overlay`
6. `system_confidence`
7. `stability_score`
8. `dominance_margin`
9. `potential_energy`
10. `regime_age`
11. `up_count`
12. `flat_count`
13. `down_count`
14. `total_count`
15. `up_weight`
16. `flat_weight`
17. `down_weight`
18. `total_weight`
19. `config_hash`
20. `data_quality_status`

If CSV width is a concern, the minimal acceptable addition is:

1. `timestamp`
2. `market_state`
3. `risk_overlay`
4. `config_hash`

#### Implementation

1. Introduce a telemetry row struct:

```rust
#[derive(Serialize)]
pub struct TelemetryRow {
    ...
}
```

2. Build the row from:
   - `DecisionPacket`
   - runtime context
   - config hash
   - provider type
3. Move CSV header definition into one place
4. Make workflow validation aware of the finalized schema only if lightweight inspection is acceptable

#### Concrete Code Changes

1. [persistence.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/persistence.rs)
   - replace raw inline formatting with typed row serialization
2. [cli.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/cli.rs)
   - pass provider/mode/config hash inputs
3. optionally add:
   - `src/core/telemetry.rs`

#### Validation

1. Telemetry header is deterministic
2. Added fields are non-empty when expected
3. Historical append behavior remains correct
4. One test verifies schema header and one row content

---

### P0-4: Stop Swallowing Core Failures

#### Target State

A completed run must have an explicit outcome summary for:

1. decisioning
2. archival
3. notification
4. execution

#### Current Problem

The system still logs and ignores certain failures.

That is acceptable for developer tooling, but not for product-grade operation.

#### Design Change

Introduce structured run outcomes.

Suggested types:

```rust
pub enum DeliveryStatus {
    Succeeded,
    Failed { reason: String },
}

pub struct RunOutcome {
    pub decisioning: DeliveryStatus,
    pub archival: DeliveryStatus,
    pub notification: DeliveryStatus,
    pub execution: DeliveryStatus,
}
```

#### Implementation

1. Treat archival failure as fatal
2. Treat notification failure as non-fatal but persistent
3. Treat execution failure as non-fatal to archival, but persistent and visible
4. Write a structured run status artifact

Suggested artifact:

`run_status_[DATE].json`

or

`run_status.jsonl`

#### Concrete Code Changes

1. [cli.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/cli.rs)
   - collect stage results
   - stop ignoring Telegram failures
2. [trader_agent.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/trader_agent.rs)
   - return per-trade result summary, not just `Result<()>`
3. [persistence.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/persistence.rs)
   - add `save_run_status(...)`

#### Validation

1. Notification failure becomes visible in an artifact
2. Trade placement failure becomes visible in an artifact
3. Run result can be audited post hoc without reading raw console output

## P1

### P1-1: Add True Runtime Integration Tests

#### Target State

At least one test should exercise the real dry-run runtime path, not just persistence helpers.

#### Design Change

Create a test-only runtime harness that injects:

1. fixture provider
2. temp output dir
3. deterministic config

#### Implementation

1. Extract the runtime body behind a testable function if needed:

```rust
async fn run_pipeline_with_context(...)
```

2. Inject fake provider data
3. Assert on real produced files

#### Concrete Code Changes

1. [cli.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/cli.rs)
2. `tests/runtime_pipeline_integration.rs`

#### Validation

1. One dry-run runtime test
2. One disabled-trading daemon test
3. One failure-path test for archival failure if feasible

---

### P1-2: Add a Shared Archive Contract Source

#### Target State

The required daily asset list should be defined once and reused everywhere.

#### Design Change

Create a shared constant or contract file.

Suggested options:

1. Rust constant list used by tests
2. JSON manifest under `docs/` or `scripts/`
3. shell-readable file used by workflow

Recommended:

`scripts/archive_contract.txt`

One line per required artifact pattern.

#### Implementation

1. Update:
   - workflow validation
   - integration test
   - operator docs
2. Reduce copy-pasted required file lists

#### Validation

1. Daily workflow and tests use the same required asset set
2. Contract changes happen in one place

---

### P1-3: Strengthen Execution Safety Semantics

#### Target State

Every non-executed trade candidate should have an auditable reason.

#### Design Change

Extend gate and execution reason taxonomy.

Suggested reason set:

1. `TradingDisabled`
2. `ProviderNotLiveCapable`
3. `MissingCredentials`
4. `CircuitBreaker`
5. `DailyBudget`
6. `GlobalExposure`
7. `BuyingPower`
8. `QuantityRounding`
9. `BrokerReject`
10. `TransportError`

#### Implementation

1. Formalize reason enum or string constants
2. Use the same taxonomy in:
   - gate audits
   - execution logs
   - run status

#### Validation

1. At least one test per new reason category where applicable
2. Audit logs become easier to aggregate historically

## P2

### P2-1: Upgrade Telegram into Product-Grade Operator Messaging

#### Target State

Telegram should answer, at a glance:

1. what state the market is in
2. what the portfolio posture is
3. what the top actionable names are
4. whether there is a warning condition

#### Design Change

Separate message composition from packet construction.

Do not keep Telegram summary logic buried inside `DecisionPacket::new()`.

Recommended split:

1. `DecisionPacket` stores factual state
2. `report.rs` or a dedicated `telegram.rs` renders operator messaging

#### Implementation

Suggested message sections:

1. headline
2. market state + overlay
3. portfolio mode + target exposure
4. top 3 actionable symbols
5. warning line if defensive or data stale

#### Validation

1. Snapshot test for Telegram rendering
2. Mobile-readable output under common cases

---

### P2-2: Add Product-Facing Run Status

#### Target State

Operators should be able to answer “did the system really complete correctly today?” from one artifact.

#### Design Change

Persist one concise run-status object per run.

Suggested schema:

```json
{
  "date": "2026-03-20",
  "decisioning": "ok",
  "archival": "ok",
  "notification": "failed",
  "execution": "skipped",
  "warnings": ["telegram_send_failed"]
}
```

#### Implementation

1. Define a typed struct
2. Save after the pipeline completes or aborts
3. Include it in operator docs, not necessarily in daily workflow gating

#### Validation

1. One file exists per run
2. Failures are visible without opening raw logs

## Recommended Delivery Sequence

### Phase A

1. P0-1
2. P0-2

Reason:

These are semantic trust issues. Fix them before adding more observability.

### Phase B

1. P0-3
2. P0-4

Reason:

Once config and execution semantics are trustworthy, make the data contract and failure semantics trustworthy too.

### Phase C

1. P1-1
2. P1-2
3. P1-3

Reason:

Now strengthen runtime protection and reduce future drift.

### Phase D

1. P2-1
2. P2-2

Reason:

Only after the core system is trustworthy should operator-facing polish be expanded.

## Final Definition of Done

This implementation plan is complete only when:

1. execution can be globally disabled with zero ambiguity
2. config and docs no longer lie to the operator
3. telemetry is a stable research contract
4. failures are structured and reviewable
5. runtime integration tests cover the real product path
