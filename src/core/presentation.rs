use crate::core::display::TopActionViewModel;
use crate::core::i18n::Language;
use serde::{Deserialize, Serialize};

fn default_language() -> Language {
    Language::ZhCn
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct MacroDisplayContext {
    pub headline: String,
    pub summary: String,
    pub risk_label: String,
    pub bias_label: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct DataAlertViewModel {
    pub prefix: String,
    pub label: String,
    pub message: String,
    pub symbols: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct PresentationPacket {
    pub date_str: String,
    #[serde(default = "default_language")]
    pub language: Language,
    pub macro_display: MacroDisplayContext,
    pub top_actions: Vec<TopActionViewModel>,
    #[serde(default)]
    pub data_alert: Option<DataAlertViewModel>,
    // For the terminal table and archival markdown
    pub terminal_rows: Vec<TerminalRowViewModel>,
    pub state_code: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct TerminalRowViewModel {
    pub symbol: String,
    pub state_label: String,
    pub intent_label: String,
    pub action_label: String,
    pub owner_dev_label: String,
    pub strength_z_label: String,
}
