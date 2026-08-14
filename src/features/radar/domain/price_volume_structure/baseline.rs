use super::eligibility::volume_quality;
use super::lifecycle::continuous_dates;
use super::*;

pub(super) fn baseline_selection(
    input: &PriceVolumeInput<'_>,
) -> (
    EligibilityStatus,
    BaselineType,
    Option<BaselineType>,
    Option<NaiveDate>,
) {
    let valid_days = input.bars.iter().filter(|bar| is_valid_ohlcv(bar)).count();
    if input.source_rate_limited {
        return (
            EligibilityStatus::Unavailable,
            BaselineType::Unavailable,
            None,
            None,
        );
    }
    if !input.volume_comparable || !continuous_dates(input.bars, input.market_holidays) {
        return (
            EligibilityStatus::Unavailable,
            BaselineType::Unavailable,
            None,
            None,
        );
    }
    let eligibility = if valid_days >= 20
        && volume_quality(
            input.bars,
            input.source_rate_limited,
            input.volume_comparable,
            input.market_holidays,
        ) == VolumeDataQuality::Healthy
    {
        EligibilityStatus::Full
    } else {
        EligibilityStatus::Partial
    };
    let supply_baseline = input.supply_context.and_then(|context| {
        let baseline = match context.event_type {
            crate::features::shared::domain::supply_event_context::SupplyEventType::Ipo => {
                BaselineType::PostIpo
            }
            crate::features::shared::domain::supply_event_context::SupplyEventType::LockupExpiry
            | crate::features::shared::domain::supply_event_context::SupplyEventType::ShareUnlock => {
                BaselineType::PostLockup
            }
            _ => BaselineType::PostEvent,
        };
        context.event_date.map(|date| (baseline, date))
    });
    let secondary_supply_baseline = input.secondary_supply_context.and_then(|context| {
        let baseline = match context.event_type {
            crate::features::shared::domain::supply_event_context::SupplyEventType::Ipo => {
                BaselineType::PostIpo
            }
            crate::features::shared::domain::supply_event_context::SupplyEventType::LockupExpiry
            | crate::features::shared::domain::supply_event_context::SupplyEventType::ShareUnlock => {
                BaselineType::PostLockup
            }
            _ => BaselineType::PostEvent,
        };
        context.event_date.map(|date| (baseline, date))
    });
    let event_baseline = input.event_baseline.or(supply_baseline);
    if valid_days < 5 {
        let (baseline, date) = event_baseline
            .map(|(baseline, date)| (baseline, Some(date)))
            .unwrap_or((BaselineType::Unavailable, None));
        return (EligibilityStatus::Insufficient, baseline, None, date);
    }
    if let Some((baseline, date)) = event_baseline {
        let event_valid_days = input
            .bars
            .iter()
            .filter(|bar| bar.date >= date && is_valid_ohlcv(bar))
            .count();
        if event_valid_days < 2 && valid_days >= 5 {
            return (eligibility, BaselineType::AvailableHistory, None, None);
        }
        if eligibility == EligibilityStatus::Full {
            return (eligibility, BaselineType::Standard20d, Some(baseline), None);
        }
        let secondary = secondary_supply_baseline
            .map(|(secondary, _)| secondary)
            .or((baseline != BaselineType::AvailableHistory)
                .then_some(BaselineType::AvailableHistory));
        return (eligibility, baseline, secondary, Some(date));
    }
    (
        eligibility,
        if eligibility == EligibilityStatus::Full {
            BaselineType::Standard20d
        } else {
            BaselineType::AvailableHistory
        },
        None,
        None,
    )
}

#[cfg(test)]
pub(crate) fn boundary_marker() -> bool {
    true
}

pub(super) fn is_valid_ohlcv(bar: &DailyBar) -> bool {
    bar.volume.is_some_and(|volume| volume > 0.0)
        && bar.open.is_some()
        && bar.high.is_some()
        && bar.low.is_some()
}

pub(super) fn secondary_baseline_start(
    input: &PriceVolumeInput<'_>,
    baseline: BaselineType,
) -> Option<NaiveDate> {
    input
        .event_baseline
        .filter(|(event_baseline, _)| *event_baseline == baseline)
        .map(|(_, date)| date)
        .or_else(|| {
            [input.supply_context, input.secondary_supply_context]
                .into_iter()
                .flatten()
                .find_map(|context| {
                    let context_baseline = match context.event_type {
                        crate::features::shared::domain::supply_event_context::SupplyEventType::Ipo => {
                            BaselineType::PostIpo
                        }
                        crate::features::shared::domain::supply_event_context::SupplyEventType::LockupExpiry
                        | crate::features::shared::domain::supply_event_context::SupplyEventType::ShareUnlock => {
                            BaselineType::PostLockup
                        }
                        _ => BaselineType::PostEvent,
                    };
                    (context_baseline == baseline).then_some(context.event_date).flatten()
                })
        })
}
