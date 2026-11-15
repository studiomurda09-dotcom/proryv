'use strict';
/* =====================================================================
   Режиссёр промо-ролика: камера, курсор, титры и сценарий.
   Каждое событие пишется в CUES с таймкодом — это лист для звукорежиссёра.
   ?rec=1 — записать ролик (вкладка снимает сама себя), без него — просто проиграть.
   ===================================================================== */
const $ = (s) => document.querySelector(s);
const W = 1920, H = 1080, AW = 1130, AH = 720;
const win = $('#win'), frame = $('#app'), cur = $('#cursor');
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const clamp = (v, a, b) => Math.min(Math.max(v, a), b);

/* ---------- Таймкоды для звука ---------- */
const CUES = [];
let T0 = performance.now();
const now = () => (performance.now() - T0) / 1000;
function cue(id, what, sound) { CUES.push({ t: +now().toFixed(3), id, what, sound }); SFX.play(id); }

/* ---------- Приложение внутри рамки ---------- */
const doc = () => frame.contentDocument;
const q = (s) => doc().querySelector(s);
const app = () => frame.contentWindow;

/* ---------- Камера ---------- */
let cam = { s: 1.3, tx: 0, ty: 0 };
function setCam(s, tx, ty, ms = 1100) {
  cam = { s, tx, ty };
  win.style.transitionDuration = `${ms}ms, 800ms`;
  win.style.transform = `translate(${tx}px, ${ty}px) scale(${s})`;
  // Курсор «приклеен» к точке в окне — едет вместе с камерой.
  if (anchor) placeCursor(toStage(anchor.x, anchor.y), ms);
}
function overview(ms) { const s = 1.3; setCam(s, (W - AW * s) / 2, (H - AH * s) / 2, ms); }
function rectOf(el) { const r = el.getBoundingClientRect(); return { x: r.left, y: r.top, w: r.width, h: r.height }; }
/** Наехать на элемент: он в центре кадра, окно не уезжает за край. */
function focus(el, s = 1.8, ms) {
  const r = rectOf(el), cx = r.x + r.w / 2, cy = r.y + r.h / 2;
  let tx = W / 2 - s * cx, ty = H / 2 - s * cy;
  tx = AW * s > W ? clamp(tx, W - AW * s, 0) : (W - AW * s) / 2;
  ty = AH * s > H ? clamp(ty, H - AH * s, 0) : (H - AH * s) / 2;
  setCam(s, tx, ty, ms);
}
const toStage = (x, y) => ({ x: cam.tx + cam.s * x, y: cam.ty + cam.s * y });

/* ---------- Курсор ---------- */
let cpos = { x: W / 2, y: H / 2 + 120 }, anchor = null;
function placeCursor(p, ms) {
  cur.style.transitionDuration = ms + 'ms';
  cur.style.transform = `translate(${p.x - 6}px, ${p.y - 3}px)`;
  cpos = p;
}
async function moveTo(el, ox = 0.5, oy = 0.5) {
  const r = rectOf(el);
  anchor = { x: r.x + r.w * ox, y: r.y + r.h * oy };
  const p = toStage(anchor.x, anchor.y);
  const ms = clamp(Math.hypot(p.x - cpos.x, p.y - cpos.y) * 0.75, 380, 950);
  placeCursor(p, ms);
  await sleep(ms + 60);
}
async function click(el, what, opts = {}) {
  await moveTo(el, opts.ox, opts.oy);
  const rp = document.createElement('div');
  rp.className = 'ripple'; rp.style.left = cpos.x + 'px'; rp.style.top = cpos.y + 'px';
  document.body.appendChild(rp); setTimeout(() => rp.remove(), 700);
  cue(opts.id || 'click', what, opts.sound || 'Клик мыши (мягкий, UI)');
  if (opts.mousedown) el.dispatchEvent(new MouseEvent('mousedown', { bubbles: true, cancelable: true }));
  else (opts.target || el).click();
  await sleep(opts.after ?? 350);
}
async function typeText(input, text, what) {
  input.focus();
  input.value = '';
  input.dispatchEvent(new Event('input', { bubbles: true }));
  cue('typing_start', `${what}: начало набора «${text}» (${text.length} клавиш)`, 'Клавиатура: серия нажатий, ~9 в секунду');
  for (const ch of text) {
    input.value += ch;
    SFX.key();
    input.dispatchEvent(new Event('input', { bubbles: true }));
    await sleep(95 + Math.random() * 40);
  }
  cue('typing_end', `${what}: набор закончен`, '—');
}

/* ---------- Титры ---------- */
async function caption(title, sub) {
  const c = $('#caption');
  if (c.classList.contains('on')) { c.classList.remove('on'); await sleep(450); }
  $('#cap-t').textContent = title; $('#cap-s').textContent = sub;
  c.classList.add('on');
  cue('caption_in', `Титр: «${title}»`, 'Мягкий свуш появления текста');
}
async function captionOff() { $('#caption').classList.remove('on'); cue('caption_out', 'Титр уходит', 'Тихий обратный свуш (по желанию)'); await sleep(400); }

