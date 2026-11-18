//! Оболочка Tauri: окно, трей, автозапуск, команды для интерфейса.

use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
};

use proryv_core::{
    autotune::{self, Diagnosis, Outcome, Progress},
    base,
    config::Settings,
    engine::{Engine, EngineState, RunConfig, Status, Trace},
    platform::{self, KNOWN_CONFLICTS},
    presets::{self, Preset},
    strategy::{self, Plan, Strategy},
    tls,
};
use serde::Serialize;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    window::{Effect, EffectsBuilder},
    AppHandle, Emitter, Manager, State, WindowEvent,
};

struct AppState {
    engine: Mutex<Engine>,
    settings: Mutex<Settings>,
    settings_path: PathBuf,
    autotune_running: AtomicBool,
    tg: Mutex<Option<proryv_tg::TgProxy>>,
    tg_error: Mutex<Option<String>>,
    /// Журнал движка для вкладки «Разработчик» (пишется, только когда режим включён).
    trace: Trace,
}

// ---------------------------------------------------------------- Telegram

fn sync_tg(state: &AppState) {
    let mut s = state.settings.lock().unwrap();
    let mut tg = state.tg.lock().unwrap();
    if !s.telegram {
        if let Some(mut p) = tg.take() {
            p.stop();
        }
        *state.tg_error.lock().unwrap() = None;
        return;
    }
    if s.telegram_secret.len() != 32 {
        s.telegram_secret = proryv_tg::gen_secret();
        let _ = s.save(&state.settings_path);
    }
    if tg.as_ref().is_some_and(|p| p.port() != s.telegram_port) {
        if let Some(mut p) = tg.take() {
            p.stop();
        }
    }
    if tg.is_none() {
        match proryv_tg::TgProxy::start(s.telegram_port, &s.telegram_secret) {
            Ok(p) => {
                *tg = Some(p);
                *state.tg_error.lock().unwrap() = None;
            }
            Err(e) => *state.tg_error.lock().unwrap() = Some(e),
        }
    }
}

#[derive(Serialize)]
struct TgInfo {
    enabled: bool,
    running: bool,
    link: String,
    error: Option<String>,
    stats: Option<proryv_tg::TgStatsView>,
}

#[tauri::command(async)]
fn tg_info(state: State<AppState>) -> TgInfo {
    let s = state.settings.lock().unwrap();
    let tg = state.tg.lock().unwrap();
    TgInfo {
        enabled: s.telegram,
        running: tg.is_some(),
        link: proryv_tg::link(s.telegram_port, &s.telegram_secret),
        error: state.tg_error.lock().unwrap().clone(),
        stats: tg.as_ref().map(|p| p.stats()),
    }
}

/// Путь к Telegram.exe: из обработчика tg:// в реестре или стандартная папка.
fn telegram_exe() -> Option<PathBuf> {
    for key in [r"HKCU\Software\Classes\tg\shell\open\command", r"HKCR\tg\shell\open\command"] {
        if let Ok(out) = hidden_cmd("reg").args(["query", key, "/ve"]).output() {
            let text = String::from_utf8_lossy(&out.stdout);
            if let Some(start) = text.find('"') {
                if let Some(len) = text[start + 1..].find('"') {
                    let p = PathBuf::from(&text[start + 1..start + 1 + len]);
                    if p.exists() {
                        return Some(p);
                    }
                }
            }
        }
    }
    let p = PathBuf::from(std::env::var("APPDATA").ok()?).join("Telegram Desktop").join("Telegram.exe");
    p.exists().then_some(p)
}

