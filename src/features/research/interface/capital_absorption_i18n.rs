use crate::config;
use crate::features::research::application::capital_absorption::{
    CapitalAbsorptionAutoEventCategory, CapitalAbsorptionAutoStatus, CapitalAbsorptionAutoTrend,
    CapitalAbsorptionIpoQueueStatus, CapitalAbsorptionObservationEventType,
    CapitalAbsorptionPotentialSupplyTrend,
};
use crate::features::shared::interface::i18n::Language;

pub(super) fn capital_absorption_title(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "📊 资本吸收早期预警传感器",
        Language::EnUs => "📊 Capital Absorption Early Warning Sensor",
        Language::JaJp => "📊 資本吸収早期警戒センサー",
    }
}

pub(super) fn capital_absorption_empty(language: Language) -> &'static str {
    match language {
        Language::ZhCn => {
            "📊 资本吸收早期预警传感器\n\n未配置资本吸收观察层。\n\n边界: 本模块当前只观察潜在未来资本供给，不生成交易信号。"
        }
        Language::EnUs => {
            "📊 Capital Absorption Early Warning Sensor\n\nNo capital absorption context configured.\n\nBoundary: this module only observes potential future capital supply in the current phase; it does not generate trade signals."
        }
        Language::JaJp => {
            "📊 資本吸収早期警戒センサー\n\n資本吸収観測レイヤーは未設定です。\n\n境界: 現段階では潜在的な将来資本供給だけを観測し、売買シグナルは生成しない。"
        }
    }
}

pub(super) fn capital_absorption_status_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "资本吸收状态:",
        Language::EnUs => "Capital Absorption Status:",
        Language::JaJp => "資本吸収状態:",
    }
}

pub(super) fn capital_absorption_source_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "自动来源:",
        Language::EnUs => "Automatic Source:",
        Language::JaJp => "自動ソース:",
    }
}

pub(super) fn capital_absorption_events_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "发现",
        Language::EnUs => "Observed Events",
        Language::JaJp => "観測イベント",
    }
}

pub(super) fn capital_absorption_no_events(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "- 未观察到大型资本吸收事件。",
        Language::EnUs => "- No large capital absorption events observed.",
        Language::JaJp => "- 大型の資本吸収イベントは未観測です。",
    }
}

pub(super) fn capital_absorption_actual_supply_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "实际资本供给",
        Language::EnUs => "Actual Capital Supply",
        Language::JaJp => "実際の資本供給",
    }
}

pub(super) fn capital_absorption_potential_supply_trend_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "潜在供给趋势",
        Language::EnUs => "Potential Supply Trend",
        Language::JaJp => "潜在供給トレンド",
    }
}

pub(super) fn capital_absorption_no_actual_supply(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "- 未观察到已发生的大型股权/可转债供给。",
        Language::EnUs => "- No completed large equity or convertible supply observed.",
        Language::JaJp => "- 発生済みの大型株式・転換社債供給は未観測です。",
    }
}

pub(super) fn capital_absorption_supply_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "资本供给趋势",
        Language::EnUs => "Capital Supply",
        Language::JaJp => "資本供給トレンド",
    }
}

pub(super) fn capital_absorption_ratio_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "资本吸收比率:",
        Language::EnUs => "Capital Absorption Ratio:",
        Language::JaJp => "資本吸収比率:",
    }
}

pub(super) fn capital_absorption_structural_impact_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "结构影响:",
        Language::EnUs => "Structural Impact:",
        Language::JaJp => "構造的影響:",
    }
}

pub(super) fn capital_absorption_boundary(language: Language) -> &'static str {
    match language {
        Language::ZhCn => {
            "边界: 本模块当前只作为 Early Warning Sensor，不测量实际资本吸收，不测量市场流动性，不产生市场结论，不生成交易信号，不进行风险评级升级，不影响 READY / EXECUTE / Position Sizing / Gate / Trend Layer。"
        }
        Language::EnUs => {
            "Boundary: this module is currently only an Early Warning Sensor. It does not measure actual capital absorption, market liquidity, or market conclusions; it does not generate trading signals or risk-rating upgrades; it does not affect READY / EXECUTE / Position Sizing / Gate / Trend Layer."
        }
        Language::JaJp => {
            "境界: このモジュールは現段階では Early Warning Sensor に限定する。実際の資本吸収、市場流動性、市場結論を測定せず、売買シグナルやリスク格上げを生成せず、READY / EXECUTE / Position Sizing / Gate / Trend Layer に影響しない。"
        }
    }
}

