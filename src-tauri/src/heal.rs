//! Автоподбор, самолечение и сторож.
//!
//! Подбор при включённом обходе идёт «на ходу»: проверяемый пресет получает пробную
//! стратегию прямо в работающем движке, а остальные пресеты (Discord с голосом и т.д.)
//! в это время продолжают работать. Раньше на весь перебор обход выключался целиком.

use std::{
    collections::HashMap,
    sync::atomic::Ordering,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use proryv_core::{
    autotune::{self, OwnEngine, Outcome, Progress, Trial},
    base,
    config::Settings,
    engine::RunConfig,
};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::{is_en, is_running, notify, preset_title, start_engine, AppState, Lock};

/// Запись в истории самолечения (видна в «Диагностике» и в отчёте).
#[derive(Serialize, Clone)]
pub struct HealEvent {
    /// Unix-время, секунды.
    pub at: u64,
    pub preset: String,
    /// "retrans" — соединения не проходят, "watch" — сторож не открыл сайт,
    /// "slow" — видео тормозит, "network" — сменилась сеть, "manual" — кнопка.
    pub reason: &'static str,
    /// "fixed" — нашли и включили, "not_found" — не нашли, "false_alarm" — перепроверка прошла,
    /// "known" — включили стратегию, которая уже работала в этой сети.
    pub result: &'static str,
    pub from: Option<String>,
    pub to: Option<String>,
}

/// Последняя проверка сторожа по пресету (для пометки «Медленно» на главной).
#[derive(Serialize, Clone)]
pub struct PresetHealth {
    pub ok: usize,
    pub of: usize,
    pub speed: Option<u64>,
    pub ref_speed: Option<u64>,
    pub slow: bool,
    pub at: u64,
}

const HISTORY_CAP: usize = 30;
/// После лечения пресет не трогаем столько времени.
const COOLDOWN: Duration = Duration::from_secs(10 * 60);
/// «Не нашлась стратегия» показываем не чаще.
const FAIL_NOTIFY_EVERY: Duration = Duration::from_secs(60 * 60);
const WATCH_EVERY: Duration = Duration::from_secs(5 * 60);
/// Скорость сторож мерит реже: это ~0,4 МБ трафика, а у многих мобильный интернет.
const SPEED_EVERY: Duration = Duration::from_secs(15 * 60);

fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

fn direct_ip() -> Option<String> {
    #[cfg(windows)]
    return proryv_core::platform::windivert::direct_source_ip();
    #[cfg(not(windows))]
    None
}

/// Ключ сети, для которой запоминаем стратегии: провайдер (ASN), а если он не
/// определился — подсеть прямого адаптера. В отчёт и интерфейс подсеть не попадает.
pub fn net_key() -> String {
    if let Some(i) = base::isp() {
        return format!("AS{}", i.asn);
    }
    match direct_ip().and_then(|ip| ip.rsplit_once('.').map(|(net, _)| net.to_string())) {
        Some(net) => format!("lan:{net}"),
        None => "unknown".into(),
    }
}

pub fn log_event(state: &AppState, ev: HealEvent) {
    let mut h = state.heal_log.lk();
    h.push_front(ev);
    h.truncate(HISTORY_CAP);
}

pub fn current_strategy(settings: &Settings, preset: &str) -> Option<String> {
    settings
        .strategy_override
        .iter()
        .find(|(p, _)| p == preset)
        .map(|(_, s)| s.clone())
        .or_else(|| base::presets().into_iter().find(|p| p.id == preset).and_then(|p| p.strategies.first().cloned()))
}

/// Проба на ходу: стратегия меняется в работающем движке только для одного пресета.
struct LiveTrial<'a> {
    state: &'a AppState,
    settings: &'a Settings,
}

impl Trial for LiveTrial<'_> {
    fn apply(&mut self, preset_id: &str, sid: Option<&str>) -> Result<(), String> {
        let strategy = match sid {
            Some(id) => Some(base::strategies().into_iter().find(|s| s.id == id).ok_or("неизвестная стратегия")?),
            None => None,
        };
        self.state.engine.lk().set_trial(RunConfig::trial_rule(self.settings, preset_id, strategy));
        Ok(())
    }
}

