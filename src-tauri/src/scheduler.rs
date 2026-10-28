use std::thread;
use std::time::{Duration, Instant};

use chrono::{Local, NaiveDate, NaiveDateTime};
use tauri::{AppHandle, Emitter, Manager};

use crate::holidays::{self, HolidayCache};
use crate::schedule::{self, RunState};
use crate::store::{self, HOLIDAYS_FILE, STATE_FILE};
use crate::{push, tray, window, AppState};

// ponytail: sleep 대신 10초 폴링 — 절전 중에는 단조 시계가 멈춰 "다음 정각까지 sleep"이 어긋난다.
// 폴링이면 절전 해제 감지가 따로 필요 없다. 정각 대비 최대 10초 지연.
const TICK: Duration = Duration::from_secs(10);
const HOLIDAY_RETRY: Duration = Duration::from_secs(60 * 60);

pub fn spawn(app: AppHandle) {
    thread::spawn(move || {
        let mut last_holiday_try: Option<Instant> = None;
        loop {
            let now = Local::now().naive_local();
            // 알림이 공휴일 다운로드(최대 10초)를 기다리지 않도록 먼저 처리
            fire_if_due(&app, now);
            // 음소거는 자정에 저절로 풀리므로 메뉴 글자도 매 tick 맞춘다
            tray::sync_mute_item(&app);
            refresh_holidays_if_stale(&app, now.date(), &mut last_holiday_try);
            thread::sleep(TICK);
        }
    });
}

fn fire_if_due(app: &AppHandle, now: NaiveDateTime) {
    let state = app.state::<AppState>();
    let settings = state.settings.lock().unwrap().clone();
    let is_day_off = holidays::is_day_off(now.date(), &state.holidays.lock().unwrap(), &settings.extra_holidays);

    // mute_today 커맨드와 경합하지 않도록 판단~갱신 동안 잠금 유지
    let mut run = state.run.lock().unwrap();
    let Some(slot) = schedule::due_slot(now, is_day_off, &settings, &run) else {
        return;
    };
    let next = RunState { last_fired: Some(slot), ..run.clone() };
    if let Err(e) = store::save(&state.dir, STATE_FILE, &next) {
        store::log_error(&state.dir, &e);
    }
    *run = next;
    drop(run);

    if let Err(e) = window::open_popup(app, settings.popup_position) {
        store::log_error(&state.dir, &format!("팝업 실패: {e}"));
    }
    if let Err(e) = app.emit(window::STATUS_CHANGED_EVENT, ()) {
        store::log_error(&state.dir, &format!("상태 이벤트 실패: {e}"));
    }
    let msg = format!("[초과콕] {}", schedule::message(now));
    let dir = state.dir.clone();
    thread::spawn(move || {
        for e in push::send_all(&settings, &msg) {
            store::log_error(&dir, &e);
        }
    });
}

fn refresh_holidays_if_stale(app: &AppHandle, today: NaiveDate, last_try: &mut Option<Instant>) {
    let state = app.state::<AppState>();
    if state.holidays.lock().unwrap().fetched_on == Some(today) {
        return;
    }
    if last_try.is_some_and(|t| t.elapsed() < HOLIDAY_RETRY) {
        return;
    }
    *last_try = Some(Instant::now());
    match holidays::fetch() {
        Ok(dates) => {
            let cache = HolidayCache { fetched_on: Some(today), dates };
            if let Err(e) = store::save(&state.dir, HOLIDAYS_FILE, &cache) {
                store::log_error(&state.dir, &e);
            }
            *state.holidays.lock().unwrap() = cache;
        }
        // 실패하면 기존 캐시로 계속 동작
        Err(e) => store::log_error(&state.dir, &e),
    }
}
