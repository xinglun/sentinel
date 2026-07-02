use crate::features::research::interface::macro_event_calendar_adapter::MacroEventCalendarReadModel;
use crate::features::research::interface::macro_event_observation::{
    FutureCalendarKind, FutureCalendarObservation, MacroEventImportance,
    MacroEventInformationContent, MacroEventLifecycle, MacroEventSourceHealth,
    MacroEventSurpriseState, MacroEventType,
};
use chrono::{Datelike, Duration, NaiveDate, Weekday};
use std::collections::BTreeSet;
use std::time::Duration as StdDuration;

const WINDOW_DAYS: i64 = 45;

pub(crate) fn load_official_future_calendar(as_of_date: NaiveDate) -> MacroEventCalendarReadModel {
    let mut observations = Vec::new();
    observations.extend(load_macro_release_observations(as_of_date));
    observations.extend(load_index_reconstitution_observations(as_of_date));
    observations.extend(load_etf_rebalance_observations(as_of_date));
    observations.extend(load_holiday_liquidity_observations(as_of_date));

    MacroEventCalendarReadModel::from_observations(
        as_of_date,
        "official-calendar-connector".to_string(),
        observations,
    )
}

fn load_macro_release_observations(as_of_date: NaiveDate) -> Vec<FutureCalendarObservation> {
    let mut observations = Vec::new();
    let client = match reqwest::blocking::Client::builder()
        .timeout(StdDuration::from_secs(15))
        .build()
    {
        Ok(client) => client,
        Err(_) => return observations,
    };

    let sources = [
        (
            "Bureau of Labor Statistics",
            "https://www.bls.gov/schedule/news_release/current_year.asp",
        ),
        (
            "Bureau of Economic Analysis",
            "https://www.bea.gov/news/schedule",
        ),
        (
            "U.S. Census Bureau",
            "https://www.census.gov/retail/release_schedule.html",
        ),
        (
            "Federal Reserve Board",
            "https://www.federalreserve.gov/newsevents/calendar.htm",
        ),
        (
            "Federal Reserve Board FOMC",
            "https://www.federalreserve.gov/monetarypolicy/fomccalendars.htm",
        ),
        (
            "ISM World",
            "https://www.ismworld.org/supply-management-news-and-reports/reports/rob-report-calendar/",
        ),
        (
            "U.S. Treasury",
            "https://www.treasurydirect.gov/TA_WS/securities/auctioned",
        ),
    ];

    for (source, url) in sources {
        let Some(text) = fetch_text(&client, url) else {
            continue;
        };
        observations.extend(parse_official_calendar_text(source, url, &text, as_of_date));
    }

    observations
}

fn load_index_reconstitution_observations(as_of_date: NaiveDate) -> Vec<FutureCalendarObservation> {
    let mut observations = Vec::new();
    let (june, december) = (
        fourth_friday(as_of_date.year(), 6),
        second_friday(as_of_date.year(), 12),
    );
    for date in [june, december].into_iter().flatten() {
        if is_exact_day(date, as_of_date) {
            observations.push(build_fact(CalendarFactSpec {
                kind: FutureCalendarKind::IndexReconstitution,
                event_name: "Russell US Index Reconstitution",
                source: "FTSE Russell",
                source_url: "https://www.ftserussell.com/resources/russell-reconstitution",
                event_date: date,
                importance: MacroEventImportance::High,
                source_health: MacroEventSourceHealth::Succeeded,
                summary: "Russell reconstitution window",
                event_type: MacroEventType::Gdp,
                information_content: MacroEventInformationContent::Low,
                lifecycle: MacroEventLifecycle::Upcoming,
            }));
        }
    }
    observations
}