/// Подобрать стратегию и, если нашлась, сохранить её (и запомнить для этой сети).
/// Пока подбор идёт, флаг `autotune_running` уже поднят вызывающим.
pub fn run_tune(app: &AppHandle, preset: &str, source: &'static str) -> Result<Outcome, String> {
    let state = app.state::<AppState>();
    let settings = state.settings.lk().clone();
    let net = net_key();
    let prefer: Vec<String> = settings.good_for(&net, preset).map(String::from).into_iter().collect();
    let progress = |p: &Progress| {
        if source == "manual" {
            let _ = app.emit("autotune", p);
        }
    };
    let live = is_running(&state) && settings.enabled_presets.iter().any(|p| p == preset);
    let res = if live {
        let mut t = LiveTrial { state: &state, settings: &settings };
        let res = autotune::run(preset, &settings, &prefer, &mut t, progress);
        state.engine.lk().set_trial(None);
        res
    } else {
        let was_running = is_running(&state);
        state.engine.lk().pause();
        let mut own = OwnEngine::new(&settings);
        let res = autotune::run(preset, &settings, &prefer, &mut own, progress);
        drop(own);
        if was_running {
            start_engine(&state);
        }
        res
    };
    if let Some(best) = res.as_ref().ok().and_then(|o| o.best.clone()) {
        {
            let mut s = state.settings.lk();
            s.strategy_override.retain(|(p, _)| p != preset);
            s.strategy_override.push((preset.to_string(), best.clone()));
            s.remember_good(&net, preset, &best);
            let _ = s.save(&state.settings_path);
        }
        if live {
            start_engine(&state);
        }
    }
    state.autotune_running.store(false, Ordering::SeqCst);
    res
}

#[derive(Serialize, Clone)]
struct TuneDone {
    /// "manual" — кнопка, "heal" — самолечение.
    source: &'static str,
    result: Result<Outcome, String>,
}

/// Запустить автоподбор в фоне. Прогресс — события "autotune", итог — "autotune-done".
#[tauri::command]
pub fn autotune_start(app: AppHandle, preset: String) -> Result<(), String> {
    let state = app.state::<AppState>();
    if state.autotune_running.swap(true, Ordering::SeqCst) {
        return Err("Подбор уже идёт".into());
    }
    std::thread::spawn(move || {
        let from = current_strategy(&app.state::<AppState>().settings.lk(), &preset);
        let result = run_tune(&app, &preset, "manual");
        let to = result.as_ref().ok().and_then(|o| o.best.clone());
        log_event(&app.state::<AppState>(), HealEvent {
            at: now_secs(), preset: preset.clone(), reason: "manual",
            result: if to.is_some() { "fixed" } else { "not_found" }, from, to,
        });
        let _ = app.emit("autotune-done", TuneDone { source: "manual", result });
    });
    Ok(())
}

#[tauri::command]
pub fn heal_history(state: State<AppState>) -> Vec<HealEvent> {
    state.heal_log.lk().iter().cloned().collect()
}

#[tauri::command]
pub fn preset_health(state: State<AppState>) -> HashMap<String, PresetHealth> {
    state.watch.lk().clone()
}

#[derive(Serialize)]
pub struct SpeedReport {
    /// Скорость «как у видео YouTube» и скорость канала, байт/с.
    youtube: Option<u64>,
    reference: Option<u64>,
    slow: bool,
    /// SoundCloud: открылось адресов из скольких и за сколько мс (самый медленный).
    soundcloud_ok: usize,
    soundcloud_of: usize,
    soundcloud_ms: u64,
}

/// Кнопка «Измерить скорость» в «Диагностике»: как сейчас грузится видео и музыка.
#[tauri::command]
pub async fn speed_test() -> SpeedReport {
    tauri::async_runtime::spawn_blocking(|| {
        let iface = direct_ip();
        let iface = iface.as_deref();
        let probe = autotune::speed_probe("youtube").expect("youtube probe");
        let sc = autotune::test_urls("soundcloud");
        std::thread::scope(|s| {
            let yt = s.spawn(|| autotune::speed_check(&probe, iface));
            let reference = s.spawn(|| autotune::speed_check(&autotune::REF_SPEED, iface));
            let (ok, _, checks) = autotune::check_all(&sc, iface);
            let ms = checks.iter().filter(|c| c.ok).map(|c| c.ms).max().unwrap_or(0);
            let (youtube, reference) = (yt.join().ok().flatten(), reference.join().ok().flatten());
            SpeedReport {
                youtube,
                reference,
                slow: autotune::is_slow(youtube, reference),
                soundcloud_ok: ok,
                soundcloud_of: sc.len(),
                soundcloud_ms: ms,
            }
        })
    })
    .await
    .expect("speed_test")
}

/// Проверка пресета сторожем: адреса и (для YouTube, если `speed`) скорость видео.
fn probe_preset(preset: &str, settings: &Settings, speed: bool) -> Option<PresetHealth> {
    let urls = autotune::urls_for(preset, settings);
    if urls.is_empty() {
        return None;
    }
    let iface = direct_ip();
    let iface = iface.as_deref();
    let urls: Vec<&str> = urls.iter().map(String::as_str).collect();
    let probe = speed.then(|| autotune::speed_probe(preset)).flatten();
    let (ok, speed, ref_speed) = std::thread::scope(|s| {
        let sp = probe.as_ref().map(|p| s.spawn(move || autotune::speed_check(p, iface)));
        let rf = probe.as_ref().map(|_| s.spawn(move || autotune::speed_check(&autotune::REF_SPEED, iface)));
        let (ok, _, _) = autotune::check_all(&urls, iface);
        (ok, sp.and_then(|j| j.join().ok().flatten()), rf.and_then(|j| j.join().ok().flatten()))
    });
    Some(PresetHealth { ok, of: urls.len(), speed, ref_speed, slow: autotune::is_slow(speed, ref_speed), at: now_secs() })
}

