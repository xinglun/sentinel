use crate::features::research::application::valuation_gravity::{
    GravityStatus, ValuationConfidence, ValuationDataQualityReason, ValuationPersistenceHealth,
    ValuationPersistenceReason, ValuationSource, ValuationSourceHealth,
};
use crate::features::shared::interface::i18n::Language;

pub(crate) fn title(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "## 🪢 Gravity Layer（估值重力层）",
        Language::EnUs => "## 🪢 Gravity Layer (Valuation Gravity)",
        Language::JaJp => "## 🪢 Gravity Layer（バリュエーション重力レイヤー）",
    }
}

pub(crate) fn observation_notice(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "观察层：表达价格相对外部价值锚的偏离，不预测价格。",
        Language::EnUs => "Observation layer: expresses price deviation from external value anchors; it does not predict prices.",
        Language::JaJp => "観測レイヤー：外部価値アンカーからの価格乖離を表現し、価格予測は行わない。",
    }
}

pub(crate) fn future_date_error(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "估值重力层不接受未来日期",
        Language::EnUs => "Valuation Gravity does not accept a future date",
        Language::JaJp => "バリュエーション重力は未来日を受け付けません",
    }
}

pub(crate) fn boundary(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "边界：Gravity 与 Trend 独立；不影响 READY / EXECUTE / Gate / Position Sizing / Trader，也不产生交易信号。",
        Language::EnUs => "Boundary: Gravity is independent from Trend; it does not affect READY / EXECUTE / Gate / Position Sizing / Trader and produces no trading signal.",
        Language::JaJp => "境界：Gravity は Trend から独立し、READY / EXECUTE / Gate / Position Sizing / Trader に影響せず、売買シグナルを生成しない。",
    }
}

pub(crate) fn gravity_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "Gravity",
        Language::EnUs => "Gravity",
        Language::JaJp => "Gravity",
    }
}

pub(crate) fn confidence_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "可信度",
        Language::EnUs => "Confidence",
        Language::JaJp => "確信度",
    }
}

pub(crate) fn source_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "来源",
        Language::EnUs => "Source",
        Language::JaJp => "ソース",
    }
}

pub(crate) fn provider_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "Provider",
        Language::EnUs => "Provider",
        Language::JaJp => "Provider",
    }
}

pub(crate) fn as_of_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "数据日期",
        Language::EnUs => "As of",
        Language::JaJp => "基準日",
    }
}

pub(crate) fn source_health_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "来源状态",
        Language::EnUs => "Source Health",
        Language::JaJp => "ソース状態",
    }
}

pub(crate) fn evidence_count_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "证据数量",
        Language::EnUs => "Evidence Count",
        Language::JaJp => "証拠件数",
    }
}

pub(crate) fn quality_reason_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "数据质量原因",
        Language::EnUs => "Data Quality Reason",
        Language::JaJp => "データ品質理由",
    }
}

pub(crate) fn persistence_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "快照持久化",
        Language::EnUs => "Snapshot Persistence",
        Language::JaJp => "スナップショット永続化",
    }
}

pub(crate) fn persistence_detail_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "持久化错误",
        Language::EnUs => "Persistence Error",
        Language::JaJp => "永続化エラー",
    }
}

pub(crate) fn persistence_failure_detail(
    reason: ValuationPersistenceReason,
    language: Language,
) -> &'static str {
    match (reason, language) {
        (ValuationPersistenceReason::SnapshotReadFailed, Language::ZhCn) => {
            "历史快照未通过读取或完整性校验；原始错误仅保留在审计日志。"
        }
        (ValuationPersistenceReason::SnapshotReadFailed, Language::EnUs) => {
            "The historical snapshot failed read or integrity validation; raw details remain audit-only."
        }
        (ValuationPersistenceReason::SnapshotReadFailed, Language::JaJp) => {
            "履歴スナップショットの読み取りまたは整合性検証に失敗しました。生の詳細は監査情報にのみ保持します。"
        }
        (_, Language::ZhCn) => "快照保存失败；原始错误仅保留在审计日志。",
        (_, Language::EnUs) => "Snapshot save failed; raw details remain audit-only.",
        (_, Language::JaJp) => {
            "スナップショットの保存に失敗しました。生の詳細は監査情報にのみ保持します。"
        }
    }
}

pub(crate) fn unavailable(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "未形成估值分类（外部证据不足）",
        Language::EnUs => "No valuation classification formed (insufficient external evidence)",
        Language::JaJp => "評価分類を形成できません（外部証拠不足）",
    }
}

