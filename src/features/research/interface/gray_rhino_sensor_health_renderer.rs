use std::collections::BTreeSet;

use crate::features::research::application::gray_rhino_daily_report::GrayRhinoDailyReportViewModel;
use crate::features::research::domain::gray_rhino_evidence::{
    GrayRhinoEvidenceCategory, GrayRhinoEvidenceRejection,
};
use crate::features::research::interface::gray_rhino_renderer::{
    render_backfill_ops_view, render_governance_sensor_health,
};
use crate::features::shared::interface::i18n::Language;

/// センサー健全性カテゴリの定義
struct SensorHealthCategory {
    category: GrayRhinoEvidenceCategory,
    label: &'static str,
}

impl SensorHealthCategory {
    fn matches(&self, category: GrayRhinoEvidenceCategory) -> bool {
        self.category == category
    }
}

/// センサー健全性カテゴリ一覧を返す
fn sensor_health_categories(language: Language) -> Vec<SensorHealthCategory> {
    vec![
        SensorHealthCategory {
            category: GrayRhinoEvidenceCategory::GovernanceConcentration,
            label: match language {
                Language::ZhCn => "治理集中",
                Language::EnUs => "Governance Concentration",
                Language::JaJp => "ガバナンス集中",
            },
        },
        SensorHealthCategory {
            category: GrayRhinoEvidenceCategory::DependencyConcentration,
            label: match language {
                Language::ZhCn => "依赖集中",
                Language::EnUs => "Dependency Concentration",
                Language::JaJp => "依存集中",
            },
        },
        SensorHealthCategory {
            category: GrayRhinoEvidenceCategory::InstitutionalMaturity,
            label: match language {
                Language::ZhCn => "制度成熟度",
                Language::EnUs => "Institutional Maturity",
                Language::JaJp => "制度成熟度",
            },
        },
        SensorHealthCategory {
            category: GrayRhinoEvidenceCategory::Redundancy,
            label: match language {
                Language::ZhCn => "冗余能力",
                Language::EnUs => "Redundancy",
                Language::JaJp => "冗長性",
            },
        },
    ]
}

/// センサー健全性セクションの見出し
fn sensor_health_heading(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "灰犀牛传感器健康度",
        Language::EnUs => "Gray Rhino Sensor Health",
        Language::JaJp => "灰色のサイセンサー健全性",
    }
}

/// 複数カテゴリのセンサー健全性をレンダリングする
pub(super) fn render_multi_category_sensor_health(
    view_model: &GrayRhinoDailyReportViewModel,
    language: Language,
) -> String {
    let records = &view_model.evidence_records;
    let scoreable_records = &view_model.scoreable_evidence_records;
    let excluded_count = records.len().saturating_sub(scoreable_records.len())
        + view_model.rejected_evidence_records.len();
    let governance = render_governance_sensor_health(&view_model.governance_audits, language);
    if records.is_empty()
        && view_model.rejected_evidence_records.is_empty()
        && governance.is_empty()
    {
        return String::new();
    }
    let mut out = String::new();
    out.push_str(sensor_health_heading(language));
    out.push('\n');
    let categories = sensor_health_categories(language);
    let ready_count = categories
        .iter()
        .filter(|category| {
            scoreable_records
                .iter()
                .any(|record| category.matches(record.category))
        })
        .count();
    out.push_str(&format!(
        "- {}: {:.1}% ({}/{})\n",
        readiness_score_label(language),
        ready_count as f64 / categories.len() as f64 * 100.0,
        ready_count,
        categories.len()
    ));
    let average_confidence = if scoreable_records.is_empty() {
        0.0
    } else {
        scoreable_records
            .iter()
            .map(|record| record.confidence)
            .sum::<f64>()
            / scoreable_records.len() as f64
    };
    let source_diversity = scoreable_records
        .iter()
        .map(|record| record.source.publisher.clone())
        .collect::<BTreeSet<_>>()
        .len();
    let quality_label = if ready_count >= 3 && average_confidence >= 0.75 {
        readiness_ready_label(language)
    } else if ready_count >= 2 && average_confidence >= 0.6 {
        readiness_partial_label(language)
    } else {
        readiness_insufficient_label(language)
    };
    out.push_str(&format!(
        "- {}: {quality_label} ({} {:.2}, {} {})\n",
        quality_score_label(language),
        average_confidence_label(language),
        average_confidence,
        source_diversity_label(language),
        source_diversity
    ));
    out.push_str(evidence_quality_dimensions_label(language));
    out.push('\n');
    if excluded_count > 0 {
        out.push_str(&format!(
            "- {}: {} ({})\n",
            excluded_evidence_count_label(language),
            excluded_count,
            excluded_evidence_reason_label(language)
        ));
        for rejection in &view_model.rejected_evidence_records {
            out.push_str(&format!(
                "  - {}: {}\n",
                rejection.source_title,
                evidence_rejection_reason_label(rejection.reason, language)
            ));
        }
    }
    for category in categories {
        let count = scoreable_records
            .iter()
            .filter(|record| category.matches(record.category))
            .count();
        let readiness = if count > 0 {
            readiness_ready_label(language)
        } else {
            readiness_insufficient_label(language)
        };
        out.push_str(&format!(
            "- {}: {count} {}, {}={readiness}\n",
            category.label,
            evidence_record_count_label(language),
            readiness_label(language)
        ));
    }
    if !governance.is_empty() {
        out.push('\n');
        out.push_str(&governance);
    }
    out.push('\n');
    out.push_str(evidence_explanation_graph_label(language));
    out.push('\n');
    out.push_str(evidence_explanation_graph_body(language));
    if let Some(ops_view) =
        render_backfill_ops_view(view_model.backfill_ops_view.as_ref(), language)
    {
        out.push('\n');
        out.push_str(&ops_view);
    }
    out
}

