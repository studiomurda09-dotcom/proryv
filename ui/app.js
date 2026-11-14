'use strict';

/* =====================================================================
   Связь с бэкендом. В Tauri — настоящие команды Rust.
   В обычном браузере — заглушка, чтобы можно было смотреть дизайн.
   ===================================================================== */
const TAURI = window.__TAURI__;

const mockBackend = (() => {
  const M = window.MOCK_DATA;
  const KEY = 'proryv-mock-settings';
  let enabled = false, started = 0;
  const load = () => { try { return JSON.parse(localStorage.getItem(KEY)) || null; } catch { return null; } };
  const store = (s) => { try { localStorage.setItem(KEY, JSON.stringify(s)); } catch {} };

  // Упрощённый порт strategy::plan — только для предпросмотра в браузере.
  function plan(host, strategy) {
    const helloLen = (h) => 95 + h.length + 9;
    const len = helloLen(host), hOff = 95, hLen = host.length;
    const pos = (p) => {
      const lab = host.split('.'), sldOff = hOff + lab.slice(0, -2).reduce((a, l) => a + l.length + 1, 0);
      const midsld = lab.length >= 2 ? sldOff + Math.floor(lab[lab.length - 2].length / 2) : hOff + Math.floor(hLen / 2);
      const base = { start: 0, host_start: hOff, host_mid: hOff + Math.floor(hLen / 2), host_end: hOff + hLen, midsld, sni_ext: hOff - 9 }[p.anchor];
      const v = base + (p.add || 0);
      return v > 0 && v < len ? v : null;
    };
    let cuts = [], disorder = false, delay = 0, segs = [], payloadLen = len;
    for (const s of strategy.steps) {
      if (s.kind === 'split') cuts.push(pos(s.at));
      if (s.kind === 'multi_split') cuts.push(...s.at.map(pos));
      if (s.kind === 'disorder') { cuts.push(pos(s.at)); disorder = true; }
      if (s.kind === 'multi_disorder') { cuts.push(...s.at.map(pos)); disorder = true; }
      if (s.kind === 'delay') delay = s.ms;
      if (s.kind === 'tls_record_split') payloadLen += 5;
    }
    cuts = [...new Set(cuts.filter((c) => c != null))].sort((a, b) => a - b);
    const pool = ['www.google.com', 'yandex.ru', 'vk.com', 'www.microsoft.com', 'ok.ru'];
    for (const s of strategy.steps) {
      if (s.kind !== 'fake') continue;
      for (let r = 0; r < Math.max(1, s.repeats); r++) {
        const name = s.sni.mode === 'fixed' ? s.sni.value : pool[(r * 3 + host.length) % pool.length];
        segs.push({ kind: 'fake', seq_offset: 0, data: { length: Math.max(len, helloLen(name)) },
          ttl: s.fooling.includes('ttl') ? s.ttl : null, fooling: s.fooling, delay_ms: 0, label: `фейк #${r + 1} (${name})` });
      }
    }
    const b = [0, ...cuts, payloadLen];
    let real = [];
    for (let i = 0; i < b.length - 1; i++)
      real.push({ kind: 'real', seq_offset: b[i], data: { length: b[i + 1] - b[i] }, ttl: null, fooling: [], delay_ms: 0, label: `часть ${i + 1} [${b[i]}..${b[i + 1]})` });
    if (disorder && real.length > 1) real.reverse();
    real.slice(1).forEach((s) => (s.delay_ms = delay));
    for (const s of strategy.steps) {
      if (s.kind !== 'seq_overlap') continue;
      const f = real.find((r) => r.seq_offset === 0);
      if (f) { f.data = { length: f.data.length + s.len }; f.label += ` + seqovl ${s.len} Б (${s.sni})`; }
    }
    return { proto: 'tls', host, payload_len: payloadLen, segments: [...segs, ...real] };
  }

  const status = () => ({
    state: enabled ? 'running' : 'stopped', presets: [], message: null, demo: true,
    stats: { connections: 0, desynced: 0, fakes_sent: 0, uptime_secs: enabled ? Math.floor((Date.now() - started) / 1000) : 0 },
  });

  return {
    get_bootstrap: () => ({ version: M.version, presets: M.presets, strategies: M.strategies, settings: { ...M.defaults, ...(load() || {}) } }),
    save_settings: ({ settings }) => { store(settings); return null; },
    get_status: status,
    set_enabled: ({ enabled: e }) => { enabled = e; started = Date.now(); return status(); },
    preview_plan: ({ host, strategyId }) => plan(host, M.strategies.find((s) => s.id === strategyId)),
    check_conflicts: () => [{ process: 'winws.exe', name: 'zapret (пример в браузере)' }],
    default_settings: () => ({ ...M.defaults }),
    tunnel_interfaces: () => ['sing-tun Tunnel (пример)'],
    tg_info: () => ({ enabled: !!(load() || {}).telegram, running: !!(load() || {}).telegram,
      link: 'tg://proxy?server=127.0.0.1&port=1443&secret=dd0123456789abcdef0123456789abcdef', error: null,
      stats: { active: 3, total: 12, via_ws: 12, via_tcp: 0, failed: 0, bytes_up: 1200000, bytes_down: 48000000 } }),
    tg_open: () => null,
    check_update: () => null,
    install_update: () => { throw 'в браузере обновление недоступно'; },
    tg_test: () => [[2, { Ok: 340 }], [4, { Ok: 370 }], [1, { Err: 'заблокирован по IP' }]],
    diagnose: ({ host }) => ({ host, ips: ['142.251.150.4'], verdict: 'dpi', direct_path: true,
      message: 'Пример: соединение есть, но рукопожатие TLS обрывается — это блокировка DPI.' }),
    autotune_start: ({ preset }) => {
      const p = M.presets.find((x) => x.id === preset);
      const steps = [['baseline', 'Без обхода', 1], ...p.strategies.slice(0, 3).map((s, i) => [s, M.strategies.find((x) => x.id === s).name, i === 0 ? 4 : 2])];
      steps.forEach(([id, name, ok], i) => setTimeout(() => mockEmit('autotune', {
        preset, strategy: id, strategy_name: name, index: i, total: steps.length, ok, of: 4, ms: 900 + i * 300, done: false }), 500 * (i + 1)));
      setTimeout(() => mockEmit('autotune-done', { source: 'manual', result: { Ok: { preset, baseline_ok: 1, of: 4, best: steps[1][0], best_name: steps[1][1], results: [] } } }), 500 * (steps.length + 1));
      return null;
    },
  };
})();