pub(crate) fn gravity(status: GravityStatus, language: Language) -> &'static str {
    match (status, language) {
        (GravityStatus::DeepUndervalued, Language::ZhCn) => "明显低估",
        (GravityStatus::Undervalued, Language::ZhCn) => "低估",
        (GravityStatus::Fair, Language::ZhCn) => "合理",
        (GravityStatus::SlightlyExpensive, Language::ZhCn) => "略贵",
        (GravityStatus::Expensive, Language::ZhCn) => "昂贵",
        (GravityStatus::VeryExpensive, Language::ZhCn) => "非常昂贵",
        (GravityStatus::DeepUndervalued, Language::JaJp) => "大幅な割安",
        (GravityStatus::Undervalued, Language::JaJp) => "割安",
        (GravityStatus::Fair, Language::JaJp) => "適正",
        (GravityStatus::SlightlyExpensive, Language::JaJp) => "やや割高",
        (GravityStatus::Expensive, Language::JaJp) => "割高",
        (GravityStatus::VeryExpensive, Language::JaJp) => "非常に割高",
        (GravityStatus::DeepUndervalued, Language::EnUs) => "Deep Undervalued",
        (GravityStatus::Undervalued, Language::EnUs) => "Undervalued",
        (GravityStatus::Fair, Language::EnUs) => "Fair",
        (GravityStatus::SlightlyExpensive, Language::EnUs) => "Slightly Expensive",
        (GravityStatus::Expensive, Language::EnUs) => "Expensive",
        (GravityStatus::VeryExpensive, Language::EnUs) => "Very Expensive",
    }
}

pub(crate) fn confidence(value: ValuationConfidence, language: Language) -> &'static str {
    match (value, language) {
        (ValuationConfidence::VeryHigh, Language::ZhCn) => "极高",
        (ValuationConfidence::High, Language::ZhCn) => "高",
        (ValuationConfidence::Medium, Language::ZhCn) => "中",
        (ValuationConfidence::Low, Language::ZhCn) => "低",
        (ValuationConfidence::VeryLow, Language::ZhCn) => "极低",
        (ValuationConfidence::VeryHigh, Language::JaJp) => "非常に高い",
        (ValuationConfidence::High, Language::JaJp) => "高い",
        (ValuationConfidence::Medium, Language::JaJp) => "中",
        (ValuationConfidence::Low, Language::JaJp) => "低い",
        (ValuationConfidence::VeryLow, Language::JaJp) => "非常に低い",
        (ValuationConfidence::VeryHigh, Language::EnUs) => "Very High",
        (ValuationConfidence::High, Language::EnUs) => "High",
        (ValuationConfidence::Medium, Language::EnUs) => "Medium",
        (ValuationConfidence::Low, Language::EnUs) => "Low",
        (ValuationConfidence::VeryLow, Language::EnUs) => "Very Low",
    }
}

pub(crate) fn source(value: ValuationSource, language: Language) -> &'static str {
    match (value, language) {
        (ValuationSource::AnalystConsensus, Language::ZhCn) => "分析师共识",
        (ValuationSource::MarketMultiple, Language::ZhCn) => "市场估值倍数",
        (ValuationSource::ManualOverride, Language::ZhCn) => "人工覆盖",
        (ValuationSource::Hybrid, Language::ZhCn) => "混合来源",
        (ValuationSource::AnalystConsensus, Language::EnUs) => "Analyst Consensus",
        (ValuationSource::MarketMultiple, Language::EnUs) => "Market Multiple",
        (ValuationSource::ManualOverride, Language::EnUs) => "Manual Override",
        (ValuationSource::Hybrid, Language::EnUs) => "Hybrid",
        (ValuationSource::AnalystConsensus, Language::JaJp) => "アナリストコンセンサス",
        (ValuationSource::MarketMultiple, Language::JaJp) => "市場評価倍率",
        (ValuationSource::ManualOverride, Language::JaJp) => "手動上書き",
        (ValuationSource::Hybrid, Language::JaJp) => "複合ソース",
    }
}

