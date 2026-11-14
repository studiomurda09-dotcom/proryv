// Запись промо-ролика: node tools/record-promo.mjs (нужен запущенный tools/serve.mjs).
// Результат: promo/out/proryv-promo.mp4 и лист таймкодов для звука (sfx-cues.md, sfx-cues.csv).
import { chromium } from 'playwright-core';
import { mkdirSync, writeFileSync, unlinkSync } from 'node:fs';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const base = process.env.UI || 'http://127.0.0.1:5174';
const outDir = fileURLToPath(new URL('../promo/out/', import.meta.url));
mkdirSync(outDir, { recursive: true });

const browser = await chromium.launch({
  channel: 'msedge',
  headless: process.env.HEADED ? false : true,
  args: ['--auto-accept-this-tab-capture', '--force-device-scale-factor=1', '--window-size=1944,1219'], // в невидимом окне Edge рамка съедает 24×139 — остаётся ровно 1920×1080
});
const page = await browser.newPage({ viewport: null, acceptDownloads: true });
page.on('console', (m) => m.type() === 'error' && console.log('console:', m.text()));
page.on('crash', () => console.log('страница упала'));
page.on('pageerror', (e) => console.log('ошибка на странице:', e.message));
await page.goto(`${base}/promo/index.html?rec=1`);

const download = await page.waitForEvent('download', { timeout: 240_000 });
const raw = outDir + 'raw-' + download.suggestedFilename();
await download.saveAs(raw);
const size = await page.evaluate(() => [innerWidth, innerHeight]);
if (size[0] !== 1920 || size[1] !== 1080) console.log('Внимание: кадр', size.join('×'), '— поправьте --window-size');
const done = await page.evaluate(() => window.PROMO_DONE || { error: window.PROMO_ERROR });
await browser.close();
if (done.error) throw new Error(done.error);

// Браузер пишет фрагментированный MP4 — перепаковываем в обычный.
const file = outDir + download.suggestedFilename();
execFileSync(process.execPath, [fileURLToPath(new URL('remux.mjs', import.meta.url)), raw, file], { stdio: 'inherit' });
unlinkSync(raw);

const tc = (t) => {
  const m = Math.floor(t / 60), s = t - m * 60;
  return `${String(m).padStart(2, '0')}:${s.toFixed(2).padStart(5, '0')}`;
};
const rows = done.cues;
const md = [
  '# Proryv — промо-ролик: лист звуковых эффектов',
  '',
  `Видео: \`${download.suggestedFilename()}\`, 1920×1080, ${done.length.toFixed(1)} с. Таймкоды от начала видео (мм:сс.сс).`,
  'Колонка «Звук» — пожелание, не требование. Музыка — на усмотрение, темп ~110–120 BPM, электроника/синт.',
  '',
  '| # | Таймкод | Событие на экране | Звук |',
  '|---|---|---|---|',
  ...rows.map((c, i) => `| ${i + 1} | ${tc(c.t)} | ${c.what} | ${c.sound} |`),
  '',
].join('\n');
writeFileSync(outDir + 'sfx-cues.md', md);
writeFileSync(outDir + 'sfx-cues.csv', '﻿' + ['n;timecode;seconds;id;event;sound', ...rows.map((c, i) => [i + 1, tc(c.t), c.t, c.id, c.what, c.sound].map((v) => `"${String(v).replace(/"/g, '""')}"`).join(';'))].join('\r\n'));
console.log(`${file}\n${rows.length} событий, ${done.length.toFixed(1)} с, ${done.mime}`);
