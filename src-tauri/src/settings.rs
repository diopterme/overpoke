use std::collections::BTreeSet;

use chrono::{NaiveDate, NaiveTime};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PopupPosition {
    #[default]
    Center,
    BottomRight,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct NtfyConfig {
    pub enabled: bool,
    pub server: String,
    pub topic: String,
    /// 비어 있으면 인증 없이 보낸다
    pub token: String,
}

impl Default for NtfyConfig {
    fn default() -> Self {
        Self { enabled: false, server: "https://ntfy.sh".into(), topic: String::new(), token: String::new() }
    }
}

#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct TelegramConfig {
    pub enabled: bool,
    pub bot_token: String,
    pub chat_id: String,
}

/// 파일에 없는 필드는 기본값으로 채운다 (#[serde(default)]) — 버전이 올라가 필드가 늘어도 기존 파일이 읽힌다.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub work_start: NaiveTime,
    pub work_end: NaiveTime,
    /// 퇴근 시각 후 첫 알림까지의 유예(분). 18:00 + 10 = 18:10
    pub end_grace_min: u32,
    pub popup_position: PopupPosition,
    pub ntfy: NtfyConfig,
    pub telegram: TelegramConfig,
    /// 구글 캘린더에 아직 반영 안 된 임시공휴일 등을 수동 추가
    pub extra_holidays: BTreeSet<NaiveDate>,
}

const MAX_GRACE_MIN: u32 = 59;

impl Default for Settings {
    fn default() -> Self {
        Self {
            work_start: NaiveTime::from_hms_opt(9, 0, 0).unwrap(),
            work_end: NaiveTime::from_hms_opt(18, 0, 0).unwrap(),
            end_grace_min: 10,
            popup_position: PopupPosition::default(),
            ntfy: NtfyConfig::default(),
            telegram: TelegramConfig::default(),
            extra_holidays: BTreeSet::new(),
        }
    }
}

impl Settings {
    pub fn validate(&self) -> Result<(), String> {
        if self.work_start >= self.work_end {
            return Err("근무 시작 시각은 종료 시각보다 빨라야 합니다".into());
        }
        if self.end_grace_min > MAX_GRACE_MIN {
            return Err(format!("퇴근 후 유예는 0~{MAX_GRACE_MIN}분이어야 합니다"));
        }
        if self.ntfy.enabled {
            if !self.ntfy.server.starts_with("http://") && !self.ntfy.server.starts_with("https://") {
                return Err("ntfy 서버 주소는 http:// 또는 https:// 로 시작해야 합니다".into());
            }
            if self.ntfy.topic.trim().is_empty() {
                return Err("ntfy topic을 입력하세요".into());
            }
        }
        if self.telegram.enabled && (self.telegram.bot_token.trim().is_empty() || self.telegram.chat_id.trim().is_empty()) {
            return Err("텔레그램 봇 토큰과 chat_id를 입력하세요".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hm(h: u32, m: u32) -> NaiveTime {
        NaiveTime::from_hms_opt(h, m, 0).unwrap()
    }

    #[test]
    fn default_is_valid() {
        assert_eq!(Settings::default().validate(), Ok(()));
    }

    #[test]
    fn rejects_start_not_before_end() {
        let s = Settings { work_start: hm(18, 0), work_end: hm(9, 0), ..Settings::default() };
        assert!(s.validate().is_err());
    }

    #[test]
    fn rejects_grace_of_an_hour_or_more() {
        let s = Settings { end_grace_min: 60, ..Settings::default() };
        assert!(s.validate().is_err());
    }

    #[test]
    fn rejects_enabled_ntfy_without_topic() {
        let s = Settings { ntfy: NtfyConfig { enabled: true, ..NtfyConfig::default() }, ..Settings::default() };
        assert!(s.validate().is_err());
    }

    #[test]
    fn rejects_enabled_telegram_without_chat_id() {
        let telegram = TelegramConfig { enabled: true, bot_token: "t".into(), chat_id: String::new() };
        let s = Settings { telegram, ..Settings::default() };
        assert!(s.validate().is_err());
    }

    #[test]
    fn missing_fields_fall_back_to_defaults() {
        let s: Settings = serde_json::from_str(r#"{"work_start":"08:30","popup_position":"bottom_right"}"#).unwrap();
        assert_eq!(s.work_start, hm(8, 30));
        assert_eq!(s.work_end, hm(18, 0));
        assert_eq!(s.popup_position, PopupPosition::BottomRight);
        assert_eq!(s.ntfy.server, "https://ntfy.sh");
    }
}
