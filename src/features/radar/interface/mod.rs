pub(crate) mod audit_daily_report;
pub mod display;
pub(crate) mod hypothesis_read_model;
pub mod presentation;
pub mod presentation_assembler;
pub mod radar_pipeline_runner;
pub mod report;
pub(crate) mod risk_taxonomy_read_model;
pub(crate) mod strategic_context_read_model;
pub mod telemetry;
pub(crate) mod trend_recognition_read_model;
pub(crate) mod weekly_state_report;

#[cfg(test)]
mod presentation_tests;
#[cfg(test)]
mod report_ui_tests;
