//! Настройки приложения (хранятся в JSON в папке приложения).

use serde::{Deserialize, Serialize};

use crate::strategy::Strategy;

/// Своя стратегия из режима разработчика.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct CustomStrategy {
    pub strategy: Strategy,
    /// Строка zapret, из которой стратегия получена (для показа и экспорта).
    #[serde(default)]
    pub zapret: String,
    /// Участвует в пресетах и автоподборе.
    #[serde(default = "yes")]
    pub enabled: bool,
    /// Последняя проверка: «YouTube 6/6».
    #[serde(default)]
    pub result: Option<String>,
}

fn yes() -> bool {
    true
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Settings {
    pub first_run_done: bool,
    /// "system" | "ru" | "en"
    pub language: String,
    /// "dark" | "light" | "black"
    pub theme: String,
    /// Своя картинка на фоне окна (data URL, уменьшенная интерфейсом).
    pub bg_image: String,
    /// Затемнение картинки, 0–90 %.
    pub bg_dim: u8,
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
    pub dev_mode: bool,
    pub custom_strategies: Vec<CustomStrategy>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            first_run_done: false,
            language: "system".into(),
            theme: "dark".into(),
            bg_image: String::new(),
            bg_dim: 60,
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
            custom_domains: vec![],
            excluded_domains: vec![],
            autostart: false,
            start_minimized: false,
            minimize_to_tray: true,
            enable_on_launch: false,
            check_updates: true,
            update_base: true,
            notifications: true,
            dev_mode: false,
            custom_strategies: vec![],
        }
    }
}

use std::path::{Path, PathBuf};

/// Рядом с settings.json: копия прошлой удачной записи и картинка фона.
fn sibling(path: &Path, ext: &str) -> PathBuf {
    let mut p = path.as_os_str().to_owned();
    p.push(ext);
    PathBuf::from(p)
}

impl Settings {
    /// Битый файл (выключили свет посреди записи) — берём копию .bak, а не сбрасываем всё.
    pub fn load(path: &Path) -> Settings {
        let read = |p: &Path| std::fs::read_to_string(p).ok().and_then(|s| serde_json::from_str::<Settings>(&s).ok());
        let mut s = read(path).or_else(|| read(&sibling(path, ".bak"))).unwrap_or_default();
        // Картинка фона лежит отдельно (старые версии хранили её прямо в JSON — тогда оставляем ту).
        if let Ok(img) = std::fs::read_to_string(sibling(path, ".bg")) {
            s.bg_image = img;
        }
        s
    }

    /// Запись через временный файл: settings.json всегда либо старый, либо новый целиком.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let bg = sibling(path, ".bg");
        if self.bg_image.is_empty() {
            let _ = std::fs::remove_file(&bg);
        } else if std::fs::read_to_string(&bg).ok().as_deref() != Some(self.bg_image.as_str()) {
            let tmp = sibling(path, ".bg.tmp");
            std::fs::write(&tmp, &self.bg_image)?;
            std::fs::rename(&tmp, &bg)?;
        }
        let json = serde_json::to_string_pretty(&Settings { bg_image: String::new(), ..self.clone() }).expect("settings serialize");
        let tmp = sibling(path, ".tmp");
        std::fs::write(&tmp, json)?;
        if path.exists() {
            let _ = std::fs::copy(path, sibling(path, ".bak"));
        }
        std::fs::rename(&tmp, path)
    }

    /// Совпадает ли всё, что влияет на перехват. Тема, язык, фон и т.п. — не повод перезапускать движок.
    pub fn same_bypass(&self, o: &Settings) -> bool {
        self.enabled_presets == o.enabled_presets
            && self.strategy_override == o.strategy_override
            && self.random_fakes == o.random_fakes
            && self.block_quic == o.block_quic
            && self.game_filter == o.game_filter
            && self.game_ips == o.game_ips
            && self.custom_domains == o.custom_domains
            && self.excluded_domains == o.excluded_domains
            && self.custom_strategies == o.custom_strategies
            && self.dev_mode == o.dev_mode
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("proryv-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn save_load_roundtrip_keeps_bg_outside_json() {
        let d = tmp_dir("rt");
        let p = d.join("settings.json");
        let s = Settings { bg_image: "data:image/jpeg;base64,AAAA".into(), theme: "black".into(), ..Default::default() };
        s.save(&p).unwrap();
        assert!(!std::fs::read_to_string(&p).unwrap().contains("base64"));
        assert_eq!(Settings::load(&p), s);
        Settings { bg_image: String::new(), ..s }.save(&p).unwrap();
        assert!(!sibling(&p, ".bg").exists());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn corrupt_file_falls_back_to_backup() {
        let d = tmp_dir("bak");
        let p = d.join("settings.json");
        let s = Settings { first_run_done: true, language: "en".into(), ..Default::default() };
        s.save(&p).unwrap();
        s.save(&p).unwrap(); // вторая запись создаёт .bak
        std::fs::write(&p, "{\"first_run_d").unwrap();
        assert_eq!(Settings::load(&p), s);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn old_json_with_inline_bg_and_doh_still_loads() {
        let d = tmp_dir("old");
        let p = d.join("settings.json");
        std::fs::write(&p, r#"{"first_run_done":true,"bg_image":"data:x","doh":true,"doh_server":"x"}"#).unwrap();
        let s = Settings::load(&p);
        assert!(s.first_run_done);
        assert_eq!(s.bg_image, "data:x");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn cosmetic_changes_do_not_affect_bypass() {
        let a = Settings::default();
        let b = Settings { theme: "light".into(), bg_dim: 10, accent: "#ffffff".into(), language: "en".into(), ..a.clone() };
        assert!(a.same_bypass(&b));
        let c = Settings { block_quic: !a.block_quic, ..a.clone() };
        assert!(!a.same_bypass(&c));
    }
}