pub(crate) fn source_health(value: ValuationSourceHealth, language: Language) -> &'static str {
    match (value, language) {
        (ValuationSourceHealth::Succeeded, Language::ZhCn) => "成功",
        (ValuationSourceHealth::Partial, Language::ZhCn) => "部分成功",
        (ValuationSourceHealth::Unavailable, Language::ZhCn) => "不可用",
        (ValuationSourceHealth::Succeeded, Language::EnUs) => "Succeeded",
        (ValuationSourceHealth::Partial, Language::EnUs) => "Partial",
        (ValuationSourceHealth::Unavailable, Language::EnUs) => "Unavailable",
        (ValuationSourceHealth::Succeeded, Language::JaJp) => "成功",
        (ValuationSourceHealth::Partial, Language::JaJp) => "一部成功",
        (ValuationSourceHealth::Unavailable, Language::JaJp) => "利用不可",
    }
}

pub(crate) fn quality_reason(
    value: ValuationDataQualityReason,
    language: Language,
) -> &'static str {
    match (value, language) {
        (ValuationDataQualityReason::PriceTargetConsensus, Language::ZhCn) => {
            "使用分析师目标价共识"
        }
        (ValuationDataQualityReason::MarketMultipleFallback, Language::ZhCn) => {
            "目标价不可用，降级为历史市场倍数"
        }
        (ValuationDataQualityReason::RecommendationFallback, Language::ZhCn) => {
            "数值锚不可用，降级为分析师评级共识"
        }
        (ValuationDataQualityReason::MissingCredential, Language::ZhCn) => "未配置外部数据凭证",
        (ValuationDataQualityReason::EntitlementDenied, Language::ZhCn) => "数据权限不足",
        (ValuationDataQualityReason::ProviderFailure, Language::ZhCn) => "外部 provider 请求失败",
        (ValuationDataQualityReason::InvalidResponse, Language::ZhCn) => "外部响应格式无效",
        (ValuationDataQualityReason::InsufficientEvidence, Language::ZhCn) => "外部估值证据不足",
        (ValuationDataQualityReason::HistoricalSnapshotMissing, Language::ZhCn) => {
            "缺少指定日期的历史快照"
        }
        (ValuationDataQualityReason::HistoricalSnapshotReadFailure, Language::ZhCn) => {
            "历史快照读取失败"
        }
        (ValuationDataQualityReason::PriceTargetConsensus, Language::EnUs) => {
            "Analyst price-target consensus used"
        }
        (ValuationDataQualityReason::MarketMultipleFallback, Language::EnUs) => {
            "Price target unavailable; fell back to historical market multiple"
        }
        (ValuationDataQualityReason::RecommendationFallback, Language::EnUs) => {
            "Numeric anchors unavailable; fell back to analyst recommendations"
        }
        (ValuationDataQualityReason::MissingCredential, Language::EnUs) => {
            "External data credential is not configured"
        }
        (ValuationDataQualityReason::EntitlementDenied, Language::EnUs) => {
            "Provider entitlement denied"
        }
        (ValuationDataQualityReason::ProviderFailure, Language::EnUs) => {
            "External provider request failed"
        }
        (ValuationDataQualityReason::InvalidResponse, Language::EnUs) => {
            "External provider response was invalid"
        }
        (ValuationDataQualityReason::InsufficientEvidence, Language::EnUs) => {
            "Insufficient external valuation evidence"
        }
        (ValuationDataQualityReason::HistoricalSnapshotMissing, Language::EnUs) => {
            "Historical snapshot is missing for the requested date"
        }
        (ValuationDataQualityReason::HistoricalSnapshotReadFailure, Language::EnUs) => {
            "Historical snapshot could not be read"
        }
        (ValuationDataQualityReason::PriceTargetConsensus, Language::JaJp) => {
            "アナリスト目標株価コンセンサスを使用"
        }
        (ValuationDataQualityReason::MarketMultipleFallback, Language::JaJp) => {
            "目標株価を利用できないため過去市場倍率へ降格"
        }
        (ValuationDataQualityReason::RecommendationFallback, Language::JaJp) => {
            "数値アンカーを利用できないためアナリスト評価へ降格"
        }
        (ValuationDataQualityReason::MissingCredential, Language::JaJp) => {
            "外部データ認証情報が未設定"
        }
        (ValuationDataQualityReason::EntitlementDenied, Language::JaJp) => "データ利用権限が不足",
        (ValuationDataQualityReason::ProviderFailure, Language::JaJp) => {
            "外部 provider の取得に失敗"
        }
        (ValuationDataQualityReason::InvalidResponse, Language::JaJp) => {
            "外部 provider の応答形式が不正"
        }
        (ValuationDataQualityReason::InsufficientEvidence, Language::JaJp) => "外部評価証拠が不足",
        (ValuationDataQualityReason::HistoricalSnapshotMissing, Language::JaJp) => {
            "指定日の履歴スナップショットが存在しない"
        }
        (ValuationDataQualityReason::HistoricalSnapshotReadFailure, Language::JaJp) => {
            "履歴スナップショットの読み取りに失敗"
        }
    }
}

