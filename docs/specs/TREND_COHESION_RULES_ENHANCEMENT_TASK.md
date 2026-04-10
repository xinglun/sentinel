# Trend Cohesion Rules Enhancement Task

## Goal

Upgrade the current `Trend Cohesion Gate` from a static threshold gate into a more realistic
"leadership consensus" evaluator.

This round is not about changing `NO TRADE`, `Participation Gate`, or `Exit Gate`.
It is specifically about improving the quality of the underlying `Trend Cohesion` rules so the
system can better distinguish:

- random candidate dispersion
- weak early attempts
- real emerging leadership
- genuinely cohesive primary trends

The target outcome is:

1. `Trend Cohesion` keeps its current role as an independent third gate.
2. Its internal rule quality improves from simple thresholding to a more market-realistic
   structure assessment.
3. The report can explain not only that the primary trend is absent, but also why it is weak,
   fragmented, or becoming coherent.

## Current Limitation

The current implementation already evaluates:

- stability
- continuity streak
- candidate count
- repeated leaders
- continuity quality

This is good enough for v1/v2 safety, but it still has several limitations:

1. It does not distinguish "repeating" leaders from "dominant" leaders.
2. It treats candidate compactness as a proxy for cohesion, but does not explicitly measure
   leadership concentration quality.
3. It does not separate healthy rotation from unstable churn strongly enough.
4. It can say `NotFormed`, `Forming`, or `Cohesive`, but the underlying score model is still too
   coarse for future evolution.

## Scope

This round should introduce a stronger rule system while preserving the current architecture:

- domain layer computes structure
- `DecisionPacket` stores structured output
- `PresentationAssembler` localizes and summarizes it
- `report.rs` only renders

Do not:

- collapse `Trend Cohesion` back into `Participation Gate`
- use report-layer heuristics
- reintroduce free-text reasons in the domain layer
- expand this task into sector/theme classification unless strictly necessary for the chosen score

## Implementation Plan

### 1. Add a structured score model

In `src/core/trend_cohesion.rs`, extend the snapshot with:

- `cohesion_score: f64`
- `leader_quality_score: f64`
- `rotation_quality_score: f64`
- `candidate_compactness_score: f64`

These should be domain metrics, not presentation strings.

The purpose is not to create a black-box score, but to expose the components that determine
whether leadership is real or noisy.

### 2. Strengthen leadership evaluation

Add a stricter `Leader Quality` concept.

At minimum, distinguish:

- repeated leaders that merely reappear
- leaders that repeatedly occupy the top cohort and dominate a compact candidate pool

Expected direction:

- higher score if a small subset of names persists across the recent window
- lower score if today’s top tier is large, unstable, or leader identity keeps changing

### 3. Strengthen rotation/churn evaluation

Current continuity quality is based on overlap and coarse churn.

Enhance it so the model can distinguish:

- healthy rotation inside a still-coherent structure
- unstable reshuffling where no leadership remains intact

This should remain deterministic and explainable.

### 4. Strengthen candidate compactness

Candidate count alone is too blunt.

Add a better compactness measure that reflects whether the candidate pool is:

- tight and leadership-driven
- moderate but still readable
- wide and noisy

### 5. Derive final status from score + hard conditions

Keep the 3 final states:

- `NotFormed`
- `Forming`
- `Cohesive`

But derive them from:

- hard minimum conditions
- structural sub-scores
- final cohesion score

Recommended design:

- hard fail conditions still prevent `Cohesive`
- `Forming` should represent "some structure exists, but not enough to trust"
- `Cohesive` should mean "leadership is compact, repeated, and stable enough to follow"

### 6. Preserve structured reason codes

Do not return free text from the domain layer.

If new reasons are needed, extend the condition enum with structured variants and localize them in
`i18n.rs`.

### 7. Presentation layer output

`PresentationAssembler` should continue to provide:

- `Primary Trend`
- current status
- formation conditions
- unmet conditions

Additionally, if useful and low-noise, it may expose one short summary of the structural quality,
for example:

- "leaders are repeating but still fragmented"
- "candidate pool remains too dispersed"

This must still be driven by structured fields, not ad hoc strings.

## Guardrails

The following must remain true after the enhancement:

1. `Trend Cohesion` must remain independent from `Participation Gate`.
2. `Trend Cohesion` must not become synonymous with:
   - bullishness
   - defensiveness
   - `NO TRADE`
3. `NO TRADE` may still happen when `Trend Cohesion` is weak, but the two systems must remain
   conceptually separate.
4. `report.rs` must not compute any cohesion logic.
5. Domain reasons must remain structured and localizable.

## Acceptance Criteria

This round is complete only if all of the following are true:

1. `Trend CohesionSnapshot` contains richer structural metrics than the current v2 version.
2. The system can distinguish:
   - highly dispersed noise
   - partially forming leadership
   - truly cohesive leadership
3. `NotFormed`, `Forming`, and `Cohesive` are no longer driven only by coarse thresholds.
4. Domain reasons remain enum-based and presentation-localized.
5. `DecisionPacket` continues to carry the structured snapshot.
6. `PresentationAssembler` and `report.rs` keep current architectural boundaries.

## Required Tests

At minimum, add or update:

1. unit tests for `TrendCohesionEvaluator`
   - fragmented candidates
   - repeated leaders with weak compactness
   - compact repeating leaders with sufficient continuity
   - healthy rotation vs destructive churn

2. `presentation_tests`
   - localized rendering of any new reason codes
   - correct summary/status mapping for `NotFormed`, `Forming`, `Cohesive`

3. `report_ui_tests`
   - final rendered trend status remains correct
   - no report-layer recomputation

4. multi-language regression tests
   - `zh-cn`
   - `en-us`
   - `ja-jp`

## Quality Gates

All of the following must pass:

- `cargo fmt`
- `cargo test --quiet`
- `cargo clippy --all-targets --all-features -- -D warnings`

## Hard Review Standard

This task is not accepted just because the report shows a more sophisticated trend label.

It is accepted only if the system gains a meaningfully better and still explainable model of
whether leadership is actually forming.

If the new implementation only renames the current thresholds or hides the same heuristic behind a
single score, it does not count as complete.