pub(super) fn capital_absorption_current_phase_boundary(language: Language) -> &'static str {
    match language {
        Language::ZhCn => {
            "当前阶段: Narrative Observation Only。\n观察对象: Potential Future Capital Supply，而不是 Actual Capital Absorption。\n解释边界: IPO 新闻增加不等于资本供给增加；资本供给增加不等于市场吸收失败；市场吸收失败不等于市场风险上升。\n状态边界: 当前阶段仅允许 NORMAL / WATCH；ACTIVE / STRESSED 保留到接入 Capital Supply 数据与 Rolling 12M Capital Model 后再评估。"
        }
        Language::EnUs => {
            "Current Phase: Narrative Observation Only.\nObject: Potential Future Capital Supply, not Actual Capital Absorption.\nInterpretation boundary: more IPO news does not equal more actual capital supply; more supply does not equal failed market absorption; failed absorption does not equal higher market risk.\nStatus boundary: only NORMAL / WATCH are allowed in this phase; ACTIVE / STRESSED are reserved until Capital Supply data and a Rolling 12M Capital Model are connected."
        }
        Language::JaJp => {
            "現段階: Narrative Observation Only。\n観測対象: Actual Capital Absorption ではなく Potential Future Capital Supply。\n解釈境界: IPO ニュース増加は実際の資本供給増加と同義ではない。資本供給増加は市場吸収失敗と同義ではない。市場吸収失敗は市場リスク上昇と同義ではない。\n状態境界: 現段階では NORMAL / WATCH のみを許可し、ACTIVE / STRESSED は Capital Supply data と Rolling 12M Capital Model 接続後に再評価する。"
        }
    }
}

pub(super) fn capital_absorption_trend_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "趋势:",
        Language::EnUs => "Trend:",
        Language::JaJp => "トレンド:",
    }
}

pub(super) fn capital_absorption_rolling_12m_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "滚动 12 个月:",
        Language::EnUs => "Rolling 12M:",
        Language::JaJp => "ローリング 12 か月:",
    }
}

pub(super) fn capital_absorption_observed_actual_amount_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "已观察实际供给:",
        Language::EnUs => "Observed actual supply:",
        Language::JaJp => "観測済み実供給:",
    }
}

pub(super) fn capital_absorption_score_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "评分:",
        Language::EnUs => "Score:",
        Language::JaJp => "スコア:",
    }
}

pub(super) fn capital_absorption_ipo_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "IPO 融资:",
        Language::EnUs => "IPO financing:",
        Language::JaJp => "IPO 調達:",
    }
}

pub(super) fn capital_absorption_secondary_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "增发融资:",
        Language::EnUs => "Secondary offering:",
        Language::JaJp => "増資:",
    }
}

pub(super) fn capital_absorption_convertible_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "可转债融资:",
        Language::EnUs => "Convertible debt:",
        Language::JaJp => "転換社債:",
    }
}

pub(super) fn capital_absorption_ai_related_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "AI 相关融资:",
        Language::EnUs => "AI-related financing:",
        Language::JaJp => "AI 関連調達:",
    }
}

pub(super) fn capital_absorption_etf_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "ETF 净流入:",
        Language::EnUs => "ETF net inflow:",
        Language::JaJp => "ETF 純流入:",
    }
}

pub(super) fn capital_absorption_mutual_fund_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "共同基金净流入:",
        Language::EnUs => "Mutual fund net inflow:",
        Language::JaJp => "投資信託純流入:",
    }
}

pub(super) fn capital_absorption_pension_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "养老金配置流:",
        Language::EnUs => "Pension allocation flow:",
        Language::JaJp => "年金配分フロー:",
    }
}

pub(super) fn capital_absorption_foreign_capital_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "外资流入:",
        Language::EnUs => "Foreign capital inflow:",
        Language::JaJp => "海外資本流入:",
    }
}

pub(super) fn capital_absorption_buyback_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "公司回购:",
        Language::EnUs => "Corporate buyback:",
        Language::JaJp => "自社株買い:",
    }
}

