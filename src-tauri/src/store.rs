use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::Path;

use serde::{de::DeserializeOwned, Serialize};

pub const SETTINGS_FILE: &str = "settings.json";
pub const STATE_FILE: &str = "state.json";
pub const HOLIDAYS_FILE: &str = "holidays.json";
const LOG_FILE: &str = "overpoke.log";

/// 파일이 없거나 깨졌으면 기본값. 깨진 경우는 로그에 남긴다.
pub fn load<T: DeserializeOwned + Default>(dir: &Path, name: &str) -> T {
    match fs::read_to_string(dir.join(name)) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_else(|e| {
            log_error(dir, &format!("{name} 파싱 실패, 기본값 사용: {e}"));
            T::default()
        }),
        Err(e) if e.kind() == ErrorKind::NotFound => T::default(),
        Err(e) => {
            log_error(dir, &format!("{name} 읽기 실패, 기본값 사용: {e}"));
            T::default()
        }
    }
}

/// 임시 파일에 쓴 뒤 rename — 쓰는 도중 전원이 꺼져도 기존 파일은 온전하다.
pub fn save<T: Serialize>(dir: &Path, name: &str, value: &T) -> Result<(), String> {
    let write = || -> std::io::Result<()> {
        fs::create_dir_all(dir)?;
        let tmp = dir.join(format!("{name}.tmp"));
        fs::write(&tmp, serde_json::to_vec_pretty(value)?)?;
        fs::rename(&tmp, dir.join(name))
    };
    write().map_err(|e| format!("{name} 저장 실패: {e}"))
}

/// 릴리스 빌드는 콘솔이 없어서 파일에도 남긴다. 로그 쓰기 실패는 보고할 곳이 없으므로 무시.
pub fn log_error(dir: &Path, msg: &str) {
    eprintln!("{msg}");
    let line = format!("{} {msg}\n", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"));
    let _ = fs::create_dir_all(dir);
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(dir.join(LOG_FILE)) {
        let _ = file.write_all(line.as_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("overpoke-test-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn missing_file_returns_default() {
        let dir = temp_dir("missing");
        let value: BTreeMap<String, u32> = load(&dir, "none.json");
        assert!(value.is_empty());
    }

    #[test]
    fn save_then_load_roundtrips() {
        let dir = temp_dir("roundtrip");
        let value = BTreeMap::from([("a".to_string(), 1u32)]);
        save(&dir, "v.json", &value).unwrap();
        let loaded: BTreeMap<String, u32> = load(&dir, "v.json");
        assert_eq!(loaded, value);
    }

    #[test]
    fn corrupt_file_returns_default_and_logs() {
        let dir = temp_dir("corrupt");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("v.json"), "{not json").unwrap();
        let loaded: BTreeMap<String, u32> = load(&dir, "v.json");
        assert!(loaded.is_empty());
        assert!(fs::read_to_string(dir.join(LOG_FILE)).unwrap().contains("v.json 파싱 실패"));
    }
}
