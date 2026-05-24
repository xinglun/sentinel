//! Sentinel の infrastructure layer。
//!
//! 外部 API、永続化、通知、時計、ファイルシステムなどの実装詳細を配置する。
//! Application port を実装し、Domain に実装詳細を漏らしてはならない。

pub mod evidence_fetcher_factory;
pub mod evidence_ingestion;
pub mod evidence_store;
pub mod market_data_provider_factory;
pub mod notification_factory;
pub mod notify;
pub mod persistence;
pub mod radar_pipeline_runner;
pub mod radar_runtime_factory;
pub mod run_status_reader;
pub mod trader_agent;
pub mod transition_log;