pub(super) fn capital_absorption_supply_event_count_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "实际供给事件",
        Language::EnUs => "Actual Supply Event Count",
        Language::JaJp => "実供給イベント数",
    }
}

pub(super) fn capital_absorption_ai_ipo_queue_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "AI IPO 队列",
        Language::EnUs => "AI IPO Queue",
        Language::JaJp => "AI IPO キュー",
    }
}

pub(super) fn capital_absorption_ipo_queue_history_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "IPO 队列历史",
        Language::EnUs => "IPO Queue History",
        Language::JaJp => "IPO キュー履歴",
    }
}

pub(super) fn capital_absorption_queue_size_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "队列规模",
        Language::EnUs => "Queue Size",
        Language::JaJp => "キュー規模",
    }
}

pub(super) fn capital_absorption_mega_cap_financing_count_label(
    language: Language,
) -> &'static str {
    match language {
        Language::ZhCn => "Mega Cap 融资",
        Language::EnUs => "Mega Cap Financing",
        Language::JaJp => "Mega Cap 調達",
    }
}

pub(super) fn capital_absorption_secondary_offering_count_label(
    language: Language,
) -> &'static str {
    match language {
        Language::ZhCn => "增发",
        Language::EnUs => "Secondary Offering",
        Language::JaJp => "増資",
    }
}

pub(super) fn capital_absorption_convertible_debt_count_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "可转债",
        Language::EnUs => "Convertible Debt",
        Language::JaJp => "転換社債",
    }
}

pub(super) fn capital_absorption_secondary_liquidity_count_label(
    language: Language,
) -> &'static str {
    match language {
        Language::ZhCn => "二级流动性",
        Language::EnUs => "Secondary Liquidity",
        Language::JaJp => "セカンダリー流動性",
    }
}

pub(super) fn capital_absorption_sources_count_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "来源",
        Language::EnUs => "Sources",
        Language::JaJp => "ソース数",
    }
}

pub(super) fn capital_absorption_event_type_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "事件类型",
        Language::EnUs => "Event Type",
        Language::JaJp => "イベント種別",
    }
}

pub(super) fn capital_absorption_ipo_stage_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "IPO 阶段",
        Language::EnUs => "IPO Stage",
        Language::JaJp => "IPO 段階",
    }
}

pub(super) fn capital_absorption_discovery_new_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "新增",
        Language::EnUs => "New",
        Language::JaJp => "新規",
    }
}

pub(super) fn capital_absorption_discovery_upgraded_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "升级",
        Language::EnUs => "Upgraded",
        Language::JaJp => "上昇",
    }
}

pub(super) fn capital_absorption_discovery_downgraded_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "降级",
        Language::EnUs => "Downgraded",
        Language::JaJp => "低下",
    }
}

pub(super) fn capital_absorption_discovery_disappeared_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "消失",
        Language::EnUs => "Disappeared",
        Language::JaJp => "消失",
    }
}

pub(super) fn capital_absorption_none_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "无",
        Language::EnUs => "None",
        Language::JaJp => "なし",
    }
}

pub(super) fn capital_absorption_ratio_disabled_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "本阶段未启用完整量化",
        Language::EnUs => "Full quantification disabled in this phase",
        Language::JaJp => "本段階では完全な定量化を未使用",
    }
}

pub(super) fn capital_absorption_observation_only_value(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "仅观察",
        Language::EnUs => "Observation Only",
        Language::JaJp => "観測のみ",
    }
}

pub(super) fn capital_absorption_structural_impact_value(
    value: Option<&str>,
    language: Language,
) -> String {
    match value.map(str::trim).filter(|value| !value.is_empty()) {
        Some("Observation Only") | None => {
            capital_absorption_observation_only_value(language).to_string()
        }
        Some(value) => value.to_string(),
    }
}

pub(super) fn capped_config_status(
    status: config::CapitalAbsorptionStatus,
) -> config::CapitalAbsorptionStatus {
    match status {
        config::CapitalAbsorptionStatus::Normal => config::CapitalAbsorptionStatus::Normal,
        config::CapitalAbsorptionStatus::Watch
        | config::CapitalAbsorptionStatus::Active
        | config::CapitalAbsorptionStatus::Stressed => config::CapitalAbsorptionStatus::Watch,
    }
}

