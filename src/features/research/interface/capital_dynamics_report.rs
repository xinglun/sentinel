use crate::features::shared::interface::i18n::Language;

/// Capital Dynamics の report shell を組み立てる。
pub(crate) fn build_capital_dynamics_report(
    supply_report: &str,
    flow_report: Option<&str>,
    language: Language,
) -> String {
    let mut out = String::new();
    out.push_str(capital_dynamics_title(language));
    out.push_str("\n\n");
    out.push_str(capital_dynamics_supply_layer_label(language));
    out.push_str("\n\n");
    out.push_str(supply_report.trim());
    if let Some(flow_report) = flow_report.filter(|report| !report.trim().is_empty()) {
        out.push_str("\n\n");
        out.push_str(capital_dynamics_demand_layer_label(language));
        out.push_str("\n\n");
        out.push_str(flow_report.trim());
    }
    out.push_str("\n\n");
    out.push_str(capital_dynamics_balance_layer_label(language));
    out.push_str("\n\n");
    out.push_str(capital_dynamics_balance_layer_note(language));
    out.push_str("\n\n");
    out.push_str(capital_dynamics_boundary(language));
    out
}

fn capital_dynamics_title(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "🧱 Capital Dynamics（供需观察）",
        Language::EnUs => "🧱 Capital Dynamics (Supply / Demand Observation)",
        Language::JaJp => "🧱 Capital Dynamics（需給観測）",
    }
}

fn capital_dynamics_supply_layer_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "### 6.1 Supply Layer（Capital Absorption）",
        Language::EnUs => "### 6.1 Supply Layer (Capital Absorption)",
        Language::JaJp => "### 6.1 Supply Layer（Capital Absorption）",
    }
}

fn capital_dynamics_demand_layer_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "### 6.2 Demand Layer（Flow Layer）",
        Language::EnUs => "### 6.2 Demand Layer (Flow Layer)",
        Language::JaJp => "### 6.2 Demand Layer（Flow Layer）",
    }
}

fn capital_dynamics_balance_layer_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "### 6.3 Balance Layer（Future Placeholder）",
        Language::EnUs => "### 6.3 Balance Layer (Future Placeholder)",
        Language::JaJp => "### 6.3 Balance Layer（Future Placeholder）",
    }
}

fn capital_dynamics_balance_layer_note(language: Language) -> &'static str {
    match language {
        Language::ZhCn => {
            "Balance Layer は将来の派生観測であり、現段階では未計算の占位節です。Supply と Flow の観測結果を壊さず、Gate、Execution、Trader、Action Matrix、Position Sizing には接続しません。"
        }
        Language::EnUs => {
            "Balance Layer is a future derived observation and is currently a placeholder section. It does not change Supply or Flow observations and does not connect to Gate, Execution, Trader, Action Matrix, or Position Sizing."
        }
        Language::JaJp => {
            "Balance Layer は将来の派生観測であり、現段階では未計算の占位節です。Supply と Flow の観測結果を壊さず、Gate、Execution、Trader、Action Matrix、Position Sizing には接続しません。"
        }
    }
}

fn capital_dynamics_boundary(language: Language) -> &'static str {
    match language {
        Language::ZhCn => {
            "边界: Capital Dynamics 仅作供给与需求观察层，不生成新的交易信号，不连接 Gate、Execution、Trader、Action Matrix 或 Position Sizing。"
        }
        Language::EnUs => {
            "Boundary: Capital Dynamics is a supply and demand observation shell only. It does not generate new trade signals or connect to Gate, Execution, Trader, Action Matrix, or Position Sizing."
        }
        Language::JaJp => {
            "境界: Capital Dynamics は供給と需要の観測 shell のみであり、新しい取引シグナルを生成せず、Gate、Execution、Trader、Action Matrix、Position Sizing に接続しない。"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::build_capital_dynamics_report;
    use crate::features::shared::interface::i18n::Language;

    #[test]
    fn capital_dynamics_report_wraps_supply_and_flow_under_one_shell() {
        let report =
            build_capital_dynamics_report("SUPPLY_REPORT", Some("FLOW_REPORT"), Language::ZhCn);

        assert!(report.contains("Capital Dynamics"));
        assert!(report.contains("### 6.1 Supply Layer"));
        assert!(report.contains("SUPPLY_REPORT"));
        assert!(report.contains("### 6.2 Demand Layer"));
        assert!(report.contains("FLOW_REPORT"));
        assert!(report.contains("### 6.3 Balance Layer"));
        assert!(report.contains("未計算"));
        assert!(report.contains("不连接 Gate"));
    }
}