fn readiness_score_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "准备度评分",
        Language::EnUs => "Readiness score",
        Language::JaJp => "準備度スコア",
    }
}

fn quality_score_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "质量评分",
        Language::EnUs => "Quality score",
        Language::JaJp => "品質スコア",
    }
}

fn average_confidence_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "平均置信度",
        Language::EnUs => "avg confidence",
        Language::JaJp => "平均信頼度",
    }
}

fn source_diversity_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "来源多样性",
        Language::EnUs => "source diversity",
        Language::JaJp => "由来の多様性",
    }
}

fn evidence_quality_dimensions_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "- 证据质量维度: 可追溯性 / 完整性 / 新鲜度 / 置信度 / 来源多样性 / 拒绝比例",
        Language::EnUs => "- Evidence quality dimensions: traceability / completeness / freshness / confidence / source diversity / rejection ratio",
        Language::JaJp => "- 証拠品質次元: 追跡可能性 / 完全性 / 鮮度 / 信頼度 / 由来の多様性 / 拒否比率",
    }
}

fn evidence_record_count_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "条证据记录",
        Language::EnUs => "evidence record(s)",
        Language::JaJp => "件の証拠記録",
    }
}

fn readiness_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "准备度",
        Language::EnUs => "readiness",
        Language::JaJp => "準備度",
    }
}

fn excluded_evidence_count_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "不可评分证据记录",
        Language::EnUs => "Non-scoreable evidence records",
        Language::JaJp => "採点対象外の証拠記録",
    }
}

fn excluded_evidence_reason_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "缺少主体或风险作用不可用于正式评分",
        Language::EnUs => "missing subject or risk effect is not scoreable",
        Language::JaJp => "主体欠落またはリスク作用が正式採点対象外",
    }
}

