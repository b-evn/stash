(function (root) {
  // Finds the parts of secrets the list should blur. What stays visible tells
  // you what it is (an IBAN's country and check digits, a token's prefix) and
  // which one (the last four characters). Passwords and other bare values are
  // blurred whole, since any part of them gives part of them away.
  const TAIL = 4;

  const IBAN = /(^|[^A-Z0-9])([A-Z]{2}\d{2}(?: ?[A-Z0-9]{4}){2,7}(?: ?[A-Z0-9]{1,3})?)(?![A-Z0-9])/g;
  const CARD = /(^|[^\d])([2-6](?:[ -]?\d){12,18})(?!\d)/g;
  const TOKEN = new RegExp('(^|[^\\w-])(sk-ant-(?:api|admin)\\d\\d-|sk-proj-|sk-|[sr]k_(?:live|test)_|whsec_|' +
    'gh[pousr]_|github_pat_|glpat-|xox[abeprs]-|AKIA|ASIA|AIza|npm_|pypi-|hf_|shp(?:at|ca|pa|ss)_|' +
    'dop_v1_|lin_api_|SG\\.|eyJ)([\\w.-]{16,})', 'g');
  const URL_PASSWORD = /(^|\/\/)([^\s:/@]+):([^\s/@]+)@(?=[\w-])/g;
  const KEY_VALUE = /(^|[^a-z0-9])([\w.-]*?(?:passw(?:or)?d|passphrase|pwd|secret|token|api[_-]?key|access[_-]?key|private[_-]?key|credentials?)[\w.-]*)(["']?\s*[:=]\s*["']?)([^\s"'&,;]{3,})/gi;
  const BEARER = /(^|[^\w])((?:Bearer|Basic) +)([\w.~+/-]{8,}=*)/g;
  const PRIVATE_KEY = /-----BEGIN [A-Z0-9 ]*PRIVATE KEY-----/;

  // Positions of the letters and digits in s, so grouping spaces don't count.
  const alnum = (s, from) => [...s].flatMap((ch, i) => (/[A-Za-z0-9]/.test(ch) ? [from + i] : []));

  function ibanValid(iban) {
    const s = iban.replace(/ /g, '');
    if (s.length < 15 || s.length > 34) return false;
    const digits = (s.slice(4) + s.slice(0, 4)).replace(/[A-Z]/g, (ch) => ch.charCodeAt(0) - 55);
    let r = 0;
    for (const d of digits) r = (r * 10 + Number(d)) % 97;
    return r === 1;
  }

  function luhn(digits) {
    let sum = 0;
    for (let i = 0; i < digits.length; i++) {
      let d = Number(digits[digits.length - 1 - i]);
      if (i % 2) { d *= 2; if (d > 9) d -= 9; }
      sum += d;
    }
    return sum % 10 === 0;
  }

  // Each finder yields { span: [start, end] of the whole match, blur: [start, end] }.
  const finders = [
    function* iban(s) {
      for (const m of s.matchAll(IBAN)) {
        if (!ibanValid(m[2])) continue;
        const at = m.index + m[1].length, pos = alnum(m[2], at);
        yield { span: [at, at + m[2].length], blur: [pos[4], pos[pos.length - TAIL]] };
      }
    },
    function* card(s) {
      for (const m of s.matchAll(CARD)) {
        const digits = m[2].replace(/\D/g, '');
        if (!luhn(digits)) continue;
        const at = m.index + m[1].length, pos = alnum(m[2], at);
        yield { span: [at, at + m[2].length], blur: [at, pos[pos.length - TAIL]] };
      }
    },
    function* token(s) {
      for (const m of s.matchAll(TOKEN)) {
        const body = m[3].replace(/[.-]+$/, '');
        const at = m.index + m[1].length, from = at + m[2].length, end = from + body.length;
        yield { span: [at, end], blur: [from, end - TAIL] };
      }
    },
    function* urlPassword(s) {
      for (const m of s.matchAll(URL_PASSWORD)) {
        const at = m.index + m[1].length, from = at + m[2].length + 1;
        yield { span: [at, from + m[3].length], blur: [from, from + m[3].length] };
      }
    },
    function* keyValue(s) {
      for (const m of s.matchAll(KEY_VALUE)) {
        const at = m.index + m[1].length, from = at + m[2].length + m[3].length;
        yield { span: [at, from + m[4].length], blur: [from, from + m[4].length] };
      }
    },
    function* bearer(s) {
      for (const m of s.matchAll(BEARER)) {
        const at = m.index + m[1].length, from = at + m[2].length;
        yield { span: [at, from + m[3].length], blur: [from, from + m[3].length] };
      }
    },
  ];

  // Sorted [start, end) ranges of s to blur. Earlier finders win where
  // matches overlap, so `API_KEY=sk-proj-…` keeps its token prefix visible.
  function find(s) {
    const taken = [], out = [];
    for (const f of finders) {
      for (const { span, blur } of f(s)) {
        if (taken.some(([a, b]) => span[0] < b && a < span[1])) continue;
        taken.push(span);
        if (blur[1] > blur[0]) out.push(blur);
      }
    }
    return out.sort((a, b) => a[0] - b[0]);
  }

  const privateKey = (text) => PRIVATE_KEY.test(text);

  const api = { find, privateKey };
  if (typeof module !== 'undefined') module.exports = api;
  else root.StashSecrets = api;
})(typeof window !== 'undefined' ? window : globalThis);