fn load_etf_rebalance_observations(as_of_date: NaiveDate) -> Vec<FutureCalendarObservation> {
    let mut observations = Vec::new();
    let source_url = format!(
        "https://www.nyse.com/publicdocs/nyse/ICE_NYSE_{}_Yearly_Trading_Calendar.pdf",
        as_of_date.year()
    );
    let rebalance_dates = [
        third_friday(as_of_date.year(), 3),
        third_friday(as_of_date.year(), 6),
        third_friday(as_of_date.year(), 9),
        third_friday(as_of_date.year(), 12),
    ];
    for date in rebalance_dates.into_iter().flatten() {
        if is_exact_day(date, as_of_date) {
            observations.push(build_fact(CalendarFactSpec {
                kind: FutureCalendarKind::EtfRebalance,
                event_name: "S&P 400, 500, 600 Rebalance",
                source: "NYSE Trading Calendar / S&P Rebalance",
                source_url: &source_url,
                event_date: date,
                importance: MacroEventImportance::High,
                source_health: MacroEventSourceHealth::Succeeded,
                summary: "official S&P rebalance calendar",
                event_type: MacroEventType::Gdp,
                information_content: MacroEventInformationContent::Low,
                lifecycle: MacroEventLifecycle::Upcoming,
            }));
        }
    }
    observations
}

fn load_holiday_liquidity_observations(as_of_date: NaiveDate) -> Vec<FutureCalendarObservation> {
    let mut observations = Vec::new();
    for holiday in nyse_market_holidays(as_of_date.year()) {
        if is_exact_day(holiday, as_of_date) {
            observations.push(build_fact(CalendarFactSpec {
                kind: FutureCalendarKind::HolidayLiquidity,
                event_name: "NYSE Holiday Liquidity",
                source: "NYSE Holidays & Trading Hours",
                source_url: "https://www.nyse.com/markets/hours-calendars",
                event_date: holiday,
                importance: MacroEventImportance::High,
                source_health: MacroEventSourceHealth::Succeeded,
                summary: "official NYSE holiday calendar",
                event_type: MacroEventType::Gdp,
                information_content: MacroEventInformationContent::Low,
                lifecycle: MacroEventLifecycle::Upcoming,
            }));
        }
    }
    observations
}

fn parse_official_calendar_text(
    source: &str,
    source_url: &str,
    text: &str,
    as_of_date: NaiveDate,
) -> Vec<FutureCalendarObservation> {
    let mut observations = Vec::new();
    let mut seen = BTreeSet::new();
    for line in text.lines() {
        let Some(parsed) = parse_official_calendar_line(line, as_of_date.year()) else {
            continue;
        };
        let (month, day, year, time, title) = parsed;
        let Some(event_date) = parse_month_day_year(&month, day, year) else {
            continue;
        };
        if event_date < as_of_date - Duration::days(WINDOW_DAYS)
            || event_date > as_of_date + Duration::days(WINDOW_DAYS)
        {
            continue;
        }
        if let Some((kind, event_type, event_name, importance, information_content)) =
            classify_macro_title(source, title.as_str())
        {
            let key = format!("{kind:?}:{event_type:?}:{event_date}:{event_name}");
            if !seen.insert(key) {
                continue;
            }
            let kind = if event_date == as_of_date {
                FutureCalendarKind::MacroEvent
            } else {
                FutureCalendarKind::MajorEventWaiting
            };
            observations.push(build_fact(CalendarFactSpec {
                kind,
                event_name: &format!("{event_name} ({source})"),
                source,
                source_url,
                event_date,
                importance,
                source_health: MacroEventSourceHealth::Succeeded,
                summary: time.as_deref().unwrap_or("official calendar event"),
                event_type,
                information_content,
                lifecycle: if event_date == as_of_date {
                    MacroEventLifecycle::Released
                } else {
                    MacroEventLifecycle::Upcoming
                },
            }));
        }
    }

    observations
}