const mockListeners = {};
function mockEmit(name, payload) { (mockListeners[name] || []).forEach((cb) => cb(payload)); }
function onEvent(name, cb) {
  if (TAURI) return TAURI.event.listen(name, (e) => cb(e.payload));
  (mockListeners[name] = mockListeners[name] || []).push(cb);
}

async function call(cmd, args = {}) {
  if (TAURI) return TAURI.core.invoke(cmd, args);
  await new Promise((r) => setTimeout(r, 60));
  return mockBackend[cmd](args);
}

/* ===================================================================== */
const $ = (s, r = document) => r.querySelector(s);
const $$ = (s, r = document) => [...r.querySelectorAll(s)];
const esc = (s) => String(s).replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]));

const ACCENTS = ['#7cc4e4', '#9fb4a3', '#a78bfa', '#f472b6', '#fb923c', '#facc15', '#4ade80', '#60a5fa'];
const PAGE_TITLES = {
  home: 'Главная', presets: 'Пресеты', strategies: 'Стратегии', autotune: 'Автоподбор', domains: 'Списки доменов',
  diag: 'Диагностика', appearance: 'Внешний вид', general: 'Общие настройки', about: 'О программе',
};

const state = { saved: null, settings: null, presets: [], strategies: [], status: null, busy: false };

/* ---------- Тема и внешний вид ---------- */
function contrastText(hex) {
  const n = parseInt(hex.slice(1), 16), r = n >> 16, g = (n >> 8) & 255, b = n & 255;
  return (r * 299 + g * 587 + b * 114) / 1000 > 140 ? '#0d1a20' : '#ffffff';
}
function applyLook() {
  const s = state.settings;
  document.documentElement.style.setProperty('--accent', s.accent);
  document.documentElement.style.setProperty('--accent-fg', contrastText(s.accent));
  document.body.classList.toggle('no-anim', !s.animations);
  document.body.classList.remove('backdrop-mica', 'backdrop-acrylic', 'sim-backdrop');
  if (s.backdrop !== 'none') {
    document.body.classList.add('backdrop-' + s.backdrop);
    if (!TAURI) document.body.classList.add('sim-backdrop');
  }
  if (TAURI) TAURI.core.invoke('set_backdrop', { backdrop: s.backdrop }).catch(() => {});
  $$('[data-accent-picker] button').forEach((b) => b.classList.toggle('sel', b.dataset.color === s.accent));
}

/* ---------- Привязка настроек к контролам ---------- */
function syncControls() {
  const s = state.settings;
  $$('[data-setting]').forEach((el) => {
    const v = s[el.dataset.setting];
    if (el.type === 'checkbox') el.checked = !!v; else el.value = v;
  });
  $$('[data-setting-list]').forEach((el) => (el.value = (s[el.dataset.settingList] || []).join('\n')));
  applyLook();
}
function markDirty() {
  const dirty = JSON.stringify(state.settings) !== JSON.stringify(state.saved);
  $('#dirty').classList.toggle('hidden', !dirty);
  $('#btn-save').disabled = !dirty;
  $('#btn-revert').disabled = !dirty;
}
function setSetting(key, value) {
  state.settings[key] = value;
  syncControls();
  markDirty();
  if (key === 'enabled_presets' || key === 'strategy_override') { renderPresets(); renderHero(); }
}
function bindSettings() {
  document.addEventListener('change', (e) => {
    const el = e.target;
    if (el.dataset.setting) setSetting(el.dataset.setting, el.type === 'checkbox' ? el.checked : el.value);
  });
  document.addEventListener('input', (e) => {
    const el = e.target;
    if (el.dataset.settingList) {
      state.settings[el.dataset.settingList] = el.value.split('\n').map((x) => x.trim()).filter(Boolean);
      markDirty();
    }
  });
  $$('[data-accent-picker]').forEach((box) => {
    box.innerHTML = ACCENTS.map((c) => `<button data-color="${c}" style="background:${c}" title="${c}"></button>`).join('');
    box.addEventListener('click', (e) => e.target.dataset.color && setSetting('accent', e.target.dataset.color));
  });
}

