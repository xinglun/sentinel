//! Sentinel の application layer。
//!
//! ユースケース、port trait、application service、transaction boundary を配置する。
//! Domain を操作し、外部実装の詳細は infrastructure adapter に委譲する。

pub mod evidence;
pub mod evidence_ingestion;
pub mod provider;
pub mod radar;
pub mod run_status;
