use std::collections::BTreeSet;
use std::time::Duration;

use chrono::{Datelike, NaiveDate, Weekday};
use serde::{Deserialize, Serialize};

/// 구글 "대한민국의 휴일" 공개 캘린더. 키 불필요, 대체·임시공휴일 포함.
const ICS_URL: &str =
    "https://calendar.google.com/calendar/ical/ko.south_korea%23holiday%40group.v.calendar.google.com/public/basic.ics";
const FETCH_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct HolidayCache {
    pub fetched_on: Option<NaiveDate>,
    pub dates: BTreeSet<NaiveDate>,
}

pub fn is_day_off(date: NaiveDate, cache: &HolidayCache, extra: &BTreeSet<NaiveDate>) -> bool {
    matches!(date.weekday(), Weekday::Sat | Weekday::Sun) || cache.dates.contains(&date) || extra.contains(&date)
}

/// DESCRIPTION이 "공휴일"인 VEVENT의 날짜만 추린다 ("기념일"은 제외).
/// 구글 캘린더는 연휴를 하루짜리 이벤트 여러 개로 주므로 DTSTART만 본다.
pub fn parse_ics(text: &str) -> BTreeSet<NaiveDate> {
    let mut dates = BTreeSet::new();
    let mut date = None;
    let mut is_public = false;
    for line in text.lines().map(str::trim_end) {
        if line == "BEGIN:VEVENT" {
            date = None;
            is_public = false;
        } else if let Some(v) = line.strip_prefix("DTSTART;VALUE=DATE:") {
            date = NaiveDate::parse_from_str(v, "%Y%m%d").ok();
        } else if let Some(v) = line.strip_prefix("DESCRIPTION:") {
            is_public = v.starts_with("공휴일");
        } else if line == "END:VEVENT" {
            if let (Some(d), true) = (date, is_public) {
                dates.insert(d);
            }
        }
    }
    dates
}

pub fn fetch() -> Result<BTreeSet<NaiveDate>, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(FETCH_TIMEOUT)).build().into();
    let text = agent
        .get(ICS_URL)
        .call()
        .and_then(|mut res| res.body_mut().read_to_string())
        .map_err(|e| format!("공휴일 다운로드 실패: {e}"))?;
    let dates = parse_ics(&text);
    // 빈 결과로 캐시를 덮어쓰지 않도록 실패 처리
    if dates.is_empty() {
        return Err("공휴일 응답에 공휴일이 없음 (형식 변경 의심)".into());
    }
    Ok(dates)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "BEGIN:VCALENDAR\r\n\
BEGIN:VEVENT\r\nDTSTART;VALUE=DATE:20261009\r\nDESCRIPTION:공휴일\r\nSUMMARY:한글날\r\nEND:VEVENT\r\n\
BEGIN:VEVENT\r\nDTSTART;VALUE=DATE:20260508\r\nDESCRIPTION:기념일\\n기념일을 숨기려면 Google Calendar 설정 > 대한민국의 휴일 캘린더로 이동하세요.\r\nSUMMARY:어버이날\r\nEND:VEVENT\r\n\
BEGIN:VEVENT\r\nDTSTART;VALUE=DATE:20261005\r\nDESCRIPTION:공휴일\r\nSUMMARY:쉬는 날 개천절\r\nEND:VEVENT\r\n\
END:VCALENDAR\r\n";

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn parse_keeps_only_public_holidays() {
        assert_eq!(parse_ics(SAMPLE), BTreeSet::from([d(2026, 10, 5), d(2026, 10, 9)]));
    }

    #[test]
    fn weekend_holiday_and_extra_are_days_off() {
        let cache = HolidayCache { fetched_on: None, dates: BTreeSet::from([d(2026, 10, 9)]) };
        let extra = BTreeSet::from([d(2026, 10, 14)]);
        assert!(is_day_off(d(2026, 10, 17), &cache, &extra)); // 토
        assert!(is_day_off(d(2026, 10, 9), &cache, &extra)); // 한글날
        assert!(is_day_off(d(2026, 10, 14), &cache, &extra)); // 수동 추가
        assert!(!is_day_off(d(2026, 10, 15), &cache, &extra)); // 평일
    }

    /// 실제 네트워크 호출 — `cargo test -- --ignored` 로만 실행
    #[test]
    #[ignore]
    fn fetch_real_calendar_contains_hangul_day() {
        assert!(fetch().unwrap().contains(&d(2026, 10, 9)));
    }
}
