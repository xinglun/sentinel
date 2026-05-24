//! Sentinel の infrastructure layer。
//!
//! 外部 API、永続化、通知、時計、ファイルシステムなどの実装詳細を配置する。
//! Application port を実装し、Domain に実装詳細を漏らしてはならない。

pub mod evidence_ingestion;
pub mod evidence_store;
pub mod notify;
pub mod persistence;
