(function () {
  const tauri = window.__TAURI__;
  const call = tauri ? tauri.core.invoke : mockInvoke;
  const listen = tauri ? tauri.event.listen : () => {};
  const params = new URLSearchParams(location.search);
  const MAC = /^mac/i.test(params.get('os') || navigator.platform);
  const MOD = MAC ? '⌘' : 'Ctrl+';
  const FILES_APP = MAC ? 'Finder' : 'Explorer';

  const TABS = [['all', 'All'], ['text', 'Text'], ['link', 'Links'], ['code', 'Code'], ['image', 'Images'], ['file', 'Files']];
  const SORT_LABEL = { recent: 'Recent', frequent: 'Frequent', 'a–z': 'A–Z', longest: 'Largest' };
  const THEMES = ['dark', 'light'];
  const MAX_ROWS = 200;
  const DAY = 864e5;

  const ICON = {
    text: '<path d="M17 6.1H3"/><path d="M21 12.1H3"/><path d="M15.1 18H3"/>',
    link: '<path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71"/><path d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71"/>',
    code: '<polyline points="16 18 22 12 16 6"/><polyline points="8 6 2 12 8 18"/>',
    email: '<rect width="20" height="16" x="2" y="4" rx="2"/><path d="m22 7-8.97 5.7a1.94 1.94 0 0 1-2.06 0L2 7"/>',
    number: '<line x1="4" x2="20" y1="9" y2="9"/><line x1="4" x2="20" y1="15" y2="15"/><line x1="10" x2="8" y1="3" y2="21"/><line x1="16" x2="14" y1="3" y2="21"/>',
    path: '<path d="M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z"/>',
    color: '<circle cx="13.5" cy="6.5" r=".5"/><circle cx="17.5" cy="10.5" r=".5"/><circle cx="8.5" cy="7.5" r=".5"/><circle cx="6.5" cy="12.5" r=".5"/><path d="M12 2C6.5 2 2 6.5 2 12s4.5 10 10 10c.926 0 1.648-.746 1.648-1.688 0-.437-.18-.835-.437-1.125-.29-.289-.438-.652-.438-1.125a1.64 1.64 0 0 1 1.668-1.668h1.996c3.051 0 5.555-2.503 5.555-5.554C21.965 6.012 17.461 2 12 2z"/>',
    file: '<path d="M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z"/><path d="M14 2v4a2 2 0 0 0 2 2h4"/>',
    files: '<path d="M20 7h-3a2 2 0 0 1-2-2V2"/><path d="M9 18a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h7l4 4v10a2 2 0 0 1-2 2Z"/><path d="M3 7.6v12.8A1.6 1.6 0 0 0 4.6 22h9.8"/>',
    image: '<rect width="18" height="18" x="3" y="3" rx="2"/><circle cx="9" cy="9" r="2"/><path d="m21 15-3.086-3.086a2 2 0 0 0-2.828 0L6 21"/>',
    pin: '<path d="M12 17v5"/><path d="M9 10.76a2 2 0 0 1-1.11 1.79l-1.78.9A2 2 0 0 0 5 15.24V16a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1v-.76a2 2 0 0 0-1.11-1.79l-1.78-.9A2 2 0 0 1 15 10.76V7a1 1 0 0 1 1-1 2 2 0 0 0 0-4H8a2 2 0 0 0 0 4 1 1 0 0 1 1 1z"/>',
    pinOff: '<path d="M12 17v5"/><path d="M15 9.34V7a1 1 0 0 1 1-1 2 2 0 0 0 0-4H7.89"/><path d="m2 2 20 20"/><path d="M9 9v1.76a2 2 0 0 1-1.11 1.79l-1.78.9A2 2 0 0 0 5 15.24V16a1 1 0 0 0 1 1h11"/>',
    trash: '<path d="M3 6h18"/><path d="M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6"/><path d="M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2"/>',
    sort: '<path d="m21 16-4 4-4-4"/><path d="M17 20V4"/><path d="m3 8 4-4 4 4"/><path d="M7 4v16"/>',
    contrast: '<circle cx="12" cy="12" r="10"/><path d="M12 18a6 6 0 0 0 0-12v12z"/>',
    // The theme icons are groups so style.css can animate their parts: a crescent
    // (circle r8.5 at 12,12 bitten by r7 at 16,8) with a twinkle in the bite, and a
    // round sun with short rays.
    dark: '<g class="moon"><path class="crescent" d="M10.53 3.63A8.5 8.5 0 1 0 20.37 13.47 7 7 0 0 1 10.53 3.63Z"/><path class="spark" d="M19.6 2.9v3M18.1 4.4h3" stroke-width="1.6"/></g>',
    light: '<g class="sun"><circle class="core" cx="12" cy="12" r="4.5"/><path class="rays" d="M12 2.25v2M12 19.75v2M2.25 12h2M19.75 12h2M5.11 5.11l1.41 1.41M18.89 5.11l-1.41 1.41M18.89 18.89l-1.41-1.41M5.11 18.89l1.41-1.41"/></g>',
    search: '<circle cx="11" cy="11" r="8"/><path d="m21 21-4.3-4.3"/>',
    clipboard: '<rect width="8" height="4" x="8" y="2" rx="1" ry="1"/><path d="M16 4h2a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h2"/>',
    x: '<path d="M18 6 6 18"/><path d="m6 6 12 12"/>',
    ban: '<circle cx="12" cy="12" r="10"/><path d="m4.9 4.9 14.2 14.2"/>',
    quote: '<path d="M17 6H3"/><path d="M21 12H8"/><path d="M21 18H8"/><path d="M3 12v6"/>',
    clock: '<circle cx="12" cy="12" r="10"/><polyline points="12 6 12 12 16 14"/>',
    stack: '<path d="m12.83 2.18a2 2 0 0 0-1.66 0L2.6 6.08a1 1 0 0 0 0 1.83l8.58 3.91a2 2 0 0 0 1.66 0l8.58-3.9a1 1 0 0 0 0-1.83Z"/><path d="m22 17.65-9.17 4.16a2 2 0 0 1-1.66 0L2 17.65"/><path d="m22 12.65-9.17 4.16a2 2 0 0 1-1.66 0L2 12.65"/>',
    stackAdd: '<path d="M11 12H3"/><path d="M16 6H3"/><path d="M16 18H3"/><path d="M18 9v6"/><path d="M21 12h-6"/>',
    stackRemove: '<path d="M11 12H3"/><path d="M16 6H3"/><path d="M16 18H3"/><path d="M21 12h-6"/>',
    external: '<path d="M15 3h6v6"/><path d="M10 14 21 3"/><path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/>',
    folder: '<path d="M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z"/><path d="m10 10 4 4"/><path d="M14 10v4h-4"/>',
    copy: '<rect width="14" height="14" x="8" y="8" rx="2" ry="2"/><path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2"/>',
    check: '<path d="M20 6 9 17l-5-5"/>',
    chevron: '<path d="m9 18 6-6-6-6"/>',
    chart: '<path d="M3 3v18h18"/><rect x="7" y="12" width="3" height="6" rx="1"/><rect x="12" y="8" width="3" height="10" rx="1"/><rect x="17" y="5" width="3" height="13" rx="1"/>',
    settings: '<path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z"/><circle cx="12" cy="12" r="3"/>',
    back: '<path d="m15 18-6-6 6-6"/>',
    keyboard: '<path d="M10 8h.01"/><path d="M12 12h.01"/><path d="M14 8h.01"/><path d="M16 12h.01"/><path d="M18 8h.01"/><path d="M6 8h.01"/><path d="M7 16h10"/><path d="M8 12h.01"/><rect width="20" height="16" x="2" y="4" rx="2"/>',
    power: '<path d="M12 2v10"/><path d="M18.4 6.6a9 9 0 1 1-12.77.04"/>',
    pause: '<circle cx="12" cy="12" r="10"/><line x1="10" x2="10" y1="15" y2="9"/><line x1="14" x2="14" y1="15" y2="9"/>',
    scan: '<path d="M3 7V5a2 2 0 0 1 2-2h2"/><path d="M17 3h2a2 2 0 0 1 2 2v2"/><path d="M21 17v2a2 2 0 0 1-2 2h-2"/><path d="M7 21H5a2 2 0 0 1-2-2v-2"/><path d="M7 8h8"/><path d="M7 12h10"/><path d="M7 16h6"/>',
    camera: '<path d="M14.5 4h-5L7 7H4a2 2 0 0 0-2 2v9a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2V9a2 2 0 0 0-2-2h-3l-2.5-3z"/><circle cx="12" cy="13" r="3"/>',
    key: '<path d="M2.586 17.414A2 2 0 0 0 2 18.828V21a1 1 0 0 0 1 1h3a1 1 0 0 0 1-1v-1a1 1 0 0 1 1-1h1a1 1 0 0 0 1-1v-1a1 1 0 0 1 1-1h.172a2 2 0 0 0 1.414-.586l.814-.814a6.5 6.5 0 1 0-4-4z"/><circle cx="16.5" cy="7.5" r=".5"/>',
    history: '<path d="M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8"/><path d="M3 3v5h5"/><path d="M12 7v5l4 2"/>',
  };
  const svg = (name) => `<svg class="icon" viewBox="0 0 24 24">${ICON[name]}</svg>`;

  const $ = (id) => document.getElementById(id);
  const q = $('q'), listEl = $('list'), tabsEl = $('tabs'), pill = $('pill'), chipsEl = $('chips');
  const pinBtn = $('pinned'), sortBtn = $('sort'), themeBtn = $('theme'), hintsEl = $('hints'), stackEl = $('stackbar'), accessEl = $('accessbar');
  const card = $('card'), homeEl = $('home'), prefsEl = $('prefs'), prefsBody = $('prefs-body'), settingsBtn = $('settings');

  const store = {
    get: (k, d) => { try { return localStorage.getItem(k) || d; } catch { return d; } },
    set: (k, v) => { try { localStorage.setItem(k, v); } catch {} },
  };

  let clips = [];
  let results = [];
  let sel = 0;
  let filter = 'all';
  let pinnedOnly = false;
  let chips = [];
  let colorMenuFor = null;
  let menuCtl = null;
  let clickGuard = null;
  let stack = { ids: [], done: 0 };
  // The last paste could only copy: stash has no Accessibility access (macOS).
  let noAccess = false;
  let mockStack = null;
  const mockPrefs = {
    config: { hotkey: 'Ctrl+Alt+V', max_items: 2000, max_clip_chars: 100000, ignore_sensitive: true, capture_images: true,
      capture_files: true, ocr_images: true, max_image_megapixels: 40, capture_screenshots: true },
    autostart: true,
    paused: false,
    version: '0.1.0',
  };
  let sort = store.get('sort', 'recent');
  if (!StashSearch.SORTS.includes(sort)) sort = 'recent';
  // Until a theme is picked, stash follows the system's light or dark.
  const systemDark = matchMedia('(prefers-color-scheme: dark)');
  const systemTheme = () => (systemDark.matches ? 'dark' : 'light');
  let pickedTheme = params.get('theme') || store.get('theme', '');
  if (!THEMES.includes(pickedTheme)) pickedTheme = '';
  let theme = pickedTheme || systemTheme();

  const el = (tag, cls, html) => {
    const e = document.createElement(tag);
    if (cls) e.className = cls;
    if (html) e.innerHTML = html;
    return e;
  };
  // On macOS Ctrl+click is a right-click.
  const secondary = (e) => e.button !== 0 || (MAC && e.ctrlKey);
  // Ctrl+click (⌘-click on macOS) adds a clip to the paste stack.
  const stackClick = (e) => (MAC ? e.metaKey : e.ctrlKey);
  const noFocusSteal = (b) => b.addEventListener('mousedown', (e) => e.preventDefault());
  const byId = (id) => clips.find((c) => c.id === id);

  // The footer button holds both icons and swaps them by data-mode, so the change animates.
  themeBtn.innerHTML = `<svg class="icon theme-ico" viewBox="0 0 24 24">${ICON.light}${ICON.dark}</svg>`;
  const themeIco = themeBtn.firstChild;

  function applyTheme() {
    document.documentElement.setAttribute('data-theme', theme);
    themeIco.dataset.mode = theme;
    themeBtn.title = `Switch to ${theme === 'dark' ? 'light' : 'dark'} (${MOD}T)`;
  }
  function followSystem() {
    if (pickedTheme || theme === systemTheme()) return;
    theme = systemTheme();
    applyTheme();
  }
  // Older WebKit (macOS 10.15) only has addListener.
  if (systemDark.addEventListener) systemDark.addEventListener('change', followSystem);
  else systemDark.addListener(followSystem);

  function age(ms) {
    const s = Math.max(0, (Date.now() - ms) / 1000);
    if (s < 60) return 'now';
    if (s < 3600) return Math.floor(s / 60) + 'm';
    if (s < 86400) return Math.floor(s / 3600) + 'h';
    if (s < 604800) return Math.floor(s / 86400) + 'd';
    return Math.floor(s / 604800) + 'w';
  }

  function size(bytes) {
    if (bytes < 1024) return bytes + ' B';
    if (bytes < 1024 * 1024) return Math.round(bytes / 1024) + ' KB';
    return (bytes / 1024 / 1024).toFixed(1) + ' MB';
  }

  const CHIP_KIND = { link: 'Links', code: 'Code', image: 'Images', file: 'Files', text: 'Text', path: 'Paths',
    color: 'Colors', number: 'Numbers', email: 'Emails' };
  const CHIP_TIME = { hour: 'Last hour', today: 'Today', week: 'This week' };

  function isToken(word) {
    if (word[0] === '@') return word.length > 1 && StashSearch.parse(word).terms.length === 0;
    return /^[!'^]./.test(word);
  }

  function chipLook(tok) {
    const rest = tok.slice(1);
    if (tok[0] === '!') return { icon: 'ban', label: rest };
    if (tok[0] === "'") return { icon: 'quote', label: `“${rest}”` };
    if (tok[0] === '^') return { icon: 'quote', label: `${rest}…` };
    const p = StashSearch.parse(tok);
    if (p.pin) return { icon: 'pin', label: 'Pinned' };
    if (p.maxAge) return { icon: 'clock', label: CHIP_TIME[rest.toLowerCase()] };
    const kind = [...p.kinds][0];
    return { icon: ICON[kind] ? kind : 'text', label: CHIP_KIND[kind] || kind };
  }

  function absorb(all = false) {
    const v = q.value;
    if (!all && !/\s$/.test(v)) return false;
    const words = v.trim().split(/\s+/).filter(Boolean);
    const keep = [];
    let moved = false;
    for (const w of words) {
      if (isToken(w) && !chips.includes(w)) { chips.push(w); moved = true; }
      else if (!isToken(w)) keep.push(w);
      else moved = true;
    }
    if (moved) q.value = keep.length ? keep.join(' ') + (all ? '' : ' ') : '';
    return moved;
  }

  function renderChips() {
    chipsEl.textContent = '';
    for (const tok of chips) {
      const look = chipLook(tok);
      const chip = el('span', 'chip', svg(look.icon));
      chip.appendChild(el('span')).textContent = look.label;
      chip.title = tok;
      chip.dataset.token = tok;
      const x = chip.appendChild(el('button', 'x', svg('x')));
      x.title = 'Remove';
      noFocusSteal(x);
      x.addEventListener('click', (e) => { e.preventDefault(); chips = chips.filter((c) => c !== tok); refresh(); q.focus(); });
      chipsEl.appendChild(chip);
    }
    q.placeholder = chips.length ? 'Refine…' : 'Search your clipboard…';
  }

  const query = () => [...chips, q.value].join(' ').trim();

  const lines = (text) => text.split('\n').map((l) => l.trim()).filter(Boolean);
  const baseName = (p) => p.split(/[\\/]/).filter(Boolean).pop() || p;
  const dirName = (p) => { const i = Math.max(p.lastIndexOf('\\'), p.lastIndexOf('/')); return i > 0 ? p.slice(0, i) : ''; };

  const homeDir = (p) => dirName(p).replace(/^(?:[A-Z]:\\Users\\[^\\]+|\/Users\/[^/]+)/i, '~');
  const dimsOf = (c) => (c.width && c.height ? `${c.width} × ${c.height}` : null);

  function describe(c) {
    if (isSvg(c)) return { title: 'SVG image', sub: [dimsOf(c), size(c.len)].filter(Boolean).join(' · ') };
    const data = dataUrlType(c);
    if (data) {
      const type = { 'svg+xml': 'SVG', 'x-icon': 'ICO', 'vnd.microsoft.icon': 'ICO' }[data.toLowerCase()] || data.toUpperCase();
      return { title: `${type} data URL`, sub: [dimsOf(c), size(c.len)].filter(Boolean).join(' · ') };
    }
    if (c.kind === 'image') {
      const dims = c.width && c.height ? `${c.width} × ${c.height}` : 'Image';
      const found = lines(c.ocr_text || '')[0];
      if (found) return { title: found, sub: `${dims} · ${size(c.bytes)}` };
      return { title: 'Image', sub: `${dims} · ${size(c.bytes)}` };
    }
    if (c.kind === 'file') {
      const paths = lines(c.text);
      const first = paths[0] || c.text;
      const name = baseName(first);
      if (paths.length > 1) return { title: `${name} +${paths.length - 1}`, sub: `${paths.length} files · ${size(c.bytes)}` };
      return { title: name, sub: [homeDir(first), size(c.bytes), dimsOf(c)].filter(Boolean).join(' · ') };
    }
    const path = textPath(c);
    if (path) return { title: baseName(path), sub: [homeDir(path), dimsOf(c)].filter(Boolean).join(' · ') };
    const ls = lines(c.text);
    if (c.kind === 'link') return { title: (ls[0] || '').replace(/^[a-z]+:\/\//i, '').replace(/\/$/, '') };
    return { title: ls[0] || c.text, sub: ls[1] ? ls[1] + (ls.length > 2 ? '  …' : '') : null };
  }

  // What Shift shows of a text clip the row cuts short: its first lines as
  // copied, with indentation kept, and what's left after them.
  const PREVIEW_LINES = 10;
  const PREVIEW_KINDS = new Set(['text', 'code', 'link', 'email', 'path']);
  function textPreview(c) {
    if (!PREVIEW_KINDS.has(c.kind) || isSvg(c) || dataUrlType(c)) return null;
    const ls = lines(c.text);
    const fits = c.kind === 'code' ? 40 : 44;
    if (ls.length <= 2 && ls.every((l) => l.length <= fits)) return null;
    const all = c.text.replace(/\s+$/, '').replace(/^\s*\n/, '').split('\n').map((l) => l.trimEnd());
    const indent = Math.min(...all.filter(Boolean).map((l) => l.match(/^[ \t]*/)[0].length));
    const hidden = all.length - PREVIEW_LINES;
    let more = null;
    if (c.len > c.text.length) more = `${size(c.len)} in total`;
    else if (hidden > 0) more = `+${hidden} more line${hidden === 1 ? '' : 's'}`;
    return { text: all.slice(0, PREVIEW_LINES).map((l) => l.slice(indent)).join('\n'), more };
  }

  const clamp = (n, lo, hi) => Math.min(hi, Math.max(lo, n));

  function hslToRgb(h, s, l) {
    h = ((h % 360) + 360) % 360 / 360;
    s = clamp(s, 0, 100) / 100;
    l = clamp(l, 0, 100) / 100;
    if (s === 0) {
      const v = Math.round(l * 255);
      return [v, v, v];
    }
    const q = l < 0.5 ? l * (1 + s) : l + s - l * s;
    const p = 2 * l - q;
    const hue = (t) => {
      t = (t + 1) % 1;
      if (t < 1 / 6) return p + (q - p) * 6 * t;
      if (t < 1 / 2) return q;
      if (t < 2 / 3) return p + (q - p) * (2 / 3 - t) * 6;
      return p;
    };
    return [hue(h + 1 / 3), hue(h), hue(h - 1 / 3)].map((v) => Math.round(v * 255));
  }

  function rgbToHsl(r, g, b) {
    const rr = r / 255, gg = g / 255, bb = b / 255;
    const max = Math.max(rr, gg, bb), min = Math.min(rr, gg, bb), l = (max + min) / 2;
    let h = 0, s = 0;
    if (max !== min) {
      const d = max - min;
      s = l > 0.5 ? d / (2 - max - min) : d / (max + min);
      h = max === rr ? (gg - bb) / d + (gg < bb ? 6 : 0) : max === gg ? (bb - rr) / d + 2 : (rr - gg) / d + 4;
      h /= 6;
    }
    return [Math.round(h * 360) % 360, Math.round(s * 100), Math.round(l * 100)];
  }

  function parseColor(text) {
    const t = text.trim().toLowerCase();
    const hex = t.replace(/^#/, '');
    if (/^[0-9a-f]{3,4}$/.test(hex)) {
      const [r, g, b, a] = hex.split('').map((c) => parseInt(c + c, 16));
      return { r, g, b, a: a === undefined ? 1 : a / 255 };
    }
    if (/^[0-9a-f]{6}([0-9a-f]{2})?$/.test(hex)) {
      return {
        r: parseInt(hex.slice(0, 2), 16),
        g: parseInt(hex.slice(2, 4), 16),
        b: parseInt(hex.slice(4, 6), 16),
        a: hex.length === 8 ? parseInt(hex.slice(6, 8), 16) / 255 : 1,
      };
    }
    const fn = t.match(/^(rgba?|hsla?)\(([^)]*)\)$/);
    if (!fn) return null;
    const parts = fn[2].split(/[,\s/]+/).filter(Boolean);
    if (parts.length < 3) return null;
    const num = (s, scale) => (s.endsWith('%') ? (parseFloat(s) / 100) * scale : parseFloat(s));
    const alpha = parts[3] === undefined ? 1 : clamp(num(parts[3], 1), 0, 1);
    if (!Number.isFinite(alpha)) return null;
    if (fn[1].startsWith('rgb')) {
      const [r, g, b] = parts.slice(0, 3).map((s) => Math.round(clamp(num(s, 255), 0, 255)));
      return [r, g, b].every(Number.isFinite) ? { r, g, b, a: alpha } : null;
    }
    const h = parseFloat(parts[0]);
    const s = clamp(num(parts[1], 100), 0, 100);
    const l = clamp(num(parts[2], 100), 0, 100);
    if (![h, s, l].every(Number.isFinite)) return null;
    const [r, g, b] = hslToRgb(h, s, l);
    return { r, g, b, a: alpha };
  }

  const hexByte = (n) => clamp(Math.round(n), 0, 255).toString(16).padStart(2, '0');
  const alphaText = (a) => String(Math.round(a * 100) / 100);

  function colorFormats(text) {
    const c = parseColor(text);
    if (!c) return [];
    const hex = `#${hexByte(c.r)}${hexByte(c.g)}${hexByte(c.b)}${c.a < 1 ? hexByte(c.a * 255) : ''}`.toUpperCase();
    const rgb = c.a < 1 ? `rgba(${c.r}, ${c.g}, ${c.b}, ${alphaText(c.a)})` : `rgb(${c.r}, ${c.g}, ${c.b})`;
    const [h, s, l] = rgbToHsl(c.r, c.g, c.b);
    const hsl = c.a < 1 ? `hsla(${h}, ${s}%, ${l}%, ${alphaText(c.a)})` : `hsl(${h}, ${s}%, ${l}%)`;
    return [{ label: 'HEX', value: hex }, { label: 'RGB', value: rgb }, { label: 'HSL', value: hsl }];
  }

  // Short labels for the tile of a code clip, in the style of file extension badges.
  const LANG_BADGE = { json: 'JSON', php: 'PHP', html: 'HTML', rust: 'RS', python: 'PY', go: 'GO', java: 'JAVA', cpp: 'C++', sql: 'SQL', js: 'JS', bash: 'SH' };

  function guessLang(text) {
    const t = text.trim();
    if (/^[[{]/.test(t)) {
      try { JSON.parse(t); return 'json'; } catch {}
    }
    if (/^<\?php/i.test(t)) return 'php';
    if (/^<(!doctype|html|div|section|template)/i.test(t)) return 'html';
    if (/\b(fn\s+\w+|let\s+mut|impl\s+\w+|use\s+std::|pub\s+(struct|enum))\b/.test(t)) return 'rust';
    if (/^\s*(def\s+\w+\s*\(|class\s+\w+.*:|from\s+\w+\s+import\s+|import\s+\w+\s*$)/m.test(t)) return 'python';
    if (/\bfunc\s+\w+\s*\(|^package\s+\w+/m.test(t)) return 'go';
    if (/\b(public|private)\s+(class|static|void)\b|System\.out\./.test(t)) return 'java';
    if (/#include\s*</.test(t)) return 'cpp';
    if (/\b(SELECT|INSERT|UPDATE|DELETE)\b[\s\S]*\b(FROM|INTO|SET|WHERE)\b/i.test(t)) return 'sql';
    if (/\b(const|let|var)\s+\w+\s*=|=>|function\s*\w*\s*\(/.test(t)) return 'js';
    if (/^\s*(#!\/bin\/(ba)?sh|sudo\s)/m.test(t)) return 'bash';
    return '';
  }

  function runThenHide(cmd, args) {
    Promise.resolve()
      .then(() => call(cmd, args))
      .catch((e) => console.error(e))
      .finally(() => run('hide_window'));
  }

  function copyAndHide(cmd, args) {
    runThenHide(cmd, args);
  }

  function formatMenu(c) {
    const formats = colorFormats(c.text);
    if (!formats.length) return null;
    const box = el('div', 'formats');
    for (const f of formats) {
      const b = el('button', 'fmt');
      b.title = `Copy ${f.label}`;
      b.appendChild(el('span', 'label')).textContent = f.label;
      b.appendChild(el('code')).textContent = f.value;
      b.addEventListener('mousedown', (e) => {
        e.preventDefault();
        e.stopPropagation();
        if (secondary(e)) return;
        copyAndHide('copy_text', { id: c.id, text: f.value });
      });
      b.addEventListener('click', (e) => e.stopPropagation());
      box.appendChild(b);
    }
    return box;
  }

  function toggleColorMenu(c, i) {
    const old = sel;
    sel = i;
    colorMenuFor = colorMenuFor === c.id ? null : c.id;
    rerender(old);
    if (i !== old) rerender(i);
    placeHighlight(true);
    scrollToSel();
  }

  // Image files, font files and SVG markup get a thumbnail the first time
  // their row is shown: the webview decodes the original, draws it small,
  // compresses it (JPEG when opaque, PNG when not) and hands it back to be
  // stored. A font's thumbnail is "Aa" set in it.
  const THUMB_MAX = 160;
  const SVG_NS = 'http://www.w3.org/2000/svg';
  // TIFF and HEIC only on macOS, whose WebKit draws them; Windows' WebView2 can't.
  const VISUAL_EXT = new RegExp(`\\.(svg|png|jpe?g|gif|webp|bmp|ico|avif|ttf|otf|woff2?${MAC ? '|tiff?|hei[cf]' : ''})$`, 'i');
  const FONT_EXT = /\.(ttf|otf|woff2?)$/i;
  const MIME = { svg: 'image/svg+xml', jpg: 'image/jpeg', tif: 'image/tiff', ico: 'image/x-icon', ttf: 'font/ttf', otf: 'font/otf', woff: 'font/woff', woff2: 'font/woff2' };
  const FONT_SAMPLE = 'Aa';
  const SVG_START = /^\s*(?:<\?[\s\S]*?\?>\s*|<!--[\s\S]*?-->\s*|<![^>]*>\s*)*<svg[\s>]/i;
  const isSvg = (c) => c.kind !== 'image' && c.kind !== 'file' && SVG_START.test(c.text);
  const tried = new Set();
  let drawing = Promise.resolve();

  // A lone image or font path copied as text, quoted (as Explorer copies it) or not.
  const PATH_START = /^(?:[A-Za-z]:[\\/]|\\\\|\/|~\/)/;
  function textPath(c) {
    if (c.kind === 'file' || c.kind === 'image') return undefined;
    const t = c.text.trim().replace(/^"(.*)"$/, '$1');
    return !t.includes('\n') && PATH_START.test(t) && VISUAL_EXT.test(t) ? t : undefined;
  }
  // An image as a data URL copied as text; returns its type, such as "png" or "svg+xml".
  const DATA_URL = /^\s*data:image\/([\w.+-]+)[;,]/i;
  const dataUrlType = (c) => (c.kind !== 'file' && c.kind !== 'image' ? (c.text.match(DATA_URL) || [])[1] : undefined);
  const visualPath = (c) => (c.kind === 'file' ? lines(c.text).find((p) => VISUAL_EXT.test(p)) : textPath(c));
  const isFontFile = (c) => FONT_EXT.test(visualPath(c) || '');

  function visualType(c) {
    if (c.thumb || c.kind === 'image') return null;
    const path = visualPath(c);
    if (path) {
      const ext = path.match(VISUAL_EXT)[1].toLowerCase();
      return MIME[ext] || `image/${ext}`;
    }
    const data = dataUrlType(c);
    if (data) return `image/${data.toLowerCase()}`;
    return isSvg(c) ? 'image/svg+xml' : null;
  }

  function drawThumb(c) {
    const type = visualType(c);
    if (!type || tried.has(c.id)) return;
    tried.add(c.id);
    // One at a time, and none may hold up the rest for long.
    const timeout = new Promise((_, fail) => setTimeout(fail, 10000));
    drawing = drawing.then(() => Promise.race([makeThumb(c, type), timeout])).catch(() => {});
  }

  // Gives the SVG a fixed size that fits the thumbnail, so it draws crisp
  // whatever size (or none) it declares. Returns null for broken markup.
  function sizedSvg(text) {
    if (!/xmlns\s*=\s*["']http:\/\/www\.w3\.org\/2000\/svg/.test(text)) text = text.replace(/<svg\b/i, `<svg xmlns="${SVG_NS}"`);
    const doc = new DOMParser().parseFromString(text, 'image/svg+xml');
    const svg = doc.documentElement;
    if (svg.namespaceURI !== SVG_NS || svg.localName !== 'svg') return null;
    const box = (svg.getAttribute('viewBox') || '').split(/[\s,]+/).map(Number);
    const attr = (name) => (/^\s*[\d.]+(px)?\s*$/.test(svg.getAttribute(name) || '') ? parseFloat(svg.getAttribute(name)) : 0);
    let w = attr('width') || box[2] || 0, h = attr('height') || box[3] || 0;
    if (!(w > 0 && h > 0)) w = h = THUMB_MAX;
    if (!svg.hasAttribute('viewBox')) svg.setAttribute('viewBox', `0 0 ${w} ${h}`);
    const scale = THUMB_MAX / Math.max(w, h);
    svg.setAttribute('width', String(Math.max(1, Math.round(w * scale))));
    svg.setAttribute('height', String(Math.max(1, Math.round(h * scale))));
    return { markup: new XMLSerializer().serializeToString(doc), w, h, scale };
  }

  async function makeThumb(c, type) {
    const bytes = await call('visual_source', { id: c.id });
    const drawn = type.startsWith('font/') ? await drawFont(bytes) : await drawPicture(bytes, type);
    if (!drawn) return;
    const { canvas, width, height } = drawn;
    const px = canvas.getContext('2d').getImageData(0, 0, canvas.width, canvas.height).data;
    let opaque = true;
    for (let i = 3; i < px.length && opaque; i += 4) opaque = px[i] === 255;
    const thumb = opaque ? canvas.toDataURL('image/jpeg', 0.82) : canvas.toDataURL('image/png');
    await call('save_thumb', { id: c.id, thumb, width, height });
    Object.assign(c, { thumb, width, height });
    const i = results.findIndex((r) => r.clip.id === c.id);
    if (i >= 0) rerender(i);
  }

  // Draws the sample in black on transparent; the tile uses it as a mask so
  // it takes the theme's text colour. Fonts have no pixel size, so it's 0 × 0.
  async function drawFont(bytes) {
    const face = new FontFace(`stash-sample-${Date.now()}`, bytes);
    await face.load();
    document.fonts.add(face);
    try {
      const canvas = Object.assign(document.createElement('canvas'), { width: THUMB_MAX, height: THUMB_MAX });
      const ctx = canvas.getContext('2d');
      const fit = (size) => {
        ctx.font = `${size}px "${face.family}"`;
        const m = ctx.measureText(FONT_SAMPLE);
        return { m, w: m.actualBoundingBoxLeft + m.actualBoundingBoxRight, h: m.actualBoundingBoxAscent + m.actualBoundingBoxDescent };
      };
      const probe = fit(100);
      if (!(probe.w > 0 && probe.h > 0)) return null;
      const { m, w, h } = fit(100 * Math.min((THUMB_MAX * 0.9) / probe.w, (THUMB_MAX * 0.9) / probe.h));
      ctx.fillText(FONT_SAMPLE, (THUMB_MAX - w) / 2 + m.actualBoundingBoxLeft, (THUMB_MAX - h) / 2 + m.actualBoundingBoxAscent);
      return { canvas, width: 0, height: 0 };
    } finally {
      document.fonts.delete(face);
    }
  }

  async function drawPicture(bytes, type) {
    let blob = new Blob([bytes], { type });
    let w, h, scale;
    if (type === 'image/svg+xml') {
      const s = sizedSvg(await blob.text());
      if (!s) return null;
      blob = new Blob([s.markup], { type });
      ({ w, h, scale } = s);
    }
    const url = URL.createObjectURL(blob);
    try {
      const img = new Image();
      // onload rather than decode(), which can wait for a frame a hidden webview never draws.
      await new Promise((loaded, failed) => {
        img.onload = loaded;
        img.onerror = failed;
        img.src = url;
      });
      if (!w) {
        w = img.naturalWidth;
        h = img.naturalHeight;
        scale = Math.min(1, THUMB_MAX / Math.max(w, h));
      }
      const canvas = document.createElement('canvas');
      canvas.width = Math.max(1, Math.round(w * scale));
      canvas.height = Math.max(1, Math.round(h * scale));
      const ctx = canvas.getContext('2d');
      ctx.imageSmoothingQuality = 'high';
      ctx.drawImage(img, 0, 0, canvas.width, canvas.height);
      return { canvas, width: Math.max(1, Math.round(w)), height: Math.max(1, Math.round(h)) };
    } finally {
      URL.revokeObjectURL(url);
    }
  }

  // Writes text into node with search hits marked and secrets blurred
  // (all of it when hideAll), each secret in one span that unblurs on hover.
  function paint(node, text, hideAll = false, max = 300) {
    const s = text.slice(0, max);
    const hits = query() ? StashSearch.hits(query(), s) : new Set();
    const blur = hideAll ? [[0, s.length]] : StashSecrets.find(s);
    let run = '', on = false, hidden = false, into = node;
    const flush = () => {
      if (!run) return;
      if (on) { const m = document.createElement('mark'); m.textContent = run; into.appendChild(m); }
      else into.appendChild(document.createTextNode(run));
      run = '';
    };
    for (let i = 0; i < s.length; i++) {
      const hit = hits.has(i);
      const secret = blur.some(([a, b]) => i >= a && i < b);
      if (secret !== hidden) {
        flush();
        hidden = secret;
        into = secret ? node.appendChild(el('span', 'secret')) : node;
      }
      if (hit !== on) { flush(); on = hit; }
      run += s[i];
    }
    flush();
  }

  function fontSample(thumb) {
    const s = el('i', 'sample');
    s.style.webkitMaskImage = s.style.maskImage = `url("${thumb}")`;
    return s;
  }

  // A CSS gradient copied on its own, or as a background declaration, if the
  // webview can draw it; the tile then shows it like a colour swatch.
  const GRADIENT = /^(?:repeating-)?(?:linear|radial|conic)-gradient\([\s\S]*\)$/i;
  function gradientOf(c) {
    if (c.kind === 'image' || c.kind === 'file') return null;
    const g = c.text.trim().replace(/^background(?:-image)?\s*:\s*/i, '').replace(/\s*;$/, '');
    return GRADIENT.test(g) && CSS.supports('background-image', g) ? g : null;
  }

  // The first emoji of a clip that is nothing but emoji (flags, skin tones and
  // joined sequences included), shown large in its tile.
  const EMOJI_ONLY = /^(?:\p{Extended_Pictographic}|\p{Regional_Indicator}|\p{Emoji_Modifier}|[#*0-9]\uFE0F?\u20E3|\u200D|\uFE0F|\s)+$/u;
  const graphemes = (s) => (Intl.Segmenter ? [...new Intl.Segmenter().segment(s)].map((g) => g.segment) : Array.from(s));
  function emojiOf(c) {
    if (c.kind !== 'text') return null;
    const t = c.text.trim();
    if (!t || t.length > 64 || !EMOJI_ONLY.test(t) || !/\p{Extended_Pictographic}|\p{Regional_Indicator}|\u20E3/u.test(t)) return null;
    return graphemes(t).find((g) => g.trim());
  }

  function tile(c) {
    const t = el('div', 'tile');
    if (c.thumb && isFontFile(c)) {
      t.classList.add('glyph');
      t.appendChild(fontSample(c.thumb));
    } else if (c.thumb) {
      t.classList.add('thumb');
      if (c.kind !== 'image' && c.thumb.startsWith('data:image/png')) t.classList.add('alpha');
      const img = new Image();
      img.src = c.thumb;
      img.alt = '';
      t.appendChild(img);
    } else if (c.kind === 'color') {
      t.classList.add('swatch');
      t.appendChild(el('i')).style.background = c.text.trim();
    } else if (emojiOf(c)) {
      t.classList.add('emoji');
      t.appendChild(el('span')).textContent = emojiOf(c);
    } else if (gradientOf(c)) {
      t.classList.add('swatch');
      t.appendChild(el('i')).style.backgroundImage = gradientOf(c);
    } else if (c.kind === 'file') {
      const paths = lines(c.text);
      const ext = (baseName(paths[0] || '').match(/\.([a-z0-9]{1,4})$/i) || [])[1];
      if (paths.length > 1) t.innerHTML = svg('files');
      else if (ext) t.appendChild(el('span', 'ext')).textContent = ext.toUpperCase();
      else t.innerHTML = svg('file');
    } else if (c.kind === 'code' && LANG_BADGE[guessLang(c.text)]) {
      t.appendChild(el('span', 'ext')).textContent = LANG_BADGE[guessLang(c.text)];
    } else {
      t.innerHTML = svg(ICON[c.kind] ? c.kind : 'text');
    }
    const pos = stack.ids.indexOf(c.id);
    if (pos >= 0) t.appendChild(el('span', 'badge')).textContent = String(stack.done + pos + 1);
    return t;
  }

  function action(icon, title, fn) {
    const b = el('button', 'ghost', svg(icon));
    b.title = title;
    noFocusSteal(b);
    b.addEventListener('mousedown', (e) => { if (e.button === 0 && !secondary(e)) { e.stopPropagation(); fn(); } });
    b.addEventListener('click', (e) => e.stopPropagation());
    return b;
  }

  const MENU_GAP = 8;
  const MENU_EDGE = 6;
  const MENU_W = 196;
  const SUB_W = 160;

  const menuSep = () => ({ type: 'separator' });

  const showItem = () => ({
    icon: 'clipboard',
    label: 'Show',
    children: TABS.map(([id, label]) => ({
      icon: id === 'all' ? 'clipboard' : id,
      label,
      checked: filter === id,
      action: () => { filter = id; refresh(); },
    })),
  });

  const pinnedItem = () => ({
    icon: 'pin',
    label: 'Pinned only',
    checked: pinnedOnly,
    action: () => { pinnedOnly = !pinnedOnly; refresh(); },
  });

  const sortItem = () => ({
    icon: 'sort',
    label: 'Sort',
    children: [
      { icon: 'clock', label: 'Recent', checked: sort === 'recent', action: () => setSortTo('recent') },
      { icon: 'chart', label: 'Frequent', checked: sort === 'frequent', action: () => setSortTo('frequent') },
      { icon: 'text', label: 'A–Z', checked: sort === 'a–z', action: () => setSortTo('a–z') },
      { icon: 'number', label: 'Largest', checked: sort === 'longest', action: () => setSortTo('longest') },
    ],
  });

  const themeItem = () => ({
    icon: theme,
    label: 'Theme',
    children: THEMES.map((id) => ({
      icon: id,
      label: id[0].toUpperCase() + id.slice(1),
      checked: theme === id,
      action: () => setThemeTo(id),
    })),
  });

  const clearSearchItem = () => ({
    icon: 'ban',
    label: 'Clear search',
    hint: 'Esc',
    action: () => { q.value = ''; chips = []; refresh(); q.focus(); },
  });

  const clearStackItem = () => ({
    icon: 'stackRemove',
    label: 'Clear stack',
    action: cancelStack,
  });

  const settingsItem = () => ({
    icon: 'settings',
    label: 'Settings',
    hint: `${MOD},`,
    action: () => showView('prefs'),
  });

  const hideItem = () => ({
    icon: 'x',
    label: 'Hide stash',
    action: () => run('hide_window'),
  });

  function appMenu({ lead = [], noClearSearch = false, noClearStack = false } = {}) {
    const items = [...lead];
    if (items.length) items.push(menuSep());
    items.push(showItem(), pinnedItem(), sortItem(), themeItem());
    const extra = [];
    if (!noClearSearch && (q.value || chips.length)) extra.push(clearSearchItem());
    if (!noClearStack && stack.ids.length) extra.push(clearStackItem());
    if (extra.length) items.push(menuSep(), ...extra);
    items.push(menuSep(), settingsItem(), hideItem());
    return items;
  }

  function clipMenu(c) {
    const noun = { image: 'image', file: 'files', link: 'link', code: 'code', color: 'colour', text: 'text' }[c.kind];
    const items = [
      { icon: 'clipboard', label: noun ? `Paste ${noun}` : 'Paste', hint: '↵', action: () => paste(c) },
      { icon: 'copy', label: noun ? `Copy ${noun}` : 'Copy', hint: `${MOD}↵`, action: () => copyOnly(c) },
    ];
    let special = null;
    if (c.kind === 'link') {
      special = { icon: 'external', label: 'Open link', action: () => runThenHide('open_url', { id: c.id }) };
    } else if (c.kind === 'file' || c.kind === 'path') {
      special = { icon: 'folder', label: `Show in ${FILES_APP}`, action: () => runThenHide('reveal_in_explorer', { id: c.id }) };
    } else if (c.kind === 'code') {
      special = { icon: 'code', label: 'Copy as fenced code', action: () => copyAndHide('copy_fence', { id: c.id, lang: guessLang(c.text) }) };
    } else if (c.kind === 'color') {
      const formats = colorFormats(c.text);
      if (formats.length) {
        special = {
          icon: 'color',
          label: 'Copy as colour',
          children: formats.map((f) => ({
            dot: f.value,
            label: f.label,
            hint: f.value,
            title: f.value,
            action: () => copyAndHide('copy_text', { id: c.id, text: f.value }),
          })),
        };
      }
    }
    if (special) items.push(menuSep(), special);
    const inStack = stack.ids.includes(c.id);
    items.push(
      menuSep(),
      { icon: inStack ? 'stackRemove' : 'stackAdd', label: inStack ? 'Remove from stack' : 'Add to stack', hint: `${MOD}click`, action: () => toggleStack(c) },
      { icon: c.pinned ? 'pinOff' : 'pin', label: c.pinned ? 'Unpin' : 'Pin', hint: `${MOD}P`, action: () => pin(c) },
      menuSep(),
      { icon: 'trash', label: 'Delete', hint: MAC ? '⌘⌫' : 'Del', danger: true, action: () => remove(c) },
    );
    return items;
  }

  function chipMenu(tok) {
    const look = chipLook(tok);
    const lead = [{
      icon: 'x',
      label: `Remove “${look.label}”`,
      action: () => { chips = chips.filter((c) => c !== tok); refresh(); q.focus(); },
    }];
    if (chips.length > 1) {
      lead.push({
        icon: 'ban',
        label: 'Clear filters',
        action: () => { chips = []; refresh(); q.focus(); },
      });
    }
    return appMenu({ lead, noClearSearch: true });
  }

  function searchMenu() {
    const lead = q.value || chips.length ? [clearSearchItem()] : [];
    return appMenu({ lead, noClearSearch: true });
  }

  function stackMenu() {
    const lead = [];
    if (stack.ids.length) lead.push({ icon: 'stack', label: 'Paste next', hint: '↵', action: pasteStack }, clearStackItem());
    return appMenu({ lead, noClearStack: true });
  }

  function setMenuActive(panel, index) {
    if (panel.active === index) return;
    const old = panel.rows[panel.active];
    if (old) old.row.classList.remove('active');
    panel.active = index;
    const cur = panel.rows[index];
    if (cur) cur.row.classList.add('active');
  }

  function moveMenuActive(panel, dir) {
    const n = panel.rows.length;
    if (!n) return;
    let i = panel.active;
    for (let step = 0; step < n; step++) {
      i = i < 0 ? (dir > 0 ? 0 : n - 1) : (i + dir + n) % n;
      if (!panel.rows[i].item.disabled) { setMenuActive(panel, i); return; }
    }
  }

  function edgeMenuActive(panel, last) {
    const n = panel.rows.length;
    let i = last ? n - 1 : 0;
    const step = last ? -1 : 1;
    for (let k = 0; k < n; k++) {
      if (!panel.rows[i].item.disabled) { setMenuActive(panel, i); return; }
      i = (i + step + n) % n;
    }
  }

  function dismissPanel(panel, animate = true) {
    panel.root.classList.remove('on');
    panel.root.classList.add('leaving');
    panel.root.style.pointerEvents = 'none';
    if (panel.parentEl) {
      panel.parentEl.classList.remove('open');
      panel.parentEl.setAttribute('aria-expanded', 'false');
    }
    if (!animate) { panel.root.remove(); return; }
    setTimeout(() => panel.root.remove(), 130);
  }

  function closeSubmenusAfter(level) {
    if (!menuCtl) return;
    const drop = menuCtl.panels.filter((p) => p.level > level);
    if (!drop.length) return;
    menuCtl.panels = menuCtl.panels.filter((p) => p.level <= level);
    for (const p of drop) dismissPanel(p);
    const owner = menuCtl.panels[menuCtl.panels.length - 1];
    if (owner) owner.openFor = null;
  }

  function openSubmenu(panel, index, focusFirst = false) {
    const entry = panel.rows[index];
    if (!menuCtl || !entry || !entry.item.children || panel.openFor === index) return;
    closeSubmenusAfter(panel.level);
    panel.openFor = index;
    entry.row.classList.add('open');
    entry.row.setAttribute('aria-expanded', 'true');
    const sub = menuPanel(entry.item.children, panel.level + 1, entry.row);
    menuCtl.panels.push(sub);
    document.body.appendChild(sub.root);

    const r = entry.row.getBoundingClientRect();
    const w = sub.root.offsetWidth;
    const h = sub.root.offsetHeight;
    let left = r.right + MENU_GAP;
    let origin = 'left';
    if (left + w > innerWidth - MENU_EDGE) {
      if (r.left - MENU_GAP - w >= MENU_EDGE) { left = r.left - MENU_GAP - w; origin = 'right'; }
      else left = Math.max(MENU_EDGE, innerWidth - w - MENU_EDGE);
    }
    let top = r.top - 4;
    if (top + h > innerHeight - MENU_EDGE) top = innerHeight - MENU_EDGE - h;
    top = Math.max(MENU_EDGE, top);

    sub.root.style.left = `${Math.round(left)}px`;
    sub.root.style.top = `${Math.round(top)}px`;
    sub.root.style.transformOrigin = `${origin} top`;
    sub.root.style.visibility = '';
    void sub.root.offsetWidth;
    requestAnimationFrame(() => {
      if (menuCtl && menuCtl.panels.includes(sub)) sub.root.classList.add('on');
    });
    if (focusFirst) setMenuActive(sub, 0);
  }

  function menuPanel(items, level, parentEl) {
    const root = el('div', 'menu');
    root.dataset.level = String(level);
    root.setAttribute('role', 'menu');
    root.style.left = '0px';
    root.style.top = '0px';
    root.style.visibility = 'hidden';
    const panel = { root, level, parentEl, rows: [], active: -1, openFor: null };

    for (const item of items) {
      if (!item) continue;
      if (item.type === 'separator') { root.appendChild(el('div', 'menu-sep')); continue; }
      const row = el('div', 'menu-item' + (item.danger ? ' danger' : '') + (item.disabled ? ' disabled' : ''));
      row.setAttribute('role', 'menuitem');
      row.tabIndex = -1;
      if (item.title) row.title = item.title;
      if (item.disabled) row.setAttribute('aria-disabled', 'true');
      if (item.checked !== undefined) row.setAttribute('aria-checked', String(Boolean(item.checked)));
      if (item.children) {
        row.setAttribute('aria-haspopup', 'menu');
        row.setAttribute('aria-expanded', 'false');
      }

      const ico = row.appendChild(el('span', 'menu-ico'));
      if (item.dot) ico.appendChild(el('i', 'menu-dot')).style.background = item.dot;
      else if (item.icon) ico.innerHTML = svg(item.icon);
      row.appendChild(el('span', 'menu-label')).textContent = item.label;
      const end = row.appendChild(el('span', 'menu-end'));
      if (item.checked) end.appendChild(el('span', 'menu-check', svg('check')));
      else if (item.children) end.appendChild(el('span', 'menu-chev', svg('chevron')));
      if (item.hint) end.appendChild(el('span', 'menu-hint')).textContent = item.hint;

      const index = panel.rows.length;
      panel.rows.push({ item, row });
      row.addEventListener('pointerenter', () => {
        setMenuActive(panel, index);
        if (item.children) openSubmenu(panel, index);
        else closeSubmenusAfter(panel.level);
      });
      row.addEventListener('mousedown', (e) => { if (e.button === 0) e.preventDefault(); });
      row.addEventListener('click', (e) => {
        e.preventDefault();
        e.stopPropagation();
        if (item.disabled) return;
        if (item.children) { openSubmenu(panel, index, true); return; }
        closeMenu();
        if (item.action) item.action();
      });
      row.addEventListener('contextmenu', (e) => { e.preventDefault(); e.stopPropagation(); });
      root.appendChild(row);
    }
    return panel;
  }

  function onMenuPointerDown(e) {
    if (!menuCtl || e.button !== 0) return;
    if (menuCtl.panels.some((p) => p.root.contains(e.target))) return;
    closeMenu();
    clearClickGuard();
    const stop = (ev) => {
      ev.preventDefault();
      ev.stopPropagation();
      clearClickGuard();
    };
    clickGuard = stop;
    stop.timer = setTimeout(clearClickGuard, 700);
    document.addEventListener('click', stop, true);
  }

  function onMenuKey(e) {
    if (!menuCtl) return;
    const panel = menuCtl.panels[menuCtl.panels.length - 1];
    const key = e.key;
    if (key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      if (panel.level > 0) {
        const parent = menuCtl.panels[panel.level - 1];
        const idx = parent.openFor;
        closeSubmenusAfter(parent.level);
        setMenuActive(parent, idx == null ? -1 : idx);
      } else closeMenu();
      return;
    }
    if (key === 'ArrowDown' || key === 'ArrowUp' || key === 'Home' || key === 'End' || key === 'Tab') {
      e.preventDefault();
      e.stopPropagation();
      if (key === 'Home') edgeMenuActive(panel, false);
      else if (key === 'End') edgeMenuActive(panel, true);
      else moveMenuActive(panel, key === 'ArrowUp' || (key === 'Tab' && e.shiftKey) ? -1 : 1);
      return;
    }
    if (key === 'ArrowRight') {
      e.preventDefault();
      e.stopPropagation();
      if (panel.active >= 0 && panel.rows[panel.active].item.children) openSubmenu(panel, panel.active, true);
      return;
    }
    if (key === 'ArrowLeft') {
      e.preventDefault();
      e.stopPropagation();
      if (panel.level > 0) {
        const parent = menuCtl.panels[panel.level - 1];
        const idx = parent.openFor;
        closeSubmenusAfter(parent.level);
        setMenuActive(parent, idx == null ? -1 : idx);
      }
      return;
    }
    if (key === 'Enter' || key === ' ') {
      e.preventDefault();
      e.stopPropagation();
      if (panel.active < 0) return;
      const entry = panel.rows[panel.active];
      if (entry.item.disabled) return;
      if (entry.item.children) { openSubmenu(panel, panel.active, true); return; }
      closeMenu();
      if (entry.item.action) entry.item.action();
      return;
    }
    if ((key.length === 1 && !e.ctrlKey && !e.metaKey && !e.altKey) || key === 'Backspace') {
      closeMenu(false);
      return;
    }
    e.preventDefault();
    e.stopPropagation();
  }

  function onMenuBlur() { closeMenu(true, false); }
  function onMenuResize() { closeMenu(false, false); }
  function onMenuScroll(e) {
    if (!menuCtl) return;
    if (!menuCtl.panels.some((p) => p.root.contains(e.target))) closeMenu(false);
  }

  function closeMenu(animate = true, focus = true) {
    if (!menuCtl) return;
    const ctl = menuCtl;
    menuCtl = null;
    document.removeEventListener('pointerdown', onMenuPointerDown, true);
    document.removeEventListener('keydown', onMenuKey, true);
    window.removeEventListener('blur', onMenuBlur);
    window.removeEventListener('resize', onMenuResize);
    document.removeEventListener('scroll', onMenuScroll, true);
    for (const p of ctl.panels) dismissPanel(p, animate);
    if (focus) q.focus();
  }

  function clearClickGuard() {
    if (!clickGuard) return;
    document.removeEventListener('click', clickGuard, true);
    clearTimeout(clickGuard.timer);
    clickGuard = null;
  }

  function openMenu(items, x, y, label) {
    clearClickGuard();
    if (menuCtl) closeMenu(false, false);
    const panel = menuPanel(items, 0, null);
    const ctl = { panels: [panel] };
    menuCtl = ctl;
    panel.root.setAttribute('aria-label', label || 'Actions');
    panel.root.style.maxHeight = 'none';
    document.body.appendChild(panel.root);

    const w = panel.root.offsetWidth;
    const h = panel.root.offsetHeight;
    let left = Math.min(Math.max(x, MENU_EDGE), Math.max(MENU_EDGE, innerWidth - w - MENU_EDGE));
    const hasSub = items.some((it) => it && it.children);
    if (hasSub && innerWidth >= w + SUB_W + MENU_GAP + 2 * MENU_EDGE) {
      const subW = Math.min(SUB_W, innerWidth - 2 * MENU_EDGE);
      const rightFit = Math.min(left, innerWidth - MENU_EDGE - subW - MENU_GAP - w);
      const leftFit = Math.max(left, MENU_EDGE + subW + MENU_GAP);
      const rightOk = rightFit >= MENU_EDGE;
      const leftOk = leftFit <= innerWidth - w - MENU_EDGE;
      if (rightOk && leftOk) left = left - rightFit <= leftFit - left ? rightFit : leftFit;
      else if (rightOk) left = rightFit;
      else if (leftOk) left = leftFit;
    }

    const below = innerHeight - MENU_EDGE - y;
    const above = y - MENU_EDGE;
    let top;
    let maxH;
    let oy;
    if (h <= below) { top = y; maxH = below; oy = 'top'; }
    else if (h <= above) { top = y - h; maxH = above; oy = 'bottom'; }
    else if (below >= above) { top = y; maxH = below; oy = 'top'; }
    else { maxH = above; top = y - maxH; oy = 'bottom'; }
    panel.root.style.maxHeight = `${Math.max(1, maxH)}px`;
    const ox = left + w < x ? 'right' : 'left';
    panel.root.style.left = `${Math.round(left)}px`;
    panel.root.style.top = `${Math.round(top)}px`;
    panel.root.style.transformOrigin = `${ox} ${oy}`;
    panel.root.style.visibility = '';
    void panel.root.offsetWidth;
    requestAnimationFrame(() => {
      if (menuCtl === ctl) panel.root.classList.add('on');
    });

    document.addEventListener('pointerdown', onMenuPointerDown, true);
    document.addEventListener('keydown', onMenuKey, true);
    window.addEventListener('blur', onMenuBlur);
    window.addEventListener('resize', onMenuResize);
    document.addEventListener('scroll', onMenuScroll, true);
    q.focus();
  }

  function renderItem(r, i) {
    const c = r.clip;
    const row = el('div', 'item' + (i === sel ? ' sel' : '') + (c.kind === 'code' && !isSvg(c) && !dataUrlType(c) ? ' mono' : ''));
    row.setAttribute('role', 'option');
    row.dataset.i = i;

    const d = describe(c);
    const main = el('div', 'main');
    paint(main.appendChild(el('span', 'title')), d.title);
    if (d.sub) paint(main.appendChild(el('span', 'sub')), d.sub, c.kind !== 'image' && StashSecrets.privateKey(c.text));
    drawThumb(c);
    const long = i === sel && shiftDown && !c.thumb && textPreview(c);
    if (i === sel && shiftDown && c.thumb) {
      row.classList.add('expanded');
      if (isFontFile(c)) {
        main.appendChild(el('div', 'preview glyph')).appendChild(fontSample(c.thumb));
      } else {
        const box = main.appendChild(el('div', 'preview' + (c.kind !== 'image' && c.thumb.startsWith('data:image/png') ? ' alpha' : '')));
        const img = box.appendChild(el('img'));
        img.src = c.thumb;
        img.alt = '';
      }
    } else if (long) {
      row.classList.add('expanded', 'long');
    }
    if (colorMenuFor === c.id) {
      const menu = formatMenu(c);
      if (menu) {
        row.classList.add('expanded');
        main.appendChild(menu);
      }
    }

    const trail = el('div', 'trail');
    const info = trail.appendChild(el('span', 'info'));
    if (c.pinned) info.innerHTML = svg('pin');
    info.appendChild(el('span')).textContent = age(c.last_used);
    const acts = trail.appendChild(el('span', 'acts'));
    acts.append(
      action(c.pinned ? 'pinOff' : 'pin', `${c.pinned ? 'Unpin' : 'Pin'} (${MOD}P)`, () => pin(c)),
      action('trash', `Delete (${MOD}D)`, () => remove(c)),
    );
    if (c.kind === 'link') {
      acts.prepend(action('external', 'Open link', () => run('open_url', { id: c.id })));
    } else if (c.kind === 'file' || c.kind === 'path') {
      acts.prepend(action('folder', `Show in ${FILES_APP}`, () => run('reveal_in_explorer', { id: c.id })));
    } else if (c.kind === 'code') {
      acts.prepend(action('code', 'Copy as fenced code block', () => copyAndHide('copy_fence', { id: c.id, lang: guessLang(c.text) })));
    } else if (c.kind === 'color') {
      acts.prepend(action('color', colorMenuFor === c.id ? 'Hide colour formats' : 'Copy as colour…', () => toggleColorMenu(c, i)));
    }

    row.append(tile(c), main, trail);
    // Text gets the row's full width under the tile, wider than the title.
    if (long) {
      const box = row.appendChild(el('div', 'preview text'));
      const body = box.appendChild(el('div', 'body'));
      // A private key stays blurred from its BEGIN line on, like in the row.
      const k = StashSecrets.privateKey(c.text) ? long.text.indexOf('PRIVATE KEY-----') : -1;
      const cut = k < 0 ? long.text.length : k + 'PRIVATE KEY-----'.length;
      paint(body, long.text.slice(0, cut), false, Infinity);
      paint(body, long.text.slice(cut), true, Infinity);
      if (long.more) box.appendChild(el('div', 'more')).textContent = long.more;
    }
    row.addEventListener('mousemove', (e) => {
      if (menuCtl) return;
      setShift(e.shiftKey);
      const n = Number(row.dataset.i);
      if (sel !== n) select(n, false);
    });
    // Keep typing in the search field after pressing a row.
    row.addEventListener('mousedown', (e) => { if (e.button === 0) e.preventDefault(); });
    row.addEventListener('click', (e) => (stackClick(e) ? toggleStack(c) : paste(c)));
    return row;
  }

  function groupOf(c, now) {
    if (c.pinned) return 'Pinned';
    const today = new Date(now).setHours(0, 0, 0, 0);
    if (c.last_used >= today) return 'Today';
    if (c.last_used >= today - DAY) return 'Yesterday';
    if (c.last_used >= today - 6 * DAY) return 'This week';
    return 'Earlier';
  }

  const hl = el('div', 'hl');

  function placeHighlight(animate) {
    const row = listEl.querySelector('.item.sel');
    if (!row) { hl.style.opacity = '0'; return; }
    if (!animate) hl.style.transition = 'none';
    hl.style.transform = `translateY(${row.offsetTop}px)`;
    hl.style.height = row.offsetHeight + 'px';
    hl.style.opacity = '1';
    if (!animate) { void hl.offsetHeight; hl.style.transition = ''; }
  }

  function renderList() {
    listEl.textContent = '';
    listEl.appendChild(hl);
    if (!results.length) {
      const e = el('div', 'empty', svg(clips.length ? 'search' : 'clipboard'));
      e.appendChild(el('span')).textContent = clips.length ? 'No matches' : 'Copy something and it shows up here';
      listEl.appendChild(e);
      placeHighlight(false);
      return;
    }
    const grouped = !query() && sort === 'recent';
    const now = Date.now();
    const frag = document.createDocumentFragment();
    let last = null;
    results.slice(0, MAX_ROWS).forEach((r, i) => {
      if (grouped) {
        const g = groupOf(r.clip, now);
        if (g !== last) { frag.appendChild(el('div', 'group')).textContent = g; last = g; }
      }
      frag.appendChild(renderItem(r, i));
    });
    listEl.appendChild(frag);
    placeHighlight(false);
    scrollToSel();
  }

  function scrollToSel() {
    const row = listEl.querySelector('.item.sel');
    if (!row) return;
    const prev = row.previousElementSibling;
    const top = prev && prev.classList.contains('group') ? prev.offsetTop : row.offsetTop;
    const bottom = row.offsetTop + row.offsetHeight + 6;
    if (top < listEl.scrollTop) listEl.scrollTop = top - 4;
    else if (bottom > listEl.scrollTop + listEl.clientHeight) listEl.scrollTop = bottom - listEl.clientHeight;
  }

  let shiftDown = false;
  function setShift(on) {
    if (shiftDown === on) return;
    shiftDown = on;
    rerender(sel);
    placeHighlight(false);
    scrollToSel();
  }
  document.addEventListener('keydown', (e) => { if (e.key === 'Shift') setShift(true); });
  document.addEventListener('keyup', (e) => { if (e.key === 'Shift') setShift(false); });
  window.addEventListener('blur', () => setShift(false));

  function rerender(i) {
    const node = listEl.querySelector(`.item[data-i="${i}"]`);
    if (node && results[i]) node.replaceWith(renderItem(results[i], i));
  }

  function select(i, scroll = true) {
    const old = sel;
    if (i !== sel) colorMenuFor = null;
    sel = i;
    rerender(old);
    rerender(i);
    placeHighlight(true);
    if (scroll) scrollToSel();
  }

  function renderTabs(animate) {
    tabsEl.querySelectorAll('button').forEach((b) => b.remove());
    for (const [id, label] of TABS) {
      const b = el('button');
      b.textContent = label;
      b.setAttribute('role', 'tab');
      b.setAttribute('aria-selected', String(id === filter));
      noFocusSteal(b);
      b.addEventListener('click', () => { filter = id; refresh(); });
      tabsEl.appendChild(b);
    }
    const on = tabsEl.querySelector('[aria-selected="true"]');
    if (!animate) pill.style.transition = 'none';
    pill.style.transform = `translateX(${on.offsetLeft - 3}px)`;
    pill.style.width = on.offsetWidth + 'px';
    if (!animate) { void pill.offsetWidth; pill.style.transition = ''; }
    pinBtn.setAttribute('aria-pressed', String(pinnedOnly));
  }

  function renderFooter() {
    const on = stack.ids.length > 0;
    hintsEl.hidden = on || noAccess;
    accessEl.hidden = on || !noAccess;
    accessEl.textContent = '';
    if (noAccess && !on) {
      accessEl.appendChild(el('span', 'hint')).textContent = 'Copied, not pasted';
      accessEl.appendChild(el('span', 'spacer'));
      const allow = accessEl.appendChild(el('button', 'ghost text warn'));
      allow.textContent = 'Allow Accessibility to paste';
      allow.title = 'Open Privacy & Security > Accessibility and switch on stash';
      allow.addEventListener('click', () => run('open_access_settings'));
    }
    stackEl.hidden = !on;
    stackEl.textContent = '';
    if (!on) return;

    const left = stackEl.appendChild(el('span', 'stack-label', svg('stack')));
    if (stack.done) {
      const c = byId(stack.ids[0]);
      left.appendChild(el('span')).textContent = `Next ${stack.done + 1} of ${stack.done + stack.ids.length}`;
      if (c) left.appendChild(el('span', 'next')).textContent = describe(c).title;
    } else {
      left.appendChild(el('span')).textContent = 'Stack';
      left.appendChild(el('span', 'count')).textContent = String(stack.ids.length);
    }
    stackEl.appendChild(action('x', 'Clear stack', cancelStack)).classList.add('small');
    stackEl.appendChild(el('span', 'spacer'));
    if (stack.blocked) {
      const allow = stackEl.appendChild(el('button', 'ghost text warn'));
      allow.textContent = 'Allow Accessibility to paste in order';
      allow.title = 'Open Privacy & Security > Accessibility and switch on stash';
      allow.addEventListener('click', () => run('open_access_settings'));
    } else {
      stackEl.appendChild(el('span', 'hint', `<kbd>${MAC ? '⌘' : 'Ctrl'}</kbd><kbd>V</kbd>Paste next`));
    }
  }

  function refresh(keepSel = false) {
    colorMenuFor = null;
    results = StashSearch.run(clips, query(), { filter, sort, pinned: pinnedOnly });
    if (!keepSel) sel = 0;
    sel = Math.min(sel, Math.max(Math.min(results.length, MAX_ROWS) - 1, 0));
    sortBtn.innerHTML = svg('sort') + SORT_LABEL[sort];
    $('esc').classList.toggle('on', Boolean(query()));
    renderChips();
    renderTabs(true);
    renderList();
    renderFooter();
  }

  const run = (cmd, args) => Promise.resolve(call(cmd, args)).catch((e) => console.error(e));
  const paste = (c) => run('paste_clip', { id: c.id }).then((pasted) => {
    if (pasted === false) { noAccess = true; renderFooter(); }
  });
  const copyOnly = (c) => run('copy_clip', { id: c.id });

  const setStack = (s) => { stack = s || { ids: [], done: 0 }; refresh(true); };
  const toggleStack = (c) => run('toggle_stack', { id: c.id }).then((s) => s && setStack(s));
  const cancelStack = () => run('cancel_stack').then(() => setStack());
  const pasteStack = () => run('paste_stack');

  function pin(c) {
    c.pinned = !c.pinned;
    run('toggle_pin', { id: c.id });
    refresh(true);
  }

  function remove(c) {
    const idx = results.findIndex((r) => r.clip === c);
    const inStack = stack.ids.includes(c.id);
    clips = clips.filter((x) => x !== c);
    stack = { ...stack, ids: stack.ids.filter((id) => id !== c.id) };
    run('delete_clip', { id: c.id });
    if (idx < 0 || inStack || results.length > MAX_ROWS) { refresh(true); return; }

    results.splice(idx, 1);
    const node = listEl.querySelector(`.item[data-i="${idx}"]`);
    if (!node) { refresh(true); return; }
    const prev = node.previousElementSibling;
    const next = node.nextElementSibling;
    if (prev && prev.classList.contains('group') && (!next || !next.classList.contains('item'))) prev.remove();
    node.remove();
    listEl.querySelectorAll('.item').forEach((n, k) => { n.dataset.i = k; });
    if (!results.length) { refresh(true); return; }
    const old = sel;
    sel = Math.min(sel > idx ? sel - 1 : sel, results.length - 1);
    if (old !== sel || old === idx) rerender(sel);
    placeHighlight(false);
    scrollToSel();
  }

  const cycle = (list, cur, dir = 1) => list[(list.indexOf(cur) + dir + list.length) % list.length];
  const setSortTo = (v) => { sort = v; store.set('sort', sort); refresh(); };
  const setThemeTo = (v) => { theme = pickedTheme = v; store.set('theme', theme); applyTheme(); };
  const setSort = () => setSortTo(cycle(StashSearch.SORTS, sort));
  const setTheme = () => setThemeTo(cycle(THEMES, theme));

  function move(delta) {
    const n = Math.min(results.length, MAX_ROWS);
    if (n) select(Math.min(Math.max(sel + delta, 0), n - 1));
  }

  document.addEventListener('keydown', (e) => {
    if (view !== 'home') { onPrefsKey(e); return; }
    const ctrl = e.ctrlKey || e.metaKey;
    const cur = results[sel] && results[sel].clip;
    if (ctrl && e.key === ',') showView('prefs');
    else if (ctrl && /^[1-9]$/.test(e.key)) { const r = results[Number(e.key) - 1]; if (r) paste(r.clip); }
    else if (e.key === 'ArrowDown' || (ctrl && e.key === 'j')) move(1);
    else if (e.key === 'ArrowUp' || (ctrl && e.key === 'k')) move(-1);
    else if (e.key === 'PageDown') move(6);
    else if (e.key === 'PageUp') move(-6);
    else if (e.key === 'Enter' && ctrl) { if (cur) copyOnly(cur); }
    else if (e.key === 'Enter') { if (stack.ids.length) pasteStack(); else if (cur) paste(cur); }
    else if (e.key === 'Escape') {
      if (q.value || chips.length) { q.value = ''; chips = []; refresh(); }
      else run('hide_window');
    }
    else if (e.key === 'Backspace' && !q.value && q.selectionStart === 0 && chips.length) { chips.pop(); refresh(); }
    else if (e.key === 'Tab') { filter = cycle(TABS.map((t) => t[0]), filter, e.shiftKey ? -1 : 1); refresh(); }
    else if (ctrl && e.key === 'p') { if (cur) pin(cur); }
    else if (ctrl && e.key === 'd') { if (cur) remove(cur); }
    else if ((e.key === 'Delete' || (MAC && e.metaKey && e.key === 'Backspace')) && !q.value) { if (cur) remove(cur); }
    else if (ctrl && e.key === 's') setSort();
    else if (ctrl && e.key === 't') setTheme();
    else return;
    e.preventDefault();
  });

  q.addEventListener('input', () => { absorb(); refresh(); });

  document.addEventListener('contextmenu', (e) => {
    e.preventDefault();
    if (view !== 'home') return;
    if (menuCtl && menuCtl.panels.some((p) => p.root.contains(e.target))) return;
    const target = e.target instanceof Element ? e.target : e.target && e.target.parentElement;
    if (!target) return;

    const row = target.closest('.item');
    if (row && row.dataset.i !== undefined) {
      const i = Number(row.dataset.i);
      const r = results[i];
      if (r) {
        select(i, false);
        openMenu(clipMenu(r.clip), e.clientX, e.clientY, 'Clip actions');
        return;
      }
    }

    const chip = target.closest('.chip');
    if (chip && chip.dataset.token) {
      openMenu(chipMenu(chip.dataset.token), e.clientX, e.clientY, 'Filter actions');
      return;
    }

    if (target.closest('#stackbar')) {
      openMenu(stackMenu(), e.clientX, e.clientY, 'Stack actions');
      return;
    }

    if (target.closest('.search')) {
      openMenu(searchMenu(), e.clientX, e.clientY, 'Search actions');
      return;
    }

    openMenu(appMenu(), e.clientX, e.clientY, 'stash actions');
  });

  [pinBtn, sortBtn, themeBtn, settingsBtn].forEach(noFocusSteal);
  pinBtn.innerHTML = svg('pin');
  pinBtn.addEventListener('click', () => { pinnedOnly = !pinnedOnly; refresh(); });
  sortBtn.addEventListener('click', setSort);
  themeBtn.addEventListener('click', setTheme);
  settingsBtn.innerHTML = svg('settings');
  settingsBtn.addEventListener('click', () => showView('prefs'));

  // Settings are a second view in the same card. Switching fades the old view
  // out, then morphs the card to the new height while the new view slides in.
  const KEEP = [500, 1000, 2000, 5000, 10000];
  const VIEW_OUT = 110;
  const VIEW_MORPH = 240;
  const reducedMotion = matchMedia('(prefers-reduced-motion: reduce)');
  const wait = (ms) => new Promise((done) => setTimeout(done, ms));
  const prefHl = el('div', 'hl');
  let view = 'home';
  let morphing = false;
  let prefs = null;
  let prefRows = [];
  let recording = null;

  function fitCard() {
    run('fit_window', { height: Math.ceil(card.getBoundingClientRect().height) })
      .then((above) => document.body.classList.toggle('above', Boolean(above)));
  }

  async function showView(next, animate = true) {
    if (next === view || morphing) return;
    stopRecording();
    closeMenu(false, false);
    if (next === 'prefs') await loadPrefs();
    const from = view === 'home' ? homeEl : prefsEl;
    const to = next === 'home' ? homeEl : prefsEl;
    const dir = next === 'prefs' ? 1 : -1;
    view = next;
    if (!animate || reducedMotion.matches) {
      from.hidden = true;
      to.style.setProperty('--dx', '0px');
      to.hidden = false;
      settle();
      return;
    }
    morphing = true;
    const h0 = card.offsetHeight;
    from.style.setProperty('--dx', `${-8 * dir}px`);
    from.classList.add('leave');
    await wait(VIEW_OUT);
    from.classList.remove('leave');
    from.hidden = true;
    to.style.setProperty('--dx', `${12 * dir}px`);
    to.hidden = false;
    settle();
    const h1 = card.offsetHeight;
    card.style.height = `${h0}px`;
    // Grow the window first so the card can animate inside it; it is fitted again at the end.
    const above = await run('fit_window', { height: Math.max(h0, h1) });
    document.body.classList.toggle('above', Boolean(above));
    card.classList.add('morph');
    void card.offsetHeight;
    card.style.height = `${h1}px`;
    await wait(VIEW_MORPH);
    card.classList.remove('morph');
    card.style.height = '';
    morphing = false;
    fitCard();
  }

  // Lays out the view that just became visible.
  function settle() {
    if (view === 'home') {
      refresh(true);
      renderTabs(false);
      q.focus();
      return;
    }
    prefsBody.scrollTop = 0;
    syncPrefs(false);
    fadeEdges();
    placePrefHl(null);
    prefsBody.focus({ preventScroll: true });
  }

  async function loadPrefs() {
    try { prefs = await call('get_settings'); } catch (e) { console.error(e); }
    if (!prefs) prefs = { config: { hotkey: 'Ctrl+Alt+V', max_items: 2000 }, autostart: false, paused: false, version: '' };
    $('version').textContent = prefs.version ? `v${prefs.version}` : '';
    buildPrefs();
  }

  async function savePref(key, value) {
    const next = { ...prefs.config, [key]: value };
    prefs.config = next;
    syncPrefs();
    prefs.config = (await run('set_config', { config: next })) || next;
    syncPrefs();
  }

  async function setAutostart(on) {
    prefs.autostart = on;
    syncPrefs();
    const now = await run('set_autostart', { on });
    if (now !== undefined) prefs.autostart = Boolean(now);
    syncPrefs();
  }

  function setPaused(on) {
    prefs.paused = on;
    syncPrefs();
    run('set_paused', { on });
  }

  const syncPrefs = (animate = true) => prefRows.forEach((r) => r.sync(animate));

  // The shortcut is stored the way the global-shortcut parser reads it ("Ctrl+Alt+V")
  // and shown as keycaps: ⌃ ⌥ V on macOS, Ctrl Alt V elsewhere.
  const MOD_NAME = { CTRL: 'Ctrl', CONTROL: 'Ctrl', ALT: 'Alt', OPTION: 'Alt', SHIFT: 'Shift', CMD: 'Cmd', COMMAND: 'Cmd', SUPER: 'Cmd',
    CMDORCTRL: MAC ? 'Cmd' : 'Ctrl', CMDORCONTROL: MAC ? 'Cmd' : 'Ctrl', COMMANDORCTRL: MAC ? 'Cmd' : 'Ctrl', COMMANDORCONTROL: MAC ? 'Cmd' : 'Ctrl' };
  const MOD_ORDER = ['Ctrl', 'Alt', 'Shift', 'Cmd'];
  const MOD_CAP = MAC ? { Ctrl: '⌃', Alt: '⌥', Shift: '⇧', Cmd: '⌘' } : { Ctrl: 'Ctrl', Alt: 'Alt', Shift: 'Shift', Cmd: 'Win' };
  const KEY_CAP = { ArrowUp: '↑', ArrowDown: '↓', ArrowLeft: '←', ArrowRight: '→', Comma: ',', Period: '.', Slash: '/', Semicolon: ';',
    Quote: "'", Backquote: '`', BracketLeft: '[', BracketRight: ']', Backslash: '\\', Minus: '-', Equal: '=', Enter: '↵',
    Backspace: '⌫', Delete: 'Del', PageUp: 'PgUp', PageDown: 'PgDn' };
  const HOTKEY_CODE = /^(Key[A-Z]|Digit\d|F([1-9]|1[0-2])|Space|Comma|Period|Slash|Semicolon|Quote|Backquote|BracketLeft|BracketRight|Backslash|Minus|Equal|Arrow(Up|Down|Left|Right)|Enter|Backspace|Delete|Home|End|PageUp|PageDown|Insert)$/;
  const keyName = (code) => code.replace(/^(?:Key|Digit)(.)$/, '$1');

  function hotkeyCaps(hotkey) {
    const parts = hotkey.split('+').map((p) => p.trim()).filter(Boolean);
    const key = keyName(parts.pop() || '');
    const mods = parts.map((p) => MOD_NAME[p.toUpperCase()] || p).sort((a, b) => MOD_ORDER.indexOf(a) - MOD_ORDER.indexOf(b));
    return [...mods.map((m) => MOD_CAP[m] || m), KEY_CAP[key] || (key.length === 1 ? key.toUpperCase() : key)];
  }

  const heldMods = (e) => [e.ctrlKey && 'Ctrl', e.altKey && 'Alt', e.shiftKey && 'Shift', e.metaKey && 'Cmd'].filter(Boolean);

  function paintCaps(box, caps, pop) {
    box.textContent = '';
    for (const cap of caps) box.appendChild(el('kbd', pop ? 'pop' : '')).textContent = cap;
  }

  function rowShell(tag, icon, label, sub) {
    const row = el(tag, 'set-row');
    row.appendChild(el('span', 'set-ico', svg(icon)));
    const text = row.appendChild(el('span', 'set-text'));
    text.appendChild(el('span', 'set-label')).textContent = label;
    const subEl = text.appendChild(el('span', 'set-sub'));
    subEl.textContent = sub || '';
    subEl.hidden = !sub;
    return { row, subEl };
  }

  function setSub(r, text, bad = false) {
    r.subEl.textContent = text;
    r.subEl.hidden = !text;
    r.subEl.classList.toggle('bad', bad);
  }

  function shake(node) {
    node.classList.remove('bad');
    void node.offsetWidth;
    node.classList.add('bad');
  }

  // The whole row is the switch.
  function switchRow({ icon, label, sub, get, set, disabled }) {
    const { row } = rowShell('button', icon, label, sub);
    row.classList.add('ctl');
    row.setAttribute('role', 'switch');
    row.appendChild(el('span', 'switch'));
    row.addEventListener('click', () => set(!get()));
    return {
      row,
      ctl: row,
      sync() {
        row.setAttribute('aria-checked', String(Boolean(get())));
        row.disabled = Boolean(disabled && disabled());
      },
    };
  }

  function segRow({ icon, label, sub, options, get, set }) {
    const { row } = rowShell('div', icon, label, sub);
    const seg = row.appendChild(el('div', 'seg ctl'));
    seg.tabIndex = 0;
    seg.setAttribute('role', 'radiogroup');
    seg.setAttribute('aria-label', label);
    const pill = seg.appendChild(el('span', 'seg-pill'));
    const buttons = options.map(([value, html, title]) => {
      const b = seg.appendChild(el('button', '', html));
      b.tabIndex = -1;
      b.setAttribute('role', 'radio');
      if (title) b.title = title;
      b.addEventListener('click', () => { if (value !== get()) set(value); seg.focus({ preventScroll: true }); });
      return [value, b];
    });
    seg.addEventListener('keydown', (e) => {
      if (e.key !== 'ArrowLeft' && e.key !== 'ArrowRight') return;
      e.preventDefault();
      e.stopPropagation();
      const values = buttons.map(([v]) => v);
      const next = values[clamp(values.indexOf(get()) + (e.key === 'ArrowRight' ? 1 : -1), 0, values.length - 1)];
      if (next !== get()) set(next);
    });
    return {
      row,
      ctl: seg,
      sync(animate = true) {
        let on = null;
        for (const [v, b] of buttons) {
          b.setAttribute('aria-checked', String(v === get()));
          if (v === get()) on = b;
        }
        if (!on || !seg.offsetWidth) return;
        if (!animate) pill.style.transition = 'none';
        pill.style.transform = `translateX(${on.offsetLeft - 2}px)`;
        pill.style.width = on.offsetWidth + 'px';
        if (!animate) { void pill.offsetWidth; pill.style.transition = ''; }
      },
    };
  }

  function hotkeyRow() {
    const sub = 'Opens stash from any app';
    const { row, subEl } = rowShell('div', 'keyboard', 'Shortcut', sub);
    const btn = row.appendChild(el('button', 'keys ctl'));
    btn.title = 'Change shortcut';
    let shown = null;
    const r = {
      row,
      ctl: btn,
      subEl,
      sub,
      sync() {
        if (recording === r || shown === prefs.config.hotkey) return;
        paintCaps(btn, hotkeyCaps(prefs.config.hotkey), shown !== null);
        shown = prefs.config.hotkey;
      },
      repaint() { shown = null; r.sync(); },
    };
    btn.addEventListener('click', () => (recording ? stopRecording() : startRecording(r)));
    return r;
  }

  function startRecording(r) {
    recording = r;
    r.ctl.classList.remove('bad');
    r.ctl.classList.add('rec');
    r.ctl.innerHTML = '<span class="rec-dot"></span><span class="rec-text">Press keys</span>';
    setSub(r, `Hold ${MAC ? '⌘, ⌃ or ⌥' : 'Ctrl or Alt'} and press a key · Esc cancels`);
  }

  function stopRecording() {
    const r = recording;
    if (!r) return;
    recording = null;
    r.ctl.classList.remove('rec');
    setSub(r, r.sub);
    r.repaint();
  }

  function onRecordKey(e) {
    if (!recording) return;
    e.preventDefault();
    e.stopPropagation();
    const r = recording;
    const mods = heldMods(e);
    if (e.type === 'keyup' || ['Control', 'Alt', 'Shift', 'Meta'].includes(e.key)) {
      if (mods.length) paintCaps(r.ctl, [...mods.map((m) => MOD_CAP[m]), '…'], false);
      else startRecording(r);
      return;
    }
    if (e.key === 'Escape') { stopRecording(); return; }
    const fn = /^F\d+$/.test(e.code);
    if (!HOTKEY_CODE.test(e.code) || !(e.ctrlKey || e.altKey || e.metaKey || fn)) { shake(r.ctl); return; }
    const hotkey = [...mods.map((m) => (m === 'Cmd' && !MAC ? 'Super' : m)), keyName(e.code)].join('+');
    stopRecording();
    setHotkey(r, hotkey);
  }
  document.addEventListener('keydown', onRecordKey, true);
  document.addEventListener('keyup', onRecordKey, true);
  window.addEventListener('blur', stopRecording);

  async function setHotkey(r, hotkey) {
    if (hotkey === prefs.config.hotkey) return;
    await savePref('hotkey', hotkey);
    if (prefs.config.hotkey === hotkey) return;
    shake(r.ctl);
    setSub(r, `${hotkeyCaps(hotkey).join(MAC ? '' : '+')} is taken, try another`, true);
  }

  function clearRow() {
    const { row, subEl } = rowShell('div', 'trash', 'Clear history', '');
    const btn = row.appendChild(el('button', 'pbtn danger ctl'));
    btn.textContent = 'Clear';
    let armed = null;
    const reset = () => {
      clearTimeout(armed);
      armed = null;
      btn.classList.remove('arm', 'done');
      btn.textContent = 'Clear';
      r.sync();
    };
    const r = {
      row,
      ctl: btn,
      sync() {
        const n = clips.filter((c) => !c.pinned).length;
        const pinned = clips.length - n;
        const text = n ? `${n.toLocaleString()} clip${n === 1 ? '' : 's'}${pinned ? ` · ${pinned} pinned stay` : ''}` : 'Nothing to clear';
        setSub(r, text);
        btn.disabled = !n && !btn.classList.contains('done');
      },
      subEl,
    };
    // First click arms the button for three seconds, the second one clears.
    btn.addEventListener('click', async () => {
      if (btn.classList.contains('done')) return;
      if (!armed) {
        btn.classList.add('arm');
        btn.textContent = 'Confirm';
        armed = setTimeout(reset, 3000);
        return;
      }
      clearTimeout(armed);
      armed = null;
      await run('clear_history');
      try { clips = await call('list_clips'); } catch (e) { console.error(e); }
      btn.classList.remove('arm');
      btn.classList.add('done');
      btn.innerHTML = svg('check') + 'Cleared';
      r.sync();
      armed = setTimeout(reset, 1600);
    });
    return r;
  }

  function buildPrefs() {
    prefsBody.textContent = '';
    prefsBody.appendChild(prefHl);
    const flag = (key) => ({ get: () => prefs.config[key], set: (v) => savePref(key, v) });
    const short = (n) => (n >= 1000 ? `${n / 1000}k` : String(n));
    const keep = [...new Set([...KEEP, prefs.config.max_items])].filter(Boolean).sort((a, b) => a - b);
    const rows = [
      'General',
      hotkeyRow(),
      switchRow({ icon: 'power', label: MAC ? 'Open at login' : 'Start with Windows', get: () => prefs.autostart, set: setAutostart }),
      segRow({
        icon: 'contrast', label: 'Theme',
        options: THEMES.map((t) => [t, svg(t), t[0].toUpperCase() + t.slice(1)]),
        get: () => theme, set: (t) => { setThemeTo(t); syncPrefs(); },
      }),
      'Capture',
      switchRow({ icon: 'pause', label: 'Pause capturing', sub: 'New copies are ignored until you resume', get: () => prefs.paused, set: setPaused }),
      switchRow({ icon: 'image', label: 'Images', ...flag('capture_images') }),
      switchRow({ icon: 'scan', label: 'Text in images', sub: 'Recognized on this device, for search', ...flag('ocr_images'), disabled: () => !prefs.config.capture_images }),
      switchRow({ icon: 'file', label: 'Files', sub: 'Keeps the paths, not the files', ...flag('capture_files') }),
      MAC && switchRow({ icon: 'camera', label: 'Screenshots', sub: '⌘⇧3 and ⌘⇧4 also go on the clipboard', ...flag('capture_screenshots') }),
      switchRow({ icon: 'key', label: 'Skip passwords', sub: 'Copies password managers mark as private', ...flag('ignore_sensitive') }),
      'History',
      segRow({
        icon: 'history', label: 'Keep', sub: 'Pinned clips don’t count',
        options: keep.map((n) => [n, short(n), `Keep the last ${n.toLocaleString()} clips`]),
        get: () => prefs.config.max_items, set: (n) => savePref('max_items', n),
      }),
      clearRow(),
    ].filter(Boolean);
    prefRows = [];
    rows.forEach((r, i) => {
      const node = typeof r === 'string' ? el('div', 'group') : r.row;
      if (typeof r === 'string') node.textContent = r;
      else {
        prefRows.push(r);
        node.addEventListener('pointerenter', () => placePrefHl(node));
      }
      node.style.setProperty('--i', i);
      prefsBody.appendChild(node);
    });
    syncPrefs(false);
  }

  function placePrefHl(row) {
    if (!row) { prefHl.style.opacity = '0'; return; }
    const appearing = prefHl.style.opacity !== '1';
    if (appearing) prefHl.style.transition = 'none';
    prefHl.style.transform = `translateY(${row.offsetTop}px)`;
    prefHl.style.height = row.offsetHeight + 'px';
    if (appearing) { void prefHl.offsetHeight; prefHl.style.transition = ''; }
    prefHl.style.opacity = '1';
  }

  const focusedRow = () => (prefsBody.contains(document.activeElement) ? document.activeElement.closest('.set-row') : null);
  prefsBody.addEventListener('focusin', () => placePrefHl(focusedRow()));
  prefsBody.addEventListener('pointerleave', () => placePrefHl(focusedRow()));

  function fadeEdges() {
    const b = prefsBody;
    b.style.setProperty('--fade-top', b.scrollTop > 1 ? '20px' : '0px');
    b.style.setProperty('--fade-bottom', b.scrollTop + b.clientHeight < b.scrollHeight - 1 ? '20px' : '0px');
  }
  prefsBody.addEventListener('scroll', fadeEdges, { passive: true });

  function onPrefsKey(e) {
    const ctrl = e.ctrlKey || e.metaKey;
    if (e.key === 'Escape' || (ctrl && e.key === ',')) { e.preventDefault(); showView('home'); return; }
    if (e.key !== 'ArrowDown' && e.key !== 'ArrowUp') return;
    e.preventDefault();
    const ctls = prefRows.map((r) => r.ctl).filter((c) => !c.disabled);
    const i = ctls.indexOf(document.activeElement);
    const down = e.key === 'ArrowDown';
    const next = ctls[i < 0 ? (down ? 0 : ctls.length - 1) : clamp(i + (down ? 1 : -1), 0, ctls.length - 1)];
    if (!next) return;
    next.focus({ preventScroll: true });
    const row = next.closest('.set-row');
    const prev = row.previousElementSibling;
    const top = prev && prev.classList.contains('group') ? prev.offsetTop : row.offsetTop;
    if (top < prefsBody.scrollTop) prefsBody.scrollTop = top - 4;
    else if (row.offsetTop + row.offsetHeight + 6 > prefsBody.scrollTop + prefsBody.clientHeight) {
      prefsBody.scrollTop = row.offsetTop + row.offsetHeight + 6 - prefsBody.clientHeight;
    }
  }

  const backBtn = $('back'), configBtn = $('config-file');
  backBtn.innerHTML = svg('back');
  backBtn.addEventListener('click', () => showView('home'));
  configBtn.innerHTML = svg('file') + 'config.json';
  configBtn.addEventListener('click', () => run('reveal_config'));
  [backBtn, configBtn].forEach(noFocusSteal);

  async function open(target) {
    clearClickGuard();
    closeMenu(false, false);
    stopRecording();
    followSystem();
    q.value = params.get('q') || '';
    chips = [];
    absorb(true);
    filter = params.get('tab') || 'all';
    pinnedOnly = false;
    noAccess = false;
    try { clips = await call('list_clips'); } catch (e) { console.error(e); clips = []; }
    try { stack = (await call('get_stack')) || stack; } catch {}
    view = target === 'settings' ? 'prefs' : 'home';
    if (view === 'prefs') await loadPrefs();
    homeEl.hidden = view !== 'home';
    prefsEl.hidden = view !== 'prefs';
    prefsEl.style.setProperty('--dx', '0px');
    card.classList.remove('enter');
    void card.offsetWidth;
    card.classList.add('enter');
    refresh();
    renderTabs(false);
    if (params.get('sel')) select(Number(params.get('sel')), true);
    if (view === 'prefs') settle();
    else q.focus();
    // The window stays see-through until this opening has been drawn.
    requestAnimationFrame(() => requestAnimationFrame(() => run('reveal_window')));
  }

  let queued = false;
  new ResizeObserver(() => {
    if (queued || morphing) return;
    queued = true;
    requestAnimationFrame(() => {
      queued = false;
      if (!morphing) fitCard();
    });
  }).observe(card);

  if (MAC) {
    hintsEl.querySelectorAll('kbd').forEach((k) => { if (k.textContent === 'Ctrl') k.textContent = '⌘'; });
    sortBtn.title = 'Sort (⌘S)';
    settingsBtn.title = 'Settings (⌘,)';
  }
  applyTheme();
  listen('stash-show', (e) => open(e && e.payload));
  listen('stash-view', (e) => showView(e.payload === 'settings' ? 'prefs' : 'home'));
  listen('stash-stack', (e) => setStack(e.payload));
  window.addEventListener('focus', () => {
    if (view === 'home') q.focus();
    else if (!prefsEl.contains(document.activeElement)) prefsBody.focus({ preventScroll: true });
  });
  open(params.get('view'));

  function mockInvoke(cmd, args) {
    if (cmd === 'list_clips') return Promise.resolve(window.STASH_DEMO || demoClips());
    if (cmd === 'visual_source') {
      if (args.id === 18) return fetch('fonts/geist.woff2').then((r) => r.arrayBuffer());
      const src = demoVisuals()[args.id];
      if (!src) return Promise.reject(new Error('nothing to draw'));
      if (src.startsWith('data:')) return fetch(src).then((r) => r.arrayBuffer());
      const svg = new TextEncoder().encode(src).buffer;
      if (args.id !== 7 && args.id !== 19) return Promise.resolve(svg);
      // The trip photos and the screenshot are raster images, so render the scene into one.
      const img = new Image();
      img.src = URL.createObjectURL(new Blob([svg], { type: 'image/svg+xml' }));
      return new Promise((loaded) => { img.onload = loaded; }).then(() => {
        const cv = Object.assign(document.createElement('canvas'), { width: 1600, height: 1050 });
        cv.getContext('2d').drawImage(img, 0, 0, cv.width, cv.height);
        return new Promise((done) => cv.toBlob(done, 'image/jpeg', 0.9));
      }).then((b) => b.arrayBuffer());
    }
    if (cmd === 'get_settings') return Promise.resolve(JSON.parse(JSON.stringify(mockPrefs)));
    if (cmd === 'set_config') {
      // Pretend system shortcuts on Space are taken, to show the error state.
      const taken = /^(Cmd|Super|Ctrl)\+Space$/.test(args.config.hotkey);
      mockPrefs.config = { ...args.config, hotkey: taken ? mockPrefs.config.hotkey : args.config.hotkey };
      return Promise.resolve({ ...mockPrefs.config });
    }
    if (cmd === 'set_autostart') { mockPrefs.autostart = args.on; return Promise.resolve(args.on); }
    if (cmd === 'set_paused') mockPrefs.paused = args.on;
    if (cmd === 'clear_history') {
      window.STASH_DEMO = (window.STASH_DEMO || demoClips()).filter((c) => c.pinned);
    }
    if (!mockStack) mockStack = { ids: (params.get('stack') || '').split(',').filter(Boolean).map(Number), done: Number(params.get('done')) || 0 };
    if (cmd === 'toggle_stack') {
      const has = mockStack.ids.includes(args.id);
      mockStack.ids = has ? mockStack.ids.filter((id) => id !== args.id) : [...mockStack.ids, args.id];
      if (!mockStack.ids.length) mockStack.done = 0;
    }
    if (cmd === 'paste_clip') return Promise.resolve(!params.has('blocked'));
    if (cmd === 'cancel_stack') mockStack = { ids: [], done: 0 };
    if (cmd === 'get_stack' || cmd === 'toggle_stack') return Promise.resolve({ ...mockStack, blocked: params.has('blocked') && mockStack.ids.length > 0 });
    return Promise.resolve();
  }

  // Sources the demo hands out for its trip photos (7), logo file (12), copied icon (17), screenshot path (19) and data URL (20).
  function demoVisuals() {
    const logo = '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><rect x="6" y="6" width="52" height="52" rx="14" fill="#6366f1"/><path d="M22 40 L32 18 L42 40 Z" fill="#fff"/></svg>';
    const icon = '<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="#f97316" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 2v4"/><path d="m16.2 7.8 2.9-2.9"/><path d="M18 12h4"/><circle cx="12" cy="12" r="4"/></svg>';
    const trip = '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 320 210"><defs><linearGradient id="s" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#0c4a6e"/><stop offset="1" stop-color="#7dd3fc"/></linearGradient></defs><rect width="320" height="210" fill="url(#s)"/><path d="M0 140 L80 70 L140 120 L210 50 L320 130 L320 210 L0 210Z" fill="#f8fafc"/><path d="M0 170 L120 130 L200 165 L320 140 L320 210 L0 210Z" fill="#14532d"/></svg>';
    const dataUrl = 'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAACAAAAAgCAYAAABzenr0AAAEqUlEQVR42hXOIQuGBgIA0PthV65YLBaLYcIhAxcME4YMvGCYMGTggmHCkIELhhMOGXjBcMIhAxcMZ7n/cve+8Pr705+T//7vLwSERMQkpGTkFJRU1DS0dPQMjEzMLKxs7BycXNw8vPzpz18IEBASEZOQkpFTUFJR09DS0TMwMjGzsLKxc3BycfPwfvEJ/FWAgJCImISUjJyCkoqahpaOnoGRiZmFlY2dg5OLm4f3r5/AlwIEhETEJKRk5BSUVNQ0tHT0DIxMzCysbOwcnFzcPLxffgJfCRAQEhGTkJKRU1BSUdPQ0tEzMDIxs7CysXNwcnHz8H71CRQCBIRExCSkZOQUlFTUNLR09AyMTMwsrGzsHJxc3Dy8xSfwtQABIRExCSkZOQUlFTUNLR09AyMTMwsrGzsHJxc3D+/Xn8A3AgSERMQkpGTkFJRU1DS0dPQMjEzMLKxs7BycXNw8vN98At8KEBASEZOQkpFTUFJR09DS0TMwMjGzsLKxc3BycfPwfvsJ/E2AgJCImISUjJyCkoqahpaOnoGRiZmFlY2dg5OLm4f3b59AI0BASERMQkpGTkFJRU1DS0fPwMjEzMLKxs7BycXNw9t8At8JEBASEZOQkpFTUFJR09DS0TMwMjGzsLKxc3BycfPwfvcJfC9AQEhETEJKRk5BSUVNQ0tHz8DIxMzCysbOwcnFzcP7/SfwgwABIRExCSkZOQUlFTUNLR09AyMTMwsrGzsHJxc3D+8Pn8CPAgSERMQkpGTkFJRU1DS0dPQMjEzMLKxs7BycXNw8vD9+AoMAASERMQkpGTkFJRU1DS0dPQMjEzMLKxs7BycXNw/v8An8JEBASERMQkpGTkFJRU1DS0fPwMjEzMLKxs7BycXNw/vTJ/CzAAEhETEJKRk5BSUVNQ0tHT0DIxMzCysbOwcnFzcP78+fwC8CBIRExCSkZOQUlFTUNLR09AyMTMwsrGzsHJxc3Dy8v3wCvwoQEBIRk5CSkVNQUlHT0NLRMzAyMbOwsrFzcHJx8/D++gksAgSERMQkpGTkFJRU1DS0dPQMjEzMLKxs7BycXNw8vMsn8HcBAkIiYhJSMnIKSipqGlo6egZGJmYWVjZ2Dk4ubh7ev38C/xAgICQiJiElI6egpKKmoaWjZ2BkYmZhZWPn4OTi5uH9xyfwmwABIRExCSkZOQUlFTUNLR09AyMTMwsrGzsHJxc3D+9vn8A/BQgIiYhJSMnIKSipqGlo6egZGJmYWVjZ2Dk4ubh5eP/5CRwCBIRExCSkZOQUlFTUNLR09AyMTMwsrGzsHJxc3Dy8xyfwLwECQiJiElIycgpKKmoaWjp6BkYmZhZWNnYOTi5uHt5/fQL/FiAgJCImISUjp6CkoqahpaNnYGRiZmFlY+fg5OLm4f33J/C7AAEhETEJKRk5BSUVNQ0tHT0DIxMzCysbOwcnFzcP7++fwB8CBIRExCSkZOQUlFTUNLR09AyMTMwsrGzsHJxc3Dy8f3wCjwABIRExCSkZOQUlFTUNLR09AyMTMwsrGzsHJxc3D+/zCfxHgICQiJiElIycgpKKmoaWjp6BkYmZhZWNnYOTi5uHl/8Dagnypja8P7UAAAAASUVORK5CYII=';
    return { 7: trip, 12: logo, 17: icon, 19: trip, 20: dataUrl };
  }

  function demoClips() {
    const now = Date.now(), m = 60000;
    const svgUrl = (body, w, h) => 'data:image/svg+xml,' + encodeURIComponent(`<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${h}" viewBox="0 0 ${w} ${h}">${body}</svg>`);
    const dashboard = svgUrl('<rect width="320" height="200" fill="#18181b"/><rect x="0" y="0" width="64" height="200" fill="#09090b"/><rect x="12" y="16" width="40" height="6" rx="3" fill="#3f3f46"/><rect x="12" y="34" width="32" height="6" rx="3" fill="#27272a"/><rect x="12" y="50" width="36" height="6" rx="3" fill="#27272a"/><rect x="80" y="16" width="70" height="40" rx="6" fill="#27272a"/><rect x="160" y="16" width="70" height="40" rx="6" fill="#27272a"/><rect x="240" y="16" width="66" height="40" rx="6" fill="#27272a"/><rect x="88" y="26" width="30" height="6" rx="3" fill="#71717a"/><rect x="88" y="38" width="44" height="10" rx="3" fill="#fafafa"/><rect x="168" y="38" width="38" height="10" rx="3" fill="#fafafa"/><rect x="248" y="38" width="30" height="10" rx="3" fill="#fafafa"/>' + [40, 62, 48, 80, 70, 96, 88, 120, 104, 130].map((h, i) => `<rect x="${86 + i * 22}" y="${184 - h}" width="14" height="${h}" rx="3" fill="${i === 9 ? '#fafafa' : '#52525b'}"/>`).join(''), 320, 200);
    const photo = svgUrl('<defs><linearGradient id="s" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#1e1b4b"/><stop offset=".55" stop-color="#f97316"/><stop offset="1" stop-color="#fde68a"/></linearGradient></defs><rect width="320" height="210" fill="url(#s)"/><circle cx="210" cy="120" r="28" fill="#fff7ed" opacity=".9"/><path d="M0 150 L70 95 L120 135 L180 80 L250 140 L320 105 L320 210 L0 210Z" fill="#1c1917"/><path d="M0 175 L90 135 L160 170 L240 130 L320 165 L320 210 L0 210Z" fill="#0c0a09"/>', 320, 210);
    const whiteboard = svgUrl('<rect width="300" height="200" fill="#fafafa"/><rect x="20" y="30" width="80" height="44" rx="8" fill="none" stroke="#18181b" stroke-width="2"/><rect x="200" y="30" width="80" height="44" rx="8" fill="none" stroke="#18181b" stroke-width="2"/><rect x="110" y="130" width="80" height="44" rx="8" fill="#18181b"/><path d="M100 52 H200 M60 74 L130 130 M240 74 L170 130" stroke="#71717a" stroke-width="2" fill="none"/>', 300, 200);
    const c = (id, kind, text, ago, o = {}) => ({ id, kind, text, len: text.length, bytes: text.length, width: null, height: null, thumb: null, pinned: false, uses: 0, created: now - ago * m, last_used: now - ago * m, ...o });
    return [
      c(1, 'link', 'https://linear.app/acme/issue/INV-212/invoice-totals-off-by-one', 1),
      c(2, 'image', '', 4, { thumb: dashboard, width: 2560, height: 1600, bytes: 412000, ocr_text: 'Q4 revenue dashboard\nARR $4.2M, up 18% year over year' }),
      c(3, 'text', 'Hi Sam,\nAttached is the invoice for September. Let me know if anything looks off.\nBest, Alex', 9, { uses: 3 }),
      c(4, 'file', 'C:\\Users\\alex\\Documents\\Invoices\\invoice-0923.pdf', 14, { bytes: 184320 }),
      c(5, 'code', 'const total = items.reduce((sum, i) => sum + i.price * i.qty, 0);\nreturn Math.round(total * 100) / 100;', 38, { uses: 5 }),
      c(6, 'email', 'billing@acme-industries.com', 75, { pinned: true, uses: 14 }),
      c(7, 'file', 'C:\\Users\\alex\\Pictures\\Trip\\IMG_2041.jpg\nC:\\Users\\alex\\Pictures\\Trip\\IMG_2042.jpg\nC:\\Users\\alex\\Pictures\\Trip\\IMG_2043.jpg', 130, { bytes: 11650000 }),
      c(8, 'color', '#18181b', 200),
      c(9, 'image', '', 1500, { thumb: photo, width: 4032, height: 3024, bytes: 3400000 }),
      c(10, 'text', 'Meeting moved to Thursday 14:00 — bring the Q3 numbers', 1700),
      c(11, 'number', 'DE89 3704 0044 0532 0130 00'.replace(/\s/g, ''), 2900, { pinned: true, uses: 8 }),
      c(12, 'file', 'C:\\Users\\alex\\Desktop\\brand\\logo.svg', 3100, { bytes: 6200 }),
      c(13, 'image', '', 5000, { thumb: whiteboard, width: 1200, height: 800, bytes: 98000, ocr_text: 'Checkout flow: cart → payment → receipt' }),
      c(14, 'path', 'C:\\Projects\\stash\\src-tauri\\src\\lib.rs', 7000),
      c(15, 'text', 'grocery list: oat milk, eggs, coffee, lemons', 12000),
      c(17, 'code', demoVisuals()[17], 600),
      c(22, 'text', '🚀 👍🏽 🇩🇪', 380),
      c(21, 'text', 'linear-gradient(135deg, #6366f1 0%, #ec4899 55%, #f59e0b 100%)', 520),
      c(20, 'text', demoVisuals()[20], 450),
      c(19, 'text', '/Users/alex/Desktop/Screenshot 2026-10-26 at 14.03.12.png', 300),
      c(18, 'file', 'C:\\Users\\alex\\Downloads\\Geist-Regular.woff2', 900, { bytes: 68200 }),
      // Made up. Split in two so secret scanners don't take it for a real key.
      c(16, 'code', 'STRIPE_SECRET_KEY=sk_' + 'live_51NzQ8vKd2Lr7XbW4eTfYp9Hm\nDATABASE_URL=postgres://app:Tr0ub4dor@db.internal:5432/main', 14000),
    ];
  }
})();