/* ---------- Запись вкладки ---------- */
let rec = null, chunks = [], mime = '';
async function startRecording() {
  const screen = await navigator.mediaDevices.getDisplayMedia({
    video: { frameRate: 60, width: W, height: H }, audio: false, preferCurrentTab: true,
  });
  // Видео вкладки + звуки из синтезатора — одной записью, синхронно.
  const stream = new MediaStream([...screen.getVideoTracks(), ...SFX.stream.getAudioTracks()]);
  mime = ['video/mp4;codecs=avc1.640028,mp4a.40.2', 'video/mp4', 'video/webm;codecs=vp9,opus'].find((m) => MediaRecorder.isTypeSupported(m));
  rec = new MediaRecorder(stream, { mimeType: mime, videoBitsPerSecond: 12_000_000, audioBitsPerSecond: 192_000 });
  rec.ondataavailable = (e) => e.data.size && chunks.push(e.data);
  const started = new Promise((r) => (rec.onstart = r));
  rec.start(1000);
  await started;
}
async function stopRecording() {
  const done = new Promise((r) => (rec.onstop = r));
  rec.stop();
  await done;
  rec.stream.getTracks().forEach((t) => t.stop());
  const blob = new Blob(chunks, { type: mime });
  const a = document.createElement('a');
  a.href = URL.createObjectURL(blob);
  a.download = mime.startsWith('video/mp4') ? 'proryv-promo.mp4' : 'proryv-promo.webm';
  document.body.appendChild(a); a.click();
}

