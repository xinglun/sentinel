#![allow(dead_code)]

use chrono::NaiveDate;

/// 個別銘柄に結び付く供給イベントの事実種別。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum SupplyEventType {
    Ipo,
    LockupExpiry,
    SecondaryOffering,
    FollowOnOffering,
    InsiderSelling,
    EmployeeLiquidityEvent,
    ConvertibleIssuance,
    IndexInclusion,
    IndexExclusion,
    MajorShareholderSale,
    ShareUnlock,
    Unknown,
}

/// 確認済みイベントが潜在的な株式供給に与える向き。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum SupplyDirection {
    Increase,
    Decrease,
    Unknown,
}

/// 供給イベントの根拠強度。投資主体の行動を表すものではない。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum SupplyEventConfidence {
    Low,
    Medium,
    High,
    Unknown,
}

/// 個別供給イベントの事実が利用可能かどうかを表す。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum SupplyEventContextAvailability {
    Available,
    Unavailable,
}

/// Observation Layer が意思決定系へ与える影響を明示する固定境界。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum ObservationEffect {
    None,
}

/// Price-Volume Observation の固定された非取引境界。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct ObservationBoundary {
    pub decision_weight_percent: u8,
    pub trade_signal: bool,
    pub gate_effect: ObservationEffect,
    pub execution_effect: ObservationEffect,
    pub position_sizing_effect: ObservationEffect,
}

impl ObservationBoundary {
    const fn observation_only() -> Self {
        Self {
            decision_weight_percent: 0,
            trade_signal: false,
            gate_effect: ObservationEffect::None,
            execution_effect: ObservationEffect::None,
            position_sizing_effect: ObservationEffect::None,
        }
    }
}

/// 外部収集器が明示的に渡す個別供給イベントの事実。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SupplyEventFact {
    pub symbol: String,
    pub event_type: SupplyEventType,
    pub event_date: Option<NaiveDate>,
    pub supply_direction: SupplyDirection,
    pub confidence: SupplyEventConfidence,
}

/// Price-Volume Structure が消費する個別銘柄の供給背景。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct SupplyEventContext {
    pub symbol: String,
    pub event_type: SupplyEventType,
    pub event_date: Option<NaiveDate>,
    pub supply_direction: SupplyDirection,
    pub confidence: SupplyEventConfidence,
    pub availability: SupplyEventContextAvailability,
    pub boundary: ObservationBoundary,
}

impl SupplyEventContext {
    pub(crate) fn from_fact(fact: SupplyEventFact) -> Self {
        if fact.symbol.trim().is_empty()
            || fact.event_date.is_none()
            || fact.event_type == SupplyEventType::Unknown
            || fact.supply_direction == SupplyDirection::Unknown
            || fact.confidence == SupplyEventConfidence::Unknown
        {
            return Self::unavailable(fact.symbol);
        }

        Self {
            symbol: fact.symbol,
            event_type: fact.event_type,
            event_date: fact.event_date,
            supply_direction: fact.supply_direction,
            confidence: fact.confidence,
            availability: SupplyEventContextAvailability::Available,
            boundary: ObservationBoundary::observation_only(),
        }
    }

    pub(crate) fn unavailable(symbol: String) -> Self {
        Self {
            symbol,
            event_type: SupplyEventType::Unknown,
            event_date: None,
            supply_direction: SupplyDirection::Unknown,
            confidence: SupplyEventConfidence::Unknown,
            availability: SupplyEventContextAvailability::Unavailable,
            boundary: ObservationBoundary::observation_only(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_lockup_expiry_fact_preserves_observation_only_boundary() {
        let context = SupplyEventContext::from_fact(SupplyEventFact {
            symbol: "SPCX".to_string(),
            event_type: SupplyEventType::LockupExpiry,
            event_date: Some(NaiveDate::from_ymd_opt(2026, 8, 6).unwrap()),
            supply_direction: SupplyDirection::Increase,
            confidence: SupplyEventConfidence::High,
        });

        assert_eq!(context.symbol, "SPCX");
        assert_eq!(context.event_type, SupplyEventType::LockupExpiry);
        assert_eq!(context.event_date, NaiveDate::from_ymd_opt(2026, 8, 6));
        assert_eq!(context.supply_direction, SupplyDirection::Increase);
        assert_eq!(context.confidence, SupplyEventConfidence::High);
        assert_eq!(context.boundary.decision_weight_percent, 0);
        assert!(!context.boundary.trade_signal);
        assert_eq!(context.boundary.gate_effect, ObservationEffect::None);
        assert_eq!(context.boundary.execution_effect, ObservationEffect::None);
        assert_eq!(
            context.boundary.position_sizing_effect,
            ObservationEffect::None
        );
        assert!(serde_json::to_string(&context).is_ok());
    }

    #[test]
    fn incomplete_event_facts_remain_unavailable_without_inference() {
        let context = SupplyEventContext::from_fact(SupplyEventFact {
            symbol: "SPCX".to_string(),
            event_type: SupplyEventType::LockupExpiry,
            event_date: None,
            supply_direction: SupplyDirection::Increase,
            confidence: SupplyEventConfidence::High,
        });

        assert_eq!(context.symbol, "SPCX");
        assert_eq!(
            context.availability,
            SupplyEventContextAvailability::Unavailable
        );
        assert_eq!(context.event_type, SupplyEventType::Unknown);
        assert_eq!(context.event_date, None);
        assert_eq!(context.supply_direction, SupplyDirection::Unknown);
        assert_eq!(context.confidence, SupplyEventConfidence::Unknown);
    }

    #[test]
    fn available_context_is_serializable() {
        let context = SupplyEventContext::unavailable("X".to_string());
        assert!(serde_json::to_string(&context).is_ok());
    }

    #[test]
    fn supply_event_type_covers_the_approved_context_taxonomy() {
        let types = [
            SupplyEventType::Ipo,
            SupplyEventType::LockupExpiry,
            SupplyEventType::SecondaryOffering,
            SupplyEventType::FollowOnOffering,
            SupplyEventType::InsiderSelling,
            SupplyEventType::EmployeeLiquidityEvent,
            SupplyEventType::ConvertibleIssuance,
            SupplyEventType::IndexInclusion,
            SupplyEventType::IndexExclusion,
            SupplyEventType::MajorShareholderSale,
            SupplyEventType::ShareUnlock,
            SupplyEventType::Unknown,
        ];

        assert_eq!(types.len(), 12);
    }

    #[test]
    fn secondary_offering_and_convertible_issuance_remain_distinct_facts() {
        let date = Some(NaiveDate::from_ymd_opt(2026, 8, 6).unwrap());
        let secondary = SupplyEventContext::from_fact(SupplyEventFact {
            symbol: "EXAMPLE".to_string(),
            event_type: SupplyEventType::SecondaryOffering,
            event_date: date,
            supply_direction: SupplyDirection::Increase,
            confidence: SupplyEventConfidence::Medium,
        });
        let convertible = SupplyEventContext::from_fact(SupplyEventFact {
            symbol: "EXAMPLE".to_string(),
            event_type: SupplyEventType::ConvertibleIssuance,
            event_date: date,
            supply_direction: SupplyDirection::Increase,
            confidence: SupplyEventConfidence::Medium,
        });

        assert_ne!(secondary.event_type, convertible.event_type);
        assert_eq!(
            secondary.availability,
            SupplyEventContextAvailability::Available
        );
        assert_eq!(
            convertible.availability,
            SupplyEventContextAvailability::Available
        );
    }
}
