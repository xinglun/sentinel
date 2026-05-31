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

pub(crate) fn backfill_ops_title(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "回填运维视图\n",
        Language::EnUs => "Backfill Ops View\n",
        Language::JaJp => "回填運用ビュー\n",
    }
}

pub(crate) fn failed_sources_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "失败来源数",
        Language::EnUs => "failed_sources",
        Language::JaJp => "失敗した由来数",
    }
}

pub(crate) fn stale_sources_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "陈旧来源数",
        Language::EnUs => "stale_sources",
        Language::JaJp => "古い由来数",
    }
}

pub(crate) fn drift_sources_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "漂移来源数",
        Language::EnUs => "drift_sources",
        Language::JaJp => "漂移した由来数",
    }
}

pub(crate) fn auto_discovery_ops_title(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "自动发现运维视图\n",
        Language::EnUs => "Auto Discovery Ops View\n",
        Language::JaJp => "自動発見運用ビュー\n",
    }
}

pub(crate) fn latest_run_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "最新运行",
        Language::EnUs => "latest_run",
        Language::JaJp => "最新実行",
    }
}

pub(crate) fn source_count_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "来源数",
        Language::EnUs => "source_count",
        Language::JaJp => "由来数",
    }
}

pub(crate) fn candidate_count_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "候选数",
        Language::EnUs => "candidate_count",
        Language::JaJp => "候補数",
    }
}

pub(crate) fn refresh_status_title(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "灰犀牛采集状态\n",
        Language::EnUs => "Gray Rhino Refresh Status\n",
        Language::JaJp => "灰色のサイ収集状態\n",
    }
}

pub(crate) fn refresh_overall_status_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "整体状态",
        Language::EnUs => "overall_status",
        Language::JaJp => "全体状態",
    }
}

pub(crate) fn refresh_status_value_label(value: &str, language: Language) -> &'static str {
    match (value, language) {
        ("succeeded", Language::ZhCn) => "成功",
        ("partial_failure", Language::ZhCn) => "部分失败",
        ("failed", Language::ZhCn) => "失败",
        ("skipped", Language::ZhCn) => "跳过",
        ("succeeded", Language::EnUs) => "succeeded",
        ("partial_failure", Language::EnUs) => "partial_failure",
        ("failed", Language::EnUs) => "failed",
        ("skipped", Language::EnUs) => "skipped",
        ("succeeded", Language::JaJp) => "成功",
        ("partial_failure", Language::JaJp) => "部分失敗",
        ("failed", Language::JaJp) => "失敗",
        ("skipped", Language::JaJp) => "未実行",
        _ => "unknown",
    }
}

pub(crate) fn refresh_coverage_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "覆盖率",
        Language::EnUs => "coverage",
        Language::JaJp => "取得カバー率",
    }
}

pub(crate) fn failed_providers_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "失败来源",
        Language::EnUs => "failed_providers",
        Language::JaJp => "失敗した取得元",
    }
}

pub(crate) fn refresh_date_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "采集日期",
        Language::EnUs => "refresh_date",
        Language::JaJp => "収集日",
    }
}

pub(crate) fn refresh_reason_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "原因",
        Language::EnUs => "reason",
        Language::JaJp => "理由",
    }
}

pub(crate) fn refresh_status_boundary(language: Language) -> &'static str {
    match language {
        Language::ZhCn => {
            "边界声明: 采集状态仅说明自动情报新鲜度；不改变交易、闸门、趋势或市场状态。"
        }
        Language::EnUs => {
            "Boundary: refresh status only explains intelligence freshness; it does not change trading, Gate, trend, or market state."
        }
        Language::JaJp => {
            "境界: 収集状態は自動情報の鮮度だけを説明し、取引、ゲート、トレンド、市場状態を変更しない。"
        }
    }
}
