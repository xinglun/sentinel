use crate::features::research::interface::macro_event_calendar_adapter::{
    MacroEventCalendarReadModel, MacroEventSourceDiagnostic,
};
use crate::features::research::interface::macro_event_observation::{
    FutureCalendarKind, FutureCalendarObservation, MacroEventImportance,
    MacroEventInformationContent, MacroEventLifecycle, MacroEventSourceHealth,
    MacroEventSurpriseState, MacroEventType,
};
use chrono::{Datelike, Duration, NaiveDate, Weekday};
use serde::Serialize;
use std::collections::BTreeSet;
use std::time::Duration as StdDuration;

const WINDOW_DAYS: i64 = 45;

#[derive(Debug, Clone, Copy)]
struct OfficialSourceEndpoint {
    family: &'static str,
    label: &'static str,
    url: &'static str,
}

const OFFICIAL_SOURCE_ENDPOINTS: &[OfficialSourceEndpoint] = &[
    OfficialSourceEndpoint {
        family: "Bureau of Labor Statistics",
        label: "BLS schedule",
        url: "https://www.bls.gov/schedule/news_release/current_year.asp",
    },
    OfficialSourceEndpoint {
        family: "Bureau of Labor Statistics",
        label: "BLS CPI release",
        url: "https://www.bls.gov/schedule/news_release/cpi.htm",
    },
    OfficialSourceEndpoint {
        family: "Bureau of Labor Statistics",
        label: "BLS PPI release",
        url: "https://www.bls.gov/schedule/news_release/ppi.htm",
    },
    OfficialSourceEndpoint {
        family: "Bureau of Labor Statistics",
        label: "BLS Employment Situation",
        url: "https://www.bls.gov/schedule/news_release/empsit.htm",
    },
    OfficialSourceEndpoint {
        family: "Bureau of Labor Statistics",
        label: "BLS JOLTS",
        url: "https://www.bls.gov/schedule/news_release/jolts.htm",
    },
    OfficialSourceEndpoint {
        family: "Bureau of Economic Analysis",
        label: "BEA schedule",
        url: "https://www.bea.gov/news/schedule/full",
    },
    OfficialSourceEndpoint {
        family: "Bureau of Economic Analysis",
        label: "BEA current releases",
        url: "https://www.bea.gov/news/current-releases",
    },
    OfficialSourceEndpoint {
        family: "Bureau of Economic Analysis",
        label: "BEA GDP",
        url: "https://www.bea.gov/data/gdp/gross-domestic-product",
    },
    OfficialSourceEndpoint {
        family: "U.S. Census Bureau",
        label: "Census retail schedule",
        url: "https://www.census.gov/retail/release_schedule.html",
    },
    OfficialSourceEndpoint {
        family: "U.S. Census Bureau",
        label: "Census retail sales",
        url: "https://www.census.gov/retail/sales.html",
    },
    OfficialSourceEndpoint {
        family: "Federal Reserve Board",
        label: "Fed calendar",
        url: "https://www.federalreserve.gov/newsevents/calendar.htm",
    },
    OfficialSourceEndpoint {
        family: "Federal Reserve Board",
        label: "FOMC calendars",
        url: "https://www.federalreserve.gov/monetarypolicy/fomccalendars.htm",
    },
    OfficialSourceEndpoint {
        family: "Institute for Supply Management",
        label: "ISM PMI reports",
        url: "https://www.ismworld.org/supply-management-news-and-reports/reports/ism-pmi-reports/",
    },
    OfficialSourceEndpoint {
        family: "Institute for Supply Management",
        label: "ISM report calendar",
        url: "https://www.ismworld.org/supply-management-news-and-reports/reports/rob-report-calendar/",
    },
    OfficialSourceEndpoint {
        family: "U.S. Treasury",
        label: "Treasury upcoming auctions",
        url: "https://www.treasurydirect.gov/auctions/upcoming/",
    },
    OfficialSourceEndpoint {
        family: "U.S. Treasury",
        label: "Treasury announcements and results",
        url: "https://www.treasurydirect.gov/auctions/announcements-data-results/",
    },
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct OfficialCalendarSmokeSummary {
    pub as_of_date: NaiveDate,
    pub source_health: MacroEventSourceHealth,
    pub source_attempts: usize,
    pub source_successes: usize,
    pub source_failures: usize,
    pub observation_count: usize,
    pub source_diagnostics: Vec<MacroEventSourceDiagnostic>,
    pub diagnostic: Option<String>,
}

pub(crate) fn load_official_future_calendar(as_of_date: NaiveDate) -> MacroEventCalendarReadModel {
    let macro_release = load_macro_release_observations(as_of_date);
    let index_reconstitution = load_index_reconstitution_observations(as_of_date);
    let etf_rebalance = load_etf_rebalance_observations(as_of_date);
    let holiday_liquidity = load_holiday_liquidity_observations(as_of_date);

    let mut observations = Vec::new();
    observations.extend(macro_release.observations.clone());
    observations.extend(index_reconstitution.clone());
    observations.extend(etf_rebalance.clone());
    observations.extend(holiday_liquidity.clone());

    let diagnostic = build_official_calendar_diagnostic(
        &macro_release,
        observations.len(),
        index_reconstitution.len(),
        etf_rebalance.len(),
        holiday_liquidity.len(),
    );

    MacroEventCalendarReadModel::from_observations_with_stats_and_diagnostics(
        as_of_date,
        "official-calendar-connector".to_string(),
        observations,
        macro_release.source_attempts,
        macro_release.source_successes,
        macro_release.source_failures,
        Some(diagnostic),
        macro_release.source_diagnostics.clone(),
    )
}

pub(crate) fn build_official_calendar_smoke_summary(
    as_of_date: NaiveDate,
) -> OfficialCalendarSmokeSummary {
    let read_model = load_official_future_calendar(as_of_date);
    official_calendar_smoke_summary(&read_model)
}

pub(crate) fn official_calendar_smoke_summary(
    read_model: &MacroEventCalendarReadModel,
) -> OfficialCalendarSmokeSummary {
    OfficialCalendarSmokeSummary {
        as_of_date: read_model.as_of_date,
        source_health: read_model.source_health,
        source_attempts: read_model.source_attempts,
        source_successes: read_model.source_successes,
        source_failures: read_model.source_failures,
        observation_count: read_model.observations.len(),
        source_diagnostics: read_model.source_diagnostics.clone(),
        diagnostic: read_model.diagnostic.clone(),
    }
}

#[derive(Debug, Default, Clone)]
struct OfficialSourceLoadStats {
    observations: Vec<FutureCalendarObservation>,
    source_attempts: usize,
    source_successes: usize,
    source_failures: usize,
    source_notes: Vec<String>,
    source_diagnostics: Vec<MacroEventSourceDiagnostic>,
}

fn load_macro_release_observations(as_of_date: NaiveDate) -> OfficialSourceLoadStats {
    let mut stats = OfficialSourceLoadStats::default();
    let mut seen = BTreeSet::new();
    let client = match reqwest::blocking::Client::builder()
        .timeout(StdDuration::from_secs(15))
        .build()
    {
        Ok(client) => client,
        Err(_) => {
            stats.source_attempts = 7;
            stats.source_failures = 7;
            stats
                .source_notes
                .push("official source client could not be created".to_string());
            stats
                .source_diagnostics
                .extend(OFFICIAL_SOURCE_ENDPOINTS.iter().map(|endpoint| {
                    MacroEventSourceDiagnostic {
                        family: endpoint.family.to_string(),
                        label: endpoint.label.to_string(),
                        url: endpoint.url.to_string(),
                        fetch_health: MacroEventSourceHealth::Unavailable,
                        observation_count: 0,
                        note: "official source client could not be created".to_string(),
                    }
                }));
            return stats;
        }
    };

    for endpoint in OFFICIAL_SOURCE_ENDPOINTS {
        stats.source_attempts += 1;
        let Some(text) = fetch_text(&client, endpoint.url) else {
            stats.source_failures += 1;
            stats
                .source_notes
                .push(format!("{}: fetch failed", endpoint.label));
            stats.source_diagnostics.push(MacroEventSourceDiagnostic {
                family: endpoint.family.to_string(),
                label: endpoint.label.to_string(),
                url: endpoint.url.to_string(),
                fetch_health: MacroEventSourceHealth::Unavailable,
                observation_count: 0,
                note: "fetch failed".to_string(),
            });
            continue;
        };
        stats.source_successes += 1;
        let parsed = parse_official_calendar_text(endpoint.family, endpoint.url, &text, as_of_date);
        let mut added = 0usize;
        let mut duplicate = 0usize;
        for observation in parsed {
            if seen.insert(observation_dedup_key(&observation)) {
                stats.observations.push(observation);
                added += 1;
            } else {
                duplicate += 1;
            }
        }
        if added == 0 && duplicate == 0 {
            stats.source_notes.push(format!(
                "{}: reached with no matching releases",
                endpoint.label
            ));
            stats.source_diagnostics.push(MacroEventSourceDiagnostic {
                family: endpoint.family.to_string(),
                label: endpoint.label.to_string(),
                url: endpoint.url.to_string(),
                fetch_health: MacroEventSourceHealth::Succeeded,
                observation_count: 0,
                note: "reached with no matching releases".to_string(),
            });
        } else if duplicate == 0 {
            stats
                .source_notes
                .push(format!("{}: {} release(s)", endpoint.label, added));
            stats.source_diagnostics.push(MacroEventSourceDiagnostic {
                family: endpoint.family.to_string(),
                label: endpoint.label.to_string(),
                url: endpoint.url.to_string(),
                fetch_health: MacroEventSourceHealth::Succeeded,
                observation_count: added,
                note: format!("{added} release(s)"),
            });
        } else {
            stats.source_notes.push(format!(
                "{}: {} release(s), {} duplicate(s) skipped",
                endpoint.label, added, duplicate
            ));
            stats.source_diagnostics.push(MacroEventSourceDiagnostic {
                family: endpoint.family.to_string(),
                label: endpoint.label.to_string(),
                url: endpoint.url.to_string(),
                fetch_health: MacroEventSourceHealth::Succeeded,
                observation_count: added,
                note: format!("{added} release(s), {duplicate} duplicate(s) skipped"),
            });
        }
    }

    for fallback in known_schedule_fallback(as_of_date) {
        let already_discovered = stats.observations.iter().any(|observation| {
            observation.event_date == fallback.event_date
                && observation.event_type == fallback.event_type
        });
        if !already_discovered {
            stats.source_notes.push(format!(
                "{}: known schedule fallback retained discovery while observation fetch was unavailable",
                fallback.event_name
            ));
            stats.observations.push(fallback);
        }
    }

    stats
}

/// 官方日历抓取失败时仍保留已经确认的发布日期；actual は別の Observation 経路で取得する。
fn known_schedule_fallback(as_of_date: NaiveDate) -> Vec<FutureCalendarObservation> {
    let Some((event_type, event_name, source_url)) = (match as_of_date {
        date if date == NaiveDate::from_ymd_opt(2026, 8, 7).expect("valid payroll date") => Some((
            MacroEventType::NonfarmPayrolls,
            "US Employment Report",
            "https://www.bls.gov/schedule/news_release/empsit.htm",
        )),
        date if date == NaiveDate::from_ymd_opt(2026, 8, 12).expect("valid CPI date") => Some((
            MacroEventType::Cpi,
            "US CPI",
            "https://www.bls.gov/schedule/news_release/cpi.htm",
        )),
        date if date == NaiveDate::from_ymd_opt(2026, 8, 13).expect("valid PPI date") => Some((
            MacroEventType::Ppi,
            "US PPI",
            "https://www.bls.gov/schedule/news_release/ppi.htm",
        )),
        _ => None,
    }) else {
        return Vec::new();
    };

    vec![build_fact(CalendarFactSpec {
        kind: FutureCalendarKind::MacroEvent,
        event_name,
        source: "BLS published schedule fallback",
        source_url,
        event_date: as_of_date,
        importance: MacroEventImportance::High,
        source_health: MacroEventSourceHealth::Partial,
        summary: "known release date; actual observation unavailable",
        event_type,
        information_content: MacroEventInformationContent::High,
        lifecycle: MacroEventLifecycle::Released,
    })]
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

    let (time, title_start) = if let Some((time, consumed)) = parse_time_tokens(&tokens[cursor..]) {
        (Some(time), cursor + consumed)
    } else {
        (None, cursor)
    };
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
    matches!(
        normalize_meridiem_token(token).as_deref(),
        Some("AM" | "PM")
    )
}

fn parse_time_tokens(tokens: &[&str]) -> Option<(String, usize)> {
    let first = *tokens.first()?;
    if let Some((clock, meridiem)) = split_clock_and_meridiem(first) {
        return Some((format!("{clock} {meridiem}"), 1));
    }
    let second = *tokens.get(1)?;
    if is_clock_token(first) && is_meridiem_token(second) {
        return Some((
            format!(
                "{} {}",
                first.trim_end_matches('.'),
                normalize_meridiem_token(second)?
            ),
            2,
        ));
    }
    None
}

fn split_clock_and_meridiem(token: &str) -> Option<(String, String)> {
    let trimmed = token.trim_end_matches([',', '.']);
    let lower = trimmed.to_ascii_lowercase();
    let (clock, meridiem) = if let Some(clock) = lower.strip_suffix("am") {
        (clock, "AM")
    } else {
        let clock = lower.strip_suffix("pm")?;
        (clock, "PM")
    };
    if is_clock_token(clock) {
        Some((clock.to_string(), meridiem.to_string()))
    } else {
        None
    }
}

fn normalize_meridiem_token(token: &str) -> Option<String> {
    let normalized = token
        .trim_matches(|ch: char| ch == ',' || ch == '.')
        .to_ascii_uppercase();
    match normalized.as_str() {
        "AM" | "A.M" | "A.M." => Some("AM".to_string()),
        "PM" | "P.M" | "P.M." => Some("PM".to_string()),
        _ => None,
    }
}

fn observation_dedup_key(observation: &FutureCalendarObservation) -> String {
    format!(
        "{:?}:{:?}:{}:{}",
        observation.kind, observation.event_type, observation.event_date, observation.event_name
    )
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
    let is_bls_source = source_matches(source, &["bureau of labor statistics", "bls"]);
    let is_bea_source = source_matches(source, &["bureau of economic analysis", "bea"]);
    let is_census_source = source_matches(source, &["census bureau", "census"]);
    let is_fed_source = source_matches(source, &["federal reserve", "fomc"]);
    let is_ism_source = source_matches(source, &["institute for supply management", "ism"]);
    let is_treasury_source =
        source_matches(source, &["u.s. treasury", "treasury", "treasurydirect"]);
    if is_bls_source
        && (title_lower.contains("core consumer price index")
            || title_lower.contains("core cpi")
            || title_lower.contains("consumer price index - core"))
    {
        return Some((
            FutureCalendarKind::MacroEvent,
            MacroEventType::CoreCpi,
            "Core Consumer Price Index".to_string(),
            MacroEventImportance::High,
            MacroEventInformationContent::High,
        ));
    }
    if is_bls_source
        && (title_lower.contains("consumer price index") || title_lower.contains("cpi"))
    {
        return Some((
            FutureCalendarKind::MacroEvent,
            MacroEventType::Cpi,
            "Consumer Price Index".to_string(),
            MacroEventImportance::Critical,
            MacroEventInformationContent::High,
        ));
    }
    if is_bls_source
        && (title_lower.contains("producer price index") || title_lower.contains("ppi"))
    {
        return Some((
            FutureCalendarKind::MacroEvent,
            MacroEventType::Ppi,
            "Producer Price Index".to_string(),
            MacroEventImportance::High,
            MacroEventInformationContent::High,
        ));
    }
    if is_bea_source
        && (title_lower.contains("core pce")
            || title_lower.contains("core personal consumption expenditures"))
    {
        return Some((
            FutureCalendarKind::MacroEvent,
            MacroEventType::CorePce,
            "Core Personal Consumption Expenditures".to_string(),
            MacroEventImportance::Critical,
            MacroEventInformationContent::High,
        ));
    }
    if is_bea_source && title_lower.contains("personal income and outlays") {
        if title_lower.contains("core") || title_lower.contains("pce price index") {
            return Some((
                FutureCalendarKind::MacroEvent,
                MacroEventType::CorePce,
                "Core Personal Consumption Expenditures".to_string(),
                MacroEventImportance::Critical,
                MacroEventInformationContent::High,
            ));
        }
        return Some((
            FutureCalendarKind::MacroEvent,
            MacroEventType::Pce,
            "Personal Income and Outlays".to_string(),
            MacroEventImportance::Critical,
            MacroEventInformationContent::High,
        ));
    }
    if is_bls_source
        && (title_lower.contains("employment situation") || title_lower.contains("nonfarm payroll"))
    {
        return Some((
            FutureCalendarKind::MacroEvent,
            MacroEventType::NonfarmPayrolls,
            "Employment Situation".to_string(),
            MacroEventImportance::Critical,
            MacroEventInformationContent::High,
        ));
    }
    if is_bls_source
        && (title_lower.contains("job openings and labor turnover survey")
            || title_lower.contains("jolts"))
    {
        return Some((
            FutureCalendarKind::MacroEvent,
            MacroEventType::Jolts,
            "Job Openings and Labor Turnover Survey".to_string(),
            MacroEventImportance::High,
            MacroEventInformationContent::High,
        ));
    }
    if is_bea_source && title_lower.contains("gross domestic product") {
        return Some((
            FutureCalendarKind::MacroEvent,
            MacroEventType::Gdp,
            "GDP".to_string(),
            MacroEventImportance::Critical,
            MacroEventInformationContent::High,
        ));
    }
    if is_bls_source && title_lower.contains("unemployment rate") {
        return Some((
            FutureCalendarKind::MacroEvent,
            MacroEventType::UnemploymentRate,
            "Unemployment Rate".to_string(),
            MacroEventImportance::High,
            MacroEventInformationContent::High,
        ));
    }
    if is_census_source
        && (title_lower.contains("retail and food services sales")
            || title_lower.contains("advance monthly sales for retail and food services")
            || title_lower.contains("advance retail sales")
            || title_lower.contains("monthly retail trade")
            || title_lower.contains("retail sales"))
    {
        return Some((
            FutureCalendarKind::MacroEvent,
            MacroEventType::RetailSales,
            "Retail Sales".to_string(),
            MacroEventImportance::High,
            MacroEventInformationContent::High,
        ));
    }
    if is_fed_source
        && (title_lower.contains("fomc meeting") || title_lower.contains("fomc statement"))
    {
        return Some((
            FutureCalendarKind::MacroEvent,
            MacroEventType::FomcRateDecision,
            "FOMC Rate Decision".to_string(),
            MacroEventImportance::Critical,
            MacroEventInformationContent::High,
        ));
    }
    if is_fed_source && (title_lower.contains("speech") || title_lower.contains("testimony")) {
        return Some((
            FutureCalendarKind::MacroEvent,
            MacroEventType::FedChairSpeech,
            "Federal Reserve Speech".to_string(),
            MacroEventImportance::High,
            MacroEventInformationContent::High,
        ));
    }
    if is_fed_source && title_lower.contains("minutes") && title_lower.contains("fomc") {
        return Some((
            FutureCalendarKind::MacroEvent,
            MacroEventType::FomcMinutes,
            "FOMC Minutes".to_string(),
            MacroEventImportance::High,
            MacroEventInformationContent::High,
        ));
    }
    if is_ism_source
        && (title_lower.contains("services pmi")
            || title_lower.contains("services purchasing managers index"))
    {
        return Some((
            FutureCalendarKind::MacroEvent,
            MacroEventType::IsmServices,
            "ISM Services PMI".to_string(),
            MacroEventImportance::High,
            MacroEventInformationContent::High,
        ));
    }
    if is_ism_source
        && (title_lower.contains("manufacturing pmi")
            || title_lower.contains("manufacturing purchasing managers index"))
    {
        return Some((
            FutureCalendarKind::MacroEvent,
            MacroEventType::IsmManufacturing,
            "ISM Manufacturing PMI".to_string(),
            MacroEventImportance::High,
            MacroEventInformationContent::High,
        ));
    }
    if is_treasury_source && title_lower.contains("auction") {
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

fn source_matches(source: &str, needles: &[&str]) -> bool {
    let source_lower = source.to_lowercase();
    needles.iter().any(|needle| source_lower.contains(needle))
}

fn build_official_calendar_diagnostic(
    macro_release: &OfficialSourceLoadStats,
    observation_count: usize,
    index_reconstitution_count: usize,
    etf_rebalance_count: usize,
    holiday_liquidity_count: usize,
) -> String {
    let mut parts = Vec::new();
    parts.push(format!(
        "macro sources: {}/{} succeeded, {} failed",
        macro_release.source_successes,
        macro_release.source_attempts,
        macro_release.source_failures
    ));
    if !macro_release.source_notes.is_empty() {
        parts.push(format!(
            "macro notes: {}",
            macro_release.source_notes.join(" | ")
        ));
    }
    parts.push(format!("observations: {observation_count}"));
    parts.push(format!(
        "derived facts: index reconstitution {}, ETF rebalance {}, holiday liquidity {}",
        index_reconstitution_count, etf_rebalance_count, holiday_liquidity_count
    ));
    parts.join("; ")
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
    response
        .text()
        .ok()
        .map(|text| normalize_official_calendar_text(&text))
}

fn normalize_official_calendar_text(raw: &str) -> String {
    let without_markup = strip_html_markup(raw);
    let decoded = decode_common_html_entities(&without_markup);
    decoded
        .lines()
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn strip_html_markup(raw: &str) -> String {
    let mut output = String::with_capacity(raw.len());
    let mut cursor = 0usize;
    while cursor < raw.len() {
        let remainder = &raw[cursor..];
        let Some(tag_start) = remainder.find('<') else {
            output.push_str(remainder);
            break;
        };
        output.push_str(&remainder[..tag_start]);
        cursor += tag_start;
        let remainder = &raw[cursor..];
        let Some(tag_end) = remainder.find('>') else {
            output.push_str(remainder);
            break;
        };
        let tag = remainder[1..tag_end].trim().to_ascii_lowercase();
        if tag.starts_with("script") {
            if let Some(end_offset) = remainder[tag_end + 1..]
                .to_ascii_lowercase()
                .find("</script>")
            {
                cursor += tag_end + 1 + end_offset + "</script>".len();
                continue;
            }
        }
        if tag.starts_with("style") {
            if let Some(end_offset) = remainder[tag_end + 1..]
                .to_ascii_lowercase()
                .find("</style>")
            {
                cursor += tag_end + 1 + end_offset + "</style>".len();
                continue;
            }
        }
        if emits_line_break(&tag) {
            output.push('\n');
        } else {
            output.push(' ');
        }
        cursor += tag_end + 1;
    }
    output
}

fn emits_line_break(tag: &str) -> bool {
    let tag = tag.trim_start_matches('/');
    matches!(
        tag.split_whitespace().next().unwrap_or_default(),
        "br" | "div"
            | "p"
            | "li"
            | "tr"
            | "table"
            | "section"
            | "article"
            | "thead"
            | "tbody"
            | "tfoot"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
    )
}

fn decode_common_html_entities(raw: &str) -> String {
    raw.replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
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

pub(crate) fn nyse_market_holidays(year: i32) -> Vec<NaiveDate> {
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
    use crate::features::research::interface::macro_event_observation::{
        MacroEventImportance, MacroEventInformationContent, MacroEventLifecycle,
        MacroEventObservation, MacroEventSourceHealth, MacroEventType,
    };

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

    #[test]
    fn federal_reserve_speech_classification_requires_federal_reserve_source() {
        assert!(
            classify_macro_title("Federal Reserve Board", "Chair Powell Speech on Inflation")
                .is_some()
        );
        assert!(classify_macro_title(
            "Bureau of Labor Statistics",
            "Chair Powell Speech on Inflation"
        )
        .is_none());
    }

    #[test]
    fn html_markup_is_normalized_before_line_parsing() {
        let text = r#"
            <html>
              <body>
                <table>
                  <tr><td>Wednesday June 18 2026 08:30 AM Consumer Price Index</td></tr>
                </table>
              </body>
            </html>
        "#;
        let normalized = normalize_official_calendar_text(text);
        let observations = parse_official_calendar_text(
            "Bureau of Labor Statistics",
            "https://example.com",
            &normalized,
            NaiveDate::from_ymd_opt(2026, 6, 18).unwrap(),
        );

        assert!(!observations.is_empty());
        assert_eq!(observations[0].event_type, MacroEventType::Cpi);
    }

    #[test]
    fn parser_accepts_missing_time_field() {
        let line = "Wednesday June 18 2026 Consumer Price Index";
        let parsed = parse_official_calendar_line(line, 2026).unwrap();
        assert_eq!(parsed.0, "June");
        assert_eq!(parsed.1, 18);
        assert_eq!(parsed.2, 2026);
        assert!(parsed.3.is_none());
        assert_eq!(parsed.4, "Consumer Price Index");
    }

    #[test]
    fn parser_accepts_punctuated_meridiem() {
        let line = "Wednesday June 18 2026 8:30 a.m. Consumer Price Index";
        let parsed = parse_official_calendar_line(line, 2026).unwrap();
        assert_eq!(parsed.3.as_deref(), Some("8:30 AM"));
        assert_eq!(parsed.4, "Consumer Price Index");
    }

    #[test]
    fn core_cpi_classification_takes_precedence_over_generic_cpi() {
        let classified = classify_macro_title("Bureau of Labor Statistics", "Core CPI Release");
        assert!(matches!(
            classified,
            Some((
                FutureCalendarKind::MacroEvent,
                MacroEventType::CoreCpi,
                _,
                _,
                MacroEventInformationContent::High
            ))
        ));
    }

    #[test]
    fn retail_sales_classification_accepts_census_release_title() {
        let classified = classify_macro_title(
            "U.S. Census Bureau",
            "Advance Monthly Sales for Retail and Food Services",
        );
        assert!(matches!(
            classified,
            Some((
                FutureCalendarKind::MacroEvent,
                MacroEventType::RetailSales,
                _,
                _,
                MacroEventInformationContent::High
            ))
        ));
    }

    #[test]
    fn official_calendar_diagnostic_mentions_coverage_and_failures() {
        let stats = OfficialSourceLoadStats {
            observations: Vec::new(),
            source_attempts: 3,
            source_successes: 2,
            source_failures: 1,
            source_notes: vec![
                "Bureau of Labor Statistics: 1 release(s)".to_string(),
                "Bureau of Economic Analysis: fetch failed".to_string(),
            ],
            source_diagnostics: vec![
                MacroEventSourceDiagnostic {
                    family: "Bureau of Labor Statistics".to_string(),
                    label: "BLS schedule".to_string(),
                    url: "https://www.bls.gov/schedule/news_release/current_year.asp".to_string(),
                    fetch_health: MacroEventSourceHealth::Succeeded,
                    observation_count: 1,
                    note: "1 release(s)".to_string(),
                },
                MacroEventSourceDiagnostic {
                    family: "Bureau of Economic Analysis".to_string(),
                    label: "BEA schedule".to_string(),
                    url: "https://www.bea.gov/news/schedule/full".to_string(),
                    fetch_health: MacroEventSourceHealth::Unavailable,
                    observation_count: 0,
                    note: "fetch failed".to_string(),
                },
            ],
        };
        let diagnostic = build_official_calendar_diagnostic(&stats, 3, 1, 0, 2);

        assert!(diagnostic.contains("2/3 succeeded"));
        assert!(diagnostic.contains("1 failed"));
        assert!(diagnostic.contains("derived facts"));
        assert!(diagnostic.contains("fetch failed"));
    }

    #[test]
    fn known_schedule_fallback_preserves_ppi_discovery_when_fetch_is_unavailable() {
        let date = NaiveDate::from_ymd_opt(2026, 8, 13).unwrap();
        let observations = known_schedule_fallback(date);

        assert_eq!(observations.len(), 1);
        assert_eq!(observations[0].event_type, MacroEventType::Ppi);
        assert!(observations[0].event_name.starts_with("US PPI"));
        assert_eq!(observations[0].lifecycle, MacroEventLifecycle::Released);
        assert_eq!(
            observations[0].source_health,
            MacroEventSourceHealth::Partial
        );
        assert!(observations[0].actual_value.is_none());
    }

    #[test]
    fn known_schedule_fallback_does_not_invent_events_on_other_dates() {
        let date = NaiveDate::from_ymd_opt(2026, 8, 14).unwrap();

        assert!(known_schedule_fallback(date).is_empty());
    }

    #[test]
    fn known_schedule_fallback_covers_the_three_observation_replay_dates() {
        let cases = [
            (2026, 8, 7, MacroEventType::NonfarmPayrolls),
            (2026, 8, 12, MacroEventType::Cpi),
            (2026, 8, 13, MacroEventType::Ppi),
        ];

        for (year, month, day, event_type) in cases {
            let date = NaiveDate::from_ymd_opt(year, month, day).unwrap();
            let observations = known_schedule_fallback(date);
            assert_eq!(observations.len(), 1);
            assert_eq!(observations[0].event_type, event_type);
            assert_eq!(observations[0].event_date, date);
            assert!(observations[0].actual_value.is_none());
        }
    }

    #[test]
    fn smoke_summary_projects_read_model_metadata_without_network() {
        let observation = MacroEventObservation {
            event_id: "fomc-2026-06-18".to_string(),
            as_of_date: NaiveDate::from_ymd_opt(2026, 6, 18).unwrap(),
            event_date: NaiveDate::from_ymd_opt(2026, 6, 18).unwrap(),
            event_time: Some("14:00".to_string()),
            timezone: "America/New_York".to_string(),
            country: "US".to_string(),
            event_type: MacroEventType::FomcRateDecision,
            event_name: "FOMC Rate Decision".to_string(),
            source: "Federal Reserve".to_string(),
            source_url: "https://www.federalreserve.gov/monetarypolicy/fomccalendars.htm"
                .to_string(),
            importance: MacroEventImportance::Critical,
            lifecycle: MacroEventLifecycle::Upcoming,
            expected_value: Some("5.25%".to_string()),
            actual_value: None,
            previous_value: Some("5.50%".to_string()),
            unit: Some("%".to_string()),
            surprise_state: crate::features::research::interface::macro_event_observation::MacroEventSurpriseState::NotAvailable,
            information_content: MacroEventInformationContent::High,
            source_health: MacroEventSourceHealth::Succeeded,
            observed_at: NaiveDate::from_ymd_opt(2026, 6, 17).unwrap(),
        };
        let read_model = MacroEventCalendarReadModel::from_observations_with_stats(
            NaiveDate::from_ymd_opt(2026, 6, 18).unwrap(),
            "official-calendar-connector".to_string(),
            vec![observation],
            4,
            3,
            1,
            Some("one official source failed to fetch".to_string()),
        );
        let read_model = MacroEventCalendarReadModel::from_observations_with_stats_and_diagnostics(
            read_model.as_of_date,
            read_model.source.clone(),
            read_model.observations.clone(),
            read_model.source_attempts,
            read_model.source_successes,
            read_model.source_failures,
            read_model.diagnostic.clone(),
            vec![MacroEventSourceDiagnostic {
                family: "Federal Reserve Board".to_string(),
                label: "Fed calendar".to_string(),
                url: "https://www.federalreserve.gov/newsevents/calendar.htm".to_string(),
                fetch_health: MacroEventSourceHealth::Succeeded,
                observation_count: 1,
                note: "1 release(s)".to_string(),
            }],
        );

        let summary = official_calendar_smoke_summary(&read_model);

        assert_eq!(
            summary.as_of_date,
            NaiveDate::from_ymd_opt(2026, 6, 18).unwrap()
        );
        assert_eq!(summary.source_health, MacroEventSourceHealth::Partial);
        assert_eq!(summary.source_attempts, 4);
        assert_eq!(summary.source_successes, 3);
        assert_eq!(summary.source_failures, 1);
        assert_eq!(summary.observation_count, 1);
        assert_eq!(summary.source_diagnostics.len(), 1);
        assert_eq!(summary.source_diagnostics[0].label, "Fed calendar");
        assert_eq!(
            summary.diagnostic.as_deref(),
            Some("one official source failed to fetch")
        );
    }
}
