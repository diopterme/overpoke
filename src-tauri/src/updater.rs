use std::thread;
use std::time::Duration;

use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons};
use tauri_plugin_updater::UpdaterExt;

use crate::{store, AppState};

const CHECK_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);
// 로그인 직후 자동 실행이면 네트워크가 아직 안 붙어 실패할 수 있다
const RETRY_INTERVAL: Duration = Duration::from_secs(60 * 60);

/// 실행 직후와 하루마다 GitHub 릴리즈의 latest.json을 확인한다
pub fn spawn(app: AppHandle) {
    // blocking_show는 메인 스레드에서 쓰면 안 되므로 전용 스레드
    thread::spawn(move || loop {
        let wait = match tauri::async_runtime::block_on(check_and_install(&app)) {
            Ok(()) => CHECK_INTERVAL,
            Err(e) => {
                store::log_error(&app.state::<AppState>().dir, &format!("업데이트 확인 실패: {e}"));
                RETRY_INTERVAL
            }
        };
        thread::sleep(wait);
    });
}

async fn check_and_install(app: &AppHandle) -> tauri_plugin_updater::Result<()> {
    let Some(update) = app.updater()?.check().await? else {
        return Ok(());
    };
    let is_accepted = app
        .dialog()
        .message(format!(
            "새 버전({})이 나왔습니다. 지금 업데이트할까요?\n업데이트하는 동안 초과콕이 잠깐 꺼졌다가 다시 켜집니다.",
            update.version
        ))
        .title("초과콕 업데이트")
        .buttons(MessageDialogButtons::OkCancelCustom("업데이트".into(), "나중에".into()))
        .blocking_show();
    if !is_accepted {
        return Ok(());
    }
    update.download_and_install(|_, _| {}, || {}).await?;
    app.restart()
}