fn evidence_rejection_reason_label(
    reason: GrayRhinoEvidenceRejection,
    language: Language,
) -> &'static str {
    match (reason, language) {
        (GrayRhinoEvidenceRejection::MissingSubject, Language::ZhCn) => "缺少主体",
        (GrayRhinoEvidenceRejection::MissingSubject, Language::EnUs) => "missing subject",
        (GrayRhinoEvidenceRejection::MissingSubject, Language::JaJp) => "主体が欠落",
        (GrayRhinoEvidenceRejection::MissingSourceReference, Language::ZhCn) => "缺少来源引用",
        (GrayRhinoEvidenceRejection::MissingSourceReference, Language::EnUs) => {
            "missing source reference"
        }
        (GrayRhinoEvidenceRejection::MissingSourceReference, Language::JaJp) => "出典参照が欠落",
        (GrayRhinoEvidenceRejection::MissingSourceTitle, Language::ZhCn) => "缺少来源标题",
        (GrayRhinoEvidenceRejection::MissingSourceTitle, Language::EnUs) => "missing source title",
        (GrayRhinoEvidenceRejection::MissingSourceTitle, Language::JaJp) => "出典タイトルが欠落",
        (GrayRhinoEvidenceRejection::MissingPublisher, Language::ZhCn) => "缺少发布方",
        (GrayRhinoEvidenceRejection::MissingPublisher, Language::EnUs) => "missing publisher",
        (GrayRhinoEvidenceRejection::MissingPublisher, Language::JaJp) => "発行元が欠落",
        (GrayRhinoEvidenceRejection::MissingExtractionNote, Language::ZhCn) => "缺少提取说明",
        (GrayRhinoEvidenceRejection::MissingExtractionNote, Language::EnUs) => {
            "missing extraction note"
        }
        (GrayRhinoEvidenceRejection::MissingExtractionNote, Language::JaJp) => "抽出メモが欠落",
        (GrayRhinoEvidenceRejection::MissingStructuralFact, Language::ZhCn) => "缺少结构事实",
        (GrayRhinoEvidenceRejection::MissingStructuralFact, Language::EnUs) => {
            "missing structural fact"
        }
        (GrayRhinoEvidenceRejection::MissingStructuralFact, Language::JaJp) => "構造的事実が欠落",
        (GrayRhinoEvidenceRejection::ConfidenceOutOfRange, Language::ZhCn) => "置信度超出范围",
        (GrayRhinoEvidenceRejection::ConfidenceOutOfRange, Language::EnUs) => {
            "confidence out of range"
        }
        (GrayRhinoEvidenceRejection::ConfidenceOutOfRange, Language::JaJp) => "信頼度が範囲外",
        (GrayRhinoEvidenceRejection::NarrativeOnly, Language::ZhCn) => "仅为叙事性表述",
        (GrayRhinoEvidenceRejection::NarrativeOnly, Language::EnUs) => "narrative-only record",
        (GrayRhinoEvidenceRejection::NarrativeOnly, Language::JaJp) => "叙述のみの記録",
        (GrayRhinoEvidenceRejection::ForbiddenBoundaryTerm, Language::ZhCn) => "包含禁止边界词",
        (GrayRhinoEvidenceRejection::ForbiddenBoundaryTerm, Language::EnUs) => {
            "forbidden boundary term"
        }
        (GrayRhinoEvidenceRejection::ForbiddenBoundaryTerm, Language::JaJp) => "禁止された境界語",
        (GrayRhinoEvidenceRejection::UnsupportedSourceType, Language::ZhCn) => "来源类型不支持",
        (GrayRhinoEvidenceRejection::UnsupportedSourceType, Language::EnUs) => {
            "unsupported source type"
        }
        (GrayRhinoEvidenceRejection::UnsupportedSourceType, Language::JaJp) => "未対応の出典種別",
        (GrayRhinoEvidenceRejection::MissingGovernanceMetric, Language::ZhCn) => "缺少治理指标",
        (GrayRhinoEvidenceRejection::MissingGovernanceMetric, Language::EnUs) => {
            "missing governance metric"
        }
        (GrayRhinoEvidenceRejection::MissingGovernanceMetric, Language::JaJp) => {
            "ガバナンス指標が欠落"
        }
        (GrayRhinoEvidenceRejection::InvalidGovernanceMetric, Language::ZhCn) => "治理指标无效",
        (GrayRhinoEvidenceRejection::InvalidGovernanceMetric, Language::EnUs) => {
            "invalid governance metric"
        }
        (GrayRhinoEvidenceRejection::InvalidGovernanceMetric, Language::JaJp) => {
            "ガバナンス指標が無効"
        }
        (GrayRhinoEvidenceRejection::MissingDependencyMetric, Language::ZhCn) => "缺少依赖指标",
        (GrayRhinoEvidenceRejection::MissingDependencyMetric, Language::EnUs) => {
            "missing dependency metric"
        }
        (GrayRhinoEvidenceRejection::MissingDependencyMetric, Language::JaJp) => "依存指標が欠落",
        (GrayRhinoEvidenceRejection::InvalidDependencyMetric, Language::ZhCn) => "依赖指标无效",
        (GrayRhinoEvidenceRejection::InvalidDependencyMetric, Language::EnUs) => {
            "invalid dependency metric"
        }
        (GrayRhinoEvidenceRejection::InvalidDependencyMetric, Language::JaJp) => "依存指標が無効",
        (GrayRhinoEvidenceRejection::MissingInstitutionalMetric, Language::ZhCn) => {
            "缺少制度成熟度指标"
        }
        (GrayRhinoEvidenceRejection::MissingInstitutionalMetric, Language::EnUs) => {
            "missing institutional metric"
        }
        (GrayRhinoEvidenceRejection::MissingInstitutionalMetric, Language::JaJp) => {
            "制度成熟度指標が欠落"
        }
        (GrayRhinoEvidenceRejection::InvalidInstitutionalMetric, Language::ZhCn) => {
            "制度成熟度指标无效"
        }
        (GrayRhinoEvidenceRejection::InvalidInstitutionalMetric, Language::EnUs) => {
            "invalid institutional metric"
        }
        (GrayRhinoEvidenceRejection::InvalidInstitutionalMetric, Language::JaJp) => {
            "制度成熟度指標が無効"
        }
        (GrayRhinoEvidenceRejection::MissingRedundancyMetric, Language::ZhCn) => "缺少冗余指标",
        (GrayRhinoEvidenceRejection::MissingRedundancyMetric, Language::EnUs) => {
            "missing redundancy metric"
        }
        (GrayRhinoEvidenceRejection::MissingRedundancyMetric, Language::JaJp) => "冗長性指標が欠落",
        (GrayRhinoEvidenceRejection::InvalidRedundancyMetric, Language::ZhCn) => "冗余指标无效",
        (GrayRhinoEvidenceRejection::InvalidRedundancyMetric, Language::EnUs) => {
            "invalid redundancy metric"
        }
        (GrayRhinoEvidenceRejection::InvalidRedundancyMetric, Language::JaJp) => "冗長性指標が無効",
    }
}

