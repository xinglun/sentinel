use super::valuation_gravity_report::build_valuation_gravity_report;
use crate::features::research::application::valuation_gravity::{
    GravityStatus, ValuationConfidence, ValuationDataQualityReason, ValuationGravityAssetSnapshot,
    ValuationGravityObservation, ValuationGravitySnapshot, ValuationPersistenceHealth,
    ValuationPersistenceReason, ValuationSource, ValuationSourceHealth,
};
use crate::features::shared::interface::i18n::Language;
use chrono::NaiveDate;

fn observation() -> ValuationGravityObservation {
    let date = NaiveDate::from_ymd_opt(2026, 6, 18).unwrap();
    ValuationGravityObservation {
        snapshot: ValuationGravitySnapshot {
            as_of_date: date,
            assets: vec![
                ValuationGravityAssetSnapshot {
                    symbol: "NVDA".to_string(),
                    gravity: Some(GravityStatus::SlightlyExpensive),
                    confidence: Some(ValuationConfidence::Low),
                    source: Some(ValuationSource::MarketMultiple),
                    provider: "Finnhub".to_string(),
                    as_of_date: date,
                    source_health: ValuationSourceHealth::Partial,
                    quality_reason: ValuationDataQualityReason::MarketMultipleFallback,
                    evidence_count: 5,
                    relative_ratio: Some(1.2),
                    message: "fixture".to_string(),
                },
                ValuationGravityAssetSnapshot {
                    symbol: "FIG".to_string(),
                    gravity: None,
                    confidence: None,
                    source: None,
                    provider: "Finnhub".to_string(),
                    as_of_date: date,
                    source_health: ValuationSourceHealth::Unavailable,
                    quality_reason: ValuationDataQualityReason::MissingCredential,
                    evidence_count: 0,
                    relative_ratio: None,
                    message: "fixture".to_string(),
                },
            ],
            observation_only: true,
        },
        persistence_health: ValuationPersistenceHealth::Saved,
        persistence_reason: ValuationPersistenceReason::SnapshotSaved,
        persistence_detail: String::new(),
    }
}

#[test]
fn report_renders_all_languages_without_unknown_or_trade_signal() {
    for (language, title, gravity, confidence, source, unavailable, reason, boundary) in [
        (
            Language::ZhCn,
            "估值重力层",
            "略贵",
            "可信度: 低",
            "来源: 市场估值倍数",
            "未形成估值分类",
            "未配置外部数据凭证",
            "不产生交易信号",
        ),
        (
            Language::EnUs,
            "Valuation Gravity",
            "Slightly Expensive",
            "Confidence: Low",
            "Source: Market Multiple",
            "No valuation classification formed",
            "External data credential is not configured",
            "produces no trading signal",
        ),
        (
            Language::JaJp,
            "バリュエーション重力",
            "やや割高",
            "確信度: 低い",
            "ソース: 市場評価倍率",
            "評価分類を形成できません",
            "外部データ認証情報が未設定",
            "売買シグナルを生成しない",
        ),
    ] {
        let report = build_valuation_gravity_report(&observation(), language);
        assert!(report.contains(title));
        assert!(report.contains(gravity));
        assert!(report.contains(confidence));
        assert!(report.contains(source));
        assert!(report.contains(unavailable));
        assert!(report.contains(reason));
        assert!(report.contains('5'));
        assert!(report.contains(boundary));
        assert!(!report.contains("Unknown"));
        if language != Language::EnUs {
            assert!(!report.contains("Market Multiple"));
        }
    }
}

#[test]
fn report_localizes_snapshot_write_failure_without_exposing_raw_detail() {
    let mut observation = observation();
    observation.persistence_health = ValuationPersistenceHealth::Failed;
    observation.persistence_reason = ValuationPersistenceReason::SnapshotWriteFailed;
    observation.persistence_detail = "permission denied".to_string();

    for (language, status, detail) in [
        (
            Language::ZhCn,
            "快照持久化: 保存失败",
            "原始错误仅保留在审计日志",
        ),
        (
            Language::EnUs,
            "Snapshot Persistence: Save failed",
            "raw details remain audit-only",
        ),
        (
            Language::JaJp,
            "スナップショット永続化: 保存失敗",
            "生の詳細は監査情報にのみ保持",
        ),
    ] {
        let report = build_valuation_gravity_report(&observation, language);
        assert!(report.contains(status));
        assert!(report.contains(detail));
        assert!(!report.contains("permission denied"));
    }
}