pub(super) fn capital_absorption_status_value(
    status: config::CapitalAbsorptionStatus,
    language: Language,
) -> String {
    match status {
        config::CapitalAbsorptionStatus::Normal => {
            capital_absorption_status_text("NORMAL", language)
        }
        config::CapitalAbsorptionStatus::Watch => capital_absorption_status_text("WATCH", language),
        config::CapitalAbsorptionStatus::Active => {
            capital_absorption_status_text("ACTIVE", language)
        }
        config::CapitalAbsorptionStatus::Stressed => {
            capital_absorption_status_text("STRESSED", language)
        }
    }
}

pub(super) fn capital_absorption_trend_value(
    trend: config::CapitalAbsorptionTrend,
    language: Language,
) -> String {
    match trend {
        config::CapitalAbsorptionTrend::Decreasing => {
            capital_absorption_trend_text("FALLING", language)
        }
        config::CapitalAbsorptionTrend::Stable => capital_absorption_trend_text("STABLE", language),
        config::CapitalAbsorptionTrend::Increasing => {
            capital_absorption_trend_text("RISING", language)
        }
        config::CapitalAbsorptionTrend::Accelerating => {
            capital_absorption_trend_text("RISING", language)
        }
    }
}

pub(super) fn capital_absorption_ratio_state_value(
    state: config::CapitalAbsorptionRatioState,
    language: Language,
) -> String {
    match state {
        config::CapitalAbsorptionRatioState::Low => capital_absorption_ratio_text("LOW", language),
        config::CapitalAbsorptionRatioState::Neutral => {
            capital_absorption_ratio_text("NEUTRAL", language)
        }
        config::CapitalAbsorptionRatioState::Elevated => {
            capital_absorption_ratio_text("ELEVATED", language)
        }
        config::CapitalAbsorptionRatioState::Stressed => {
            capital_absorption_ratio_text("STRESSED", language)
        }
    }
}

pub(super) fn capital_absorption_event_category_value(
    category: config::CapitalAbsorptionEventCategory,
    language: Language,
) -> String {
    match category {
        config::CapitalAbsorptionEventCategory::MegaCapFinancing => {
            capital_absorption_auto_event_category_value(
                CapitalAbsorptionAutoEventCategory::MegaCapFinancing,
                language,
            )
        }
        config::CapitalAbsorptionEventCategory::IpoSupply => {
            capital_absorption_auto_event_category_value(
                CapitalAbsorptionAutoEventCategory::IpoSupply,
                language,
            )
        }
        config::CapitalAbsorptionEventCategory::SecondaryLiquidity => {
            capital_absorption_auto_event_category_value(
                CapitalAbsorptionAutoEventCategory::SecondaryLiquidity,
                language,
            )
        }
    }
}

pub(super) fn capital_absorption_auto_status_value(
    status: CapitalAbsorptionAutoStatus,
    language: Language,
) -> String {
    match status {
        CapitalAbsorptionAutoStatus::Normal => capital_absorption_status_text("NORMAL", language),
        CapitalAbsorptionAutoStatus::Watch => capital_absorption_status_text("WATCH", language),
    }
}

pub(super) fn capital_absorption_auto_trend_value(
    trend: CapitalAbsorptionAutoTrend,
    language: Language,
) -> String {
    match trend {
        CapitalAbsorptionAutoTrend::Decreasing => {
            capital_absorption_trend_text("FALLING", language)
        }
        CapitalAbsorptionAutoTrend::Stable => capital_absorption_trend_text("STABLE", language),
    }
}

pub(super) fn capital_absorption_potential_supply_trend_value(
    trend: CapitalAbsorptionPotentialSupplyTrend,
    language: Language,
) -> String {
    match trend {
        CapitalAbsorptionPotentialSupplyTrend::Falling => {
            capital_absorption_trend_text("FALLING", language)
        }
        CapitalAbsorptionPotentialSupplyTrend::Stable => {
            capital_absorption_trend_text("STABLE", language)
        }
        CapitalAbsorptionPotentialSupplyTrend::Rising => {
            capital_absorption_trend_text("RISING", language)
        }
    }
}

