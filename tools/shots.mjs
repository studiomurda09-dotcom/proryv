// Скриншоты интерфейса для README: node tools/shots.mjs (нужен запущенный tools/serve.mjs).
// Снимает браузерное превью через установленный Microsoft Edge.
import { chromium } from 'playwright-core';
import { mkdirSync } from 'node:fs';

const base = process.env.UI || 'http://127.0.0.1:5174';
const out = new URL('../docs/screenshots/', import.meta.url);
mkdirSync(out, { recursive: true });

const browser = await chromium.launch({ channel: 'msedge' });
const page = await browser.newPage({ viewport: { width: 1130, height: 720 }, deviceScaleFactor: 2 });
for (const lang of ['ru', 'en']) {
  for (const p of ['home', 'presets', 'autotune', 'strategies', 'diag']) {
    await page.goto(`${base}/index.html?shot=1&lang=${lang}&p=${p}#${p}`);
    await page.waitForTimeout(700);
    if (p === 'autotune') {
      await page.click('#at-start');
      await page.waitForTimeout(3600);
    }
    if (p === 'diag') {
      await page.fill('#diag-host', 'youtube.com');
      await page.click('#diag-run');
      await page.waitForTimeout(600);
    }
    // В превью нет настоящего окна — прячем плашку «Просмотр в браузере».
    await page.evaluate(() => document.querySelector('#demo-banner')?.classList.add('hidden'));
    await page.screenshot({ path: new URL(`${lang}-${p}.png`, out).pathname.slice(1) });
    console.log(`${lang}-${p}.png`);
  }
}
await browser.close();
