# Product-Grade Audit Tasks

## Purpose

This document translates the latest product-grade audit into executable engineering tasks.

The standard here is stricter than code-green or feature-complete.  
The target is product-grade behavior:

1. configuration semantics must be trustworthy
2. execution safety switches must actually work
3. core delivery failures must be visible and actionable
4. research data contracts must match what is really persisted
5. integration tests must validate real runtime behavior, not only isolated modules

## Severity Model

### P0

Product-blocking issues. These must be fixed before calling the system product-grade.

### P1

High-value hardening items. These materially improve operational reliability and long-term maintainability.

### P2

Non-blocking improvements that increase operator confidence and product consistency.

## P0 Tasks

### P0-1: Make `trading.enabled` a Real Hard Kill Switch

**Problem**

`daemon` mode currently enters the execution path even when `[trading].enabled = false`.

This is a product-grade blocker. A disabled trading switch must guarantee no live execution.

**Current Risk Points**

1. [cli.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/cli.rs):85
2. [cli.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/cli.rs):291
3. [execution_gate.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/execution_gate.rs):39
4. [config.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/config.rs):54

**Task**

1. Enforce `trading.enabled` before any live execution path
2. Ensure `daemon` mode can still produce audits and reports while blocking order dispatch
3. Make the runtime behavior explicit in logs and report artifacts

**Implementation Guidance**

1. Gate `execute_trades` with both:
   - run mode
   - `trading.enabled`
2. Ensure Futu execution objects are not used for live order placement when disabled
3. Add structured audit reason such as `TradingDisabled`

**Acceptance Criteria**

1. With `[trading].enabled = false`, no orders are sent in `daemon` mode
2. Audits and daily archival still complete
3. A dedicated test proves no execution occurs when disabled
4. A log or audit trail explicitly records that trading was disabled

### P0-2: Eliminate Config-Behavior Drift

**Problem**

The repository still contains config and documentation fields that are not consumed by runtime logic.  
This creates silent misconfiguration and false operator confidence.

**Examples**

1. `include_summary`
2. `caution_ma_days`
3. `watchlist.action_overrides`
4. legacy `bear_mode` semantics claimed in docs but not enforced in the runtime path

**Current Drift Surface**

1. [config.toml](/Users/sei-rinn/dev/workspace_rust/sentinel/config.toml):7
2. [config.toml](/Users/sei-rinn/dev/workspace_rust/sentinel/config.toml):87
3. [config.toml](/Users/sei-rinn/dev/workspace_rust/sentinel/config.toml):142
4. [PRD.md](/Users/sei-rinn/dev/workspace_rust/sentinel/docs/specs/PRD.md):113
5. [PRD.md](/Users/sei-rinn/dev/workspace_rust/sentinel/docs/specs/PRD.md):124
6. [PRD.md](/Users/sei-rinn/dev/workspace_rust/sentinel/docs/specs/PRD.md):132
7. [config.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/config.rs):21
8. [config.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/config.rs):95

**Task**

Choose one of two strategies and apply it consistently:

1. Restore these fields into actual runtime behavior
2. Remove them from config and docs, and fail fast on unknown fields

**Implementation Guidance**

1. Add `deny_unknown_fields` where appropriate if the intent is strict config control
2. Align:
   - `config.toml`
   - `src/config.rs`
   - `docs/specs/PRD.md`
   - `docs/architecture/IMPLEMENTATION_WALKTHROUGH.md`
3. Do not keep “documented but ignored” fields

**Acceptance Criteria**

1. Every documented config field is either:
   - consumed by runtime
   - or rejected at parse time
2. `config.toml` no longer contains silent no-op fields
3. PRD and runtime config schema match
4. Tests cover invalid/unknown config cases

### P0-3: Upgrade `telemetry.csv` to a Trustworthy Research Contract

**Problem**

`telemetry.csv` is treated as a core long-term research asset, but its current schema is narrower than the documented and implied product contract.

This creates a research integrity problem.

**Current Mismatch**

1. Runtime only writes a compact feature subset:
   [persistence.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/persistence.rs):115
2. Workflow enforces the file as a required daily artifact:
   [daily_radar.yml](/Users/sei-rinn/dev/workspace_rust/sentinel/.github/workflows/daily_radar.yml):117
3. Docs describe a richer telemetry role:
   [PRD.md](/Users/sei-rinn/dev/workspace_rust/sentinel/docs/specs/PRD.md):88
   [PRD.md](/Users/sei-rinn/dev/workspace_rust/sentinel/docs/specs/PRD.md):95

**Task**

Define the telemetry contract explicitly and enforce it.

**Required Decision**

Choose one:

1. Keep telemetry minimal and update docs/workflows to match
2. Expand telemetry to include the research-grade fields actually promised

**Minimum Recommended Fields**

1. `timestamp`
2. `date`
3. `state_code`
4. `risk_overlay`
5. `system_confidence`
6. `stability_score`
7. `dominance_margin`
8. `regime_age`
9. `config_hash`
10. provider or data-quality context

