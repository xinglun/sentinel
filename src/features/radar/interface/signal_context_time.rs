use chrono::{DateTime, Datelike, Duration, NaiveDate, Utc, Weekday};

/// UTC のイベント時刻を America/New_York の取引日へ写像する。
#[allow(dead_code)]
pub(crate) fn market_date_from_utc(value: &str) -> Option<NaiveDate> {
    let timestamp = DateTime::parse_from_rfc3339(value)
        .ok()?
        .with_timezone(&Utc);
    let offset_hours = if is_new_york_daylight_time(timestamp) {
        -4
    } else {
        -5
    };
    Some((timestamp + Duration::hours(offset_hours)).date_naive())
}

fn is_new_york_daylight_time(timestamp: DateTime<Utc>) -> bool {
    let year = timestamp.year();
    let march_second_sunday = nth_sunday(year, 3, 2);
    let november_first_sunday = nth_sunday(year, 11, 1);
    let start = march_second_sunday.and_hms_opt(7, 0, 0).unwrap().and_utc();
    let end = november_first_sunday
        .and_hms_opt(6, 0, 0)
        .unwrap()
        .and_utc();
    timestamp >= start && timestamp < end
}

fn nth_sunday(year: i32, month: u32, ordinal: u32) -> NaiveDate {
    let first = NaiveDate::from_ymd_opt(year, month, 1).expect("valid month");
    let days_to_sunday = (7 + Weekday::Sun.num_days_from_monday() as i64
        - first.weekday().num_days_from_monday() as i64)
        % 7;
    first + Duration::days(days_to_sunday + 7 * (ordinal as i64 - 1))
}

#[cfg(test)]
mod tests {
    use super::market_date_from_utc;
    use chrono::NaiveDate;

    #[test]
    fn japan_next_day_still_belongs_to_previous_us_session() {
        assert_eq!(
            market_date_from_utc("2026-08-08T01:00:00Z"),
            NaiveDate::from_ymd_opt(2026, 8, 7)
        );
    }

    #[test]
    fn winter_utc_boundary_uses_new_york_standard_time() {
        assert_eq!(
            market_date_from_utc("2026-01-03T01:00:00Z"),
            NaiveDate::from_ymd_opt(2026, 1, 2)
        );
    }
}