/// Сломан ли пресет: не всё открылось или видео тормозит.
fn broken(h: &PresetHealth) -> Option<&'static str> {
    if h.ok < h.of {
        Some("watch")
    } else if h.slow {
        Some("slow")
    } else {
        None
    }
}

/// Сторож: проверить включённые пресеты и запомнить результат. Возвращает сломанные.
fn watch_presets(state: &AppState, settings: &Settings, speed: bool) -> Vec<(String, &'static str)> {
    let mut out = Vec::new();
    for p in &settings.enabled_presets {
        let Some(h) = probe_preset(p, settings, speed) else { continue };
        // Скорость без нового замера берём из прошлого: пометка «Медленно» не должна мигать.
        let h = match (speed, state.watch.lk().get(p)) {
            (false, Some(old)) if old.speed.is_some() => PresetHealth { speed: old.speed, ref_speed: old.ref_speed, slow: old.slow, ..h },
            _ => h,
        };
        if let Some(reason) = broken(&h) {
            out.push((p.clone(), reason));
        }
        state.watch.lk().insert(p.clone(), h);
    }
    out
}

/// Перед лечением перепроверяем: одна неудачная проверка бывает и на рабочей стратегии
/// (просела сеть, сайт моргнул). Лечим, только если сломано оба раза.
fn confirm_broken(preset: &str, settings: &Settings) -> bool {
    for i in 0..2 {
        if i > 0 {
            std::thread::sleep(Duration::from_secs(3));
        }
        match probe_preset(preset, settings, true) {
            Some(h) if broken(&h).is_none() => return false,
            None => return false,
            _ => {}
        }
    }
    true
}

/// Сеть сменилась: заново определяем провайдера и включаем то, что в этой сети уже работало.
fn on_network_change(app: &AppHandle) {
    // Даём сети подняться.
    std::thread::sleep(Duration::from_secs(4));
    base::set_isp(base::detect_isp());
    let _ = app.emit("base-updated", crate::isp_info());
    apply_known(app);
}

/// Если в текущей сети для включённых пресетов уже что-то работало — включить это сразу.
pub fn apply_known(app: &AppHandle) {
    let state = app.state::<AppState>();
    let net = net_key();
    let mut switched = Vec::new();
    {
        let mut s = state.settings.lk();
        let presets = s.enabled_presets.clone();
        for p in presets {
            let Some(good) = s.good_for(&net, &p).map(String::from) else { continue };
            let cur = current_strategy(&s, &p);
            if cur.as_deref() != Some(good.as_str()) {
                s.strategy_override.retain(|(id, _)| *id != p);
                s.strategy_override.push((p.clone(), good.clone()));
                switched.push((p, cur, good));
            }
        }
        if !switched.is_empty() {
            let _ = s.save(&state.settings_path);
        }
    }
    if switched.is_empty() {
        return;
    }
    if is_running(&state) {
        let _ = app.emit("status", start_engine(&state));
    }
    let en = is_en(&state);
    let names: Vec<String> = switched.iter().map(|(p, _, _)| preset_title(p, en)).collect();
    notify(
        app,
        ("Сеть сменилась", "Network changed"),
        (
            format!("Включили стратегии, которые уже работали в этой сети: {}.", names.join(", ")),
            format!("Switched to strategies that already worked on this network: {}.", names.join(", ")),
        ),
    );
    for (p, from, to) in switched {
        log_event(&state, HealEvent { at: now_secs(), preset: p, reason: "network", result: "known", from, to: Some(to) });
    }
    let _ = app.emit("heal-log", ());
    let _ = app.emit("overrides-changed", ());
}

