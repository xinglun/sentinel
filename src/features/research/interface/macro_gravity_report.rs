use crate::config;
use crate::features::shared::interface::i18n::Language;

pub(crate) fn build_macro_gravity_report_from_config(
    macro_gravity: Option<&config::MacroGravityConfig>,
    language: Language,
) -> String {
    let Some(macro_gravity) =
        macro_gravity.filter(|macro_gravity| macro_gravity.enable.unwrap_or(true))
    else {
        return macro_gravity_empty(language).to_string();
    };

    let mut out = String::new();
    out.push_str(macro_gravity_title(language));
    out.push_str("\n\n");
    out.push_str(&format!(
        "{} {}\n",
        macro_gravity_rate_pressure_label(language),
        macro_pressure_label(macro_gravity.rate_pressure)
    ));
    out.push_str(&format!(
        "{} {}\n",
        macro_gravity_real_yield_label(language),
        macro_pressure_label(macro_gravity.real_yield_pressure)
    ));
    out.push_str(&format!(
        "{} {}\n",
        macro_gravity_curve_label(language),
        yield_curve_label(macro_gravity.yield_curve)
    ));
    out.push_str(&format!(
        "{} {}\n",
        macro_gravity_credit_label(language),
        credit_stress_label(macro_gravity.credit_stress)
    ));
    out.push_str(&format!(
        "{} {}\n",
        macro_gravity_liquidity_label(language),
        liquidity_condition_label(macro_gravity.liquidity)
    ));
    out.push_str(&format!(
        "{} {}\n",
        macro_gravity_growth_valuation_label(language),
        growth_valuation_impact_label(macro_gravity.growth_valuation_impact)
    ));
    out.push('\n');
    out.push_str(macro_gravity_boundary(language));
    out
}

fn macro_gravity_title(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "🌐 宏观重力",
        Language::EnUs => "🌐 Macro Gravity",
        Language::JaJp => "🌐 マクロ重力",
    }
}

fn macro_gravity_empty(language: Language) -> &'static str {
    match language {
        Language::ZhCn => {
            "🌐 宏观重力\n\n未配置宏观重力支线。\n\n边界: 债券与信用环境只解释折现率和流动性，不生成交易信号。"
        }
        Language::EnUs => {
            "🌐 Macro Gravity\n\nNo macro gravity context configured.\n\nBoundary: bond and credit context only explains discount-rate and liquidity conditions; it does not generate trade signals."
        }
        Language::JaJp => {
            "🌐 マクロ重力\n\nマクロ重力コンテキストは未設定です。\n\n境界: 債券と信用環境は割引率と流動性だけを説明し、売買シグナルは生成しない。"
        }
    }
}

fn macro_gravity_rate_pressure_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "- 利率压力:",
        Language::EnUs => "- Rate pressure:",
        Language::JaJp => "- 金利圧力:",
    }
}

fn macro_gravity_real_yield_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "- 实际利率:",
        Language::EnUs => "- Real yield:",
        Language::JaJp => "- 実質金利:",
    }
}

fn macro_gravity_curve_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "- 收益率曲线:",
        Language::EnUs => "- Yield curve:",
        Language::JaJp => "- イールドカーブ:",
    }
}

fn macro_gravity_credit_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "- 信用压力:",
        Language::EnUs => "- Credit stress:",
        Language::JaJp => "- 信用圧力:",
    }
}

fn macro_gravity_liquidity_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "- 流动性:",
        Language::EnUs => "- Liquidity:",
        Language::JaJp => "- 流動性:",
    }
}

fn macro_gravity_growth_valuation_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "- 成长股估值:",
        Language::EnUs => "- Growth valuation:",
        Language::JaJp => "- 成長株バリュエーション:",
    }
}

fn macro_gravity_boundary(language: Language) -> &'static str {
    match language {
        Language::ZhCn => {
            "边界: 宏观重力只解释市场折现率、流动性和估值压力；不参与 Gate，不生成交易指令。"
        }
        Language::EnUs => {
            "Boundary: macro gravity only explains discount rates, liquidity, and valuation pressure; it does not enter Gate or generate trade instructions."
        }
        Language::JaJp => {
            "境界: マクロ重力は割引率、流動性、バリュエーション圧力だけを説明し、Gate に入らず、売買指示も生成しない。"
        }
    }
}

pub(crate) fn macro_pressure_label(value: config::MacroPressure) -> &'static str {
    match value {
        config::MacroPressure::Falling => "FALLING",
        config::MacroPressure::Neutral => "NEUTRAL",
        config::MacroPressure::Rising => "RISING",
        config::MacroPressure::Tight => "TIGHT",
    }
}

pub(crate) fn yield_curve_label(value: config::YieldCurveState) -> &'static str {
    match value {
        config::YieldCurveState::Normal => "NORMAL",
        config::YieldCurveState::Flat => "FLAT",
        config::YieldCurveState::Inverted => "INVERTED",
        config::YieldCurveState::Steepening => "STEEPENING",
    }
}

pub(crate) fn credit_stress_label(value: config::CreditStress) -> &'static str {
    match value {
        config::CreditStress::Normal => "NORMAL",
        config::CreditStress::Watch => "WATCH",
        config::CreditStress::Stress => "STRESS",
    }
}

pub(crate) fn liquidity_condition_label(value: config::LiquidityCondition) -> &'static str {
    match value {
        config::LiquidityCondition::Loose => "LOOSE",
        config::LiquidityCondition::Neutral => "NEUTRAL",
        config::LiquidityCondition::Tight => "TIGHT",
    }
}

pub(crate) fn growth_valuation_impact_label(value: config::GrowthValuationImpact) -> &'static str {
    match value {
        config::GrowthValuationImpact::Supportive => "SUPPORTIVE",
        config::GrowthValuationImpact::Neutral => "NEUTRAL",
        config::GrowthValuationImpact::Compressing => "COMPRESSING",
    }
}