/* ---------- Навигация ---------- */
function showPage(id) {
  $$('.sidebar a').forEach((a) => a.classList.toggle('active', a.dataset.page === id));
  $$('.page').forEach((p) => p.classList.toggle('active', p.dataset.page === id));
  $('#tb-page').textContent = PAGE_TITLES[id];
  $('#content').scrollTop = 0;
  try { localStorage.setItem('proryv-page', id); } catch {}
}

/* ---------- Главная ---------- */
function fmtUptime(s) {
  if (!s) return '—';
  const h = Math.floor(s / 3600), m = Math.floor((s % 3600) / 60), sec = s % 60;
  return h ? `${h} ч ${m} мин` : m ? `${m} мин ${sec} с` : `${sec} с`;
}
function renderHero() {
  const st = state.status, on = st && st.state === 'running', err = st && st.state === 'error';
  $('#power').classList.toggle('on', on);
  $('#power').classList.toggle('err', !!err);
  $('#power').classList.toggle('busy', state.busy);
  $('#hero-title').textContent = state.busy ? (on ? 'Выключаем…' : 'Включаем…') : on ? 'Обход включён' : err ? 'Не удалось включить' : 'Обход выключен';
  const n = state.settings.enabled_presets.length;
  $('#hero-sub').textContent = err
    ? st.message
    : on
      ? (st.stats.last_host ? 'Последний обработанный сайт: ' + st.stats.last_host : 'Откройте заблокированный сайт — он должен загрузиться.')
      : n ? 'Нажмите кнопку, чтобы включить.' : 'Сначала выберите хотя бы один пресет.';
  const sys = state.system || { elevated: true, driver_found: true };
  $('#admin-banner').classList.toggle('hidden', !TAURI || sys.elevated);
  $('#driver-banner').classList.toggle('hidden', !TAURI || sys.driver_found);
  $('#hero-chips').innerHTML = state.presets
    .filter((p) => state.settings.enabled_presets.includes(p.id))
    .map((p) => `<span class="chip">${icon(p.icon)}${esc(p.name)}</span>`).join('');
  const pill = $('#tb-status');
  pill.classList.toggle('on', on);
  pill.lastElementChild.textContent = on ? 'Включён' : 'Выключен';
  $('#demo-banner').classList.toggle('hidden', !!TAURI);
  if (st) {
    $('#st-uptime').textContent = fmtUptime(st.stats.uptime_secs);
    $('#st-conn').textContent = st.stats.connections;
    $('#st-desync').textContent = st.stats.desynced;
    $('#st-fakes').textContent = st.stats.fakes_sent;
  }
}
async function togglePower() {
  if (state.busy) return;
  if (atRunning) return toast('Идёт подбор стратегии — подождите');
  const on = state.status && state.status.state === 'running';
  if (!on && !state.settings.enabled_presets.length) { toast('Выберите хотя бы один пресет'); showPage('presets'); return; }
  // Движок берёт сохранённые настройки — сохраняем перед включением.
  if (!on && JSON.stringify(state.settings) !== JSON.stringify(state.saved)) await save();
  state.busy = true; renderHero();
  try {
    state.status = await call('set_enabled', { enabled: !on });
  } catch (e) { toast('Ошибка: ' + e); }
  state.busy = false; renderHero();
}

/* ---------- Пресеты ---------- */
function strategyName(id) { return (state.strategies.find((s) => s.id === id) || {}).name || id; }
function overrideFor(pid) { const o = state.settings.strategy_override.find(([p]) => p === pid); return o ? o[1] : ''; }
function renderPresets() {
  const en = state.settings.enabled_presets;
  $('#preset-list').innerHTML = state.presets.map((p) => {
    const ports = [...p.tcp_ports.map((x) => 'TCP ' + x), ...p.udp_ports.map(([a, b]) => 'UDP ' + (a === b ? a : a + '–' + b))];
    const opts = ['<option value="">Авто (рекомендуется)</option>',
      ...p.strategies.map((s) => `<option value="${s}" ${overrideFor(p.id) === s ? 'selected' : ''}>${esc(strategyName(s))}</option>`)].join('');
    return `<div class="card preset-card">
      <div class="p-icon">${icon(p.icon)}</div>
      <div><b>${esc(p.name)}</b><small>${esc(p.description)}</small>
        <div class="p-meta">${p.domains.length ? p.domains.length + ' доменов · ' : 'Свои домены · '}${ports.join(', ')}</div></div>
      <select data-preset-strategy="${p.id}">${opts}</select>
      <label class="toggle"><input type="checkbox" data-preset="${p.id}" ${en.includes(p.id) ? 'checked' : ''}><i></i></label>
    </div>`;
  }).join('');
  $('#wz-presets').innerHTML = state.presets.map((p) => `
    <button class="preset-tile ${en.includes(p.id) ? 'sel' : ''}" data-preset-tile="${p.id}">
      <span class="p-icon">${icon(p.icon)}</span>
      <span><b>${esc(p.name)}</b><small>${esc(p.description)}</small></span>
      <span class="check">${icon('check')}</span>
    </button>`).join('');
}
function togglePreset(id, on) {
  const set = new Set(state.settings.enabled_presets);
  on ? set.add(id) : set.delete(id);
  setSetting('enabled_presets', state.presets.map((p) => p.id).filter((x) => set.has(x)));
}
function bindPresets() {
  document.addEventListener('change', (e) => {
    const t = e.target;
    if (t.dataset.preset) togglePreset(t.dataset.preset, t.checked);
    if (t.dataset.presetStrategy !== undefined) {
      const pid = t.dataset.presetStrategy;
      const rest = state.settings.strategy_override.filter(([p]) => p !== pid);
      setSetting('strategy_override', t.value ? [...rest, [pid, t.value]] : rest);
    }
  });
  $('#wz-presets').addEventListener('click', (e) => {
    const b = e.target.closest('[data-preset-tile]');
    if (b) togglePreset(b.dataset.presetTile, !state.settings.enabled_presets.includes(b.dataset.presetTile));
  });
}

