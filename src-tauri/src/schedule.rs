use chrono::{NaiveDate, NaiveDateTime, TimeDelta, Timelike};
use serde::{Deserialize, Serialize};

use crate::settings::Settings;

/// 재부팅해도 유지돼야 하는 실행 상태 (state.json)
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RunState {
    /// 마지막으로 알림을 띄운 슬롯 키 ("2026-10-15T19")
    pub last_fired: Option<String>,
    /// "오늘은 그만"을 누른 날짜. 날짜가 바뀌면 자동으로 무효
    pub mute_date: Option<NaiveDate>,
}

/// 근무시간 외면 현재 슬롯 키, 근무시간이면 None.
/// 슬롯은 "날짜+시" 단위 — 퇴근 직후 슬롯만 유예 시각(18:10)부터 시작한다.
pub fn slot_key(now: NaiveDateTime, is_day_off: bool, settings: &Settings) -> Option<String> {
    let t = now.time();
    let (first_evening_alert, wrapped) =
        settings.work_end.overflowing_add_signed(TimeDelta::minutes(i64::from(settings.end_grace_min)));
    // 퇴근+유예가 자정을 넘으면 그날 저녁 알림은 없다. 자정 이후는 출근 전 구간이 맡는다
    let is_evening = wrapped == 0 && t >= first_evening_alert;
    let is_off_hours = is_day_off || t < settings.work_start || is_evening;
    is_off_hours.then(|| now.format("%Y-%m-%dT%H").to_string())
}

/// 지금 띄워야 할 슬롯. 이미 띄운 슬롯이거나 오늘 음소거면 None.
/// 실행 직후·절전 해제·정각 도래 모두 이 한 규칙으로 처리된다.
pub fn due_slot(now: NaiveDateTime, is_day_off: bool, settings: &Settings, run: &RunState) -> Option<String> {
    if run.mute_date == Some(now.date()) {
        return None;
    }
    let slot = slot_key(now, is_day_off, settings)?;
    if !is_day_off && in_last_hour_before_work(now, settings) && !fired_earlier_today(now, run) {
        return None;
    }
    (run.last_fired.as_deref() != Some(slot.as_str())).then_some(slot)
}

/// 평일 출근 전 초과는 1시간 이상이어야 인정된다.
/// 출근 1시간 전 이후에 처음 켠 거면 정시 출근이므로 알리지 않는다.
fn in_last_hour_before_work(now: NaiveDateTime, settings: &Settings) -> bool {
    let (cutoff, wrapped) = settings.work_start.overflowing_sub_signed(TimeDelta::hours(1));
    let t = now.time();
    t < settings.work_start && (wrapped != 0 || t >= cutoff)
}

/// 오늘 이미 알림이 떴다 = 그 전부터 PC 앞에 있었다
fn fired_earlier_today(now: NaiveDateTime, run: &RunState) -> bool {
    let today = now.format("%Y-%m-%dT").to_string();
    run.last_fired.as_deref().is_some_and(|s| s.starts_with(&today))
}

const NEXT_ALERT_SCAN_MINUTES: i64 = 3 * 24 * 60;

/// 설정 창의 "다음 알림" 표시용. 지금 슬롯이 아직 안 떴으면 지금을 돌려준다.
// ponytail: 분 단위 선형 탐색(최대 3일) — 설정 창이 요청할 때만 호출하므로 충분
pub fn next_alert(
    now: NaiveDateTime,
    settings: &Settings,
    run: &RunState,
    is_day_off: impl Fn(NaiveDate) -> bool,
) -> Option<NaiveDateTime> {
    let start = now.with_second(0)?.with_nanosecond(0)?;
    (0..NEXT_ALERT_SCAN_MINUTES)
        .map(|m| start + TimeDelta::minutes(m))
        .find(|&t| due_slot(t, is_day_off(t.date()), settings, run).is_some())
}

