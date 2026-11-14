// Мини-сервер для просмотра интерфейса в браузере без Tauri: node tools/serve.mjs
import { createServer } from 'node:http';
import { readFile } from 'node:fs/promises';
import { extname, join, normalize } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(fileURLToPath(new URL('.', import.meta.url)), '..', 'ui');
// Промо-ролик лежит отдельно, чтобы не попасть в установщик.
const promoRoot = join(fileURLToPath(new URL('.', import.meta.url)), '..', 'promo');
const types = { '.html': 'text/html', '.css': 'text/css', '.js': 'text/javascript', '.mjs': 'text/javascript', '.svg': 'image/svg+xml', '.json': 'application/json', '.png': 'image/png', '.mp4': 'video/mp4' };
const port = Number(process.env.PORT) || 5173;

createServer(async (req, res) => {
  const path = normalize(decodeURIComponent(new URL(req.url, 'http://x').pathname)).replace(/^([\\/])+/, '');
  const promo = path.startsWith('promo') && (path[5] === '\\' || path[5] === '/');
  const base = promo ? promoRoot : root;
  const file = promo ? join(promoRoot, path.slice(6)) : join(root, path || 'index.html');
  if (!file.startsWith(base)) return res.writeHead(403).end();
  try {
    const body = await readFile(file);
    res.writeHead(200, { 'content-type': (types[extname(file)] || 'application/octet-stream') + '; charset=utf-8', 'cache-control': 'no-store' });
    res.end(body);
  } catch {
    res.writeHead(404).end('not found');
  }
}).listen(port, '127.0.0.1', () => console.log(`UI: http://127.0.0.1:${port}`));