/* ---------- Стратегии ---------- */
function summary(st) {
  const pos = (p) => {
    const a = { start: '', host_start: 'host', host_mid: 'midhost', host_end: 'endhost', midsld: 'midsld', sni_ext: 'sniext' }[p.anchor];
    if (!a) return String(p.add || 0);
    return p.add ? a + (p.add > 0 ? '+' : '') + p.add : a;
  };
  return st.steps.map((s) => ({
    split: () => 'split=' + pos(s.at),
    multi_split: () => 'multisplit=' + s.at.map(pos).join(','),
    disorder: () => 'disorder=' + pos(s.at),
    multi_disorder: () => 'multidisorder=' + s.at.map(pos).join(','),
    seq_overlap: () => `seqovl=${s.len} (${s.sni})`,
    fake: () => `fake ttl=${s.ttl} fool=${s.fooling.join('+')} x${s.repeats}`,
    tls_record_split: () => 'tlsrec=' + pos(s.at),
    host_case: () => 'hostcase',
    delay: () => `delay=${s.ms}ms`,
  }[s.kind] || (() => s.kind))()).join('  ');
}
function renderStrategies() {
  $('#strategy-list').innerHTML = state.strategies.map((s) => `
    <div class="card"><b>${esc(s.name)}</b><small>${esc(s.description)}</small><div class="strat-sum">${esc(summary(s))}</div></div>`).join('');
  $('#pv-strategy').innerHTML = state.strategies.map((s) => `<option value="${s.id}">${esc(s.name)}</option>`).join('');
  $('#pv-strategy').value = 'seqovl_google';
}
async function renderPreview() {
  const host = $('#pv-host').value.trim() || 'www.youtube.com';
  const plan = await call('preview_plan', { host, strategyId: $('#pv-strategy').value });
  const total = plan.segments.reduce((a, s) => a + s.data.length, 0);
  const bar = plan.segments.map((s) =>
    `<div class="pv-seg ${s.kind}" style="flex:${s.data.length}" title="${esc(s.label)}">${s.data.length}</div>`).join('');
  const list = plan.segments.map((s, i) => {
    const extra = [s.ttl != null && 'TTL ' + s.ttl, s.fooling.filter((f) => f !== 'ttl').join('+'), s.delay_ms && 'пауза ' + s.delay_ms + ' мс'].filter(Boolean).join(', ');
    return `<div class="${s.kind === 'fake' ? 'f' : ''}">${i + 1}. ${esc(s.label)} — ${s.data.length} Б${extra ? ' · ' + esc(extra) : ''}</div>`;
  }).join('');
  $('#pv-out').innerHTML = `
    <div class="pv-legend"><span>настоящие данные</span><span class="f">фейк (до сервера не дойдёт)</span>
      <span style="margin-left:auto">ClientHello ${plan.payload_len} Б → ${plan.segments.length} пакетов, всего ${total} Б</span></div>
    <div class="pv-bar">${bar}</div><div class="pv-list">${list}</div>`;
}

/* ---------- Конфликты ---------- */
async function renderConflicts() {
  let list = [];
  try { list = await call('check_conflicts'); } catch {}
  const html = list.length
    ? list.map((c) => `<div class="card row"><div><b class="conf-warn">Найдено: ${esc(c.name)}</b>
        <small>Процесс ${esc(c.process)} тоже перехватывает трафик. Выключите его перед включением Proryv, иначе обе программы будут мешать друг другу.</small></div></div>`).join('')
    : `<div class="card row"><div><b class="conf-ok">Конфликтов не найдено</b><small>Другие программы обхода сейчас не запущены.</small></div></div>`;
  $('#conflicts').innerHTML = html;
  $('#wz-conflicts').innerHTML = html;
}

