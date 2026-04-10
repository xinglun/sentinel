# Trend Cohesion Topology Task

## Goal

Enhance the existing `Trend Cohesion Gate` with an explicit structural topology layer so the
system can distinguish:

- `NO_LEADER`
- `SINGLE_LEADER`
- `FRAGMENTED_LEADERS`

This is not a new gate. The gate already exists.
This round adds structural interpretation on top of the existing gate so the system can explain
whether the current market lacks leadership entirely, has a single emerging leader, or contains
multiple competing leaders.

## Problem

The current system already tells the user:

- whether participation is allowed
- whether the primary trend is formed
- which gate conditions are still unmet

But it still does not explicitly distinguish between:

1. no credible leadership at all
2. one clear leader beginning to emerge
3. multiple leaders competing without convergence

Those are all different trading situations, even when the final outcome is still `NO TRADE`.

## Scope

This round should:

1. add a structured topology enum to the domain layer
2. store it in `TrendCohesionSnapshot`
3. localize it in `PresentationAssembler`
4. render it in the report

This round must not:

- redefine `Trend Cohesion Gate`
- turn topology into a trading action
- mix topology with bullish/bearish direction
- recompute topology inside `report.rs`

## Required Domain Change

In `src/core/trend_cohesion.rs`, add:

- `TrendCohesionTopology`
  - `NoLeader`
  - `SingleLeader`
  - `FragmentedLeaders`

And add it to `TrendCohesionSnapshot`.

## Required Rule Semantics

Topology must answer:

- Is there no leader?
- Is there one dominant emerging leader?
- Are there multiple leaders competing without convergence?

Suggested signal inputs:

- `leader_count`
- `candidate_count`
- `leader_quality_score`
- `rotation_quality_score`
- `candidate_compactness_score`

### Hard semantic rule

`TrendCohesionTopology` must not be treated as:

- a market direction label
- a risk regime
- a trading permission

It is a structural description only.

## Presentation Requirements

`PresentationAssembler` must generate:

- `主线结构 / Trend Topology / 主線構造`
- localized value:
  - `无主线 / No Leader / 主導不在`
  - `单主线 / Single Leader / 単一主導`
  - `多主线分散 / Fragmented Leaders / 多主導分散`

`report.rs` must render the topology alongside the existing `Primary Trend` status.

## Acceptance Criteria

The task is complete only if:

1. `TrendCohesionSnapshot` carries a structured topology enum.
2. The system explicitly distinguishes:
   - no leader
   - single leader
   - fragmented leaders
3. The rendered report shows the topology label and value.
4. Topology remains independent from:
   - `NO TRADE`
   - `DEFENSIVE`
   - bullish/bearish direction
5. `report.rs` remains render-only.

## Required Tests

At minimum, add:

1. `trend_cohesion` unit tests for:
   - no leader
   - single leader
   - fragmented leaders
2. `presentation_tests` for localized topology labels/values
3. `report_ui_tests` for final rendered topology output
4. multi-language regression coverage

## Quality Gates

All of the following must pass:

- `cargo fmt`
- `cargo test --quiet`
- `cargo clippy --all-targets --all-features -- -D warnings`

## Hard Review Standard

This task is not accepted just because the report shows one more line.

It is accepted only if the system gains a real ability to distinguish:

- there is nothing to follow
- one leader is emerging
- multiple leaders are competing

If the report still collapses all of those states into a generic “not formed”, the task is not
complete.
