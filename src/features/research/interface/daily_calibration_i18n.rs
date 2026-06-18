use crate::features::shared::interface::i18n::Language;

pub(crate) fn daily_calibration_title(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "🧭 每日认知校准",
        Language::EnUs => "🧭 Daily Cognitive Calibration",
        Language::JaJp => "🧭 日次認知校正",
    }
}

pub(crate) fn daily_calibration_audit_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "## 1. 今日审计摘要",
        Language::EnUs => "## 1. Daily Audit Summary",
        Language::JaJp => "## 1. 日次監査サマリー",
    }
}

pub(crate) fn daily_calibration_questions_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "## 2. 日报校准问题",
        Language::EnUs => "## 2. Daily Calibration Questions",
        Language::JaJp => "## 2. 日次校正質問",
    }
}

pub(crate) fn daily_calibration_attention_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "## 3. 认知关注校准",
        Language::EnUs => "## 3. Research Attention Calibration",
        Language::JaJp => "## 3. 認知注目の校正",
    }
}

pub(crate) fn daily_calibration_thesis_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "## 4. 资产观察命题",
        Language::EnUs => "## 4. Asset Observation Theses",
        Language::JaJp => "## 4. 銘柄別観測命題",
    }
}

pub(crate) fn daily_calibration_macro_gravity_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "## 5. 宏观重力校准",
        Language::EnUs => "## 5. Macro Gravity Calibration",
        Language::JaJp => "## 5. マクロ重力校正",
    }
}

pub(crate) fn daily_calibration_gray_rhino_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "## 8. 灰犀牛升级监控",
        Language::EnUs => "## 8. Gray Rhino Escalation",
        Language::JaJp => "## 8. 灰色のサイ昇格監視",
    }
}

pub(crate) fn daily_calibration_valuation_gravity_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "## 7. 估值重力层",
        Language::EnUs => "## 7. Valuation Gravity Layer",
        Language::JaJp => "## 7. バリュエーション重力レイヤー",
    }
}

pub(crate) fn daily_calibration_capital_absorption_label(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "## 6. 市场资本吸收监控",
        Language::EnUs => "## 6. Capital Absorption Monitor",
        Language::JaJp => "## 6. 資本吸収モニター",
    }
}

pub(crate) fn daily_calibration_question_market(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "固定问题: 今天是市场理解变化，还是只是噪音变化？",
        Language::EnUs => "Fixed question: did market understanding change today, or only noise?",
        Language::JaJp => "固定質問: 今日変化したのは市場理解か、それともノイズだけか？",
    }
}

pub(crate) fn daily_calibration_question_gate(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "- 战术状态:",
        Language::EnUs => "- Tactical state:",
        Language::JaJp => "- 戦術状態:",
    }
}

pub(crate) fn daily_calibration_question_evidence(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "- 证据状态:",
        Language::EnUs => "- Evidence state:",
        Language::JaJp => "- 証拠状態:",
    }
}

pub(crate) fn daily_calibration_question_attention(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "- 需校准认知对象数:",
        Language::EnUs => "- Attention entries to calibrate:",
        Language::JaJp => "- 校正対象の認知項目数:",
    }
}

pub(crate) fn daily_calibration_question_thesis(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "- 需复查观察命题数:",
        Language::EnUs => "- Observation theses to review:",
        Language::JaJp => "- 再確認する観測命題数:",
    }
}

pub(crate) fn daily_calibration_question_boundary(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "校准口径: 战术状态、证据状态、认知对象和观察命题只用于复盘，不构成新信号。",
        Language::EnUs => {
            "Calibration rule: tactical state, evidence state, attention entries, and observation theses are for review only, not new signals."
        }
        Language::JaJp => {
            "校正口径: 戦術状態、証拠状態、認知項目、観測命題は復盤専用であり、新シグナルではない。"
        }
    }
}

pub(crate) fn daily_calibration_evidence_strong(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "结构证据较强，重点检查价格/扩散是否跟上",
        Language::EnUs => {
            "structural evidence is strong; check whether price/diffusion is following"
        }
        Language::JaJp => "構造証拠は強い。価格/拡散が追随しているか確認",
    }
}

pub(crate) fn daily_calibration_evidence_observed(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "已有结构证据，重点检查质量而非数量",
        Language::EnUs => "structural evidence observed; check quality, not quantity",
        Language::JaJp => "構造証拠を観測中。数量ではなく品質を確認",
    }
}

pub(crate) fn daily_calibration_evidence_none(language: Language) -> &'static str {
    match language {
        Language::ZhCn => "无可用结构证据或审计记录",
        Language::EnUs => "no usable structural evidence or audit record",
        Language::JaJp => "利用可能な構造証拠または監査記録なし",
    }
}

pub(crate) fn daily_calibration_boundary(language: Language) -> &'static str {
    match language {
        Language::ZhCn => {
            "边界: 本日报只校准系统理解、证据质量、认知资源与观察命题；不生成新的交易指令。"
        }
        Language::EnUs => {
            "Boundary: this daily report only calibrates system understanding, evidence quality, cognitive resources, and observation theses; it does not generate new trade instructions."
        }
        Language::JaJp => {
            "境界: この日報はシステム理解、証拠品質、認知資源、観測命題だけを校正し、新しい売買指示は生成しない。"
        }
    }
}
