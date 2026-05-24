//! Sentinel の interface layer。
//!
//! CLI、report、Telegram、audit output などの入出力変換を配置する。
//! 業務判断そのものは domain / application へ委譲する。

pub mod display;
pub mod evidence_cli;
pub mod i18n;
pub mod presentation;
pub mod presentation_assembler;
pub mod report;
pub mod threshold_format;

#[cfg(test)]
mod presentation_tests;
#[cfg(test)]
mod report_ui_tests;