fn parse_official_calendar_line(
    line: &str,
    default_year: i32,
) -> Option<(String, u32, i32, Option<String>, String)> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }
    let tokens: Vec<&str> = line.split_whitespace().collect();
    if tokens.len() < 4 {
        return None;
    }

    let mut index = 0usize;
    if is_weekday_token(tokens[0]) {
        index = 1;
    }
    if tokens.len() < index + 4 {
        return None;
    }

    let month = tokens[index].trim_end_matches(',').to_string();
    let day = tokens[index + 1]
        .trim_end_matches(',')
        .parse::<u32>()
        .ok()?;

    let mut cursor = index + 2;
    let year = if tokens
        .get(cursor)
        .is_some_and(|token| token.chars().all(|ch| ch.is_ascii_digit()) && token.len() == 4)
    {
        let year = tokens[cursor].parse::<i32>().ok()?;
        cursor += 1;
        year
    } else {
        default_year
    };

    let time_token = *tokens.get(cursor)?;
    let meridiem = *tokens.get(cursor + 1)?;
    if !is_clock_token(time_token) || !is_meridiem_token(meridiem) {
        return None;
    }
    let time = Some(format!("{} {}", time_token, meridiem));
    let title_start = cursor + 2;
    if title_start >= tokens.len() {
        return None;
    }
    let title = tokens[title_start..].join(" ");
    Some((month, day, year, time, title))
}

fn is_weekday_token(token: &str) -> bool {
    matches!(
        token.trim_end_matches(','),
        "Monday" | "Tuesday" | "Wednesday" | "Thursday" | "Friday" | "Saturday" | "Sunday"
    )
}

fn is_clock_token(token: &str) -> bool {
    let mut parts = token.split(':');
    let hour = parts.next();
    let minute = parts.next();
    hour.is_some()
        && minute.is_some()
        && parts.next().is_none()
        && hour
            .is_some_and(|value| !value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit()))
        && minute
            .is_some_and(|value| value.len() == 2 && value.chars().all(|ch| ch.is_ascii_digit()))
}

fn is_meridiem_token(token: &str) -> bool {
    matches!(token, "AM" | "PM")
}

fn classify_macro_title(
    source: &str,
    title: &str,
) -> Option<(
    FutureCalendarKind,
    MacroEventType,
    String,
    MacroEventImportance,
    MacroEventInformationContent,
)> {
    let title_lower = title.to_lowercase();
    if title_lower.contains("consumer price index") {
        return Some((
            FutureCalendarKind::MacroEvent,
            MacroEventType::Cpi,
            "Consumer Price Index".to_string(),
            MacroEventImportance::Critical,
            MacroEventInformationContent::High,
        ));
    }
    if title_lower.contains("producer price index") {
        return Some((
            FutureCalendarKind::MacroEvent,
            MacroEventType::Ppi,
            "Producer Price Index".to_string(),
            MacroEventImportance::High,
            MacroEventInformationContent::High,
        ));
    }
    if title_lower.contains("employment situation") {
        return Some((
            FutureCalendarKind::MacroEvent,
            MacroEventType::NonfarmPayrolls,
            "Employment Situation".to_string(),
            MacroEventImportance::Critical,
            MacroEventInformationContent::High,
        ));
    }
    if title_lower.contains("job openings and labor turnover survey") {
        return Some((
            FutureCalendarKind::MacroEvent,
            MacroEventType::Jolts,
            "Job Openings and Labor Turnover Survey".to_string(),
            MacroEventImportance::High,
            MacroEventInformationContent::High,
        ));
    }
    if title_lower.contains("gross domestic product") || title_lower.contains("gdp") {
        return Some((
            FutureCalendarKind::MacroEvent,
            MacroEventType::Gdp,
            "GDP".to_string(),
            MacroEventImportance::Critical,
            MacroEventInformationContent::High,
        ));
    }
    if title_lower.contains("personal income and outlays") {
        return Some((
            FutureCalendarKind::MacroEvent,
            MacroEventType::Pce,
            "Personal Income and Outlays".to_string(),
            MacroEventImportance::Critical,
            MacroEventInformationContent::High,
        ));
    }
    if title_lower.contains("retail and food services sales")
        || title_lower.contains("monthly retail trade")
    {
        return Some((
            FutureCalendarKind::MacroEvent,
            MacroEventType::RetailSales,
            "Retail Sales".to_string(),
            MacroEventImportance::High,
            MacroEventInformationContent::High,
        ));
    }
    if title_lower.contains("fomc meeting") || title_lower.contains("fomc statement") {
        return Some((
            FutureCalendarKind::MacroEvent,
            MacroEventType::FomcRateDecision,
            "FOMC Rate Decision".to_string(),
            MacroEventImportance::Critical,
            MacroEventInformationContent::High,
        ));
    }
    if title_lower.contains("minutes") && title_lower.contains("fomc") {
        return Some((
            FutureCalendarKind::MacroEvent,
            MacroEventType::FomcMinutes,
            "FOMC Minutes".to_string(),
            MacroEventImportance::High,
            MacroEventInformationContent::High,
        ));
    }
    if title_lower.contains("services pmi") {
        return Some((
            FutureCalendarKind::MacroEvent,
            MacroEventType::IsmServices,
            "ISM Services PMI".to_string(),
            MacroEventImportance::High,
            MacroEventInformationContent::High,
        ));
    }
    if title_lower.contains("manufacturing pmi") {
        return Some((
            FutureCalendarKind::MacroEvent,
            MacroEventType::IsmManufacturing,
            "ISM Manufacturing PMI".to_string(),
            MacroEventImportance::High,
            MacroEventInformationContent::High,
        ));
    }
    if source.to_lowercase().contains("treasury") && title_lower.contains("auction") {
        return Some((
            FutureCalendarKind::MacroEvent,
            MacroEventType::TreasuryAuction,
            "Treasury Auction".to_string(),
            MacroEventImportance::High,
            MacroEventInformationContent::High,
        ));
    }
    None
}

