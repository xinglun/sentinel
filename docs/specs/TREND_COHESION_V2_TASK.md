# Trend Cohesion V2 Task

## 1. Goal

Add a second-generation `Trend Cohesion` layer that no longer treats “candidate count + stability + continuity” as the full definition of a tradable main trend.

V2 must answer a stricter question:

> Is there a **coherent, followable primary trend** in the market right now?

This is an upgrade of explanation quality, not a rewrite of trading rules.

The system already answers:

- whether the market may be participated in
- whether new positions are forbidden
- whether existing positions should be held / trimmed / exited

V2 adds an explicit answer to:

- whether a real primary trend exists
- whether leaders are converging into a stable, followable structure

## 2. Non-Goals

This task must **not**:

- change `ParticipationReadiness` thresholds
- change `NO TRADE` semantics
- change `ExitDecision`
- change `ActionMatrix`
- change execution behavior
- introduce sector/theme NLP or external classification systems in v2

This is a diagnostics and structural-quality upgrade, not a rule expansion for buying or selling.

## 3. Current Problem

Current `TrendCohesionStatus::evaluate(...)` is a light proxy:

- participation ready or not
- stability score
- continuity streak
- top-tier count

That is good enough for V1 messaging, but too weak for V2 because it cannot distinguish:

1. a small but internally inconsistent candidate set
2. a dispersed watchlist with no clear leaders
3. a stable leader set that is actually converging
4. a short-lived restart that looks cleaner than it really is

In short:

V1 detects “is the market obviously not ready?”

V2 should detect “is there a primary trend worth following?”

## 4. Required Output

Keep the existing enum shape for display simplicity:

- `NotFormed`
- `Forming`
- `Cohesive`

But add structured factors behind it.

Introduce a new domain struct:

```rust
pub struct TrendCohesionSnapshot {
    pub status: TrendCohesionStatus,
    pub candidate_count: usize,
    pub leader_count: usize,
    pub leader_concentration_score: f64,
    pub continuity_quality_score: f64,
    pub dispersion_score: f64,
    pub reasons: Vec<String>,
}
```

`DecisionPacket` should store the full snapshot, not just the enum.

## 5. V2 Evaluation Model

### 5.1 Inputs

V2 may use only data already available in the domain layer:

- `participation.participation_ready`
- `participation.core_tier_streak`
- `market_features.stability_score`
- current `top_tier_symbols`
- recent decision history packets
- asset-level `unified_position_intent`
- asset-level state snapshots

Do not add external market taxonomy in V2.

### 5.2 Derived Factors

V2 should compute at least these factors:

1. `candidate_count`
   number of current top-tier symbols

2. `leader_count`
   number of symbols that are both:
   - in current top tier
   - and repeatedly present in recent history window

3. `leader_concentration_score`
   higher when a small number of repeating leaders dominate recent top-tier composition

4. `continuity_quality_score`
   higher when the recent top-tier set changes slowly instead of churning daily

5. `dispersion_score`
   higher when the candidate pool is too wide and leadership is fragmented

### 5.3 Suggested First-Cut Heuristics

Use a recent history window of 3 trading days for V2.

Suggested initial logic:

- `NotFormed`
  when any of the following is true:
  - `stability_score < 10.0`
  - `core_tier_streak < 2`
  - `candidate_count == 0`
  - `candidate_count >= 6`
  - recent top-tier membership churn is high
  - repeated leaders are absent

- `Cohesive`
  when all of the following are true:
  - `participation_ready == true`
  - `stability_score >= 10.0`
  - `core_tier_streak >= 3`
  - `candidate_count <= 4`
  - at least 2 repeating leaders persist across the recent window
  - membership churn is low

- `Forming`
  everything between the two

This is intentionally conservative.

## 6. Architectural Requirements

### 6.1 Domain Layer

Add:

- `src/core/trend_cohesion.rs`

V2 should expose:

- `TrendCohesionStatus`
- `TrendCohesionSnapshot`
- `TrendCohesionEvaluator`

### 6.2 Decision Layer

Update:

- `src/core/decision.rs`

Replace the current scalar field with:

```rust
pub trend_cohesion: TrendCohesionSnapshot
```

Compatibility:

- add `#[serde(default)]`
- preserve legacy packet loading

### 6.3 Engine / Domain Assembly

Update:

- `src/core/engine.rs`

The engine should compute the snapshot from domain facts and recent history.

Do not compute final display text here.

### 6.4 Presentation Layer

Update:

- `src/core/presentation.rs`
- `src/core/presentation_assembler.rs`
- `src/core/i18n.rs`

Presentation should derive:

- `主线状态 / Primary Trend / 主線状態`
- localized value:
  - `主线未形成`
  - `主线形成中`
  - `主线已收敛`
- optional short explanation based on reasons

### 6.5 Report Layer

Update:

- `src/core/report.rs`

`report.rs` must continue to render only.

It may display:

- trend cohesion label
- trend cohesion value
- optional one-line explanation

It must not evaluate cohesion by itself.

## 7. Required Behavior

### 7.1 For Current Typical `NO TRADE` Restart

For a state like:

- `stability = 1.5`
- `continuity = 1d`
- `candidate_count = 8+`
- `participation_ready = false`

the report must explicitly show:

- `主线状态: 主线未形成`

### 7.2 Important Semantic Separation

V2 must not collapse these concepts:

- `NO TRADE`
- `DEFENSIVE`
- `Trend Not Formed`

They are different:

- `NO TRADE` = do not open new positions
- `DEFENSIVE` = market/risk posture
- `Trend Not Formed` = no coherent followable primary trend exists

## 8. Tests Required

### 8.1 Domain Tests

Add `trend_cohesion` tests covering:

1. low stability + 1d continuity + dispersed candidates -> `NotFormed`
2. ready + stable + persistent leaders + low churn -> `Cohesive`
3. intermediate case -> `Forming`

### 8.2 History-Aware Tests

Add tests using recent packets to verify:

1. leader repetition improves cohesion
2. top-tier churn degrades cohesion

### 8.3 Presentation Tests

Add:

- zh/en/ja localized output tests
- `DecisionSummaryViewModel` includes trend cohesion field

### 8.4 UI Tests

Add final render tests asserting that:

1. `NO TRADE + weak restart` shows `主线未形成`
2. the trend label is visible in report output
3. report text does not require the user to infer cohesion manually from candidate count alone

## 9. Acceptance Criteria

This task is complete only if all of the following are true:

1. `Trend Cohesion` is a real domain snapshot, not only a display string
2. `DecisionPacket` stores the structured snapshot
3. `PresentationAssembler` renders a localized “Primary Trend” line
4. the common weak-restart scenario explicitly displays `主线未形成`
5. `report.rs` does not compute cohesion
6. legacy packet loading still works
7. `cargo fmt` passes
8. `cargo test --quiet` passes
9. `cargo clippy --all-targets --all-features -- -D warnings` passes

## 10. Final Principle

V2 is successful when the system no longer merely says:

- “do not trade”

but can also explicitly say:

- “there is no coherent primary trend to follow yet”

That distinction is the entire point of this upgrade.
