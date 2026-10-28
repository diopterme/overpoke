use chrono::{Local, NaiveDateTime};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_autostart::ManagerExt;

use crate::schedule::{self, RunState};
use crate::settings::{PopupPosition, Settings};
use crate::store::{self, SETTINGS_FILE, STATE_FILE};
use crate::{holidays, push, tray, window, AppState};

#[derive(Serialize)]
pub struct SettingsView {
    settings: Settings,
    autostart: bool,
}

#[tauri::command]
pub fn get_settings(app: AppHandle, state: State<'_, AppState>) -> Result<SettingsView, String> {
    let autostart = app.autolaunch().is_enabled().map_err(|e| format!("자동 실행 상태 확인 실패: {e}"))?;
    Ok(SettingsView { settings: state.settings.lock().unwrap().clone(), autostart })
}

#[tauri::command]
pub fn save_settings(app: AppHandle, state: State<'_, AppState>, settings: Settings, autostart: bool) -> Result<(), String> {
    settings.validate()?;
    store::save(&state.dir, SETTINGS_FILE, &settings)?;
    *state.settings.lock().unwrap() = settings;

    let launcher = app.autolaunch();
    let is_enabled = launcher.is_enabled().map_err(|e| format!("자동 실행 상태 확인 실패: {e}"))?;
    // 이미 원하는 상태면 건드리지 않는다 (등록 안 된 항목 disable 시 OS별 에러 회피)
    if is_enabled != autostart {
        if autostart { launcher.enable() } else { launcher.disable() }.map_err(|e| format!("자동 실행 설정 실패: {e}"))?;
    }
    Ok(())
}

/// 팝업 버튼, 설정 창, 트레이 메뉴가 함께 쓴다
pub fn set_muted(app: &AppHandle, is_muted: bool) -> Result<(), String> {
    let state = app.state::<AppState>();
    let mut run = state.run.lock().unwrap();
    let next = RunState { mute_date: is_muted.then(|| Local::now().date_naive()), ..run.clone() };
    store::save(&state.dir, STATE_FILE, &next)?;
    *run = next;
    drop(run);
    tray::sync_mute_item(app);
    if is_muted {
        window::close_popup(app).map_err(|e| e.to_string())?;
    }
    app.emit(window::STATUS_CHANGED_EVENT, ()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn mute_today(app: AppHandle) -> Result<(), String> {
    set_muted(&app, true)
}

#[tauri::command]
pub fn unmute_today(app: AppHandle) -> Result<(), String> {
    set_muted(&app, false)
}

#[derive(Serialize)]
pub struct StatusView {
    day_off: bool,
    muted: bool,
    next_alert: Option<NaiveDateTime>,
}

/// 저장된 설정 기준의 오늘 상태. 설정 창 상단 표시용
#[tauri::command]
pub fn get_status(state: State<'_, AppState>) -> StatusView {
    let now = Local::now().naive_local();
    let settings = state.settings.lock().unwrap().clone();
    let cache = state.holidays.lock().unwrap().clone();
    let run = state.run.lock().unwrap().clone();
    let is_day_off = |d| holidays::is_day_off(d, &cache, &settings.extra_holidays);
    StatusView {
        day_off: is_day_off(now.date()),
        muted: run.mute_date == Some(now.date()),
        next_alert: schedule::next_alert(now, &settings, &run, is_day_off),
    }
}

/// 저장 전 폼의 위치로 띄운다. 슬롯·푸시와 무관.
/// 창을 만들 수 있으므로 async — 동기 커맨드에서 창 생성 시 Windows 데드락 (Tauri 문서)
#[tauri::command]
pub async fn preview_popup(app: AppHandle, position: PopupPosition) -> Result<(), String> {
    window::preview_popup(&app, position).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn close_popup(app: AppHandle) -> Result<(), String> {
    window::close_popup(&app).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn popup_message() -> String {
    schedule::message(Local::now().naive_local())
}

/// 저장 전 폼 값으로 바로 시험할 수 있게 settings를 인자로 받는다.
/// 네트워크 대기 중 UI가 멈추지 않도록 async (메인 스레드 밖에서 실행됨).
#[tauri::command]
pub async fn test_push(settings: Settings) -> Result<String, String> {
    settings.validate()?;
    if !settings.ntfy.enabled && !settings.telegram.enabled {
        return Err("켜진 푸시 채널이 없습니다".into());
    }
    let errors = push::send_all(&settings, "[초과콕] 테스트 알림");
    if errors.is_empty() { Ok("전송 완료".into()) } else { Err(errors.join("\n")) }
}
