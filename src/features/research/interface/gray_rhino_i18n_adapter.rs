use crate::features::shared::interface::i18n::Language;

pub(crate) fn governance_sensor_health_heading(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "治理传感器健康度",
        Language::EnUs => "Governance Sensor Health",
        Language::JaJp => "ガバナンスセンサー健全性",
    }
}

pub(crate) fn governance_sensor_source_count_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "来源数",
        Language::EnUs => "Source count",
        Language::JaJp => "由来数",
    }
}

pub(crate) fn governance_sensor_accepted_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "已接受",
        Language::EnUs => "Accepted",
        Language::JaJp => "受理済み",
    }
}

pub(crate) fn governance_sensor_rejected_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "已拒绝",
        Language::EnUs => "Rejected",
        Language::JaJp => "拒否済み",
    }
}

pub(crate) fn governance_sensor_coverage_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "覆盖率",
        Language::EnUs => "Coverage ratio",
        Language::JaJp => "カバー率",
    }
}

pub(crate) fn governance_sensor_latest_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "最新观测日",
        Language::EnUs => "Latest observed date",
        Language::JaJp => "最新観測日",
    }
}

pub(crate) fn governance_sensor_boundary_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => {
            "边界声明: 治理传感器健康度仅用于证据覆盖检查，不更新升级状态、交易执行或交易状态。"
        }
        Language::EnUs => {
            "Boundary: Governance sensor health only; no escalation, Gate, execution, or trading state is updated."
        }
        Language::JaJp => {
            "境界声明: ガバナンスセンサー健全性は証拠カバー率の確認のみで、昇格状態、実行、取引状態を更新しない。"
        }
    }
}
