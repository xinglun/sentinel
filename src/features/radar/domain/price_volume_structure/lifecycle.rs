use super::*;
use chrono::Datelike;

pub(super) fn lifecycle(
    eligibility: EligibilityStatus,
    persistence_days: u8,
    structure: PriceVolumeStructure,
    event_specific_evidence: bool,
) -> CandidateLifecycle {
    if structure == PriceVolumeStructure::Unavailable {
        return CandidateLifecycle::Unavailable;
    }
    match (eligibility, persistence_days) {
        (EligibilityStatus::Full, 0..=1) => CandidateLifecycle::Candidate,
        (EligibilityStatus::Full, 2) => CandidateLifecycle::Developing,
        (EligibilityStatus::Full, _) => CandidateLifecycle::Confirmed,
        (EligibilityStatus::Partial, 0..=1) => CandidateLifecycle::Candidate,
        (EligibilityStatus::Partial, 3..=u8::MAX) if event_specific_evidence => {
            CandidateLifecycle::Confirmed
        }
        (EligibilityStatus::Partial, _) => CandidateLifecycle::Developing,
        _ => CandidateLifecycle::Unavailable,
    }
}

#[cfg(test)]
pub(crate) fn boundary_marker() -> bool {
    true
}

pub(super) fn continuous_dates(bars: &[DailyBar], market_holidays: Option<&[NaiveDate]>) -> bool {
    bars.windows(2).all(|pair| {
        let previous = pair[0].date;
        let current = pair[1].date;
        if current <= previous {
            return false;
        }
        let mut date = previous + chrono::Duration::days(1);
        while date < current {
            let weekend = matches!(date.weekday(), chrono::Weekday::Sat | chrono::Weekday::Sun);
            let holiday = market_holidays.is_some_and(|holidays| holidays.contains(&date));
            if !weekend && !holiday {
                return false;
            }
            date += chrono::Duration::days(1);
        }
        true
    })
}