pub(super) fn capital_absorption_auto_ratio_state_value(
    state: crate::features::research::application::capital_absorption::CapitalAbsorptionAutoRatioState,
    language: Language,
) -> String {
    match state {
        crate::features::research::application::capital_absorption::CapitalAbsorptionAutoRatioState::Low => capital_absorption_ratio_text("LOW", language),
        crate::features::research::application::capital_absorption::CapitalAbsorptionAutoRatioState::Neutral => capital_absorption_ratio_text("NEUTRAL", language),
    }
}

pub(super) fn capital_absorption_auto_event_category_value(
    category: CapitalAbsorptionAutoEventCategory,
    language: Language,
) -> String {
    match category {
        CapitalAbsorptionAutoEventCategory::MegaCapFinancing => match language {
            Language::ZhCn => "Mega Cap 融资".to_string(),
            Language::EnUs => "Mega Cap Financing".to_string(),
            Language::JaJp => "Mega Cap 調達".to_string(),
        },
        CapitalAbsorptionAutoEventCategory::IpoSupply => match language {
            Language::ZhCn => "IPO 供给".to_string(),
            Language::EnUs => "IPO Supply".to_string(),
            Language::JaJp => "IPO 供給".to_string(),
        },
        CapitalAbsorptionAutoEventCategory::SecondaryLiquidity => match language {
            Language::ZhCn => "二级流动性".to_string(),
            Language::EnUs => "Secondary Liquidity".to_string(),
            Language::JaJp => "セカンダリー流動性".to_string(),
        },
    }
}

pub(super) fn capital_absorption_status_text(code: &str, language: Language) -> String {
    match (code, language) {
        ("NORMAL", Language::ZhCn) => "正常（NORMAL）".to_string(),
        ("WATCH", Language::ZhCn) => "观察（WATCH）".to_string(),
        ("ACTIVE", Language::ZhCn) => "结构观察（ACTIVE）".to_string(),
        ("STRESSED", Language::ZhCn) => "流动性压力（STRESSED）".to_string(),
        ("NORMAL", Language::JaJp) => "通常（NORMAL）".to_string(),
        ("WATCH", Language::JaJp) => "観察（WATCH）".to_string(),
        ("ACTIVE", Language::JaJp) => "構造観察（ACTIVE）".to_string(),
        ("STRESSED", Language::JaJp) => "流動性圧力（STRESSED）".to_string(),
        _ => code.to_string(),
    }
}

pub(super) fn capital_absorption_trend_text(code: &str, language: Language) -> String {
    match (code, language) {
        ("FALLING", Language::ZhCn) => "下降（FALLING）".to_string(),
        ("RISING", Language::ZhCn) => "上升（RISING）".to_string(),
        ("DECREASING", Language::ZhCn) => "下降（DECREASING）".to_string(),
        ("STABLE", Language::ZhCn) => "稳定（STABLE）".to_string(),
        ("INCREASING", Language::ZhCn) => "上升（INCREASING）".to_string(),
        ("ACCELERATING", Language::ZhCn) => "加速（ACCELERATING）".to_string(),
        ("FALLING", Language::JaJp) => "低下（FALLING）".to_string(),
        ("RISING", Language::JaJp) => "上昇（RISING）".to_string(),
        ("DECREASING", Language::JaJp) => "低下（DECREASING）".to_string(),
        ("STABLE", Language::JaJp) => "安定（STABLE）".to_string(),
        ("INCREASING", Language::JaJp) => "上昇（INCREASING）".to_string(),
        ("ACCELERATING", Language::JaJp) => "加速（ACCELERATING）".to_string(),
        _ => code.to_string(),
    }
}

pub(super) fn capital_absorption_event_type_value(
    event_type: CapitalAbsorptionObservationEventType,
    language: Language,
) -> &'static str {
    match (event_type, language) {
        (CapitalAbsorptionObservationEventType::Confirmed, Language::ZhCn) => "确认（Confirmed）",
        (CapitalAbsorptionObservationEventType::Reported, Language::ZhCn) => "报道（Reported）",
        (CapitalAbsorptionObservationEventType::Rumor, Language::ZhCn) => "传闻（Rumor）",
        (CapitalAbsorptionObservationEventType::Confirmed, Language::EnUs) => "Confirmed",
        (CapitalAbsorptionObservationEventType::Reported, Language::EnUs) => "Reported",
        (CapitalAbsorptionObservationEventType::Rumor, Language::EnUs) => "Rumor",
        (CapitalAbsorptionObservationEventType::Confirmed, Language::JaJp) => "確認（Confirmed）",
        (CapitalAbsorptionObservationEventType::Reported, Language::JaJp) => "報道（Reported）",
        (CapitalAbsorptionObservationEventType::Rumor, Language::JaJp) => "噂（Rumor）",
    }
}