pub fn message(now: NaiveDateTime) -> String {
    format!("{} 초과근무 기록 버튼을 누르세요", now.format("%H:%M"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveTime;

    fn at(s: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M").unwrap()
    }

    fn weekday_slot(s: &str) -> Option<String> {
        slot_key(at(s), false, &Settings::default())
    }

    #[test]
    fn weekday_early_morning_is_off_hours() {
        assert_eq!(weekday_slot("2026-10-15 00:30"), Some("2026-10-15T00".into()));
        assert_eq!(weekday_slot("2026-10-15 07:30"), Some("2026-10-15T07".into()));
        assert_eq!(weekday_slot("2026-10-15 08:00"), Some("2026-10-15T08".into()));
        assert_eq!(weekday_slot("2026-10-15 08:59"), Some("2026-10-15T08".into()));
    }

    #[test]
    fn weekday_work_hours_and_grace_are_silent() {
        assert_eq!(weekday_slot("2026-10-15 09:00"), None);
        assert_eq!(weekday_slot("2026-10-15 13:00"), None);
        assert_eq!(weekday_slot("2026-10-15 18:00"), None);
        assert_eq!(weekday_slot("2026-10-15 18:09"), None);
    }

    #[test]
    fn weekday_evening_starts_after_grace() {
        assert_eq!(weekday_slot("2026-10-15 18:10"), Some("2026-10-15T18".into()));
        assert_eq!(weekday_slot("2026-10-15 19:00"), Some("2026-10-15T19".into()));
        assert_eq!(weekday_slot("2026-10-15 23:59"), Some("2026-10-15T23".into()));
    }

    #[test]
    fn late_work_end_past_midnight_keeps_work_hours_silent() {
        // 23:50 퇴근 + 20분 유예 = 자정 넘김. 낮 근무시간에 알림이 뜨면 안 된다
        let settings = Settings {
            work_end: NaiveTime::from_hms_opt(23, 50, 0).unwrap(),
            end_grace_min: 20,
            ..Settings::default()
        };
        assert_eq!(slot_key(at("2026-10-15 13:00"), false, &settings), None);
        assert_eq!(slot_key(at("2026-10-15 23:55"), false, &settings), None);
        assert_eq!(slot_key(at("2026-10-15 08:00"), false, &settings), Some("2026-10-15T08".into()));
    }

    #[test]
    fn day_off_is_always_off_hours() {
        assert_eq!(slot_key(at("2026-10-17 13:00"), true, &Settings::default()), Some("2026-10-17T13".into()));
    }

    #[test]
    fn same_slot_does_not_fire_twice() {
        // 19:00에 띄움 → 19:30 절전 해제 → 안 뜸
        let run = RunState { last_fired: Some("2026-10-15T19".into()), mute_date: None };
        assert_eq!(due_slot(at("2026-10-15 19:30"), false, &Settings::default(), &run), None);
    }

    #[test]
    fn missed_slot_fires_on_wake() {
        // 18시 슬롯 후 잠듦 → 19:30 해제 → 19시 슬롯 즉시
        let run = RunState { last_fired: Some("2026-10-15T18".into()), mute_date: None };
        assert_eq!(due_slot(at("2026-10-15 19:30"), false, &Settings::default(), &run), Some("2026-10-15T19".into()));
    }

    #[test]
    fn weekday_boot_in_last_hour_before_work_is_silent() {
        // 8:30·8:40에 켬 = 정시 출근. 초과 1시간 인정이 안 되니 알림 없음
        let run = fired("2026-10-14T23");
        assert_eq!(due_slot(at("2026-10-15 08:30"), false, &Settings::default(), &run), None);
        assert_eq!(due_slot(at("2026-10-15 08:40"), false, &Settings::default(), &run), None);
    }

    #[test]
    fn weekday_boot_before_last_hour_fires_now_and_at_hour() {
        // 7:30에 켬 → 즉시, 이어서 8:00 정각에도
        let run = fired("2026-10-14T23");
        assert_eq!(due_slot(at("2026-10-15 07:30"), false, &Settings::default(), &run), Some("2026-10-15T07".into()));
        let run = fired("2026-10-15T07");
        assert_eq!(due_slot(at("2026-10-15 08:00"), false, &Settings::default(), &run), Some("2026-10-15T08".into()));
    }

    #[test]
    fn day_off_boot_any_time_fires() {
        let run = fired("2026-10-16T23");
        assert_eq!(due_slot(at("2026-10-17 08:30"), true, &Settings::default(), &run), Some("2026-10-17T08".into()));
        assert_eq!(due_slot(at("2026-10-17 10:30"), true, &Settings::default(), &run), Some("2026-10-17T10".into()));
    }

    #[test]
    fn muted_today_is_silent_but_resumes_after_midnight() {
        let run = RunState { last_fired: None, mute_date: NaiveDate::from_ymd_opt(2026, 10, 15) };
        assert_eq!(due_slot(at("2026-10-15 21:00"), false, &Settings::default(), &run), None);
        assert_eq!(due_slot(at("2026-10-16 00:00"), false, &Settings::default(), &run), Some("2026-10-16T00".into()));
    }

    fn next(now: &str, run: &RunState) -> Option<NaiveDateTime> {
        // 2026-10-17(토)만 휴일
        next_alert(at(now), &Settings::default(), run, |d| d == NaiveDate::from_ymd_opt(2026, 10, 17).unwrap())
    }

    fn fired(slot: &str) -> RunState {
        RunState { last_fired: Some(slot.into()), mute_date: None }
    }

    #[test]
    fn next_alert_is_now_when_current_slot_not_fired() {
        assert_eq!(next("2026-10-15 19:30", &RunState::default()), Some(at("2026-10-15 19:30")));
    }

    #[test]
    fn next_alert_is_next_hour_after_firing() {
        assert_eq!(next("2026-10-15 19:30", &fired("2026-10-15T19")), Some(at("2026-10-15 20:00")));
    }

    #[test]
    fn next_alert_skips_work_hours_to_grace_end() {
        assert_eq!(next("2026-10-15 08:30", &fired("2026-10-15T08")), Some(at("2026-10-15 18:10")));
        assert_eq!(next("2026-10-15 13:00", &RunState::default()), Some(at("2026-10-15 18:10")));
    }

    #[test]
    fn next_alert_when_muted_is_midnight() {
        let run = RunState { last_fired: None, mute_date: NaiveDate::from_ymd_opt(2026, 10, 15) };
        assert_eq!(next("2026-10-15 21:00", &run), Some(at("2026-10-16 00:00")));
    }

    #[test]
    fn next_alert_on_day_off_ignores_work_hours() {
        assert_eq!(next("2026-10-17 10:20", &fired("2026-10-17T10")), Some(at("2026-10-17 11:00")));
    }

    #[test]
    fn message_shows_time() {
        assert_eq!(message(at("2026-10-15 18:10")), "18:10 초과근무 기록 버튼을 누르세요");
    }
}
