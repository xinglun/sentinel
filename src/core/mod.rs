pub mod action_matrix;
pub mod asset_state;
pub mod breakout_detection;
pub mod decision;
pub mod display;
pub mod engine;
pub mod execution_gate;
pub mod exit;
pub mod features;
pub mod i18n;
pub mod ledger;
pub mod market_regime;
pub mod market_state;
pub mod notify;
pub mod portfolio_policy;
pub mod position_intent;
pub mod report;
pub mod run_status;
pub mod runtime_mode;
pub mod telemetry;
pub mod threshold_format;
pub mod trader_agent;
pub mod transition_log;

pub mod intent_synthesizer;
pub mod presentation;
pub mod presentation_assembler;

#[cfg(test)]
mod report_ui_tests;

#[cfg(test)]
mod presentation_tests;
pub mod trend_cohesion;