struct CalendarFactSpec<'a> {
    kind: FutureCalendarKind,
    event_name: &'a str,
    source: &'a str,
    source_url: &'a str,
    event_date: NaiveDate,
    importance: MacroEventImportance,
    source_health: MacroEventSourceHealth,
    summary: &'a str,
    event_type: MacroEventType,
    information_content: MacroEventInformationContent,
    lifecycle: MacroEventLifecycle,
}

fn build_fact(spec: CalendarFactSpec<'_>) -> FutureCalendarObservation {
    FutureCalendarObservation {
        kind: spec.kind,
        event_id: format!(
            "{}-{}",
            spec.source.replace(' ', "-").to_lowercase(),
            spec.event_name.replace(' ', "-").to_lowercase()
        ),
        as_of_date: spec.event_date,
        event_date: spec.event_date,
        event_time: Some("08:30".to_string()),
        timezone: "America/New_York".to_string(),
        country: "US".to_string(),
        event_type: spec.event_type,
        event_name: format!("{} [{}]", spec.event_name, spec.summary),
        source: spec.source.to_string(),
        source_url: spec.source_url.to_string(),
        importance: spec.importance,
        lifecycle: spec.lifecycle,
        expected_value: None,
        actual_value: None,
        previous_value: None,
        unit: None,
        surprise_state: MacroEventSurpriseState::NotAvailable,
        information_content: spec.information_content,
        source_health: spec.source_health,
        observed_at: spec.event_date,
    }
}

fn fetch_text(client: &reqwest::blocking::Client, url: &str) -> Option<String> {
    let response = client.get(url).send().ok()?;
    if !response.status().is_success() {
        return None;
    }
    response.text().ok()
}

fn parse_month_day_year(month: &str, day: u32, year: i32) -> Option<NaiveDate> {
    let month = match month.to_lowercase().as_str() {
        "january" => 1,
        "february" => 2,
        "march" => 3,
        "april" => 4,
        "may" => 5,
        "june" => 6,
        "july" => 7,
        "august" => 8,
        "september" => 9,
        "october" => 10,
        "november" => 11,
        "december" => 12,
        _ => return None,
    };
    NaiveDate::from_ymd_opt(year, month, day)
}

fn fourth_friday(year: i32, month: u32) -> Option<NaiveDate> {
    nth_weekday_of_month(year, month, Weekday::Fri, 4)
}