pub(crate) fn persistence(
    health: ValuationPersistenceHealth,
    reason: ValuationPersistenceReason,
    language: Language,
) -> &'static str {
    match (health, reason, language) {
        (ValuationPersistenceHealth::Saved, _, Language::ZhCn) => "已保存",
        (ValuationPersistenceHealth::Replayed, _, Language::ZhCn) => "已读取历史快照",
        (ValuationPersistenceHealth::Missing, _, Language::ZhCn) => "历史快照缺失",
        (
            ValuationPersistenceHealth::Failed,
            ValuationPersistenceReason::SnapshotReadFailed,
            Language::ZhCn,
        ) => "读取失败",
        (ValuationPersistenceHealth::Failed, _, Language::ZhCn) => "保存失败",
        (ValuationPersistenceHealth::Saved, _, Language::EnUs) => "Saved",
        (ValuationPersistenceHealth::Replayed, _, Language::EnUs) => "Historical snapshot replayed",
        (ValuationPersistenceHealth::Missing, _, Language::EnUs) => "Historical snapshot missing",
        (
            ValuationPersistenceHealth::Failed,
            ValuationPersistenceReason::SnapshotReadFailed,
            Language::EnUs,
        ) => "Read failed",
        (ValuationPersistenceHealth::Failed, _, Language::EnUs) => "Save failed",
        (ValuationPersistenceHealth::Saved, _, Language::JaJp) => "保存済み",
        (ValuationPersistenceHealth::Replayed, _, Language::JaJp) => "履歴スナップショットを再生",
        (ValuationPersistenceHealth::Missing, _, Language::JaJp) => "履歴スナップショットなし",
        (
            ValuationPersistenceHealth::Failed,
            ValuationPersistenceReason::SnapshotReadFailed,
            Language::JaJp,
        ) => "読み取り失敗",
        (ValuationPersistenceHealth::Failed, _, Language::JaJp) => "保存失敗",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_quality_reason_and_health_has_three_language_labels() {
        let reasons = [
            ValuationDataQualityReason::PriceTargetConsensus,
            ValuationDataQualityReason::MarketMultipleFallback,
            ValuationDataQualityReason::RecommendationFallback,
            ValuationDataQualityReason::MissingCredential,
            ValuationDataQualityReason::EntitlementDenied,
            ValuationDataQualityReason::ProviderFailure,
            ValuationDataQualityReason::InvalidResponse,
            ValuationDataQualityReason::InsufficientEvidence,
            ValuationDataQualityReason::HistoricalSnapshotMissing,
            ValuationDataQualityReason::HistoricalSnapshotReadFailure,
        ];
        let languages = [Language::ZhCn, Language::EnUs, Language::JaJp];
        for reason in reasons {
            for language in languages {
                assert!(!quality_reason(reason, language).is_empty());
            }
        }
        for health in [
            ValuationSourceHealth::Succeeded,
            ValuationSourceHealth::Partial,
            ValuationSourceHealth::Unavailable,
        ] {
            for language in languages {
                assert!(!source_health(health, language).is_empty());
            }
        }
    }

    #[test]
    fn every_persistence_health_has_three_language_labels() {
        let values = [
            (
                ValuationPersistenceHealth::Saved,
                ValuationPersistenceReason::SnapshotSaved,
            ),
            (
                ValuationPersistenceHealth::Replayed,
                ValuationPersistenceReason::HistoricalSnapshotReplayed,
            ),
            (
                ValuationPersistenceHealth::Missing,
                ValuationPersistenceReason::HistoricalSnapshotMissing,
            ),
            (
                ValuationPersistenceHealth::Failed,
                ValuationPersistenceReason::SnapshotReadFailed,
            ),
            (
                ValuationPersistenceHealth::Failed,
                ValuationPersistenceReason::SnapshotWriteFailed,
            ),
        ];
        for (health, reason) in values {
            for language in [Language::ZhCn, Language::EnUs, Language::JaJp] {
                assert!(!persistence(health, reason, language).is_empty());
            }
        }
    }
}