**Acceptance Criteria**

1. Telemetry schema is documented in one source of truth
2. Runtime output matches the documented schema exactly
3. Workflow validates the same contract
4. Historical analysis requirements are supportable from the file

### P0-4: Stop Swallowing Core Delivery and Execution Failures

**Problem**

Core failures are still being logged and ignored instead of being turned into structured outcomes.

Examples:

1. Telegram send failure is ignored
2. Individual trade placement failure does not propagate as an execution failure state

**Current Risk Points**

1. [cli.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/cli.rs):320
2. [trader_agent.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/trader_agent.rs):72
3. [trader_agent.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/trader_agent.rs):92

**Task**

1. Replace silent failure swallowing with structured error handling
2. Classify failures into:
   - archival failure
   - notification failure
   - execution rejection
   - execution transport failure
3. Decide which failures are fatal and which are recoverable

**Implementation Guidance**

1. Telegram failure may remain non-fatal, but must be explicitly logged in a structured way
2. Execution failures must be persisted as structured outcomes
3. The final run result should clearly distinguish:
   - successful decision generation
   - successful archival
   - successful notification
   - successful execution

**Acceptance Criteria**

1. No core failure is silently discarded
2. Failure classes are observable in logs and/or audit assets
3. Operator can tell whether a run failed in decisioning, archival, notification, or execution

## P1 Tasks

### P1-1: Add True Runtime Integration Tests

**Problem**

Current integration tests are useful, but they still stop short of exercising the real runtime path end-to-end.

**Current Gap**

`archival_integration` uses persistence primitives and `TransitionLogger`, but does not drive the actual main runtime flow.

Relevant file:

[archival_integration.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/tests/archival_integration.rs):34

**Task**

1. Add one or more tests that exercise a realistic pipeline entry path
2. Verify:
   - decision packet generation
   - transition logging
   - telemetry writing
   - archival package generation
3. Use deterministic fixture providers rather than live network data

**Acceptance Criteria**

1. At least one runtime-level dry-run test exists
2. The test validates the real archival outputs created by the runtime path
3. Transition artifacts are created via the same flow used in production

### P1-2: Add Workflow Contract Tests or Validation Scripts

**Problem**

GitHub workflow behavior is currently validated mostly by reading YAML and by CI runtime behavior, not by testable contract checks in the repository.

**Task**

1. Add a validation script or test helper that asserts the daily required asset list
2. Reuse the same source of truth for:
   - workflow validation
   - local archival integration test
   - operator docs

**Acceptance Criteria**

1. Required daily assets are defined in one place
2. Both test code and workflow validation consume the same list
3. Drift between CI and runtime archive contract becomes harder

### P1-3: Strengthen Execution Safety Semantics

**Problem**

Execution gating is materially better than before, but product-grade safety needs one more pass.

**Task**

1. Add explicit audit reasons for:
   - trading disabled
   - provider not live-capable
   - missing Futu credentials
   - unsupported execution mode
2. Ensure live execution cannot happen under ambiguous states
3. Confirm that simulated and real execution are unambiguously distinguishable in logs

**Acceptance Criteria**

1. Every non-executed trade candidate has a clear reason
2. Live vs mock vs disabled execution mode is explicit
3. Operator can audit why no order was sent

## P2 Tasks

### P2-1: Upgrade Telegram to Product-Grade Operator Messaging

**Problem**

Telegram delivery is functional, but still too thin for a product-grade operator experience.

**Current Limitation**

The message is still a compact state summary with limited operational detail.

Relevant files:

1. [decision.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/decision.rs):39
2. [report.rs](/Users/sei-rinn/dev/workspace_rust/sentinel/src/core/report.rs):121

**Task**

Improve Telegram content while preserving brevity.

**Minimum Desired Fields**

1. market state
2. risk mode
3. top actionable assets
4. one-line execution posture
5. data-quality or stale warning if relevant

**Acceptance Criteria**

1. Telegram stays readable on mobile
2. Message is more actionable than a generic summary
3. Archive markdown remains the richer artifact

### P2-2: Add Product-Facing Failure Reporting

**Problem**

There is still no single operator-facing run status artifact.

**Task**

Add a compact run-status summary that records:

1. pipeline success
2. archival success
3. notification success
4. execution success
5. data-quality warnings

Possible forms:

1. `run_status_[DATE].json`
2. one appended `run_status.jsonl`

**Acceptance Criteria**

1. A single run status artifact exists
2. Operators can review system health without reading raw logs
3. Failures become easier to analyze historically

## Suggested Execution Order

1. P0-1
2. P0-2
3. P0-3
4. P0-4
5. P1-1
6. P1-2
7. P1-3
8. P2-1
9. P2-2

## Definition of Done

This product-grade hardening pass should be considered complete only when:

1. trading safety switches are trustworthy
2. config semantics and docs no longer drift
3. telemetry is a real research contract
4. core failures are structured and observable
5. integration tests validate the actual runtime path
