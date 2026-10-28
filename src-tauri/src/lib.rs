mod commands;
mod holidays;
mod push;
mod schedule;
mod scheduler;
mod settings;
mod store;
mod tray;
mod window;

use std::path::PathBuf;
use std::sync::Mutex;

use tauri::Manager;
use tauri_plugin_autostart::MacosLauncher;

use holidays::HolidayCache;
use schedule::RunState;
use settings::Settings;
use store::{HOLIDAYS_FILE, SETTINGS_FILE, STATE_FILE};

pub struct AppState {
    pub dir: PathBuf,
    pub settings: Mutex<Settings>,
    pub run: Mutex<RunState>,
    pub holidays: Mutex<HolidayCache>,
}

impl AppState {
    fn load(dir: PathBuf) -> Self {
        let settings: Settings = store::load(&dir, SETTINGS_FILE);
        // 손으로 고친 파일이 규칙을 어기면 기본값으로 동작
        let settings = match settings.validate() {
            Ok(()) => settings,
            Err(e) => {
                store::log_error(&dir, &format!("settings.json 값 오류, 기본값 사용: {e}"));
                Settings::default()
            }
        };
        Self {
            settings: Mutex::new(settings),
            run: Mutex::new(store::load(&dir, STATE_FILE)),
            holidays: Mutex::new(store::load(&dir, HOLIDAYS_FILE)),
            dir,
        }
    }
}

pub fn run() {
    tauri::Builder::default()
        // 반드시 첫 플러그인. 두 번째 실행 시도는 설정 창을 띄우는 것으로 대신한다.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| tray::open_settings(app)))
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, None))
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::save_settings,
            commands::mute_today,
            commands::unmute_today,
            commands::get_status,
            commands::preview_popup,
            commands::close_popup,
            commands::popup_message,
            commands::test_push,
        ])
        .setup(|app| {
            // 개발 환경(macOS)에서 Dock 아이콘 숨김 — 트레이 전용
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            let dir = app.path().app_data_dir()?;
            // 설정을 한 번도 저장하지 않았으면 첫 실행으로 보고 설정 창을 연다
            let is_first_run = !dir.join(SETTINGS_FILE).exists();
            app.manage(AppState::load(dir));
            tray::build(app)?;
            if is_first_run {
                if let Err(e) = window::open_settings(app.handle()) {
                    store::log_error(&app.state::<AppState>().dir, &format!("설정 창 열기 실패: {e}"));
                }
            }
            scheduler::spawn(app.handle().clone());
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("overpoke 초기화 실패")
        .run(|_app, event| {
            // 창을 모두 닫아도 트레이로 상주. 트레이 "종료"(app.exit → code: Some)만 실제 종료
            if let tauri::RunEvent::ExitRequested { code: None, api, .. } = event {
                api.prevent_exit();
            }
        });
}
