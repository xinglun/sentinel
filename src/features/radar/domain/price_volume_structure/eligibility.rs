use super::lifecycle::continuous_dates;
use super::*;

pub(crate) fn observation_confidence(eligibility: EligibilityStatus) -> ObservationConfidence {
    match eligibility {
        EligibilityStatus::Full => ObservationConfidence::High,
        EligibilityStatus::Partial => ObservationConfidence::Partial,
        EligibilityStatus::Insufficient => ObservationConfidence::Low,
        EligibilityStatus::Unavailable => ObservationConfidence::Unavailable,
    }
}

#[cfg(test)]
pub(crate) fn boundary_marker() -> bool {
    true
}

pub(super) fn volume_quality(
    bars: &[DailyBar],
    rate_limited: bool,
    volume_comparable: bool,
    market_holidays: Option<&[NaiveDate]>,
) -> VolumeDataQuality {
    if rate_limited || !volume_comparable || !continuous_dates(bars, market_holidays) {
        return VolumeDataQuality::Degraded;
    }
    if bars.len() < 5 {
        return VolumeDataQuality::Unavailable;
    }
    let present = bars
        .iter()
        .filter(|bar| bar.volume.is_some_and(|value| value > 0.0))
        .count();
    if present < bars.len() {
        if present >= 5 {
            VolumeDataQuality::Degraded
        } else {
            VolumeDataQuality::Unavailable
        }
    } else {
        let complete_ohlc = bars
            .iter()
            .all(|bar| bar.open.is_some() && bar.high.is_some() && bar.low.is_some());
        if complete_ohlc {
            if bars.len() >= 20 {
                VolumeDataQuality::Healthy
            } else {
                VolumeDataQuality::Partial
            }
        } else {
            VolumeDataQuality::Unavailable
        }
    }
}

pub(super) fn observation_is_unavailable(
    input: &PriceVolumeInput<'_>,
    quality: VolumeDataQuality,
) -> bool {
    input.source_rate_limited
        || !input.volume_comparable
        || !continuous_dates(input.bars, input.market_holidays)
        || quality == VolumeDataQuality::Unavailable
}

pub(super) fn unavailable_reason(
    input: &PriceVolumeInput<'_>,
    eligibility: EligibilityStatus,
) -> Option<UnavailableReason> {
    if input.source_rate_limited {
        Some(UnavailableReason::ApiFailure)
    } else if !input.volume_comparable {
        Some(UnavailableReason::CorporateActionConflict)
    } else if !continuous_dates(input.bars, input.market_holidays) {
        Some(UnavailableReason::DataGap)
    } else if input.bars.iter().all(|bar| bar.volume.is_none()) {
        Some(UnavailableReason::MissingVolume)
    } else if input
        .bars
        .iter()
        .any(|bar| bar.open.is_none() || bar.high.is_none() || bar.low.is_none())
    {
        Some(UnavailableReason::MissingOhlcv)
    } else if eligibility == EligibilityStatus::Insufficient {
        Some(UnavailableReason::InsufficientValidHistory)
    } else {
        None
    }
}

pub(super) fn next_eligibility_condition(
    eligibility: EligibilityStatus,
    reason: Option<UnavailableReason>,
) -> Option<String> {
    if reason == Some(UnavailableReason::MissingSupplyContext) {
        return Some("Need Supply Event Context to evaluate absorption.".to_string());
    }
    match (eligibility, reason) {
        (EligibilityStatus::Insufficient, _) => {
            Some("Need 2 additional valid OHLCV sessions for PARTIAL observation.".to_string())
        }
        (EligibilityStatus::Unavailable, Some(UnavailableReason::ApiFailure)) => {
            Some("Need a successful provider response or a valid cached history.".to_string())
        }
        (EligibilityStatus::Unavailable, Some(UnavailableReason::MissingVolume)) => {
            Some("Need one valid volume observation.".to_string())
        }
        (EligibilityStatus::Unavailable, Some(UnavailableReason::MissingOhlcv)) => {
            Some("Need one valid OHLCV observation.".to_string())
        }
        (EligibilityStatus::Unavailable, Some(UnavailableReason::DataGap)) => {
            Some("Need a continuous comparable session sequence.".to_string())
        }
        (EligibilityStatus::Partial, _) => Some(
            "Need FULL history or event-specific minimum evidence before confirmation.".to_string(),
        ),
        _ => None,
    }
}