fn readiness_ready_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "就绪",
        Language::EnUs => "ready",
        Language::JaJp => "準備完了",
    }
}

fn readiness_partial_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "部分就绪",
        Language::EnUs => "partial",
        Language::JaJp => "部分的に準備",
    }
}

fn readiness_insufficient_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "不足",
        Language::EnUs => "insufficient",
        Language::JaJp => "不足",
    }
}

fn evidence_explanation_graph_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "证据解释图",
        Language::EnUs => "Evidence Explanation Graph",
        Language::JaJp => "証拠説明グラフ",
    }
}

fn evidence_explanation_graph_body(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "- 依赖集中 -> 依赖集中证据 -> 供应商 / 云服务 / 基础设施披露\n- 后备生存风险 -> 依赖集中 + 冗余缺口 -> 后备与故障切换证据\n- 约束成长 -> 制度成熟度 -> 审计、监督、合规与继任证据\n- 风险扩张 -> 治理集中 + 依赖集中 -> 结构集中证据\n",
        Language::EnUs => "- dependency_centralization -> DependencyConcentration -> supplier/cloud/infrastructure disclosures\n- fallback_survivability_risk -> DependencyConcentration + Redundancy gap -> fallback and failover evidence\n- constraint_growth_rate -> InstitutionalMaturity -> audit, oversight, compliance maturity evidence\n- risk_expansion_rate -> GovernanceConcentration + DependencyConcentration -> structural concentration evidence\n",
        Language::JaJp => "- 依存集中 -> 依存集中証拠 -> 供給元 / クラウド / インフラ開示\n- 代替生存リスク -> 依存集中 + 冗長性不足 -> 代替とフェイルオーバー証拠\n- 制約成長 -> 制度成熟度 -> 監査、監督、コンプライアンス、継承証拠\n- リスク拡張 -> ガバナンス集中 + 依存集中 -> 構造集中証拠\n",
    }
}

/// 未分類の旧証拠の通知をレンダリングする
pub(super) fn render_unclassified_evidence_notice(count: usize, language: Language) -> String {
    match language {
        Language::ZhCn => format!(
            "旧证据记录不可评分\n- 缺少风险作用的记录数: {count}\n- 处理: 已载入但不参与正式升级评分，请重新投影或重新采集。"
        ),
        Language::EnUs => format!(
            "Unclassified legacy evidence\n- records missing risk_effect: {count}\n- handling: loaded but excluded from formal escalation scoring until re-projected or re-collected."
        ),
        Language::JaJp => format!(
            "未分類の旧証拠\n- リスク作用が欠落した記録数: {count}\n- 処理: 読み込みは行うが、再投影または再収集まで正式な昇格採点から除外する。"
        ),
    }
}