pub(super) fn capital_absorption_ratio_text(code: &str, language: Language) -> String {
    match (code, language) {
        ("LOW", Language::ZhCn) => "低（LOW）".to_string(),
        ("NEUTRAL", Language::ZhCn) => "中性（NEUTRAL）".to_string(),
        ("ELEVATED", Language::ZhCn) => "偏高（ELEVATED）".to_string(),
        ("STRESSED", Language::ZhCn) => "压力（STRESSED）".to_string(),
        ("LOW", Language::JaJp) => "低い（LOW）".to_string(),
        ("NEUTRAL", Language::JaJp) => "中立（NEUTRAL）".to_string(),
        ("ELEVATED", Language::JaJp) => "高め（ELEVATED）".to_string(),
        ("STRESSED", Language::JaJp) => "圧迫（STRESSED）".to_string(),
        _ => code.to_string(),
    }
}

pub(super) fn capital_absorption_ipo_queue_status_value(
    status: CapitalAbsorptionIpoQueueStatus,
    language: Language,
) -> String {
    match (status, language) {
        (CapitalAbsorptionIpoQueueStatus::Rumor, Language::ZhCn) => "传闻（Rumor）".to_string(),
        (CapitalAbsorptionIpoQueueStatus::Reported, Language::ZhCn) => {
            "报道（Reported）".to_string()
        }
        (CapitalAbsorptionIpoQueueStatus::Preparing, Language::ZhCn) => {
            "准备中（Preparing）".to_string()
        }
        (CapitalAbsorptionIpoQueueStatus::Filed, Language::ZhCn) => "已提交（Filed）".to_string(),
        (CapitalAbsorptionIpoQueueStatus::Roadshow, Language::ZhCn) => {
            "路演（Roadshow）".to_string()
        }
        (CapitalAbsorptionIpoQueueStatus::Priced, Language::ZhCn) => "已定价（Priced）".to_string(),
        (CapitalAbsorptionIpoQueueStatus::Listed, Language::ZhCn) => "已上市（Listed）".to_string(),
        (CapitalAbsorptionIpoQueueStatus::Rumor, Language::JaJp) => "噂（Rumor）".to_string(),
        (CapitalAbsorptionIpoQueueStatus::Reported, Language::JaJp) => {
            "報道（Reported）".to_string()
        }
        (CapitalAbsorptionIpoQueueStatus::Preparing, Language::JaJp) => {
            "準備中（Preparing）".to_string()
        }
        (CapitalAbsorptionIpoQueueStatus::Filed, Language::JaJp) => "提出済み（Filed）".to_string(),
        (CapitalAbsorptionIpoQueueStatus::Roadshow, Language::JaJp) => {
            "ロードショー（Roadshow）".to_string()
        }
        (CapitalAbsorptionIpoQueueStatus::Priced, Language::JaJp) => {
            "価格決定済み（Priced）".to_string()
        }
        (CapitalAbsorptionIpoQueueStatus::Listed, Language::JaJp) => {
            "上場済み（Listed）".to_string()
        }
        (CapitalAbsorptionIpoQueueStatus::Rumor, Language::EnUs) => "Rumor".to_string(),
        (CapitalAbsorptionIpoQueueStatus::Reported, Language::EnUs) => "Reported".to_string(),
        (CapitalAbsorptionIpoQueueStatus::Preparing, Language::EnUs) => "Preparing".to_string(),
        (CapitalAbsorptionIpoQueueStatus::Filed, Language::EnUs) => "Filed".to_string(),
        (CapitalAbsorptionIpoQueueStatus::Roadshow, Language::EnUs) => "Roadshow".to_string(),
        (CapitalAbsorptionIpoQueueStatus::Priced, Language::EnUs) => "Priced".to_string(),
        (CapitalAbsorptionIpoQueueStatus::Listed, Language::EnUs) => "Listed".to_string(),
    }
}
