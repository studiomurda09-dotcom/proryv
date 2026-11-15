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
  // ?shot=1&lang=en — чистые настройки для скриншотов и видео, без мастера.
  const shot = new URLSearchParams(location.search);
  const load = () => {
    if (shot.has('shot')) return { first_run_done: true, language: shot.get('lang') || 'ru' };
    try { return JSON.parse(localStorage.getItem(KEY)) || null; } catch { return null; }
  };
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
    let cuts = [], disorder = false, segs = [], payloadLen = len;
    for (const s of strategy.steps) {
      if (s.kind === 'split') cuts.push(pos(s.at));
      if (s.kind === 'multi_split') cuts.push(...s.at.map(pos));
      if (s.kind === 'disorder') { cuts.push(pos(s.at)); disorder = true; }
      if (s.kind === 'multi_disorder') { cuts.push(...s.at.map(pos)); disorder = true; }
      if (s.kind === 'tls_record_split') payloadLen += 5;
    }
    cuts = [...new Set(cuts.filter((c) => c != null))].sort((a, b) => a - b);
    const pool = ['www.google.com', 'yandex.ru', 'vk.com', 'www.microsoft.com', 'ok.ru'];
    for (const s of strategy.steps) {
      if (s.kind !== 'fake') continue;
      for (let r = 0; r < Math.max(1, s.repeats); r++) {
        const name = s.sni.mode === 'fixed' ? s.sni.value : pool[(r * 3 + host.length) % pool.length];
        segs.push({ kind: 'fake', seq_offset: 0, data: { length: Math.max(len, helloLen(name)) },
          ttl: s.fooling.includes('ttl') ? s.ttl : null, fooling: s.fooling, label: `фейк #${r + 1} (${name})` });
      }
    }
    const b = [0, ...cuts, payloadLen];
    let real = [];
    for (let i = 0; i < b.length - 1; i++)
      real.push({ kind: 'real', seq_offset: b[i], data: { length: b[i + 1] - b[i] }, ttl: null, fooling: [], label: `часть ${i + 1} [${b[i]}..${b[i + 1]})` });
    if (disorder && real.length > 1) real.reverse();
    for (const s of strategy.steps) {
      if (s.kind !== 'seq_overlap') continue;
      const f = real.find((r) => r.seq_offset === 0);
      if (f) { f.data = { length: f.data.length + s.len }; f.label += ` + seqovl ${s.len} Б (${s.sni})`; }
    }
    return { proto: 'tls', host, payload_len: payloadLen, segments: [...segs, ...real] };
  }

  const status = () => ({
    state: enabled ? 'running' : 'stopped', presets: [], message: null, demo: true,
    stats: (() => {
      // В режиме съёмки счётчики растут, как в настоящей работе.
      const sec = enabled ? (Date.now() - started) / 1000 : 0, live = shot.has('shot');
      return { connections: live ? Math.floor(sec * 2.6) : 0, desynced: live ? Math.floor(sec * 2.1) : 0,
        fakes_sent: live ? Math.floor(sec * 17) : 0, uptime_secs: Math.floor(sec),
        last_host: live && sec > 1 ? ['rr3---sn-4g5edn6r.googlevideo.com', 'discord.com', 'i.ytimg.com'][Math.floor(sec / 3) % 3] : undefined };
    })(),
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
    isp_info: () => ({ isp: { asn: 64512, name: 'Example Telecom' }, entry: { asn: 64512, name: 'Example Telecom', strategies: { youtube: 'alt11' } }, base_version: 1, done: true }),
    open_github: () => null,
    open_link: () => null,
    zapret_parse: ({ text }) => {
      if (!/--dpi-desync=/.test(text)) throw 'Не нашёл ни одного TCP-блока с --dpi-desync';
      const alt11 = M.strategies.find((s) => s.id === 'alt11');
      return [{ label: 'TCP 443 · list-google', zapret: text.trim(), recognized: ['fake ×8', 'fooling=ts', 'multisplit @1', 'seqovl 681 (www.google.com)'],
        warnings: /autottl/.test(text) ? ['--dpi-desync-autottl пока не поддерживается — пропущен'] : [],
        strategy: { ...alt11, id: 'custom_demo', name: 'TCP 443 · list-google', description: text.trim() } }];
    },
    preview_strategy: ({ host, strategy }) => plan(host, strategy),
    dev_log: ({ since }) => {
      const hosts = ['rr3---sn-4g5e6nsz.googlevideo.com', 'i.ytimg.com', 'gateway.discord.gg', 'www.youtube.com'];
      const n = enabled && Math.random() < 0.6 ? 1 : 0;
      return [since + n, n ? [`${hosts[since % 4]} → youtube / ALT11: фейки (ts) + наложение · 8 фейков + 2 части`] : []];
    },
    dev_check: () => [6, 6],
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
  diag: 'Диагностика', appearance: 'Внешний вид', general: 'Общие настройки', dev: 'Разработчик', about: 'О программе',
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
  applyDevMode();
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
  if (key === 'language') applyLang(value);
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
    const extra = [s.ttl != null && 'TTL ' + s.ttl, s.fooling.filter((f) => f !== 'ttl').join('+')].filter(Boolean).join(', ');
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
    box.innerHTML = `<span class="verdict ok">Готово</span><div class="grow">Лучшая стратегия: <b>${esc(o.best_name)}</b>. Она сохранена для пресета и уже применяется.</div>
      <button class="btn" id="at-share">Поделиться результатом</button>`;
    $('#at-share').onclick = () => shareResult(o);
  } else if (o.baseline_ok === o.of) {
    box.innerHTML = `<span class="verdict ok">Без блокировки</span><div>Сайты открываются и без обхода — менять ничего не нужно.</div>`;
  } else {
    box.innerHTML = `<span class="verdict dpi">Не нашли</span><div>Ни одна стратегия не помогла. Возможно, сайт заблокирован по IP — проверьте его в «Диагностике».</div>`;
  }
  if (o.direct_ip === null && TAURI) box.insertAdjacentHTML('beforeend', '<small>Не нашёлся прямой адаптер — проверка могла идти через VPN.</small>');
  // Подбор сохранил стратегию в настройках — подтягиваем их.
  await refreshOverrides();
}