/* ---------- Автоподбор ---------- */
const AT_PRESETS = ['youtube', 'discord', 'general', 'roblox', 'social'];
let atRunning = false;
function renderAutotunePresets() {
  const sel = $('#at-preset'), cur = sel.value;
  const list = state.presets.filter((p) => AT_PRESETS.includes(p.id));
  sel.innerHTML = list.map((p) => `<option value="${p.id}">${esc(p.name)}</option>`).join('');
  sel.value = cur || 'youtube';
}
function atRow(p) {
  const mark = p.ok === p.of ? `<span style="color:var(--ok)">${icon('check')}</span>`
    : p.ok === 0 ? `<span style="color:var(--err)">${icon('x')}</span>` : `<span style="color:var(--warn)">◐</span>`;
  return `<div class="card at-row" data-sid="${p.strategy}">
    <span class="st">${mark}</span>
    <div><b>${esc(p.strategy_name)}</b><small>${p.strategy === 'baseline' ? 'проверка без обхода' : esc(p.strategy)}</small></div>
    <div class="bar"><i style="width:${(p.ok / p.of) * 100}%"></i></div>
    <span class="n">${p.ok}/${p.of} · ${(p.ms / 1000).toFixed(1)} с</span>
  </div>`;
}
async function startAutotune() {
  if (atRunning) return;
  const preset = $('#at-preset').value;
  atRunning = true;
  $('#at-start').disabled = true;
  $('#at-start').textContent = 'Подбираем…';
  $('#at-results').classList.remove('hidden');
  $('#at-results').innerHTML = '';
  $('#at-summary').classList.add('hidden');
  try {
    await call('autotune_start', { preset });
  } catch (e) {
    atRunning = false;
    $('#at-start').disabled = false;
    $('#at-start').textContent = 'Подобрать';
    toast(String(e));
  }
}
function onAutotuneProgress(p) {
  $('#at-results').insertAdjacentHTML('beforeend', atRow(p));
}
async function refreshOverrides() {
  const boot = await call('get_bootstrap');
  state.settings.strategy_override = boot.settings.strategy_override;
  state.saved.strategy_override = boot.settings.strategy_override;
  renderPresets(); markDirty();
}
function presetName(id) { return (state.presets.find((p) => p.id === id) || {}).name || id; }
async function onTuneDone(ev) {
  if (ev.source === 'heal') {
    const o = ev.result.Ok;
    if (o && o.best) toast(`Самолечение: ${presetName(o.preset)} → «${o.best_name}»`);
    else toast(`Самолечение: для ${presetName(o ? o.preset : '')} новой стратегии не нашлось`);
    return refreshOverrides();
  }
  return onAutotuneDone(ev.result);
}
async function onAutotuneDone(res) {
  atRunning = false;
  $('#at-start').disabled = false;
  $('#at-start').textContent = 'Подобрать';
  const box = $('#at-summary');
  box.classList.remove('hidden');
  if (res.Err) {
    box.innerHTML = `<span class="verdict ip">Ошибка</span><div>${esc(res.Err)}</div>`;
    return;
  }
  const o = res.Ok;
  if (o.best) {
    $$(`#at-results [data-sid="${o.best}"]`).forEach((r) => r.classList.add('best'));
    box.innerHTML = `<span class="verdict ok">Готово</span><div>Лучшая стратегия: <b>${esc(o.best_name)}</b>. Она сохранена для пресета и уже применяется.</div>`;
  } else if (o.baseline_ok === o.of) {
    box.innerHTML = `<span class="verdict ok">Без блокировки</span><div>Сайты открываются и без обхода — менять ничего не нужно.</div>`;
  } else {
    box.innerHTML = `<span class="verdict dpi">Не нашли</span><div>Ни одна стратегия не помогла. Возможно, сайт заблокирован по IP — проверьте его в «Диагностике».</div>`;
  }
  if (o.direct_ip === null && TAURI) box.insertAdjacentHTML('beforeend', '<small>Не нашёлся прямой адаптер — проверка могла идти через VPN.</small>');
  // Подбор сохранил стратегию в настройках — подтягиваем их.
  await refreshOverrides();
}

/* ---------- Диагностика ---------- */
async function runDiagnose() {
  const host = $('#diag-host').value.trim();
  if (!host) return toast('Введите адрес сайта');
  const out = $('#diag-out');
  out.classList.remove('hidden');
  out.innerHTML = '<span class="spin"></span> Проверяем…';
  $('#diag-run').disabled = true;
  try {
    const d = await call('diagnose', { host });
    const label = { ok: 'Открывается', dpi: 'Блокировка DPI', ip: 'Блокировка по IP', dns: 'Проблема DNS', other: 'Не удалось проверить' }[d.verdict];
    out.innerHTML = `<span class="verdict ${d.verdict}">${label}</span><div>${esc(d.message)}</div>
      <div class="mono">${esc(d.host)} → ${esc(d.ips.join(', ') || '—')}${d.direct_path ? ' · проверено напрямую, мимо VPN' : ''}</div>`;
  } catch (e) {
    out.innerHTML = `<span class="verdict ip">Ошибка</span><div>${esc(String(e))}</div>`;
  }
  $('#diag-run').disabled = false;
}
async function renderTunnels() {
  let list = [];
  try { list = await call('tunnel_interfaces'); } catch {}
  $('#tunnels').innerHTML = list.length
    ? list.map((t) => `<div class="card row"><div><b>${esc(t)}</b></div><span class="tag">пропускается</span></div>`).join('')
    : '<div class="card row"><div><b>VPN-туннелей нет</b><small>Весь трафик идёт напрямую.</small></div></div>';
}

