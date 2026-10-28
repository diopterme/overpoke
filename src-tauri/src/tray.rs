use chrono::Local;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{App, AppHandle, Manager, Wry};

use crate::{commands, store, window, AppState};

/// 상태에 따라 "끄기"/"다시 켜기"로 글자가 바뀌는 메뉴 항목. 바꾸려면 핸들을 보관해야 한다
pub struct MuteMenuItem(MenuItem<Wry>);

fn is_muted_today(app: &AppHandle) -> bool {
    app.state::<AppState>().run.lock().unwrap().mute_date == Some(Local::now().date_naive())
}

fn mute_label(is_muted: bool) -> &'static str {
    if is_muted { "오늘 알림 다시 켜기" } else { "오늘 알림 끄기" }
}

/// 음소거가 바뀌었거나 자정이 지나 풀렸을 때 메뉴 글자를 현재 상태에 맞춘다
pub fn sync_mute_item(app: &AppHandle) {
    if let Err(e) = app.state::<MuteMenuItem>().0.set_text(mute_label(is_muted_today(app))) {
        store::log_error(&app.state::<AppState>().dir, &format!("트레이 메뉴 갱신 실패: {e}"));
    }
}

/// 이벤트 핸들러 안에서 창을 만들면 Windows에서 데드락 (Tauri 문서) — 별도 스레드에서 연다
pub fn open_settings(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        if let Err(e) = window::open_settings(&app) {
            store::log_error(&app.state::<AppState>().dir, &format!("설정 창 열기 실패: {e}"));
        }
    });
}

fn set_muted(app: &AppHandle, is_muted: bool) {
    if let Err(e) = commands::set_muted(app, is_muted) {
        store::log_error(&app.state::<AppState>().dir, &format!("음소거 변경 실패: {e}"));
    }
}

pub fn build(app: &App) -> tauri::Result<()> {
    let settings = MenuItem::with_id(app, "settings", "설정", true, None::<&str>)?;
    let mute = MenuItem::with_id(app, "mute_toggle", mute_label(is_muted_today(app.handle())), true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "종료", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&settings, &mute, &separator, &quit])?;
    app.manage(MuteMenuItem(mute));
    let icon = app.default_window_icon().cloned().expect("tauri.conf.json bundle.icon 누락");
    TrayIconBuilder::new()
        .icon(icon)
        .tooltip("초과콕")
        .menu(&menu)
        // 왼쪽 클릭은 설정 창, 메뉴는 오른쪽 클릭 (Windows 관례)
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                open_settings(tray.app_handle());
            }
        })
        .on_menu_event(|app, event| match event.id.as_ref() {
            "settings" => open_settings(app),
            "mute_toggle" => set_muted(app, !is_muted_today(app)),
            "quit" => app.exit(0),
            _ => {}
        })
        .build(app)?;
    Ok(())
}