/* ---------- Провайдер и база ---------- */
let ispInfo = null;
function renderIsp(info) {
  ispInfo = info;
  const name = $('#isp-name'), sub = $('#isp-sub'), tag = $('#isp-tag');
  if (!info.isp) {
    if (!info.done) return;
    name.textContent = 'Не удалось определить провайдера';
    sub.textContent = 'Проверка шла через VPN или без интернета. Стратегии подбираются как обычно.';
    tag.textContent = '';
    return;
  }
  name.textContent = `${info.isp.name} · AS${info.isp.asn}`;
  tag.textContent = info.entry ? 'есть в базе' : 'нет в базе';
  sub.textContent = info.entry
    ? 'Для «Авто» берутся стратегии, которые сработали у других пользователей этого провайдера.'
    : 'Для этого провайдера в базе пока нет стратегий. Запустите автоподбор и поделитесь результатом.';
}
async function onBaseUpdated(info) {
  renderIsp(info);
  const boot = await call('get_bootstrap');
  state.presets = boot.presets; state.strategies = boot.strategies;
  const pv = $('#pv-strategy').value;
  renderPresets(); renderStrategies(); renderAutotunePresets(); renderHero();
  $('#pv-strategy').value = pv || 'seqovl_google';
}
/** Результат подбора — в GitHub Issues, оттуда он попадает в общую базу. */
function shareResult(o) {
  const isp = ispInfo && ispInfo.isp;
  const title = `Стратегия: ${isp ? isp.name + ' AS' + isp.asn : 'провайдер не определён'} — ${o.preset} → ${o.best}`;
  const body = [
    `Провайдер: ${isp ? isp.name : '—'}`, `AS: ${isp ? isp.asn : '—'}`, `Пресет: ${o.preset}`, `Лучшая стратегия: ${o.best} (${o.best_name})`,
    `Без обхода открылось: ${o.baseline_ok}/${o.of}`, `Версия Proryv: ${$('#about-ver').textContent}`, '',
    'Все результаты:', ...(o.results || []).map((r) => `- ${r.strategy}: ${r.ok}/${r.of}, ${(r.ms / 1000).toFixed(1)} с`),
  ].join('\n');
  call('open_github', { path: `issues/new?labels=strategy&title=${encodeURIComponent(title)}&body=${encodeURIComponent(body)}` })
    .catch((e) => toast(String(e)));
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

/* ---------- Разработчик ---------- */
let devParsed = [], devSince = 0, devPaused = false, devLines = [];
const PRESET_NAMES = { youtube: 'YouTube', discord: 'Discord', general: 'Прочие' };

function applyDevMode() {
  const on = !!state.settings.dev_mode;
  $('#nav-dev').classList.toggle('hidden', !on);
  if (!on && $('.page.active')?.dataset.page === 'dev') showPage('general');
}
function devCurrent() { return devParsed[+$('#dev-block').value || 0]; }
function devBar(plan) {
  return plan.segments.map((s) => `<div class="${s.kind === 'fake' ? 'f' : 'r'}" style="flex:${Math.max(s.data.length, 20)}" title="${esc(s.label)}">${s.data.length > 60 ? s.data.length : ''}</div>`).join('');
}
async function devShowBlock() {
  const p = devCurrent();
  if (!p) return;
  $('#dev-chips').innerHTML = p.recognized.map((r) => `<span class="chip2 ok">${esc(r)}</span>`).join('')
    + p.warnings.map((w) => `<span class="chip2 warn">${esc((w.match(/--[\w-]+|«[^»]+»|\w+=\w+|режим \w+/) || ['⚠'])[0])}</span>`).join('');
  $('#dev-warn').innerHTML = p.warnings.map((w) => `<div>⚠ <span>${esc(w)}</span></div>`).join('');
  $('#dev-name').value = p.strategy.name;
  $('#dev-result').textContent = '';
  try { $('#dev-bar').innerHTML = devBar(await call('preview_strategy', { host: 'www.youtube.com', strategy: p.strategy })); } catch {}
}
let devParseTimer;
function devParse() {
  clearTimeout(devParseTimer);
  devParseTimer = setTimeout(async () => {
    const text = $('#dev-z').value.trim();
    $('#dev-err').classList.add('hidden');
    if (!text) { $('#dev-parsed').classList.add('hidden'); return; }
    try {
      devParsed = await call('zapret_parse', { text });
    } catch (e) {
      devParsed = [];
      $('#dev-parsed').classList.add('hidden');
      $('#dev-err').textContent = String(e);
      $('#dev-err').classList.remove('hidden');
      return;
    }
    const sel = $('#dev-block');
    sel.innerHTML = devParsed.map((p, i) => `<option value="${i}">${esc(p.label)}</option>`).join('');
    sel.classList.toggle('hidden', devParsed.length < 2);
    $('#dev-parsed').classList.remove('hidden');
    devShowBlock();
  }, 250);
}
function devStrategy() {
  const p = devCurrent();
  return { ...p.strategy, name: $('#dev-name').value.trim() || p.strategy.name };
}
async function devSaveCustom(list) {
  state.settings.custom_strategies = list;
  await save();
  const boot = await call('get_bootstrap');
  state.presets = boot.presets; state.strategies = boot.strategies;
  renderPresets(); renderStrategies(); renderDevList();
}
function devUpsert(item) {
  const list = (state.settings.custom_strategies || []).filter((c) => c.strategy.id !== item.strategy.id);
  return [...list, item];
}
async function devSave() {
  const p = devCurrent();
  if (!p) return;
  const old = (state.settings.custom_strategies || []).find((c) => c.strategy.id === p.strategy.id);
  await devSaveCustom(devUpsert({ strategy: devStrategy(), zapret: p.zapret, enabled: true, result: old ? old.result : null }));
}
async function devCheck(strategy, preset, out) {
  out.textContent = 'Проверяем…';
  try {
    const [ok, of] = await call('dev_check', { preset, strategy });
    const text = `${PRESET_NAMES[preset] || preset} ${ok}/${of}`;
    out.textContent = text;
    const saved = (state.settings.custom_strategies || []).find((c) => c.strategy.id === strategy.id);
    if (saved) await devSaveCustom(devUpsert({ ...saved, result: text }));
    return text;
  } catch (e) { out.textContent = String(e); }
}
function renderDevList() {
  const list = state.settings.custom_strategies || [];
  $('#dev-count').textContent = list.length;
  $('#dev-list').innerHTML = list.map((c, i) => {
    const r = c.result, cls = !r ? '' : /\b(\d+)\/\1\b/.test(r) ? 'ok' : / 0\//.test(r) ? 'bad' : '';
    return `<div class="card strat" data-i="${i}">
      <div><b>${esc(c.strategy.name)}</b><div class="z" title="${esc(c.zapret)}">${esc(c.zapret || c.strategy.description)}</div></div>
      <span class="tag ${cls}">${esc(r || 'не проверялась')}</span>
      <button class="icon-btn sm" data-del="${i}" title="Удалить" data-icon="x"></button>
      <label class="toggle"><input type="checkbox" data-custom-on="${i}" ${c.enabled ? 'checked' : ''}><i></i></label>
    </div>`;
  }).join('');
  renderIcons($('#dev-list'));
}
function devLogLine(l) {
  const t = new Date().toTimeString().slice(0, 8);
  return { t, l, bad: /ПОВТОР/.test(l) };
}
function renderDevLog() {
  const f = $('#dev-filter').value.trim().toLowerCase();
  const rows = devLines.filter((x) => !f || x.l.toLowerCase().includes(f));
  const box = $('#dev-log');
  const atBottom = box.scrollTop + box.clientHeight >= box.scrollHeight - 20;
  box.innerHTML = rows.length
    ? rows.map((x) => `<div class="${x.bad ? 'e' : ''}"><span class="t">${x.t}</span> ${esc(x.l)}</div>`).join('')
    : `<div class="t">${state.status && state.status.state === 'running' ? 'Откройте сайт — здесь появятся строки.' : 'Включите обход и откройте сайт — здесь появятся строки.'}</div>`;
  if (atBottom) box.scrollTop = box.scrollHeight;
}
async function pollDevLog() {
  if (!state.settings.dev_mode || devPaused || $('.page.active')?.dataset.page !== 'dev') return;
  const st = state.status && state.status.stats;
  if (st) $('#dev-stats').innerHTML = `соединений <b>${st.connections}</b> · обработано <b>${st.desynced}</b> · фейков <b>${st.fakes_sent}</b>`;
  try {
    const [next, lines] = await call('dev_log', { since: devSince });
    devSince = next;
    if (lines.length) {
      devLines.push(...lines.map(devLogLine));
      devLines = devLines.slice(-400);
      renderDevLog();
    }
  } catch {}
}
function bindDev() {
  $('#dev-z').addEventListener('input', devParse);
  $('#dev-block').addEventListener('change', devShowBlock);
  $('#dev-bat').onclick = () => $('#dev-file').click();
  $('#dev-file').addEventListener('change', async (e) => {
    const f = e.target.files[0];
    if (!f) return;
    $('#dev-z').value = await f.text();
    e.target.value = '';
    devParse();
  });
  $('#dev-save').onclick = devSave;
  $('#dev-check').onclick = () => { const p = devCurrent(); if (p) devCheck(devStrategy(), $('#dev-preset').value, $('#dev-result')); };
  $('#dev-list').addEventListener('change', (e) => {
    const i = e.target.dataset.customOn;
    if (i === undefined) return;
    const list = [...state.settings.custom_strategies];
    list[+i] = { ...list[+i], enabled: e.target.checked };
    devSaveCustom(list);
  });
  $('#dev-list').addEventListener('click', (e) => {
    const b = e.target.closest('[data-del]');
    if (!b) return;
    const list = state.settings.custom_strategies.filter((_, i) => i !== +b.dataset.del);
    state.settings.strategy_override = state.settings.strategy_override.filter(([, s]) => list.some((c) => c.strategy.id === s) || !s.startsWith('custom_'));
    devSaveCustom(list);
  });
  $('#dev-export').onclick = () => openModal({
    title: 'Свои стратегии', sub: 'Скопируйте и отправьте другу или в Discord.', text: JSON.stringify(state.settings.custom_strategies || [], null, 2),
    ok: 'Скопировать', readonly: true,
    onOk: async (t) => { try { await navigator.clipboard.writeText(t); toast('Скопировано'); } catch {} },
  });
  $('#dev-import').onclick = () => openModal({
    title: 'Импорт стратегий', sub: 'Вставьте экспорт своих стратегий из Proryv.', ok: 'Импортировать',
    onOk: async (t) => {
      let arr;
      try { arr = JSON.parse(t); } catch { toast('Это не похоже на стратегии Proryv'); return false; }
      if (!Array.isArray(arr) || !arr.every((c) => c && c.strategy && Array.isArray(c.strategy.steps))) { toast('Это не похоже на стратегии Proryv'); return false; }
      let list = state.settings.custom_strategies || [];
      for (const c of arr) list = [...list.filter((x) => x.strategy.id !== c.strategy.id), { ...c, enabled: c.enabled !== false }];
      await devSaveCustom(list);
      toast(`Добавлено: ${arr.length}`);
    },
  });
  $('#dev-filter').addEventListener('input', renderDevLog);
  $('#dev-pause').onclick = () => {
    devPaused = !devPaused;
    $('#dev-pause').textContent = devPaused ? 'Продолжить' : 'Пауза';
    $('#dev-live').classList.toggle('off', devPaused);
    $('#dev-live').lastChild.textContent = devPaused ? 'пауза' : 'запись';
  };
  $('#dev-clear').onclick = () => { devLines = []; renderDevLog(); };
  $('#dev-copy').onclick = async () => {
    try { await navigator.clipboard.writeText(devLines.map((x) => `${x.t} ${x.l}`).join('\n')); toast('Скопировано'); } catch {}
  };
  $('#dev-docs').onclick = () => call('open_link', { kind: 'zapret_docs' }).catch((e) => toast(String(e)));
  setInterval(pollDevLog, 700);
  renderDevList();
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
  applyLang(boot.settings.language);

  bindSettings(); bindPresets(); bindWizard(); bindWindow(); bindSearch(); bindDev();
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
  onEvent('base-updated', onBaseUpdated);
  call('isp_info').then(renderIsp).catch(() => {});
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
  $$('[data-discord]').forEach((b) => (b.onclick = () => call('open_link', { kind: 'discord' }).catch((e) => toast(String(e)))));
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
  if (PAGE_TITLES[location.hash.slice(1)]) page = location.hash.slice(1);
  showPage(PAGE_TITLES[page] ? page : 'home');
  showWizard(!state.settings.first_run_done);
}

init().catch((e) => { document.body.insertAdjacentHTML('beforeend', `<pre style="color:#f88;padding:20px">${esc(e.stack || e)}</pre>`); });
