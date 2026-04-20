use super::models::{ActionStatus, BreakoutStatus, LifecycleState, MarketStateOutput};

pub struct MarketStateRenderer;

impl MarketStateRenderer {
    pub fn render(output: &MarketStateOutput) -> String {
        let state_str = match output.lifecycle {
            LifecycleState::Startup => "起動期",
            LifecycleState::Transition => "移行期",
            LifecycleState::Ready => "レディ",
        };

        let mut lines = Vec::new();
        lines.push("🌍 マーケット状態サマリー".to_string());
        lines.push(format!("市場状態: {}", state_str));

        match &output.action_status {
            ActionStatus::NoTrade(reasons) => {
                lines.push("\n🚫 意思決定: 禁止アクション（NO TRADE）".to_string());
                lines.push("未準備の理由:".to_string());
                for reason in reasons {
                    lines.push(format!("• {}", reason));
                }
            }
            ActionStatus::Participate => {
                lines.push("\n✅ 意思決定: 参加許可（TRADE ALLOWED）".to_string());
                lines.push("すべての定量的な条件が満たされています。".to_string());
            }
        }

        lines.push("\n🚀 ブレイクアウト識別".to_string());
        if output.breakout_changes.is_empty() {
            lines.push("• 変化なし".to_string());
        } else {
            for change in &output.breakout_changes {
                let status_str = match change.status {
                    BreakoutStatus::New => "新規",
                    BreakoutStatus::Removed => "除外",
                    BreakoutStatus::Unchanged => "継続",
                };
                lines.push(format!("• {} · {}", change.symbol, status_str));
            }
        }

        lines.join("\n")
    }
}