/* ---------- Telegram ---------- */
let tgLink = '';
function fmtBytes(n) {
  const u = ['Б', 'КБ', 'МБ', 'ГБ']; let i = 0;
  while (n >= 1024 && i < u.length - 1) { n /= 1024; i++; }
  return `${n.toFixed(i ? 1 : 0)} ${u[i]}`;
}
async function renderTg() {
  let info;
  try { info = await call('tg_info'); } catch { return; }
  tgLink = info.link;
  $('#tg-body').classList.toggle('hidden', !info.enabled);
  const st = $('#tg-status');
  if (info.error) st.innerHTML = `<span style="color:var(--err)">Ошибка: ${esc(info.error)}</span>`;
  else if (!info.running) st.textContent = 'Включите и нажмите «Сохранить», чтобы запустить прокси.';
  else {
    const s = info.stats;
    st.textContent = `Работает на 127.0.0.1:${s.port || 1443} · соединений ${s.active} (всего ${s.total}) · ↑ ${fmtBytes(s.bytes_up)} ↓ ${fmtBytes(s.bytes_down)}`
      + (s.failed ? ` · ошибок ${s.failed}` : '') + (s.bad_secret ? ' · неверный секрет — переподключите Telegram' : '');
  }
}
async function tgTest() {
  const out = $('#tg-test-out');
  out.textContent = 'Проверяем дата-центры…';
  const res = await call('tg_test');
  out.innerHTML = res.map(([dc, r]) => r.Ok !== undefined
    ? `<span style="color:var(--ok)">DC${dc}: ✓ ${r.Ok} мс</span>`
    : `<span style="color:var(--err)">DC${dc}: ✗ ${esc(r.Err)}</span>`).join(' · ')
    + (res.some(([dc, r]) => dc === 1 && r.Err) ? '<br>DC1/3/5 без VPN недоступны (блокировка по IP) — фото и видео из них могут не грузиться.' : '');
}

/* ---------- Импорт / экспорт ---------- */
function openModal({ title, sub, text = '', ok, readonly = false, onOk }) {
  $('#modal-title').textContent = title;
  $('#modal-sub').textContent = sub;
  $('#modal-text').value = text;
  $('#modal-text').readOnly = readonly;
  $('#modal-ok').textContent = ok;
  $('#modal').classList.remove('hidden');
  $('#modal-ok').onclick = async () => { if ((await onOk($('#modal-text').value)) !== false) $('#modal').classList.add('hidden'); };
  $('#modal-cancel').onclick = () => $('#modal').classList.add('hidden');
  setTimeout(() => $('#modal-text').focus(), 50);
}
function exportSettings() {
  const { first_run_done, ...rest } = state.settings;
  openModal({
    title: 'Экспорт настроек', sub: 'Скопируйте текст и сохраните его или отправьте другу.',
    text: JSON.stringify(rest, null, 2), ok: 'Скопировать', readonly: true,
    onOk: async (t) => {
      try { await navigator.clipboard.writeText(t); toast('Скопировано'); }
      catch { $('#modal-text').select(); document.execCommand('copy'); toast('Скопировано'); }
    },
  });
}
function importSettings() {
  openModal({
    title: 'Импорт настроек', sub: 'Вставьте текст настроек, экспортированный из Proryv.', ok: 'Импортировать',
    onOk: (t) => {
      let obj;
      try { obj = JSON.parse(t); } catch { toast('Это не похоже на настройки Proryv'); return false; }
      if (typeof obj !== 'object' || !obj || !Array.isArray(obj.enabled_presets)) { toast('Это не похоже на настройки Proryv'); return false; }
      state.settings = { ...state.settings, ...obj, first_run_done: true };
      syncControls(); renderPresets(); renderHero(); markDirty();
      toast('Импортировано — нажмите «Сохранить»');
    },
  });
}

/* ---------- Мастер ---------- */
const WZ_STEPS = 5;
let wzStep = 0;
function showWizard(on) {
  $('#wizard').classList.toggle('hidden', !on);
  $('#app').classList.toggle('hidden', on);
  $('#tb-page').textContent = on ? 'Настройка Proryv' : PAGE_TITLES[$('.sidebar a.active')?.dataset.page || 'home'];
  if (on) wzGo(0);
}
function wzGo(n) {
  wzStep = n;
  $$('.wz-step').forEach((s) => s.classList.toggle('active', +s.dataset.step === n));
  $('#wz-back').disabled = n === 0;
  $('#wz-next').textContent = n === WZ_STEPS - 1 ? 'Готово' : 'Далее';
  $('#wz-progress').style.width = ((n + 1) / WZ_STEPS) * 100 + '%';
  $$('#wz-lang button').forEach((b) => b.classList.toggle('sel', b.dataset.val === state.settings.language));
  if (n === 4) renderConflicts();
}
function bindWizard() {
  $('#wz-back').onclick = () => wzGo(Math.max(0, wzStep - 1));
  $('#wz-next').onclick = async () => {
    if (wzStep < WZ_STEPS - 1) return wzGo(wzStep + 1);
    state.settings.first_run_done = true;
    await save();
    showWizard(false);
    showPage('home');
  };
  $('#wz-lang').addEventListener('click', (e) => {
    if (e.target.dataset.val) { setSetting('language', e.target.dataset.val); wzGo(wzStep); }
  });
}

/* ---------- Сохранение ---------- */
async function save() {
  await call('save_settings', { settings: state.settings });
  state.saved = structuredClone(state.settings);
  markDirty();
  toast('Настройки сохранены');
  setTimeout(renderTg, 300);
}
let toastTimer;
function toast(text) {
  const t = $('#toast');
  t.textContent = text;
  t.classList.remove('hidden');
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => t.classList.add('hidden'), 2200);
}

