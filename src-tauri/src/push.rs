use std::time::Duration;

use crate::settings::{NtfyConfig, Settings, TelegramConfig};

const PUSH_TIMEOUT: Duration = Duration::from_secs(10);

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder().timeout_global(Some(PUSH_TIMEOUT)).build().into()
}

/// 켜진 채널 모두로 보내고, 실패한 채널의 에러 메시지만 모아 돌려준다.
pub fn send_all(settings: &Settings, msg: &str) -> Vec<String> {
    let mut errors = Vec::new();
    if settings.ntfy.enabled {
        errors.extend(ntfy(&settings.ntfy, msg).err());
    }
    if settings.telegram.enabled {
        errors.extend(telegram(&settings.telegram, msg).err());
    }
    errors
}

/// 헤더는 ASCII만 안전해서 한글은 본문에만 넣는다.
fn ntfy(config: &NtfyConfig, msg: &str) -> Result<(), String> {
    let url = format!("{}/{}", config.server.trim_end_matches('/'), config.topic.trim());
    let mut req = agent().post(&url).header("Title", "overpoke").header("Priority", "high");
    if !config.token.is_empty() {
        req = req.header("Authorization", format!("Bearer {}", config.token));
    }
    req.send(msg).map(|_| ()).map_err(|e| format!("ntfy 전송 실패: {e}"))
}

fn telegram(config: &TelegramConfig, msg: &str) -> Result<(), String> {
    let url = format!("https://api.telegram.org/bot{}/sendMessage", config.bot_token.trim());
    agent()
        .post(&url)
        .send_form([("chat_id", config.chat_id.trim()), ("text", msg)])
        .map(|_| ())
        // 에러 문자열에 URL이 섞여 토큰이 로그에 남지 않게 가린다
        .map_err(|e| format!("텔레그램 전송 실패: {e}").replace(config.bot_token.trim(), "***"))
}
