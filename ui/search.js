(function (root) {
  const KIND = { link: 'link', url: 'link', code: 'code', path: 'path', folder: 'path', color: 'color',
    colour: 'color', number: 'number', num: 'number', email: 'email', mail: 'email', text: 'text',
    image: 'image', images: 'image', img: 'image', file: 'file', files: 'file' };
  const TABS = { text: ['text', 'email', 'number', 'color', 'path'], link: ['link'], code: ['code'],
    image: ['image'], file: ['file'] };
  const AGE = { hour: 36e5, today: 864e5, week: 6048e5 };

  function parse(q) {
    const p = { terms: [], not: [], kinds: new Set(), pin: false, maxAge: 0 };
    for (const tok of q.trim().split(/\s+/).filter(Boolean)) {
      const head = tok[0], rest = tok.slice(1), key = rest.toLowerCase();
      if (head === '@' && (KIND[key] || key === 'pin' || key === 'pinned' || AGE[key])) {
        if (KIND[key]) p.kinds.add(KIND[key]);
        else if (AGE[key]) p.maxAge = AGE[key];
        else p.pin = true;
      } else if (head === '!' && rest) p.not.push(key);
      else if (head === "'" && rest) p.terms.push({ t: rest, mode: 'exact' });
      else if (head === '^' && rest) p.terms.push({ t: rest, mode: 'prefix' });
      else p.terms.push({ t: tok, mode: 'fuzzy' });
    }
    return p;
  }

  const isAlnum = (c) => /[\p{L}\p{N}]/u.test(c);
  const isUpper = (c) => c !== c.toLowerCase();
  const isLower = (c) => c !== c.toUpperCase();

  function scoreAt(text, idx) {
    let s = 0;
    for (let k = 0; k < idx.length; k++) {
      const i = idx[k], prev = text[i - 1];
      s += 16;
      if (k > 0) {
        const gap = i - idx[k - 1] - 1;
        s += gap === 0 ? 14 : -(3 + Math.min(gap - 1, 10));
      }
      if (i === 0 || !isAlnum(prev) || (isLower(prev) && isUpper(text[i]))) s += k === 0 ? 14 : 9;
    }
    return s - idx[0] * 0.05;
  }

  function exact(term, text, cs) {
    const T = cs ? text : text.toLowerCase(), P = cs ? term : term.toLowerCase();
    let best = null, from = 0;
    for (let n = 0; n < 8; n++) {
      const at = T.indexOf(P, from);
      if (at < 0) break;
      const idx = Array.from({ length: P.length }, (_, k) => at + k);
      const score = scoreAt(text, idx) + 20;
      if (!best || score > best.score) best = { score, idx };
      from = at + 1;
    }
    return best;
  }

  function fuzzy(term, text, cs) {
    const T = cs ? text : text.toLowerCase(), P = cs ? term : term.toLowerCase();
    const m = P.length, limit = m * 3 + 6;
    let best = null, tried = 0;
    for (let s = T.indexOf(P[0]); s >= 0 && tried < 16; s = T.indexOf(P[0], s + 1), tried++) {
      const idx = [s];
      for (let i = s + 1, pi = 1; pi < m && i < T.length && i - s < limit; i++) {
        if (T[i] === P[pi]) { idx.push(i); pi++; }
      }
      if (idx.length < m) continue;
      const span = T.slice(idx[0], idx[m - 1] + 1);
      const acronym = idx.every((i) => i === 0 || !isAlnum(text[i - 1]));
      if (/\s/.test(span) && !acronym) continue;
      const score = scoreAt(text, idx);
      if (!best || score > best.score) best = { score, idx };
    }
    const tight = exact(term, text, cs);
    return tight && (!best || tight.score > best.score) ? tight : best;
  }

  function matchTerm(t, text) {
    const cs = t.t !== t.t.toLowerCase();
    if (t.mode === 'exact') return exact(t.t, text, cs);
    if (t.mode === 'fuzzy') return fuzzy(t.t, text, cs);
    const lead = text.length - text.trimStart().length;
    const head = text.slice(lead, lead + t.t.length);
    if ((cs ? head : head.toLowerCase()) !== (cs ? t.t : t.t.toLowerCase())) return null;
    const idx = Array.from({ length: t.t.length }, (_, k) => lead + k);
    return { score: scoreAt(text, idx) + 30, idx };
  }

  const SORTS = {
    recent: (a, b) => b.last_used - a.last_used,
    frequent: (a, b) => b.uses - a.uses || b.last_used - a.last_used,
    'a–z': (a, b) => a.text.trim().localeCompare(b.text.trim(), undefined, { sensitivity: 'base' }),
    longest: (a, b) => b.len - a.len || b.last_used - a.last_used,
  };

  const haystack = (clip) => (clip.ocr_text ? `${clip.text}\n${clip.ocr_text}` : clip.text);

  function run(clips, query, opts) {
    const o = Object.assign({ filter: 'all', sort: 'recent', now: Date.now() }, opts);
    const p = parse(query);
    if (o.pinned) p.pin = true;
    const tab = TABS[o.filter] ? new Set(TABS[o.filter]) : null;
    const cmp = SORTS[o.sort] || SORTS.recent;
    const out = [];
    for (const clip of clips) {
      if (p.pin && !clip.pinned) continue;
      if (p.kinds.size && !p.kinds.has(clip.kind)) continue;
      if (tab && !tab.has(clip.kind)) continue;
      if (p.maxAge && o.now - clip.last_used > p.maxAge) continue;
      const text = haystack(clip);
      const lower = text.toLowerCase();
      if (p.not.some((w) => lower.includes(w))) continue;
      let score = 0;
      const idx = new Set();
      let ok = true;
      for (const t of p.terms) {
        const r = matchTerm(t, text);
        if (!r) { ok = false; break; }
        score += r.score;
        r.idx.forEach((i) => idx.add(i));
      }
      if (!ok) continue;
      out.push({ clip, idx, score: score - text.length * 0.01 });
    }
    if (p.terms.length) out.sort((a, b) => b.score - a.score || cmp(a.clip, b.clip));
    else out.sort((a, b) => (b.clip.pinned - a.clip.pinned) || cmp(a.clip, b.clip));
    return out;
  }

  function hits(query, text) {
    const out = new Set();
    for (const t of parse(query).terms) {
      const r = matchTerm(t, text);
      if (r) r.idx.forEach((i) => out.add(i));
    }
    return out;
  }

  const api = { parse, run, fuzzy, hits, SORTS: Object.keys(SORTS), TABS: Object.keys(TABS) };
  if (typeof module !== 'undefined') module.exports = api;
  else root.StashSearch = api;
})(typeof window !== 'undefined' ? window : globalThis);