/* ---------- Обновления ---------- */
async function checkUpdate(manual) {
  const st = $('#upd-status');
  if (manual) { st.textContent = 'Проверяем…'; $('#upd-check').disabled = true; }
  try {
    const u = await call('check_update');
    if (u) {
      $('#upd-banner-text').textContent = `Вышла новая версия ${u.version} (у вас ${u.current}).` + (u.notes ? ' ' + u.notes.split('\n')[0] : '');
      $('#upd-banner').classList.remove('hidden');
      st.textContent = `Доступна версия ${u.version}.`;
      $('#upd-check').textContent = 'Обновить';
      $('#upd-check').onclick = installUpdate;
    } else if (manual) st.textContent = 'У вас последняя версия.';
  } catch (e) {
    if (manual) st.textContent = 'Не удалось проверить: ' + e;
  }
  $('#upd-check').disabled = false;
}
async function installUpdate() {
  $$('#upd-install, #upd-check').forEach((b) => (b.disabled = true));
  $('#upd-banner').classList.remove('hidden');
  $('#upd-banner-text').textContent = 'Скачиваем обновление…';
  try {
    await call('install_update');
  } catch (e) {
    $('#upd-banner-text').textContent = 'Не удалось обновиться: ' + e;
    $$('#upd-install, #upd-check').forEach((b) => (b.disabled = false));
  }
}

/* ---------- Поиск по настройкам ---------- */
// Индекс собирается из самой разметки при каждом запросе — так в него попадают и пресеты, и стратегии.
const norm = (s) => s.toLowerCase().replace(/ё/g, 'е');
function searchIndex() {
  const items = [];
  $$('#app .page').forEach((page) => {
    const pid = page.dataset.page;
    if (pid === 'home') return;
    $$('.card', page).forEach((el) => {
      const b = el.querySelector('b');
      if (!b || el.classList.contains('stat')) return;
      const desc = el.querySelector('small');
      items.push({ el, pid, title: b.textContent.trim(), desc: desc ? desc.textContent.trim() : '', keys: el.dataset.keywords || '' });
    });
    $$('h2', page).forEach((el) => {
      const next = el.nextElementSibling;
      items.push({ el, pid, title: el.textContent.trim(), desc: next && next.classList.contains('sub') ? next.textContent.trim() : '' });
    });
  });
  return items;
}
function hl(text, words) {
  let out = esc(text);
  for (const w of words) {
    const i = norm(text).indexOf(w);
    if (i >= 0) { out = esc(text.slice(0, i)) + '<mark>' + esc(text.slice(i, i + w.length)) + '</mark>' + esc(text.slice(i + w.length)); break; }
  }
  return out;
}
let searchHits = [], searchSel = 0;
function runSearch() {
  const q = norm($('#search-input').value.trim());
  const box = $('#search-results');
  if (!q) { box.classList.add('hidden'); searchHits = []; return; }
  const words = q.split(/\s+/);
  searchHits = searchIndex()
    .map((it) => {
      const t = norm(it.title), hay = t + ' ' + norm(it.desc) + ' ' + norm(it.keys || '') + ' ' + norm(PAGE_TITLES[it.pid]);
      if (!words.every((w) => hay.includes(w))) return null;
      return { ...it, score: words.filter((w) => t.includes(w)).length * 10 + (t.startsWith(words[0]) ? 5 : 0) };
    })
    .filter(Boolean)
    .sort((a, b) => b.score - a.score)
    .slice(0, 10);
  searchSel = 0;
  box.classList.remove('hidden');
  box.innerHTML = searchHits.length
    ? searchHits.map((h, i) => `<button data-i="${i}" class="${i === 0 ? 'sel' : ''}"><span>${esc(PAGE_TITLES[h.pid])}</span>${hl(h.title, words)}${h.desc ? `<small>${hl(h.desc, words)}</small>` : ''}</button>`).join('')
    : '<div class="search-empty">Ничего не нашлось</div>';
}
function openSearchHit(i) {
  const h = searchHits[i];
  if (!h) return;
  closeSearch();
  showPage(h.pid);
  requestAnimationFrame(() => {
    h.el.scrollIntoView({ block: 'center', behavior: state.settings.animations ? 'smooth' : 'auto' });
    h.el.classList.remove('found'); void h.el.offsetWidth; h.el.classList.add('found');
  });
}
function openSearch() {
  if (!$('#wizard').classList.contains('hidden')) return;
  $('#search').classList.remove('hidden');
  $('#tb-page').classList.add('hidden');
  $('#search-input').focus();
  $('#search-input').select();
  runSearch();
}
function closeSearch() {
  $('#search').classList.add('hidden');
  $('#search-results').classList.add('hidden');
  $('#tb-page').classList.remove('hidden');
}
function bindSearch() {
  $('#btn-search').onclick = () => ($('#search').classList.contains('hidden') ? openSearch() : closeSearch());
  const inp = $('#search-input');
  inp.addEventListener('input', runSearch);
  inp.addEventListener('keydown', (e) => {
    const n = searchHits.length;
    if (e.key === 'Escape') closeSearch();
    else if (e.key === 'Enter') openSearchHit(searchSel);
    else if ((e.key === 'ArrowDown' || e.key === 'ArrowUp') && n) {
      e.preventDefault();
      searchSel = (searchSel + (e.key === 'ArrowDown' ? 1 : n - 1)) % n;
      $$('#search-results button').forEach((b, i) => b.classList.toggle('sel', i === searchSel));
      $$('#search-results button')[searchSel].scrollIntoView({ block: 'nearest' });
    }
  });
  $('#search-results').addEventListener('mousedown', (e) => {
    const b = e.target.closest('button[data-i]');
    if (b) { e.preventDefault(); openSearchHit(+b.dataset.i); }
  });
  inp.addEventListener('blur', () => setTimeout(() => { if (document.activeElement !== inp) closeSearch(); }, 150));
  document.addEventListener('keydown', (e) => {
    if ((e.ctrlKey || e.metaKey) && e.code === 'KeyF') { e.preventDefault(); openSearch(); }
  });
}