fn telegram_running() -> bool {
    hidden_cmd("tasklist")
        .args(["/FI", "IMAGENAME eq Telegram.exe", "/NH"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_lowercase().contains("telegram.exe"))
        .unwrap_or(false)
}

/// Добавить прокси в Telegram Desktop. Ссылку передаём прямо в Telegram.exe
/// (как это делает сам обработчик tg://): у многих протокол tg:// в Windows
/// зарегистрирован не полностью, и открытие ссылки молча не срабатывает.
#[tauri::command(async)]
fn tg_open(state: State<AppState>) -> Result<(), String> {
    let link = {
        let s = state.settings.lock().unwrap();
        proryv_tg::link(s.telegram_port, &s.telegram_secret)
    };
    let exe = telegram_exe().ok_or("Telegram Desktop не найден. Скопируйте ссылку и откройте её в Telegram вручную.")?;
    if !telegram_running() {
        // Запускаем через explorer — тогда Telegram стартует от имени пользователя, а не администратора.
        let _ = std::process::Command::new("explorer.exe").arg(&exe).spawn();
        for _ in 0..20 {
            std::thread::sleep(std::time::Duration::from_millis(500));
            if telegram_running() {
                break;
            }
        }
        std::thread::sleep(std::time::Duration::from_secs(2));
    }
    hidden_cmd(&exe.to_string_lossy()).arg("--").arg(link).spawn().map_err(|e| e.to_string())?;
    Ok(())
}

/// Проверить, что через прокси отвечают DC2 и DC4.
#[tauri::command]
async fn tg_test(app: AppHandle) -> Vec<(i16, Result<u128, String>)> {
    let (port, secret) = {
        let st = app.state::<AppState>();
        let s = st.settings.lock().unwrap();
        (s.telegram_port, s.telegram_secret.clone())
    };
    tauri::async_runtime::spawn_blocking(move || {
        [2i16, 4, 1].into_iter().map(|dc| (dc, proryv_tg::self_test(port, &secret, dc))).collect()
    })
    .await
    .unwrap_or_default()
}

#[derive(Serialize)]
struct Bootstrap {
    version: &'static str,
    presets: Vec<Preset>,
    strategies: Vec<Strategy>,
    settings: Settings,
}

#[derive(Serialize)]
struct Conflict {
    process: String,
    name: String,
}

// ---------------------------------------------------------------- язык и уведомления

/// Английский ли интерфейс: по настройке, а для «Как в системе» — по языку Windows.
fn is_en(state: &AppState) -> bool {
    match state.settings.lock().unwrap().language.as_str() {
        "en" => true,
        "ru" => false,
        _ => !system_is_russian(),
    }
}

#[cfg(windows)]
fn system_is_russian() -> bool {
    #[link(name = "kernel32")]
    extern "system" {
        fn GetUserDefaultUILanguage() -> u16;
    }
    // Основной язык: русский, украинский, белорусский, казахский.
    matches!(unsafe { GetUserDefaultUILanguage() } & 0x3ff, 0x19 | 0x22 | 0x23 | 0x3f)
}
#[cfg(not(windows))]
fn system_is_russian() -> bool {
    true
}

fn notify(app: &AppHandle, title: (&str, &str), body: (String, String)) {
    use tauri_plugin_notification::NotificationExt;
    let state = app.state::<AppState>();
    if !state.settings.lock().unwrap().notifications {
        return;
    }
    let en = is_en(&state);
    let _ = app
        .notification()
        .builder()
        .title(if en { title.1 } else { title.0 })
        .body(if en { body.1 } else { body.0 })
        .show();
}

fn preset_title(id: &str, en: bool) -> String {
    let en_name = match id {
        "general" => Some("Other blocked sites"),
        "social" => Some("Social networks"),
        "universal" => Some("Custom domains"),
        _ => None,
    };
    match (en, en_name) {
        (true, Some(n)) => n.to_string(),
        _ => presets::builtin_presets().into_iter().find(|p| p.id == id).map(|p| p.name).unwrap_or_else(|| id.to_string()),
    }
}

/// Включить обход не из окна (трей, запуск) — если не вышло, сказать уведомлением.
fn start_engine_notify(app: &AppHandle) -> Status {
    let st = start_engine(&app.state::<AppState>());
    if st.state == EngineState::Error {
        let msg = st.message.clone().unwrap_or_default();
        notify(app, ("Обход не включился", "Bypass failed to start"), (msg.clone(), msg));
    }
    st
}

// ---------------------------------------------------------------- движок

fn start_engine(state: &AppState) -> Status {
    let mut engine = state.engine.lock().unwrap();
    let settings = state.settings.lock().unwrap();
    engine.trace = settings.dev_mode.then(|| state.trace.clone());
    let cfg = RunConfig::from_settings(&settings);
    drop(settings);
    match platform::open_default(&cfg.filter()) {
        Ok(io) => engine.start(cfg, io),
        Err(e) => engine.fail(e.to_string()),
    }
    engine.status()
}

fn stop_engine(state: &AppState) -> Status {
    let mut engine = state.engine.lock().unwrap();
    engine.stop();
    engine.status()
}

fn is_running(state: &AppState) -> bool {
    state.engine.lock().unwrap().status().state == EngineState::Running
}

// ---------------------------------------------------------------- команды

#[tauri::command]
fn get_bootstrap(state: State<AppState>) -> Bootstrap {
    Bootstrap {
        version: proryv_core::APP_VERSION,
        presets: base::presets(),
        strategies: base::strategies(),
        settings: state.settings.lock().unwrap().clone(),
    }
}

#[tauri::command(async)]
fn save_settings(state: State<AppState>, mut settings: Settings) -> Result<(), String> {
    let (old_autostart, old_secret) = {
        let s = state.settings.lock().unwrap();
        (s.autostart, s.telegram_secret.clone())
    };
    // Секрет Telegram создаётся на стороне приложения — интерфейс мог прислать старый пустой.
    if settings.telegram_secret.len() != 32 {
        settings.telegram_secret = old_secret;
    }
    settings.save(&state.settings_path).map_err(|e| e.to_string())?;
    let autostart = settings.autostart;
    base::set_custom(&settings.custom_strategies);
    *state.settings.lock().unwrap() = settings;
    if autostart != old_autostart {
        set_autostart(autostart)?;
    }
    // Обход включён — перезапускаем движок, чтобы новые пресеты и режимы применились сразу.
    if is_running(&state) {
        start_engine(&state);
    }
    sync_tg(&state);
    Ok(())
}

#[tauri::command]
fn default_settings() -> Settings {
    Settings::default()
}

#[tauri::command(async)]
fn get_status(state: State<AppState>) -> Status {
    state.engine.lock().unwrap().status()
}

#[tauri::command(async)]
fn set_enabled(state: State<AppState>, enabled: bool) -> Status {
    if enabled {
        start_engine(&state)
    } else {
        stop_engine(&state)
    }
}

#[tauri::command]
fn preview_plan(host: String, strategy_id: String) -> Result<Plan, String> {
    let strategy = base::strategies()
        .into_iter()
        .find(|s| s.id == strategy_id)
        .ok_or("неизвестная стратегия")?;
    let host = if host.trim().is_empty() { "www.youtube.com".to_string() } else { host.trim().to_string() };
    let seed = host.bytes().fold(0x9e37_79b9_u64, |a, b| a.rotate_left(5) ^ b as u64);
    Ok(strategy::plan(&tls::build_client_hello(&host), &strategy, seed))
}

/// Ищем запущенные программы, которые будут мешать (другие обходы и VPN).
#[tauri::command(async)]
fn check_conflicts() -> Vec<Conflict> {
    let mut cmd = std::process::Command::new("tasklist");
    cmd.args(["/FO", "CSV", "/NH"]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    let Ok(out) = cmd.output() else { return vec![] };
    let text = String::from_utf8_lossy(&out.stdout).to_lowercase();
    let mut found: Vec<Conflict> = KNOWN_CONFLICTS
        .iter()
        .filter(|(exe, _)| text.contains(&format!("\"{exe}\"")))
        .map(|(exe, name)| Conflict { process: exe.to_string(), name: name.to_string() })
        .collect();
    for (svc, name) in platform::CONFLICT_SERVICES {
        if service_running(svc) && !found.iter().any(|c| c.name == *name) {
            found.push(Conflict { process: svc.to_string(), name: name.to_string() });
        }
    }
    found
}

/// `sc query <name>` → служба установлена и запущена.
fn service_running(name: &str) -> bool {
    let mut cmd = std::process::Command::new("sc");
    cmd.args(["query", name]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    cmd.output().map(|o| String::from_utf8_lossy(&o.stdout).contains("RUNNING")).unwrap_or(false)
}

#[tauri::command(async)]
fn tunnel_interfaces() -> Vec<String> {
    #[cfg(windows)]
    return platform::windivert::tunnel_interfaces()
        .into_iter()
        .map(|(_, d)| d)
        .filter(|d| !d.to_lowercase().contains("wan miniport"))
        .collect();
    #[cfg(not(windows))]
    vec![]
}

#[tauri::command]
async fn diagnose(host: String) -> Diagnosis {
    tauri::async_runtime::spawn_blocking(move || autotune::diagnose(&host)).await.expect("diagnose")
}

#[derive(Serialize, Clone)]
struct TuneDone {
    /// "manual" — кнопка, "heal" — самолечение.
    source: &'static str,
    result: Result<Outcome, String>,
}

/// Подбор в текущем потоке: остановить движок, перебрать, сохранить лучшую, вернуть движок.
fn run_tune(app: &AppHandle, preset: &str, source: &'static str) {
    let state = app.state::<AppState>();
    let was_running = is_running(&state);
    state.engine.lock().unwrap().pause();
    let base = state.settings.lock().unwrap().clone();
    let res: Result<Outcome, String> = autotune::run(preset, &base, |p: &Progress| {
        if source == "manual" {
            let _ = app.emit("autotune", p);
        }
    });
    if let Ok(out) = &res {
        if let Some(best) = &out.best {
            let mut s = state.settings.lock().unwrap();
            s.strategy_override.retain(|(p, _)| p != preset);
            s.strategy_override.push((preset.to_string(), best.clone()));
            let _ = s.save(&state.settings_path);
        }
    }
    if was_running {
        start_engine(&state);
    }
    state.autotune_running.store(false, Ordering::SeqCst);
    if source == "heal" {
        let en = is_en(&state);
        let name = preset_title(preset, en);
        match res.as_ref().ok().and_then(|o| o.best.as_ref().map(|b| (b, &o.best_name))) {
            Some((id, ru_name)) => notify(
                app,
                ("Самолечение", "Self-healing"),
                (
                    format!("{name} перестал открываться — переключили на «{}».", ru_name.as_deref().unwrap_or(id)),
                    format!("{name} stopped opening — switched to strategy {id}."),
                ),
            ),
            None => notify(
                app,
                ("Самолечение", "Self-healing"),
                (
                    format!("{name} перестал открываться, а новая стратегия не нашлась. Попробуйте автоподбор или «Диагностику»."),
                    format!("{name} stopped opening and no new strategy was found. Try auto-tune or Diagnostics."),
                ),
            ),
        }
    }
    let _ = app.emit("autotune-done", TuneDone { source, result: res });
}

/// Запустить автоподбор в фоне. Прогресс — события "autotune", итог — "autotune-done".
#[tauri::command]
fn autotune_start(app: AppHandle, preset: String) -> Result<(), String> {
    let state = app.state::<AppState>();
    if state.autotune_running.swap(true, Ordering::SeqCst) {
        return Err("Подбор уже идёт".into());
    }
    std::thread::spawn(move || run_tune(&app, &preset, "manual"));
    Ok(())
}

/// Самолечение: раз в 20 с смотрим долю повторных ClientHello по пресетам.
/// Если за окно много повторов (соединения не проходят) — тихо переподбираем стратегию.
/// Плюс сторож: раз в 5 минут сам открывает адреса проверки включённых пресетов —
/// так поломка находится, даже если сайтом сейчас никто не пользуется.
fn spawn_healer(app: AppHandle) {
    std::thread::spawn(move || {
        use std::{collections::HashMap, time::{Duration, Instant}};
        let mut prev: HashMap<String, (u64, u64)> = HashMap::new();
        let mut last_heal: HashMap<String, Instant> = HashMap::new();
        let mut last_watch = Instant::now();
        loop {
            std::thread::sleep(Duration::from_secs(20));
            let state = app.state::<AppState>();
            let settings = state.settings.lock().unwrap().clone();
            if !settings.auto_heal || state.autotune_running.load(Ordering::SeqCst) || !is_running(&state) {
                prev.clear();
                continue;
            }
            let cooled = |p: &str| last_heal.get(p).is_none_or(|t| t.elapsed() > Duration::from_secs(15 * 60));
            let now_h = state.engine.lock().unwrap().health();
            let mut target = None;
            for (preset, (d, r)) in &now_h {
                let (pd, pr) = prev.get(preset).copied().unwrap_or((0, 0));
                let (dd, dr) = (d.saturating_sub(pd), r.saturating_sub(pr));
                // ≥4 повторов и не меньше трети всех соединений за окно.
                if dr >= 4 && dr * 3 >= dd && cooled(preset) && !autotune::urls_for(preset, &settings).is_empty() {
                    target = Some(preset.clone());
                    break;
                }
            }
            prev = now_h;
            if target.is_none() && last_watch.elapsed() > Duration::from_secs(5 * 60) {
                last_watch = Instant::now();
                target = watch_presets(&settings).into_iter().find(|p| cooled(p));
            }
            if let Some(preset) = target {
                if state.autotune_running.swap(true, Ordering::SeqCst) {
                    continue;
                }
                last_heal.insert(preset.clone(), Instant::now());
                let _ = app.emit("heal-start", &preset);
                run_tune(&app, &preset, "heal");
                prev.clear();
            }
        }
    });
}

/// Пресеты, у которых сейчас открывается меньше половины адресов проверки.
fn watch_presets(settings: &Settings) -> Vec<String> {
    #[cfg(windows)]
    let iface = platform::windivert::direct_source_ip();
    #[cfg(not(windows))]
    let iface: Option<String> = None;
    settings
        .enabled_presets
        .iter()
        .filter(|p| {
            let urls = autotune::urls_for(p, settings);
            if urls.is_empty() {
                return false;
            }
            let urls: Vec<&str> = urls.iter().map(String::as_str).collect();
            let (ok, _, _) = autotune::check_all(&urls, iface.as_deref());
            ok * 2 < urls.len()
        })
        .cloned()
        .collect()
}

#[derive(Serialize)]
struct SystemInfo {
    elevated: bool,
    driver_found: bool,
}

#[tauri::command(async)]
fn system_info() -> SystemInfo {
    #[cfg(windows)]
    let driver_found = platform::windivert::find_dll().is_some();
    #[cfg(not(windows))]
    let driver_found = false;
    SystemInfo { elevated: is_elevated(), driver_found }
}

#[cfg(windows)]
fn is_elevated() -> bool {
    #[link(name = "shell32")]
    extern "system" {
        fn IsUserAnAdmin() -> i32;
    }
    unsafe { IsUserAnAdmin() != 0 }
}
#[cfg(not(windows))]
fn is_elevated() -> bool {
    false
}

#[tauri::command]
fn relaunch_as_admin(app: AppHandle) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let status = hidden_cmd("powershell")
        .args(["-NoProfile", "-WindowStyle", "Hidden", "-Command"])
        .arg(format!("Start-Process -FilePath '{}' -Verb RunAs", exe.display().to_string().replace('\'', "''")))
        .status()
        .map_err(|e| e.to_string())?;
    if !status.success() {
        return Err("Запуск от имени администратора отменён".into());
    }
    app.exit(0);
    Ok(())
}

fn hidden_cmd(program: &str) -> std::process::Command {
    #[allow(unused_mut)]
    let mut cmd = std::process::Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    cmd
}

/// Автозапуск через Планировщик задач: только так программа с правами
/// администратора стартует при входе без окна UAC.
fn set_autostart(enabled: bool) -> Result<(), String> {
    let out = if enabled {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        hidden_cmd("schtasks")
            .args(["/Create", "/F", "/TN", "Proryv", "/SC", "ONLOGON", "/RL", "HIGHEST", "/TR"])
            .arg(format!("\"{}\" --autostart", exe.display()))
            .output()
    } else {
        hidden_cmd("schtasks").args(["/Delete", "/F", "/TN", "Proryv"]).output()
    }
    .map_err(|e| e.to_string())?;
    if out.status.success() || !enabled {
        Ok(())
    } else {
        Err(format!("Не удалось настроить автозапуск: {}", String::from_utf8_lossy(&out.stderr).trim()))
    }
}

fn apply_backdrop(window: &tauri::WebviewWindow, backdrop: &str) {
    let effect = match backdrop {
        "mica" => Effect::Mica,
        "acrylic" => Effect::Acrylic,
        _ => {
            let _ = window.set_effects(None);
            return;
        }
    };
    let _ = window.set_effects(EffectsBuilder::new().effect(effect).build());
}

#[tauri::command]
fn set_backdrop(window: tauri::WebviewWindow, backdrop: String) {
    apply_backdrop(&window, &backdrop);
}

// ---------------------------------------------------------------- режим разработчика

#[tauri::command]
fn zapret_parse(text: String) -> Result<Vec<proryv_core::zapret::Parsed>, String> {
    proryv_core::zapret::parse(&text)
}

#[tauri::command]
fn preview_strategy(host: String, strategy: Strategy) -> Plan {
    let host = if host.trim().is_empty() { "www.youtube.com".to_string() } else { host.trim().to_string() };
    strategy::plan(&tls::build_client_hello(&host), &strategy, 0x9e37_79b9)
}

/// Строки журнала начиная с `since` и номер для следующего запроса.
#[tauri::command]
fn dev_log(state: State<AppState>, since: u64) -> (u64, Vec<String>) {
    state.trace.lock().unwrap().since(since)
}

/// Проверить стратегию (даже ещё не сохранённую) на адресах пресета, мимо VPN.
#[tauri::command(async)]
fn dev_check(state: State<AppState>, preset: String, strategy: Strategy) -> Result<(usize, usize), String> {
    base::check_strategy(&strategy)?;
    if state.autotune_running.swap(true, Ordering::SeqCst) {
        return Err("Подбор уже идёт".into());
    }
    let settings = state.settings.lock().unwrap().clone();
    let mut custom = settings.custom_strategies.clone();
    custom.retain(|c| c.strategy.id != strategy.id);
    custom.push(proryv_core::config::CustomStrategy { strategy: strategy.clone(), zapret: String::new(), enabled: true, result: None });
    base::set_custom(&custom);
    let was_running = is_running(&state);
    state.engine.lock().unwrap().pause();
    let res = autotune::check_one(&preset, &strategy.id, &settings);
    base::set_custom(&settings.custom_strategies);
    if was_running {
        start_engine(&state);
    }
    state.autotune_running.store(false, Ordering::SeqCst);
    res
}

// ---------------------------------------------------------------- база и провайдер

#[derive(Serialize, Clone)]
struct IspInfo {
    isp: Option<base::Isp>,
    entry: Option<base::IspEntry>,
    base_version: u32,
    /// Определение провайдера уже закончилось (удачно или нет).
    done: bool,
}

static BASE_DONE: AtomicBool = AtomicBool::new(false);

#[tauri::command]
fn isp_info() -> IspInfo {
    IspInfo {
        isp: base::isp(),
        entry: base::isp_entry(),
        base_version: base::base().map_or(0, |b| b.version),
        done: BASE_DONE.load(Ordering::SeqCst),
    }
}

/// Отчёт для помощи в Discord: версия, система, провайдер, пресеты, ошибки и проверка сайтов.
/// Без личных данных: ни IP, ни своих доменов, ни имён адаптеров.
#[tauri::command(async)]
fn diag_report(state: State<AppState>) -> String {
    let settings = state.settings.lock().unwrap().clone();
    let status = state.engine.lock().unwrap().status();
    let en = is_en(&state);
    let tr = |ru: &'static str, e: &'static str| if en { e } else { ru };
    let winver = hidden_cmd("cmd").args(["/C", "ver"]).output().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default();
    let mut r = vec![
        format!("Proryv {} · {winver}", proryv_core::APP_VERSION),
        format!("{}: {}", tr("Права администратора", "Administrator"), if is_elevated() { tr("да", "yes") } else { tr("нет", "no") }),
        match base::isp() {
            Some(i) => format!("{}: {} (AS{})", tr("Провайдер", "Provider"), i.name, i.asn),
            None => tr("Провайдер: не определён", "Provider: unknown").into(),
        },
        format!("{}: v{}", tr("База стратегий", "Strategy base"), base::base().map_or(0, |b| b.version)),
        format!("{}: {:?}{}", tr("Обход", "Bypass"), status.state, status.message.as_ref().map(|m| format!(" — {m}")).unwrap_or_default()),
        format!("{} {} · {} {} · {} {}", tr("Соединений:", "Connections:"), status.stats.connections, tr("обработано:", "processed:"), status.stats.desynced, tr("фейков:", "fakes:"), status.stats.fakes_sent),
        format!("{}: {} · QUIC: {} · Game Filter: {}", tr("Свои домены", "Custom domains"), settings.custom_domains.len(), settings.block_quic, settings.game_filter),
    ];
    let conflicts = check_conflicts();
    r.push(format!("{}: {}", tr("Конфликты", "Conflicts"), if conflicts.is_empty() { tr("нет", "none").into() } else { conflicts.iter().map(|c| c.name.clone()).collect::<Vec<_>>().join(", ") }));
    r.push(format!("{}: {}", tr("VPN-туннелей", "VPN tunnels"), tunnel_interfaces().len()));
    #[cfg(windows)]
    let iface = platform::windivert::direct_source_ip();
    #[cfg(not(windows))]
    let iface: Option<String> = None;
    r.push(format!("{}: {}", tr("Проверка напрямую мимо VPN", "Direct check bypassing VPN"), if iface.is_some() { tr("да", "yes") } else { tr("нет", "no") }));
    let all = base::strategies();
    for p in &settings.enabled_presets {
        let chosen = settings.strategy_override.iter().find(|(id, _)| id == p).map(|(_, s)| s.clone());
        let sid = chosen.clone().or_else(|| base::presets().into_iter().find(|x| &x.id == p).and_then(|x| x.strategies.first().cloned())).unwrap_or_default();
        let name = all.iter().find(|s| s.id == sid).map_or(sid.clone(), |s| s.name.clone());
        let urls = if p == "universal" { vec![] } else { autotune::urls_for(p, &settings) };
        let check = if urls.is_empty() {
            String::new()
        } else {
            let u: Vec<&str> = urls.iter().map(String::as_str).collect();
            let (ok, _, checks) = autotune::check_all(&u, iface.as_deref());
            let fails: Vec<String> = checks.iter().filter(|c| !c.ok).map(|c| format!("{} (curl {})", c.url.split('/').nth(2).unwrap_or(""), c.curl_error)).collect();
            format!(" · {} {ok}/{}{}", tr("открылось", "opened"), urls.len(), if fails.is_empty() { String::new() } else { format!(" · {}: {}", tr("не открылись", "failed"), fails.join(", ")) })
        };
        r.push(format!("[{p}] {name}{}{check}", if chosen.is_some() { "" } else { tr(" (авто)", " (auto)") }));
    }
    r.join("
")
}

#[tauri::command]
fn open_github(path: String) -> Result<(), String> {
    if path.contains(['"', ' ', '\\']) {
        return Err("bad path".into());
    }
    let url = format!("https://github.com/studiomurda09-dotcom/proryv/{path}");
    hidden_cmd("explorer").arg(url).status().map_err(|e| e.to_string())?;
    Ok(())
}

/// Внешние ссылки, которые интерфейс может открыть: только из этого списка.
#[tauri::command]
fn open_link(kind: String) -> Result<(), String> {
    let url = match kind.as_str() {
        "discord" => "https://discord.gg/SzRm7GNyU9",
        "zapret_docs" => "https://github.com/bol-van/zapret/blob/master/docs/readme.md",
        _ => return Err("неизвестная ссылка".into()),
    };
    hidden_cmd("explorer").arg(url).status().map_err(|e| e.to_string())?;
    Ok(())
}

/// При запуске: база из кэша сразу, потом свежая с GitHub и провайдер.
fn spawn_base_loader(app: AppHandle, cache: PathBuf) {
    if let Some(b) = std::fs::read_to_string(&cache).ok().and_then(|s| base::parse(&s).ok()) {
        base::set_base(b);
    }
    std::thread::spawn(move || {
        let state = app.state::<AppState>();
        let update = state.settings.lock().unwrap().update_base;
        let before = (base::presets(), base::strategies());
        if update {
            let cached = base::base().map_or(0, |b| b.version);
            if let Some(fresh) = base::fetch().filter(|b| b.version >= cached) {
                if let Ok(text) = serde_json::to_string_pretty(&fresh) {
                    let _ = std::fs::write(&cache, text);
                }
                base::set_base(fresh);
            }
        }
        base::set_isp(base::detect_isp());
        BASE_DONE.store(true, Ordering::SeqCst);
        let _ = app.emit("base-updated", isp_info());
        // Пресеты поменялись (домены, стратегия провайдера) — применяем без перезапуска программы.
        if (base::presets(), base::strategies()) != before && is_running(&state) && !state.autotune_running.load(Ordering::SeqCst) {
            let _ = app.emit("status", start_engine(&state));
        }
    });
}

// ---------------------------------------------------------------- обновления

#[derive(Serialize)]
struct UpdateInfo {
    version: String,
    current: String,
    notes: Option<String>,
}

#[tauri::command]
async fn check_update(app: AppHandle) -> Result<Option<UpdateInfo>, String> {
    use tauri_plugin_updater::UpdaterExt;
    let update = app.updater().map_err(|e| e.to_string())?.check().await.map_err(|e| e.to_string())?;
    Ok(update.map(|u| UpdateInfo { version: u.version.clone(), current: u.current_version.clone(), notes: u.body.clone() }))
}

/// Скачать обновление, проверить подпись, поставить его и перезапуститься.
#[tauri::command]
async fn install_update(app: AppHandle) -> Result<(), String> {
    use tauri_plugin_updater::UpdaterExt;
    let update = app
        .updater()
        .map_err(|e| e.to_string())?
        .check()
        .await
        .map_err(|e| e.to_string())?
        .ok_or("Обновлений нет")?;
    let mut done = 0usize;
    let progress = app.clone();
    let bytes = update
        .download(
            move |chunk, total| {
                done += chunk;
                let _ = progress.emit("update-progress", (done, total));
            },
            || {},
        )
        .await
        .map_err(|e| e.to_string())?;
    // Установщик заменит WinDivert64.sys — сначала отпускаем драйвер.
    stop_engine(&app.state::<AppState>());
    let _ = hidden_cmd("sc").args(["stop", "WinDivert"]).status();
    update.install(bytes).map_err(|e| e.to_string())?;
    app.restart();
}

// ---------------------------------------------------------------- трей

fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

fn build_tray(app: &tauri::App) -> tauri::Result<()> {
    let en = is_en(&app.state::<AppState>());
    let tr = |ru: &'static str, en_s: &'static str| if en { en_s } else { ru };
    let show = MenuItem::with_id(app, "show", tr("Открыть Proryv", "Open Proryv"), true, None::<&str>)?;
    let toggle = MenuItem::with_id(app, "toggle", tr("Включить / выключить обход", "Turn bypass on / off"), true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", tr("Выход", "Quit"), true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &toggle, &quit])?;
    TrayIconBuilder::with_id("main")
        .icon(app.default_window_icon().cloned().expect("icon"))
        .tooltip("Proryv")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => show_main(app),
            "toggle" => {
                let state = app.state::<AppState>();
                let st = if is_running(&state) { stop_engine(&state) } else { start_engine_notify(app) };
                let _ = app.emit("status", st);
            }
            "quit" => {
                stop_engine(&app.state::<AppState>());
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                show_main(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

/// Значок в трее с цветной точкой: зелёная — обход работает, красная — ошибка, без точки — выключен.
fn tray_icon(base: &tauri::image::Image<'_>, state: EngineState) -> tauri::image::Image<'static> {
    let (w, h) = (base.width(), base.height());
    let mut rgba = base.rgba().to_vec();
    let color = match state {
        EngineState::Running => Some([0x6c, 0xcb, 0x5f]),
        EngineState::Error => Some([0xff, 0x5a, 0x5a]),
        _ => None,
    };
    if let Some(c) = color {
        let r = w.min(h) as f32 * 0.24;
        let (cx, cy) = (w as f32 - r - 0.5, h as f32 - r - 0.5);
        for y in 0..h {
            for x in 0..w {
                let d = ((x as f32 + 0.5 - cx).powi(2) + (y as f32 + 0.5 - cy).powi(2)).sqrt();
                let i = ((y * w + x) * 4) as usize;
                if d <= r {
                    rgba[i..i + 4].copy_from_slice(&[c[0], c[1], c[2], 255]);
                } else if d <= r + 1.5 * (w as f32 / 32.0).max(1.0) {
                    rgba[i..i + 4].copy_from_slice(&[0, 0, 0, 255]);
                }
            }
        }
    }
    tauri::image::Image::new_owned(rgba, w, h)
}

fn spawn_tray_watch(app: AppHandle) {
    std::thread::spawn(move || {
        let Some(base) = app.default_window_icon().cloned() else { return };
        let mut last = None;
        loop {
            let st = app.state::<AppState>().engine.lock().unwrap().status().state;
            if last != Some(st) {
                if let Some(tray) = app.tray_by_id("main") {
                    let en = is_en(&app.state::<AppState>());
                    let tip = match (st, en) {
                        (EngineState::Running, false) => "Proryv — обход включён",
                        (EngineState::Running, true) => "Proryv — bypass on",
                        (EngineState::Error, false) => "Proryv — ошибка",
                        (EngineState::Error, true) => "Proryv — error",
                        (_, false) => "Proryv — выключен",
                        (_, true) => "Proryv — off",
                    };
                    let _ = tray.set_icon(Some(tray_icon(&base, st)));
                    let _ = tray.set_tooltip(Some(tip));
                }
                last = Some(st);
            }
            std::thread::sleep(std::time::Duration::from_millis(1500));
        }
    });
}

/// Настройки лежат рядом с Proryv.exe (их можно унести вместе с программой).
/// Старые из AppData переносятся один раз. Если в папку программы писать нельзя — остаёмся в AppData.
fn settings_location(app_dir: &std::path::Path) -> PathBuf {
    let legacy = app_dir.join("settings.json");
    let Some(dir) = std::env::current_exe().ok().and_then(|e| e.parent().map(PathBuf::from)) else { return legacy };
    let local = dir.join("settings.json");
    if local.exists() {
        return local;
    }
    let data = std::fs::read(&legacy).unwrap_or_else(|_| serde_json::to_vec_pretty(&Settings::default()).expect("settings"));
    if std::fs::write(&local, data).is_ok() {
        local
    } else {
        legacy
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let autostarted = std::env::args().any(|a| a == "--autostart");
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_notification::init())
        .setup(move |app| {
            let settings_path = settings_location(&app.path().app_config_dir()?);
            let settings = Settings::load(&settings_path);
            if let Some(window) = app.get_webview_window("main") {
                apply_backdrop(&window, &settings.backdrop);
                if autostarted && settings.start_minimized && settings.minimize_to_tray {
                    let _ = window.hide();
                }
            }
            let enable_now = settings.enable_on_launch;
            base::set_custom(&settings.custom_strategies);
            app.manage(AppState {
                engine: Mutex::new(Engine::default()),
                settings: Mutex::new(settings),
                settings_path,
                autotune_running: AtomicBool::new(false),
                tg: Mutex::new(None),
                tg_error: Mutex::new(None),
                trace: Trace::default(),
            });
            sync_tg(&app.state::<AppState>());
            spawn_base_loader(app.handle().clone(), app.path().app_config_dir()?.join("base.json"));
            build_tray(app)?;
            if enable_now {
                start_engine_notify(app.handle());
            }
            spawn_healer(app.handle().clone());
            spawn_tray_watch(app.handle().clone());
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let state = window.state::<AppState>();
                if state.settings.lock().unwrap().minimize_to_tray {
                    api.prevent_close();
                    let _ = window.hide();
                } else {
                    stop_engine(&state);
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_bootstrap,
            save_settings,
            default_settings,
            get_status,
            set_enabled,
            preview_plan,
            check_conflicts,
            tunnel_interfaces,
            diagnose,
            autotune_start,
            set_backdrop,
            system_info,
            relaunch_as_admin,
            tg_info,
            tg_open,
            diag_report,
            tg_test,
            check_update,
            install_update,
            isp_info,
            open_github,
            open_link,
            zapret_parse,
            preview_strategy,
            dev_log,
            dev_check
        ])
        .run(tauri::generate_context!())
        .expect("не удалось запустить Proryv");
}
