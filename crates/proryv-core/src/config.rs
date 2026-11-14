//! Настройки приложения (хранятся в JSON в папке приложения).

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Settings {
    pub first_run_done: bool,
    /// "system" | "ru" | "en"
    pub language: String,
    /// "system" | "dark" | "light"
    pub theme: String,
    /// "none" | "mica" | "acrylic"
    pub backdrop: String,
    /// Акцентный цвет, #rrggbb
    pub accent: String,
    pub animations: bool,

    pub enabled_presets: Vec<String>,
    /// Ручной выбор стратегии для пресета: preset_id -> strategy_id. Пусто = автоподбор.
    pub strategy_override: Vec<(String, String)>,
    pub auto_heal: bool,
    pub random_fakes: bool,
    /// Блокировать QUIC (UDP 443), чтобы браузер сразу шёл по TCP, где работает обход.
    pub block_quic: bool,
    /// Telegram: локальный MTProto-прокси через WebSocket.
    pub telegram: bool,
    pub telegram_port: u16,
    /// Секрет прокси (32 hex); создаётся при первом включении.
    pub telegram_secret: String,
    /// Game Filter, как в zapret: "off" | "tcp" | "udp" | "all".
    pub game_filter: String,
    /// Диапазоны IP игровых серверов (CIDR), к которым применяется Game Filter.
    pub game_ips: Vec<String>,
    pub doh: bool,
    pub doh_server: String,

    pub custom_domains: Vec<String>,
    pub excluded_domains: Vec<String>,

    pub autostart: bool,
    pub start_minimized: bool,
    pub minimize_to_tray: bool,
    pub enable_on_launch: bool,
    pub check_updates: bool,
    /// Скачивать базу доменов, стратегий и провайдеров с GitHub при запуске.
    pub update_base: bool,
    pub notifications: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            first_run_done: false,
            language: "system".into(),
            theme: "dark".into(),
            backdrop: "acrylic".into(),
            accent: "#7cc4e4".into(),
            animations: true,
            enabled_presets: vec!["youtube".into(), "discord".into(), "general".into()],
            strategy_override: vec![],
            auto_heal: true,
            random_fakes: true,
            block_quic: true,
            telegram: false,
            telegram_port: 1443,
            telegram_secret: String::new(),
            game_filter: "off".into(),
            game_ips: vec!["128.116.0.0/17".into()], // Roblox
            doh: true,
            doh_server: "https://cloudflare-dns.com/dns-query".into(),
            custom_domains: vec![],
            excluded_domains: vec![],
            autostart: false,
            start_minimized: false,
            minimize_to_tray: true,
            enable_on_launch: false,
            check_updates: true,
            update_base: true,
            notifications: true,
        }
    }
}

impl Settings {
    pub fn load(path: &std::path::Path) -> Settings {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &std::path::Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(path, serde_json::to_string_pretty(self).expect("settings serialize"))
    }
}
