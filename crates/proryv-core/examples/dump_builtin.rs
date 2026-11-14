//! Выгружает встроенные пресеты и стратегии в ui/mock-data.js,
//! чтобы интерфейс можно было смотреть в обычном браузере без Tauri.
//!   cargo run -p proryv-core --example dump_builtin > ui/mock-data.js

use proryv_core::{config::Settings, presets};

fn main() {
    let data = serde_json::json!({
        "version": proryv_core::APP_VERSION,
        "presets": presets::builtin_presets(),
        "strategies": presets::builtin_strategies(),
        "defaults": Settings::default(),
    });
    println!("// Сгенерировано dump_builtin.rs — не редактировать вручную.");
    println!("window.MOCK_DATA = {};", serde_json::to_string_pretty(&data).unwrap());
}
