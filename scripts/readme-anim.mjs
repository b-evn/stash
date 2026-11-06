// Records an animated README picture: loads docs/readme-art.html?shot=<name>&theme=<theme> in headless
// Chrome, steps the scene's clock with window.setT(seconds) and screenshots every frame, then packs the
// frames into an animated WebP with ffmpeg. Scenes that animate define window.SCENE = { duration, fps }.
//   node scripts/readme-anim.mjs hero dark docs/hero-dark.webp
// Needs Node 22+, ffmpeg built with libwebp, and Chrome (CHROME to point at another Chromium).
import { spawn, spawnSync } from 'node:child_process';
import { mkdtempSync, writeFileSync, rmSync, statSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const [shot, theme = 'dark', out] = process.argv.slice(2);
if (!shot || !out) { console.error('usage: readme-anim.mjs <shot> <dark|light> <out.webp> [scale]'); process.exit(1); }
const SCALE = Number(process.argv[5] || process.env.SCALE || 1.5);
const BASE = process.env.BASE || 'http://localhost:8765';
const CHROME = process.env.CHROME || (process.platform === 'win32'
  ? 'C:/Program Files/Google/Chrome/Application/chrome.exe'
  : process.platform === 'darwin' ? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome' : 'google-chrome');

const tmp = mkdtempSync(join(tmpdir(), 'stash-anim-'));
const port = 9300 + Math.floor(Math.random() * 500);
const chrome = spawn(CHROME, ['--headless=new', `--remote-debugging-port=${port}`, `--user-data-dir=${join(tmp, 'profile')}`,
  '--hide-scrollbars', '--no-first-run', '--no-default-browser-check', 'about:blank'], { stdio: 'ignore' });
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

let ws, id = 0;
const pending = new Map();
const send = (method, params = {}, sessionId) => new Promise((resolve, reject) => {
  const n = ++id;
  pending.set(n, { resolve, reject });
  ws.send(JSON.stringify({ id: n, method, params, sessionId }));
});

try {
  let info;
  for (let i = 0; i < 50 && !info; i++) {
    try { info = await (await fetch(`http://127.0.0.1:${port}/json/version`)).json(); } catch { await sleep(200); }
  }
  ws = new WebSocket(info.webSocketDebuggerUrl);
  await new Promise((r) => ws.addEventListener('open', r));
  ws.addEventListener('message', (e) => {
    const m = JSON.parse(e.data);
    const p = m.id && pending.get(m.id);
    if (!p) return;
    pending.delete(m.id);
    m.error ? p.reject(new Error(m.error.message)) : p.resolve(m.result);
  });

  const { targetId } = await send('Target.createTarget', { url: 'about:blank' });
  const { sessionId: s } = await send('Target.attachToTarget', { targetId, flatten: true });
  const ev = (expr) => send('Runtime.evaluate', { expression: expr, awaitPromise: true, returnByValue: true }, s).then((r) => r.result.value);
  await send('Page.enable', {}, s);
  await send('Emulation.setDefaultBackgroundColorOverride', { color: { r: 0, g: 0, b: 0, a: 0 } }, s);
  await send('Page.navigate', { url: `${BASE}/docs/readme-art.html?shot=${shot}&theme=${theme}&anim=1` }, s);
  for (let i = 0; i < 100 && !(await ev('!!(window.SCENE && window.SCENE.ready)').catch(() => false)); i++) await sleep(100);
  const scene = await ev('({ w: innerWidth, h: innerHeight, ...window.SCENE })');
  const [w, h] = await ev('[document.documentElement.offsetWidth, document.documentElement.offsetHeight]');
  await send('Emulation.setDeviceMetricsOverride', { width: w, height: h, deviceScaleFactor: SCALE, mobile: false }, s);
  await sleep(600);

  const frames = Math.round(scene.duration * scene.fps);
  for (let f = 0; f < frames; f++) {
    await ev(`window.setT(${f / scene.fps}).then(() => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))))`);
    const { data } = await send('Page.captureScreenshot', { format: 'png', captureBeyondViewport: false }, s);
    writeFileSync(join(tmp, `f${String(f).padStart(4, '0')}.png`), Buffer.from(data, 'base64'));
    if (f % 20 === 0) process.stdout.write(`\r${shot}-${theme}: frame ${f + 1}/${frames}`);
  }
  process.stdout.write('\n');

  const q = String(process.env.QUALITY || 82);
  const r = spawnSync('ffmpeg', ['-y', '-loglevel', 'error', '-framerate', String(scene.fps), '-i', join(tmp, 'f%04d.png'),
    '-c:v', 'libwebp_anim', '-pix_fmt', 'yuva420p', '-quality', q, '-compression_level', '6', '-loop', '0', out], { stdio: 'inherit' });
  if (r.status !== 0) throw new Error('ffmpeg failed');
  console.log(`${out} (${(statSync(out).size / 1048576).toFixed(2)} MB, ${frames} frames)`);
} finally {
  ws?.close();
  chrome.kill();
  await sleep(500);
  try { rmSync(tmp, { recursive: true, force: true }); } catch {}
}
