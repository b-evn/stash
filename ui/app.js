(function () {
  const tauri = window.__TAURI__;
  const call = tauri ? tauri.core.invoke : mockInvoke;
  const listen = tauri ? tauri.event.listen : () => {};
  const params = new URLSearchParams(location.search);

  const TABS = [['all', 'All'], ['text', 'Text'], ['link', 'Links'], ['code', 'Code'], ['image', 'Images'], ['file', 'Files']];
  const SORT_LABEL = { recent: 'Recent', frequent: 'Frequent', 'a–z': 'A–Z', longest: 'Largest' };
  const THEMES = ['auto', 'dark', 'light'];
  const MAX_ROWS = 200;
  const DAY = 864e5;
  const TEXTUAL = new Set(['text', 'link', 'code', 'email', 'number', 'color', 'path']);

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
    auto: '<circle cx="12" cy="12" r="10"/><path d="M12 18a6 6 0 0 0 0-12v12z"/>',
    dark: '<path d="M12 3a6 6 0 0 0 9 9 9 9 0 1 1-9-9Z"/>',
    light: '<circle cx="12" cy="12" r="4"/><path d="M12 2v2"/><path d="M12 20v2"/><path d="m4.93 4.93 1.41 1.41"/><path d="m17.66 17.66 1.41 1.41"/><path d="M2 12h2"/><path d="M20 12h2"/><path d="m6.34 17.66-1.41 1.41"/><path d="m19.07 4.93-1.41 1.41"/>',
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
  };
  const svg = (name) => `<svg class="icon" viewBox="0 0 24 24">${ICON[name]}</svg>`;

  const $ = (id) => document.getElementById(id);
  const q = $('q'), listEl = $('list'), tabsEl = $('tabs'), pill = $('pill'), chipsEl = $('chips');
  const pinBtn = $('pinned'), sortBtn = $('sort'), themeBtn = $('theme'), hintsEl = $('hints'), stackEl = $('stackbar');

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
  let stack = [];
  let running = { ids: [], total: 0 };
  let sort = store.get('sort', 'recent');
  if (!StashSearch.SORTS.includes(sort)) sort = 'recent';
  let theme = params.get('theme') || store.get('theme', 'auto');
  if (!THEMES.includes(theme)) theme = 'auto';

  const el = (tag, cls, html) => {
    const e = document.createElement(tag);
    if (cls) e.className = cls;
    if (html) e.innerHTML = html;
    return e;
  };
  const noFocusSteal = (b) => b.addEventListener('mousedown', (e) => e.preventDefault());
  const byId = (id) => clips.find((c) => c.id === id);

  function applyTheme() {
    if (theme === 'auto') document.documentElement.removeAttribute('data-theme');
    else document.documentElement.setAttribute('data-theme', theme);
    themeBtn.innerHTML = svg(theme);
    themeBtn.title = `Theme: ${theme} (Ctrl+T)`;
  }

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

  function describe(c) {
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
      const dir = dirName(first).replace(/^[A-Z]:\\Users\\[^\\]+/i, '~');
      return { title: name, sub: [dir, size(c.bytes)].filter(Boolean).join(' · ') };
    }
    const ls = lines(c.text);
    if (c.kind === 'link') return { title: (ls[0] || '').replace(/^[a-z]+:\/\//i, '').replace(/\/$/, '') };
    return { title: ls[0] || c.text, sub: ls[1] ? ls[1] + (ls.length > 2 ? '  …' : '') : null };
  }

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

  function copyAndHide(cmd, args) {
    Promise.resolve()
      .then(() => call(cmd, args))
      .catch((e) => console.error(e))
      .finally(() => run('hide_window'));
  }

  function paint(node, text) {
    const s = text.slice(0, 300);
    const hits = query() ? StashSearch.hits(query(), s) : new Set();
    let run = '', on = false;
    const flush = () => {
      if (!run) return;
      if (on) { const m = document.createElement('mark'); m.textContent = run; node.appendChild(m); }
      else node.appendChild(document.createTextNode(run));
      run = '';
    };
    for (let i = 0; i < s.length; i++) {
      const hit = hits.has(i);
      if (hit !== on) { flush(); on = hit; }
      run += s[i];
    }
    flush();
  }

  function tile(c) {
    const t = el('div', 'tile');
    if (c.kind === 'image' && c.thumb) {
      t.classList.add('thumb');
      const img = new Image();
      img.src = c.thumb;
      img.alt = '';
      t.appendChild(img);
    } else if (c.kind === 'color') {
      t.classList.add('swatch');
      t.appendChild(el('i')).style.background = c.text.trim();
    } else if (c.kind === 'file') {
      const paths = lines(c.text);
      const ext = (baseName(paths[0] || '').match(/\.([a-z0-9]{1,4})$/i) || [])[1];
      if (paths.length > 1) t.innerHTML = svg('files');
      else if (ext) t.appendChild(el('span', 'ext')).textContent = ext.toUpperCase();
      else t.innerHTML = svg('file');
    } else {
      t.innerHTML = svg(ICON[c.kind] ? c.kind : 'text');
    }
    const pos = stack.indexOf(c.id);
    if (pos >= 0) t.appendChild(el('span', 'badge')).textContent = String(pos + 1);
    return t;
  }

  function action(icon, title, fn) {
    const b = el('button', 'ghost', svg(icon));
    b.title = title;
    noFocusSteal(b);
    b.addEventListener('mousedown', (e) => { if (e.button === 0) { e.stopPropagation(); fn(); } });
    b.addEventListener('click', (e) => e.stopPropagation());
    return b;
  }

  function renderItem(r, i) {
    const c = r.clip;
    const row = el('div', 'item' + (i === sel ? ' sel' : '') + (c.kind === 'code' ? ' mono' : ''));
    row.setAttribute('role', 'option');
    row.dataset.i = i;

    const d = describe(c);
    const main = el('div', 'main');
    paint(main.appendChild(el('span', 'title')), d.title);
    if (d.sub) paint(main.appendChild(el('span', 'sub')), d.sub);
    if (i === sel && shiftDown && c.kind === 'image' && c.thumb) {
      row.classList.add('expanded');
      const box = main.appendChild(el('div', 'preview'));
      const img = box.appendChild(el('img'));
      img.src = c.thumb;
      img.alt = '';
    }

    const trail = el('div', 'trail');
    const info = trail.appendChild(el('span', 'info'));
    if (c.pinned) info.innerHTML = svg('pin');
    info.appendChild(el('span')).textContent = age(c.last_used);
    const acts = trail.appendChild(el('span', 'acts'));
    const inStack = stack.includes(c.id);
    acts.append(
      action(inStack ? 'stackRemove' : 'stackAdd', inStack ? 'Remove from stack (Shift+Enter)' : 'Add to stack (Shift+Enter)', () => toggleStack(c)),
      action(c.pinned ? 'pinOff' : 'pin', c.pinned ? 'Unpin (Ctrl+P)' : 'Pin (Ctrl+P)', () => pin(c)),
      action('trash', 'Delete (Ctrl+D)', () => remove(c)),
    );
    if (c.kind === 'link') {
      acts.prepend(action('external', 'Open link', () => run('open_url', { id: c.id })));
    } else if (c.kind === 'file' || c.kind === 'path') {
      acts.prepend(action('folder', 'Show in Explorer', () => run('reveal_in_explorer', { id: c.id })));
    } else if (c.kind === 'code') {
      acts.prepend(action('code', 'Copy as fenced code block', () => copyAndHide('copy_fence', { id: c.id, lang: guessLang(c.text) })));
    }

    row.append(tile(c), main, trail);
    row.addEventListener('mousemove', (e) => { setShift(e.shiftKey); const n = Number(row.dataset.i); if (sel !== n) select(n, false); });
    row.addEventListener('click', () => (stack.length ? toggleStack(c) : paste(c)));
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
    const building = stack.length > 0;
    const live = !building && running.ids.length > 0;
    hintsEl.hidden = building || live;
    stackEl.hidden = !(building || live);
    stackEl.textContent = '';
    if (!building && !live) return;

    const left = stackEl.appendChild(el('span', 'stack-label', svg('stack')));
    if (building) {
      left.appendChild(el('span')).textContent = 'Stack';
      left.appendChild(el('span', 'count')).textContent = String(stack.length);
    } else {
      const next = running.total - running.ids.length + 1;
      const c = byId(running.ids[0]);
      left.appendChild(el('span')).textContent = `Next ${next} of ${running.total}`;
      if (c) left.appendChild(el('span', 'next')).textContent = describe(c).title;
    }
    const clear = stackEl.appendChild(action('x', building ? 'Clear stack (Esc)' : 'Stop the stack', () => {
      if (building) { stack = []; refresh(true); } else { call('cancel_stack'); running = { ids: [], total: 0 }; renderFooter(); }
    }));
    clear.classList.add('small');
    stackEl.appendChild(el('span', 'spacer'));
    if (building) {
      stackEl.appendChild(el('span', 'hint', '<kbd>↵</kbd>Paste in order'));
      if (stack.every((id) => byId(id) && TEXTUAL.has(byId(id).kind)) && stack.length > 1) {
        stackEl.appendChild(el('span', 'hint', '<kbd>Ctrl</kbd><kbd>⇧</kbd><kbd>↵</kbd>Join'));
      }
    } else {
      stackEl.appendChild(el('span', 'hint')).textContent = 'Ctrl+V pastes the next';
    }
  }

  function refresh(keepSel = false) {
    results = StashSearch.run(clips, query(), { filter, sort, pinned: pinnedOnly });
    if (!keepSel) sel = 0;
    sel = Math.min(sel, Math.max(Math.min(results.length, MAX_ROWS) - 1, 0));
    sortBtn.innerHTML = svg('sort') + SORT_LABEL[sort];
    $('esc').classList.toggle('on', Boolean(query() || stack.length));
    renderChips();
    renderTabs(true);
    renderList();
    renderFooter();
  }

  const run = (cmd, args) => Promise.resolve(call(cmd, args)).catch((e) => console.error(e));
  const paste = (c) => run('paste_clip', { id: c.id });
  const copyOnly = (c) => run('copy_clip', { id: c.id });

  function toggleStack(c) {
    stack = stack.includes(c.id) ? stack.filter((id) => id !== c.id) : [...stack, c.id];
    refresh(true);
  }

  function startStack() {
    const ids = stack;
    stack = [];
    run('start_stack', { ids });
  }

  function joinStack() {
    const ids = stack;
    stack = [];
    run('paste_joined', { ids });
  }

  function pin(c) {
    c.pinned = !c.pinned;
    run('toggle_pin', { id: c.id });
    refresh(true);
  }

  function remove(c) {
    const idx = results.findIndex((r) => r.clip === c);
    const inStack = stack.includes(c.id);
    clips = clips.filter((x) => x !== c);
    stack = stack.filter((id) => id !== c.id);
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
  const setSort = () => { sort = cycle(StashSearch.SORTS, sort); store.set('sort', sort); refresh(); };
  const setTheme = () => { theme = cycle(THEMES, theme); store.set('theme', theme); applyTheme(); };

  function move(delta) {
    const n = Math.min(results.length, MAX_ROWS);
    if (n) select(Math.min(Math.max(sel + delta, 0), n - 1));
  }

  document.addEventListener('keydown', (e) => {
    const ctrl = e.ctrlKey || e.metaKey;
    const cur = results[sel] && results[sel].clip;
    if (ctrl && /^[1-9]$/.test(e.key)) { const r = results[Number(e.key) - 1]; if (r) paste(r.clip); }
    else if (e.key === 'ArrowDown' || (ctrl && e.key === 'j')) move(1);
    else if (e.key === 'ArrowUp' || (ctrl && e.key === 'k')) move(-1);
    else if (e.key === 'PageDown') move(6);
    else if (e.key === 'PageUp') move(-6);
    else if (e.key === 'Enter' && e.shiftKey && ctrl) { if (stack.length) joinStack(); }
    else if (e.key === 'Enter' && e.shiftKey) { if (cur) toggleStack(cur); }
    else if (e.key === 'Enter' && ctrl) { if (cur) copyOnly(cur); }
    else if (e.key === 'Enter') { if (stack.length) startStack(); else if (cur) paste(cur); }
    else if (e.key === 'Escape') {
      if (q.value || chips.length) { q.value = ''; chips = []; refresh(); }
      else if (stack.length) { stack = []; refresh(true); }
      else run('hide_window');
    }
    else if (e.key === 'Backspace' && !q.value && q.selectionStart === 0 && chips.length) { chips.pop(); refresh(); }
    else if (e.key === 'Tab') { filter = cycle(TABS.map((t) => t[0]), filter, e.shiftKey ? -1 : 1); refresh(); }
    else if (ctrl && e.key === 'p') { if (cur) pin(cur); }
    else if (ctrl && e.key === 'd') { if (cur) remove(cur); }
    else if (e.key === 'Delete' && !q.value) { if (cur) remove(cur); }
    else if (ctrl && e.key === 's') setSort();
    else if (ctrl && e.key === 't') setTheme();
    else return;
    e.preventDefault();
  });

  q.addEventListener('input', () => { absorb(); refresh(); });
  [pinBtn, sortBtn, themeBtn].forEach(noFocusSteal);
  pinBtn.innerHTML = svg('pin');
  pinBtn.addEventListener('click', () => { pinnedOnly = !pinnedOnly; refresh(); });
  sortBtn.addEventListener('click', setSort);
  themeBtn.addEventListener('click', setTheme);

  async function open() {
    q.value = params.get('q') || '';
    chips = [];
    absorb(true);
    filter = params.get('tab') || 'all';
    pinnedOnly = false;
    stack = (params.get('stack') || '').split(',').filter(Boolean).map(Number);
    try { clips = await call('list_clips'); } catch (e) { console.error(e); clips = []; }
    try { running = (await call('get_stack')) || running; } catch {}
    const card = $('card');
    card.classList.remove('enter');
    void card.offsetWidth;
    card.classList.add('enter');
    refresh();
    renderTabs(false);
    if (params.get('sel')) select(Number(params.get('sel')), true);
    q.focus();
  }

  let queued = false;
  new ResizeObserver(() => {
    if (queued) return;
    queued = true;
    requestAnimationFrame(() => {
      queued = false;
      run('fit_window', { height: Math.ceil($('card').getBoundingClientRect().height) });
    });
  }).observe($('card'));

  applyTheme();
  listen('stash-show', open);
  listen('stash-stack', (e) => { running = e.payload; renderFooter(); });
  window.addEventListener('focus', () => q.focus());
  open();

  function mockInvoke(cmd) {
    if (cmd === 'list_clips') return Promise.resolve(window.STASH_DEMO || demoClips());
    if (cmd === 'get_stack') {
      const ids = (params.get('running') || '').split(',').filter(Boolean).map(Number);
      return Promise.resolve({ ids, total: ids.length ? Number(params.get('total')) || ids.length : 0 });
    }
    return Promise.resolve();
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
    ];
  }
})();