/* ---------- Сценарий ---------- */
async function play() {
  // Приложение в рамке: прячем плашку браузерного превью.
  const st = doc().createElement('style');
  st.textContent = '#demo-banner{display:none!important}';
  doc().head.appendChild(st);

  // Стартовая позиция окна: чуть крупнее и ниже, невидимое.
  setCam(1.12, (W - AW * 1.12) / 2, (H - AH * 1.12) / 2 + 140, 0);
  await sleep(300);

  await SFX.init();
  if (new URLSearchParams(location.search).get('rec') === '1') await startRecording();
  T0 = performance.now();

  // 1. Заставка
  $('#intro').classList.add('on');
  cue('intro', 'Логотип и название появляются', 'Удар + блестящий «шиммер», задаёт тон ролика');
  await sleep(2800);
  $('#intro').classList.remove('on');
  cue('intro_out', 'Заставка уходит', 'Свуш на уход');
  await sleep(400);

  // 2. Окно влетает
  win.style.opacity = 1; overview(1200);
  cue('window_in', 'Окно программы выезжает снизу и встаёт по центру', 'Плотный свуш с лёгким «бэйсом» в конце');
  await sleep(1300);
  cur.style.opacity = 1; placeCursor({ x: W / 2 + 260, y: H - 140 }, 0);

  // 3. Включение
  await caption('Одна кнопка', 'Включил — и YouTube, Discord, Telegram открываются');
  focus(q('.hero'), 1.75); await sleep(1100);
  await click(q('#power'), 'Нажатие большой кнопки включения', { id: 'power_click', sound: 'Тяжёлый клик кнопки питания' });
  await sleep(500);
  cue('power_on', 'Кнопка загорается, «Обход включён», счётчики начинают расти', 'Включение: восходящий электрический «вжух» + мягкий гул');
  await sleep(1300);
  focus(q('.stats'), 1.55); await sleep(1900);
  cue('stats_tick', 'Счётчики соединений и фейков бегут', 'Тихие цифровые тики (луп ~2 с)');
  await sleep(1000);
  await captionOff();

  // 4. Автоподбор
  overview(900); await sleep(950);
  await click(q('.sidebar a[data-page="autotune"]'), 'Переход в «Автоподбор»');
  await caption('Автоподбор под провайдера', 'Сам находит дыру в DPI именно вашего провайдера');
  focus(q('#at-start').closest('.card'), 1.6); await sleep(1100);
  const results = q('#at-results');
  const mo = new MutationObserver((list) => list.forEach((m) => m.addedNodes.forEach((n) => {
    if (n.nodeType === 1 && n.classList.contains('at-row')) cue('row', `Строка результата: ${n.querySelector('b').textContent}`, 'Короткий «блип» проверки');
  })));
  mo.observe(results, { childList: true });
  await click(q('#at-start'), 'Нажатие «Подобрать»', { sound: 'Клик + старт «сканирования» (тихий нарастающий тон)' });
  focus(q('#at-start').closest('.page'), 1.35, 900);
  await sleep(3000);
  mo.disconnect();
  await sleep(250);
  cue('success', 'Подсветка лучшей стратегии и плашка «Готово»', 'Успех: светлый аккорд / чайм');
  focus(q('#at-summary'), 1.7); await sleep(1900);
  await caption('Самолечение', 'Провайдер что-то поменял — Proryv сам переподберёт стратегию');
  overview(900); await sleep(700);
  app().toast('Самолечение: YouTube → «ALT11: фейки (ts) + наложение»');
  cue('toast', 'Всплывает уведомление «Самолечение»', 'Мягкий «поп» уведомления');
  await sleep(2400);
  await captionOff();

  // 5. Пресеты, голос Discord и Telegram
  await click(q('.sidebar a[data-page="presets"]'), 'Переход в «Пресеты»');
  await caption('Discord с голосом, Roblox, Telegram', 'Нужное включается переключателем');
  focus(q('#preset-list'), 1.45); await sleep(1100);
  const roblox = q('input[data-preset="roblox"]').closest('.toggle');
  await click(roblox, 'Переключатель Roblox — вкл', { id: 'toggle_on', sound: 'Щелчок тумблера (вкл)' });
  await sleep(500);
  const content = q('#content');
  cue('scroll', 'Прокрутка к блоку «Дополнительно»', 'Короткий свуш прокрутки');
  content.scrollTo({ top: content.scrollHeight, behavior: 'smooth' });
  await sleep(900);
  focus(q('.tg-card'), 1.7); await sleep(1100);
  await click(q('.tg-card .toggle'), 'Переключатель Telegram — вкл', { id: 'toggle_on', sound: 'Щелчок тумблера (вкл)' });
  await sleep(1600);
  await captionOff();

  // 6. Стратегии
  overview(900); await sleep(400);
  await click(q('.sidebar a[data-page="strategies"]'), 'Переход в «Стратегии»');
  await caption('Видно, как режется пакет', 'Фейки и части ClientHello — наглядно');
  focus(q('.card.preview'), 1.55); await sleep(1100);
  await click(q('#pv-host'), 'Клик в поле адреса', { after: 150 });
  await typeText(q('#pv-host'), 'discord.com', 'Поле адреса');
  const sel = q('#pv-strategy');
  await click(sel, 'Выбор стратегии ALT11', { after: 200 });
  sel.value = 'alt11'; sel.dispatchEvent(new Event('change', { bubbles: true }));
  cue('bar_redraw', 'Полоса сегментов перестраивается: много фейков', 'Цифровой «перещёлк» / глитч');
  await sleep(2200);
  await captionOff();

  // 7. Поиск
  overview(900); await sleep(950);
  await caption('Поиск по всем настройкам', 'Ctrl+F — и сразу к нужному');
  doc().dispatchEvent(new KeyboardEvent('keydown', { code: 'KeyF', key: 'f', ctrlKey: true, bubbles: true }));
  cue('search_open', 'Открывается строка поиска (Ctrl+F)', 'Короткий «поп»');
  await moveTo(q('#search-input'));
  await typeText(q('#search-input'), 'голос', 'Поиск');
  cue('search_results', 'Выпадают результаты поиска', 'Лёгкий шелест списка');
  await sleep(1000);
  await click(q('#search-results button'), 'Клик по результату', { after: 100, mousedown: true });
  cue('jump', 'Переход на страницу и подсветка найденной настройки', 'Свуш перехода + «дзынь» подсветки');
  await sleep(500);
  const found = q('.card.found');
  if (found) { focus(found, 1.7); }
  await sleep(1800);
  await captionOff();

  // 8. Английский
  overview(900); await sleep(400);
  await click(q('.sidebar a[data-page="appearance"]'), 'Переход во «Внешний вид»');
  await caption('Русский и English', 'Язык меняется на лету');
  const lang = q('select[data-setting="language"]');
  focus(lang.closest('.card'), 1.7); await sleep(1100);
  await click(lang, 'Выбор языка', { after: 300 });
  lang.value = 'en'; lang.dispatchEvent(new Event('change', { bubbles: true }));
  cue('lang_switch', 'Весь интерфейс мгновенно становится английским', 'Глитч/«перелистывание» + свуш');
  await sleep(700);
  overview(1000); await sleep(1900);
  await captionOff();

  // 9. Финал
  cur.style.opacity = 0;
  setCam(1.0, (W - AW) / 2, (H - AH) / 2 + 60, 1000); win.style.opacity = 0;
  cue('window_out', 'Окно отъезжает и растворяется', 'Обратный свуш');
  await sleep(700);
  $('#outro').classList.add('on');
  cue('outro', 'Финальный экран: логотип, «Бесплатно · Открытый код», ссылки на GitHub и Discord', 'Финальный удар + хвост (риверб), можно с лого-звуком');
  await sleep(4200);
  cue('end', 'Конец ролика', '—');

  if (rec) await stopRecording();
  window.PROMO_DONE = { cues: CUES, mime, length: now() };
}

frame.addEventListener('load', () => setTimeout(() => play().catch((e) => { window.PROMO_ERROR = String(e.stack || e); console.error(e); }), 600));
