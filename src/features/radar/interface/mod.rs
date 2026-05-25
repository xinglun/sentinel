pub(crate) mod audit_daily_report;
pub mod display;
pub mod presentation;
pub mod presentation_assembler;
pub mod radar_pipeline_runner;
pub mod report;
pub mod telemetry;
pub(crate) mod weekly_state_report;

#[cfg(test)]
mod presentation_tests;
#[cfg(test)]
mod report_ui_tests;
