pub mod engine;
pub mod features;
pub mod market_regime;
pub mod portfolio_policy;
pub mod asset_state;
pub mod action_matrix;
pub mod decision;
pub mod ledger;
pub mod notify;
pub mod report;
pub mod trader_agent;
pub mod persistence;
pub mod transition_log;
pub mod execution_gate;
pub mod runtime_mode;
pub mod telemetry;
pub mod run_status;

#[cfg(test)]
mod report_ui_tests;
