# Product-Grade Review 2: Actionable Tasks

This document tracks the remedial actions required to solve the findings from the second Product-Grade Review.

## P0: Failure Semantics Hardening (Safety & Integrity)

- [ ] **Hard Exit on Critical Failures**: 
    - Ensure `archival`, `notification`, and `execution` stages can propagate errors that force a non-zero exit code in `cli.rs` when appropriate.
- [ ] **Prohibit Silent `get_funds()` Downgrade**:
    - If `get_funds()` fails, the system must bubble up the error and stop, rather than defaulting to an empty/fake account state.
- [ ] **TraderAgent Error Propagation**:
    - Refactor `TraderAgent` to return a structured error/result if *any* single `place_order` fails, ensuring `run_status.json` reflects the true execution integrity.
- [ ] **Workflow run_status Validation**:
    - Update `daily_radar.yml` to treat `run_status_[DATE].json` as a mandatory output and potentially add a check step.

## P1: Diagnostic & Telemetry Realism

- [ ] **Real-world Telemetry `data_quality_status`**:
    - Link `data_quality_status` in `telemetry.rs` to the actual `fetch_failures` count (e.g., "OK", "DEGRADED", "FAILED").
- [ ] **Finer-grained `run_status` Stats**:
    - Include more detailed execution statistics in the outcome JSON (e.g., specific symbols failed, count of succeeded/failed orders).
- [ ] **Standardized Failure Reasons**:
    - Align error message formats across Telegram, Archival Markdown, and Execution Gate for easier log searching.

## P2: Documentation Governance

- [ ] **Clean up `architecture_design.md`**:
    - Remove legacy fields (`bear_mode`, `caution_ma_days`, etc.).
    - Update schema counts (20-column telemetry).
- [ ] **Clean up `hosting_spec.md`**:
    - Align with current multi-branch deployment and artifact retention policy.
- [ ] **Unified Vocabulary Audit**:
    - Ensure all docs use consistent terms for Market State and Execution Modes.
