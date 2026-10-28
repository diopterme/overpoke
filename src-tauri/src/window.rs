use tauri::{AppHandle, Emitter, LogicalPosition, Manager, WebviewUrl, WebviewWindowBuilder};

use crate::settings::PopupPosition;

const POPUP_LABEL: &str = "popup";
const SETTINGS_LABEL: &str = "settings";
const POPUP_SIZE: (f64, f64) = (360.0, 150.0);
const SETTINGS_SIZE: (f64, f64) = (600.0, 480.0);
const SCREEN_MARGIN: f64 = 16.0;
/// 팝업이 떠 있는 채로 다음 알림이 오면 새 창 대신 이 이벤트로 문구만 갱신
pub const POPUP_REFRESH_EVENT: &str = "popup-refresh";
/// 음소거·알림 발사로 상태가 바뀜. 설정 창이 상태 표시를 다시 불러온다
pub const STATUS_CHANGED_EVENT: &str = "status-changed";

/// area = 작업 영역(작업표시줄 제외)의 논리 좌표 (x, y, w, h)
pub fn popup_xy(pos: PopupPosition, area: (f64, f64, f64, f64)) -> (f64, f64) {
    let (x, y, w, h) = area;
    let (pw, ph) = POPUP_SIZE;
    match pos {
        PopupPosition::Center => (x + (w - pw) / 2.0, y + (h - ph) / 2.0),
        PopupPosition::BottomRight => (x + w - pw - SCREEN_MARGIN, y + h - ph - SCREEN_MARGIN),
    }
}

/// 주 모니터 기준 팝업 좌표. 모니터 정보를 못 얻으면 None
fn primary_popup_xy(app: &AppHandle, pos: PopupPosition) -> tauri::Result<Option<(f64, f64)>> {
    Ok(app.primary_monitor()?.map(|monitor| {
        let sf = monitor.scale_factor();
        let wa = monitor.work_area();
        let area = (
            f64::from(wa.position.x) / sf,
            f64::from(wa.position.y) / sf,
            f64::from(wa.size.width) / sf,
            f64::from(wa.size.height) / sf,
        );
        popup_xy(pos, area)
    }))
}

pub fn open_popup(app: &AppHandle, pos: PopupPosition) -> tauri::Result<()> {
    if let Some(w) = app.get_webview_window(POPUP_LABEL) {
        w.emit(POPUP_REFRESH_EVENT, ())?;
        return w.set_focus();
    }
    let builder = WebviewWindowBuilder::new(app, POPUP_LABEL, WebviewUrl::App("popup.html".into()))
        .title("초과콕")
        .inner_size(POPUP_SIZE.0, POPUP_SIZE.1)
        .decorations(false)
        .resizable(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .focused(true);
    let builder = match primary_popup_xy(app, pos)? {
        Some((x, y)) => builder.position(x, y),
        None => builder.center(),
    };
    builder.build().map(|_| ())
}

/// 설정 창의 미리보기 버튼. 이미 떠 있으면 저장 전 위치로 옮기기만 한다
pub fn preview_popup(app: &AppHandle, pos: PopupPosition) -> tauri::Result<()> {
    let Some(w) = app.get_webview_window(POPUP_LABEL) else {
        return open_popup(app, pos);
    };
    if let Some((x, y)) = primary_popup_xy(app, pos)? {
        w.set_position(LogicalPosition::new(x, y))?;
    }
    w.set_focus()
}

pub fn close_popup(app: &AppHandle) -> tauri::Result<()> {
    match app.get_webview_window(POPUP_LABEL) {
        Some(w) => w.close(),
        None => Ok(()),
    }
}

pub fn open_settings(app: &AppHandle) -> tauri::Result<()> {
    if let Some(w) = app.get_webview_window(SETTINGS_LABEL) {
        return w.set_focus();
    }
    WebviewWindowBuilder::new(app, SETTINGS_LABEL, WebviewUrl::App("settings.html".into()))
        .title("초과콕 설정")
        .inner_size(SETTINGS_SIZE.0, SETTINGS_SIZE.1)
        .resizable(false)
        .center()
        .build()
        .map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    const FULL_HD_WORK_AREA: (f64, f64, f64, f64) = (0.0, 0.0, 1920.0, 1040.0);

    #[test]
    fn center_position() {
        assert_eq!(popup_xy(PopupPosition::Center, FULL_HD_WORK_AREA), (780.0, 445.0));
    }

    #[test]
    fn bottom_right_position_keeps_margin() {
        assert_eq!(popup_xy(PopupPosition::BottomRight, FULL_HD_WORK_AREA), (1544.0, 874.0));
    }

    #[test]
    fn respects_work_area_offset() {
        // 작업표시줄이 위/왼쪽에 있으면 작업 영역 원점이 0이 아니다
        assert_eq!(popup_xy(PopupPosition::BottomRight, (100.0, 40.0, 1820.0, 1040.0)), (1544.0, 914.0));
    }
}
