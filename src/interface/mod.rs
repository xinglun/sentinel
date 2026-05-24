//! Sentinel の interface layer。
//!
//! CLI、report、Telegram、audit output などの入出力変換を配置する。
//! 業務判断そのものは domain / application へ委譲する。

pub mod evidence_cli;