/// Самолечение: раз в 20 с смотрим долю повторных ClientHello по пресетам.
/// Сторож: раз в 5 минут сам открывает адреса проверки (раз в 15 — ещё и мерит скорость
/// видео), так что поломка и торможение находятся, даже если сайтом сейчас никто не пользуется.
pub fn spawn_healer(app: AppHandle) {
    std::thread::spawn(move || {
        let mut prev: HashMap<String, (u64, u64)> = HashMap::new();
        let mut last_heal: HashMap<String, Instant> = HashMap::new();
        let mut last_fail_note: HashMap<String, Instant> = HashMap::new();
        let mut last_watch = Instant::now();
        let mut last_speed = Instant::now() - SPEED_EVERY + Duration::from_secs(60);
        let mut last_ip = direct_ip();
        let mut tick = 0u64;
        loop {
            std::thread::sleep(Duration::from_secs(20));
            tick += 1;
            let state = app.state::<AppState>();
            let settings = state.settings.lk().clone();
            if !settings.auto_heal || state.autotune_running.load(Ordering::SeqCst) || !is_running(&state) {
                prev.clear();
                continue;
            }
            // Сеть проверяем раз в минуту: адрес адаптера узнаётся через PowerShell.
            let ip = if tick.is_multiple_of(3) { direct_ip() } else { last_ip.clone() };
            if ip != last_ip {
                last_ip = ip.clone();
                prev.clear();
                if ip.is_some() {
                    on_network_change(&app);
                }
                continue;
            }
            let cooled = |p: &str| last_heal.get(p).is_none_or(|t| t.elapsed() > COOLDOWN);
            let now_h = state.engine.lk().health();
            let mut target: Option<(String, &'static str)> = None;
            for (preset, (d, r)) in &now_h {
                let (pd, pr) = prev.get(preset).copied().unwrap_or((0, 0));
                let (dd, dr) = (d.saturating_sub(pd), r.saturating_sub(pr));
                // ≥4 повторов и не меньше трети всех соединений за окно (даже после спасения запасной).
                if dr >= 4 && dr * 3 >= dd && cooled(preset) && !autotune::urls_for(preset, &settings).is_empty() {
                    target = Some((preset.clone(), "retrans"));
                    break;
                }
            }
            prev = now_h;
            if target.is_none() && last_watch.elapsed() > WATCH_EVERY {
                last_watch = Instant::now();
                let speed = last_speed.elapsed() > SPEED_EVERY;
                if speed {
                    last_speed = Instant::now();
                }
                target = watch_presets(&state, &settings, speed).into_iter().find(|(p, _)| cooled(p));
                let _ = app.emit("preset-health", ());
            }
            let Some((preset, reason)) = target else { continue };
            // Нет сети вообще — обход не виноват, подбирать нечего.
            if !autotune::internet_ok(direct_ip().as_deref()) {
                prev.clear();
                continue;
            }
            last_heal.insert(preset.clone(), Instant::now());
            let from = current_strategy(&settings, &preset);
            if !confirm_broken(&preset, &settings) {
                log_event(&state, HealEvent { at: now_secs(), preset, reason, result: "false_alarm", from, to: None });
                let _ = app.emit("heal-log", ());
                prev.clear();
                continue;
            }
            if state.autotune_running.swap(true, Ordering::SeqCst) {
                continue;
            }
            let _ = app.emit("heal-start", &preset);
            let result = run_tune(&app, &preset, "heal");
            let to = result.as_ref().ok().and_then(|o| o.best.clone());
            let en = is_en(&state);
            let name = crate::preset_title(&preset, en);
            match result.as_ref().ok().and_then(|o| o.best.as_ref().map(|b| (b, &o.best_name))) {
                Some((id, ru_name)) => notify(
                    &app,
                    ("Самолечение", "Self-healing"),
                    (
                        format!("{name}: {} — переключили на «{}». Остальное в это время работало.", reason_ru(reason), ru_name.as_deref().unwrap_or(id)),
                        format!("{name}: {} — switched to strategy {id}. Everything else kept working.", reason_en(reason)),
                    ),
                ),
                None if last_fail_note.get(&preset).is_none_or(|t| t.elapsed() > FAIL_NOTIFY_EVERY) => {
                    last_fail_note.insert(preset.clone(), Instant::now());
                    notify(
                        &app,
                        ("Самолечение", "Self-healing"),
                        (
                            format!("{name}: {}, а лучшей стратегии не нашлось. Попробуйте автоподбор или «Диагностику».", reason_ru(reason)),
                            format!("{name}: {}, and no better strategy was found. Try auto-tune or Diagnostics.", reason_en(reason)),
                        ),
                    );
                }
                None => {}
            }
            log_event(&state, HealEvent {
                at: now_secs(), preset: preset.clone(), reason,
                result: if to.is_some() { "fixed" } else { "not_found" }, from, to,
            });
            // Лечение могло заодно починить пресет — пусть сторож перепроверит его в следующий раз.
            state.watch.lk().remove(&preset);
            let _ = app.emit("autotune-done", TuneDone { source: "heal", result });
            let _ = app.emit("heal-log", ());
            prev.clear();
        }
    });
}

fn reason_ru(r: &str) -> &'static str {
    match r {
        "slow" => "видео тормозит",
        "retrans" => "соединения перестали проходить",
        _ => "перестал открываться",
    }
}

fn reason_en(r: &str) -> &'static str {
    match r {
        "slow" => "video is slow",
        "retrans" => "connections stopped going through",
        _ => "stopped opening",
    }
}
