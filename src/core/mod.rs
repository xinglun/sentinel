pub mod action_matrix;
pub mod asset_state;
pub mod decision;
pub mod engine;
pub mod execution_gate;
pub mod features;
pub mod ledger;
pub mod market_regime;
pub mod notify;
pub mod persistence;
pub mod portfolio_policy;
pub mod report;
pub mod run_status;
pub mod runtime_mode;
pub mod telemetry;
pub mod trader_agent;
pub mod transition_log;

#[cfg(test)]
mod report_ui_tests;
