//! Ядро Proryv: всё, что не зависит от ОС.
//!
//! - [`tls`] / [`http`] — поиск SNI и Host в первом пакете соединения;
//! - [`strategy`] — описание методов обхода и планирование сегментов;
//! - [`presets`] — встроенные пресеты (YouTube, Discord, Roblox…);
//! - [`engine`] — состояние движка и статистика;
//! - [`platform`] — интерфейс перехвата пакетов (WinDivert / NFQUEUE реализуются отдельно);
//! - [`config`] — настройки приложения.

pub mod autotune;
pub mod base;
pub mod config;
pub mod engine;
pub mod http;
pub mod packet;
pub mod platform;
pub mod presets;
pub mod quic;
pub mod strategy;
pub mod tls;
pub mod zapret;

pub const APP_NAME: &str = "Proryv";
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