/* ---------- Окно ---------- */
function bindWindow() {
  const win = TAURI ? TAURI.window.getCurrentWindow() : null;
  $('#win-min').onclick = () => win && win.minimize();
  $('#win-max').onclick = () => win && win.toggleMaximize();
  const close = () => (win ? win.close() : toast('В браузере окно не закрыть'));
  $('#win-close').onclick = close;
  $('#btn-close').onclick = close;
  $('#btn-sidebar').onclick = () => $('#app').classList.toggle('collapsed');
}

/* ---------- Запуск ---------- */
async function init() {
  renderIcons();
  const boot = await call('get_bootstrap');
  Object.assign(state, { presets: boot.presets, strategies: boot.strategies, settings: boot.settings });
  state.saved = structuredClone(boot.settings);
  $('#about-ver').textContent = 'v' + boot.version;

  bindSettings(); bindPresets(); bindWizard(); bindWindow(); bindSearch();
  renderPresets(); renderStrategies(); syncControls(); markDirty();

  $$('.sidebar a').forEach((a) => (a.onclick = () => showPage(a.dataset.page)));
  $('#power').onclick = togglePower;
  $('#btn-save').onclick = save;
  $('#btn-revert').onclick = () => { state.settings = structuredClone(state.saved); syncControls(); renderPresets(); renderHero(); markDirty(); };
  $('#act-wizard').onclick = () => showWizard(true);
  $('#act-reset').onclick = async () => {
    const d = await call('default_settings');
    state.settings = { ...d, first_run_done: true };
    syncControls(); renderPresets(); renderHero(); markDirty();
    toast('Настройки сброшены — нажмите «Сохранить»');
  };
  $('#act-import').onclick = importSettings;
  $('#act-export').onclick = exportSettings;
  $('#at-start').onclick = startAutotune;
  $('#diag-run').onclick = runDiagnose;
  $('#diag-host').addEventListener('keydown', (e) => e.key === 'Enter' && runDiagnose());
  onEvent('autotune', onAutotuneProgress);
  onEvent('autotune-done', onTuneDone);
  onEvent('heal-start', (preset) => toast(`Самолечение: ${presetName(preset)} перестал открываться — подбираем стратегию…`));
  onEvent('status', (st) => { state.status = st; renderHero(); });
  renderAutotunePresets(); renderTunnels(); renderTg();
  setInterval(() => { if ($('.page.active')?.dataset.page === 'presets') renderTg(); }, 3000);
  $('#tg-open').onclick = async () => {
    try { await call('tg_open'); toast('Открываем Telegram — подтвердите добавление прокси'); } catch (e) { toast(String(e)); }
  };
  $('#tg-copy').onclick = async () => {
    try { await navigator.clipboard.writeText(tgLink); toast('Ссылка скопирована — отправьте её себе в «Избранное» и нажмите'); }
    catch { openModal({ title: 'Ссылка для Telegram', sub: 'Скопируйте и откройте в Telegram.', text: tgLink, ok: 'Готово', readonly: true, onOk: () => {} }); }
  };
  $('#tg-test').onclick = tgTest;
  $('#upd-check').onclick = () => checkUpdate(true);
  $('#upd-install').onclick = installUpdate;
  onEvent('update-progress', ([done, total]) => {
    $('#upd-banner-text').textContent = total
      ? `Скачиваем обновление… ${Math.floor((done / total) * 100)}%` : `Скачиваем обновление… ${fmtBytes(done)}`;
  });
  if (state.settings.check_updates) checkUpdate(false);
  $('#pv-host').addEventListener('input', renderPreview);
  $('#pv-strategy').addEventListener('change', renderPreview);

  if (TAURI) state.system = await call('system_info');
  $('#btn-admin').onclick = async () => {
    if (JSON.stringify(state.settings) !== JSON.stringify(state.saved)) await save();
    try { await call('relaunch_as_admin'); } catch (e) { toast(String(e)); }
  };
  state.status = await call('get_status');
  renderHero(); renderPreview(); renderConflicts();
  setInterval(async () => { state.status = await call('get_status'); if (!state.busy) renderHero(); }, 1000);

  let page = 'home';
  try { page = localStorage.getItem('proryv-page') || 'home'; } catch {}
  showPage(PAGE_TITLES[page] ? page : 'home');
  showWizard(!state.settings.first_run_done);
}

init().catch((e) => { document.body.insertAdjacentHTML('beforeend', `<pre style="color:#f88;padding:20px">${esc(e.stack || e)}</pre>`); });