fn second_friday(year: i32, month: u32) -> Option<NaiveDate> {
    nth_weekday_of_month(year, month, Weekday::Fri, 2)
}

fn third_friday(year: i32, month: u32) -> Option<NaiveDate> {
    nth_weekday_of_month(year, month, Weekday::Fri, 3)
}

fn nth_weekday_of_month(
    year: i32,
    month: u32,
    weekday: Weekday,
    occurrence: u32,
) -> Option<NaiveDate> {
    let mut day = NaiveDate::from_ymd_opt(year, month, 1)?;
    let mut count = 0;
    while day.month() == month {
        if day.weekday() == weekday {
            count += 1;
            if count == occurrence {
                return Some(day);
            }
        }
        day = day.succ_opt()?;
    }
    None
}

fn nyse_market_holidays(year: i32) -> Vec<NaiveDate> {
    vec![
        NaiveDate::from_ymd_opt(year, 1, 1).expect("valid new year"),
        observed_mlk_day(year),
        observed_presidents_day(year),
        observed_memorial_day(year),
        NaiveDate::from_ymd_opt(year, 6, 19).expect("valid juneteenth"),
        NaiveDate::from_ymd_opt(year, 7, 4).expect("valid independence day"),
        observed_labor_day(year),
        observed_thanksgiving(year),
        NaiveDate::from_ymd_opt(year, 12, 25).expect("valid christmas"),
    ]
}

fn observed_mlk_day(year: i32) -> NaiveDate {
    nth_weekday_of_month(year, 1, Weekday::Mon, 3).expect("valid MLK day")
}

fn observed_presidents_day(year: i32) -> NaiveDate {
    nth_weekday_of_month(year, 2, Weekday::Mon, 3).expect("valid presidents day")
}

fn observed_memorial_day(year: i32) -> NaiveDate {
    let mut day = NaiveDate::from_ymd_opt(year, 5, 31).expect("valid memorial range");
    while day.weekday() != Weekday::Mon {
        day = day.pred_opt().expect("previous day");
    }
    day
}

fn observed_labor_day(year: i32) -> NaiveDate {
    nth_weekday_of_month(year, 9, Weekday::Mon, 1).expect("valid labor day")
}

fn observed_thanksgiving(year: i32) -> NaiveDate {
    nth_weekday_of_month(year, 11, Weekday::Thu, 4).expect("valid thanksgiving")
}

fn is_exact_day(event_date: NaiveDate, as_of_date: NaiveDate) -> bool {
    event_date == as_of_date
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn computes_index_reconstitution_dates() {
        let date = NaiveDate::from_ymd_opt(2026, 6, 26).unwrap();
        let observations = load_index_reconstitution_observations(date);
        assert!(!observations.is_empty());
        assert_eq!(
            observations[0].kind,
            FutureCalendarKind::IndexReconstitution
        );
    }

    #[test]
    fn computes_etf_rebalance_dates() {
        let date = NaiveDate::from_ymd_opt(2026, 9, 18).unwrap();
        let observations = load_etf_rebalance_observations(date);
        assert!(!observations.is_empty());
        assert_eq!(observations[0].kind, FutureCalendarKind::EtfRebalance);
        assert_eq!(
            observations[0].source,
            "NYSE Trading Calendar / S&P Rebalance"
        );
    }

    #[test]
    fn nyse_market_holidays_cover_common_dates() {
        let observations =
            load_holiday_liquidity_observations(NaiveDate::from_ymd_opt(2026, 12, 25).unwrap());
        assert!(!observations.is_empty());
        assert_eq!(observations[0].kind, FutureCalendarKind::HolidayLiquidity);
        assert_eq!(observations[0].source, "NYSE Holidays & Trading Hours");
    }

    #[test]
    fn treasury_auction_classification_requires_treasury_source() {
        assert!(classify_macro_title("Bureau of Labor Statistics", "Treasury Auction").is_none());
        assert!(classify_macro_title("U.S. Treasury", "Treasury Auction").is_some());
    }
}
