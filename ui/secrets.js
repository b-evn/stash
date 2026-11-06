(function (root) {
  // Finds secrets in text: which parts the list should blur, and what kind of
  // secret each one is. What stays visible tells you what it is (an IBAN's
  // country and check digits, a token's prefix) and which one (the last four
  // characters). Passwords and other bare values are blurred whole, since any
  // part of them gives part of them away.
  //
  // Each check is narrow on purpose: a valid checksum, a known prefix, a name
  // that says password. Hiding ordinary text makes the blur noise.
  const TAIL = 4;

  // What a secret is: its name in the list and the icon its row gets.
  const T = (label, icon) => ({ label, icon });
  const IBAN_T = T('IBAN', 'bank');
  const PASSWORD_T = T('Password', 'lock');

  const IBAN = /(^|[^A-Z0-9])([A-Z]{2}\d{2}(?: ?[A-Z0-9]{4}){2,7}(?: ?[A-Z0-9]{1,3})?)(?![A-Z0-9])/g;
  const CARD = /(^|[^\d])([2-6](?:[ -]?\d){12,18})(?!\d)/g;

  // Token prefixes, longest first where one starts another.
  const TOKENS = [
    ['sk-ant-(?:api|admin)\\d\\d-', 'Anthropic key'],
    ['sk-(?:proj|svcacct|admin)-', 'OpenAI key'],
    ['sk-or-v1-', 'OpenRouter key'],
    ['sk-', 'API key'],
    ['[sr]k_(?:live|test)_', 'Stripe key'],
    ['whsec_', 'Webhook secret'],
    ['gh[pousr]_|github_pat_', 'GitHub token'],
    ['glpat-', 'GitLab token'],
    ['xox[abeprs]-|xapp-', 'Slack token'],
    ['AKIA|ASIA', 'AWS key'],
    ['AIza', 'Google API key'],
    ['ya29\\.', 'Google token'],
    ['npm_', 'npm token'],
    ['pypi-', 'PyPI token'],
    ['hf_', 'Hugging Face token'],
    ['shp(?:at|ca|pa|ss)_', 'Shopify token'],
    ['dop_v1_', 'DigitalOcean token'],
    ['lin_api_', 'Linear key'],
    ['SG\\.', 'SendGrid key'],
    ['xai-', 'xAI key'],
    ['gsk_', 'Groq key'],
    ['r8_', 'Replicate token'],
    ['glsa_', 'Grafana token'],
    ['figd_', 'Figma token'],
    ['ntn_', 'Notion token'],
    ['ATATT', 'Atlassian token'],
    ['eyJ', 'JWT'],
  ].map(([p, label]) => ({ re: new RegExp(`^(?:${p})$`), type: T(label, 'key') }));
  const TOKEN = new RegExp(`(^|[^\\w-])(${TOKENS.map((t) => t.re.source.slice(1, -1)).join('|')})([\\w.-]{16,})`, 'g');
  const TELEGRAM = /(^|[^\w:])(\d{8,10}:)(AA[\w-]{33})(?![\w-])/g;

  const URL_PASSWORD = /(^|\/\/)([^\s:/@]*):([^\s/@]+)@(?=[\w-])/g;
  const DB_SCHEME = /(?:postgres(?:ql)?|mysql|mariadb|mongodb(?:\+srv)?|rediss?|amqps?|mssql|sqlserver|clickhouse|cockroachdb|neo4j(?:\+s)?|couchdb):\/\/$/i;

  // A name that says what follows is a secret: DB_PASSWORD, "client_secret",
  // X-Api-Key, ?access_token. The keyword ends the name (max_tokens and
  // secret_name are not secrets); short ones like pass and pin start it too.
  const NAME = '(?:[\\w.-]*?(passw(?:or)?[dt]|passphrase|passcode|kennwort|secret|token|credentials?|' +
    'api[_-]?key|(?:access|account|private|secret|encryption|signing|master|client|shared[_-]?access)[_-]?key)|' +
    '(?:[\\w.-]*?[_.-])?(pass|pwd|pw|pin))' +
    '(?:[_.-]?(?:key|value|hash|digest|base|b64|base64|code|confirm(?:ation)?|repeat|again|enc(?:rypted)?|\\d))?';
  // The value: quoted, or up to a space or the next `name=` in a query or
  // connection string, so `Tr0ub4dor&3` stays whole but `&state=x` is left.
  const VALUE = `(?:"([^"\\n]{2,})"|'([^'\\n]{2,})'|((?:(?![;,&][ \\t]*[\\w.-]+[ \\t]*[:=])[^\\s"'\`])+))`;
  const KEY_VALUE = new RegExp(`(^|[^a-z0-9])(${NAME})(["']?[ \\t]*(?::=|=>|[:=])[ \\t]*)${VALUE}`, 'gi');
  const BEARER = /(^|[^\w])((Bearer|Basic|Token) +)([\w.~+/-]{8,}=*)/gi;
  const PRIVATE_KEY = /-----BEGIN ([A-Z0-9 ]*?)PRIVATE KEY(?: BLOCK)?-----/;

  // Values after a secret's name that are not the secret itself: types,
  // flags, placeholders and references to where the secret really lives.
  const NOT_SECRET = [
    /^(?:true|false|null|nil|none|undefined|yes|no|string|str|number|int|integer|bool|boolean|any|object|bytes|required|optional|include|omit|same-origin)$/i,
    /^(?:\*+|x+|•+|\.+|\[redacted\])$/i,
    /^(?:\$\{[^}]+\}|\$[A-Z_][A-Z0-9_]*|\{\{.*\}\}|<[\w -]*>?|%\(\w+\)s|%s)$/,
    /^(?:process\.env|import\.meta\.env|os\.environ)\b/,
    /^[A-Za-z_$][\w$.]*\(.*\)[;,]?$/,
    /^(?:this|self|req|request|res|config|cfg|settings|options|opts|args|params|props|data|body|form|values|ctx|context|app|user|input|payload|credentials|secrets|auth|session|window|env)\.[\w$.]+[;,]?$/,
  ];
  const notSecret = (v) => NOT_SECRET.some((re) => re.test(v.replace(/[;,]$/, '')));
  // Random enough to be a key rather than a word: a digit, or mixed case.
  const random = (s) => /\d/.test(s) || (/[A-Z]/.test(s.slice(1)) && /[a-z]/.test(s));

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

  // The card network a number belongs to, by its leading digits and length.
  // Numbers no network issues are not cards, however their checksum comes out.
  function network(d) {
    const n = d.length;
    if (/^3[47]/.test(d)) return n === 15 && 'Amex';
    if (/^3(?:0[0-5]|[689])/.test(d)) return n >= 14 && 'Diners';
    if (/^35(?:2[89]|[3-8])/.test(d)) return n >= 16 && 'JCB';
    if (/^4/.test(d)) return [13, 16, 19].includes(n) && 'Visa';
    if (/^(?:5[1-5]|2(?:2[2-9]|[3-6]|7[01]|720))/.test(d)) return n === 16 && 'Mastercard';
    if (/^(?:6011|64[4-9]|65)/.test(d)) return n >= 16 && 'Discover';
    if (/^62/.test(d)) return n >= 16 && 'UnionPay';
    if (/^(?:5[06-8]|6)/.test(d)) return n >= 12 && 'Maestro';
    return null;
  }

  function basicAuth(value) {
    if (typeof atob !== 'function') return random(value);
    try { return atob(value).includes(':'); } catch { return false; }
  }

  // Each finder yields { span: [start, end] of the whole match, blur: [start, end], type }.
  const finders = [
    function* iban(s) {
      for (const m of s.matchAll(IBAN)) {
        if (!ibanValid(m[2])) continue;
        const at = m.index + m[1].length, pos = alnum(m[2], at);
        yield { span: [at, at + m[2].length], blur: [pos[4], pos[pos.length - TAIL]], type: IBAN_T };
      }
    },
    function* card(s) {
      for (const m of s.matchAll(CARD)) {
        const digits = m[2].replace(/\D/g, ''), net = network(digits);
        if (!net || !luhn(digits)) continue;
        const at = m.index + m[1].length, pos = alnum(m[2], at);
        yield { span: [at, at + m[2].length], blur: [at, pos[pos.length - TAIL]], type: T(net === 'Mastercard' ? net : `${net} card`, 'card') };
      }
    },
    function* token(s) {
      for (const m of s.matchAll(TOKEN)) {
        const body = m[3].replace(/[.-]+$/, '');
        if (!random(body)) continue;
        const at = m.index + m[1].length, from = at + m[2].length, end = from + body.length;
        yield { span: [at, end], blur: [from, end - TAIL], type: TOKENS.find((t) => t.re.test(m[2])).type };
      }
      for (const m of s.matchAll(TELEGRAM)) {
        const at = m.index + m[1].length, from = at + m[2].length, end = from + m[3].length;
        yield { span: [at, end], blur: [from, end - TAIL], type: T('Telegram token', 'key') };
      }
    },
    function* urlPassword(s) {
      for (const m of s.matchAll(URL_PASSWORD)) {
        if (notSecret(m[3])) continue;
        const at = m.index + m[1].length, from = at + m[2].length + 1;
        const db = DB_SCHEME.test(s.slice(Math.max(0, at - 24), at));
        yield { span: [at, from + m[3].length], blur: [from, from + m[3].length], type: db ? T('Database URL', 'db') : T('URL password', 'lock') };
      }
    },
    function* keyValue(s) {
      for (const m of s.matchAll(KEY_VALUE)) {
        const value = m[6] ?? m[7] ?? m[8];
        if (notSecret(value) || (m[8] && value.length < 3)) continue;
        const at = m.index + m[1].length, from = at + m[2].length + m[5].length + (m[8] ? 0 : 1);
        yield { span: [at, from + value.length], blur: [from, from + value.length], type: nameType(m[3] || m[4]) };
      }
    },
    function* bearer(s) {
      for (const m of s.matchAll(BEARER)) {
        const scheme = m[3].toLowerCase();
        if (!(scheme === 'basic' ? basicAuth(m[4]) : random(m[4]))) continue;
        const at = m.index + m[1].length, from = at + m[2].length;
        const type = scheme === 'basic' ? T('Basic auth', 'lock') : T(scheme === 'bearer' ? 'Bearer token' : 'Token', 'key');
        yield { span: [at, from + m[4].length], blur: [from, from + m[4].length], type };
      }
    },
  ];

  // What a secret's name says it holds: DB_PASSWORD a password, X-Api-Key a key.
  function nameType(keyword) {
    const k = keyword.toLowerCase().replace(/[_-]/g, '');
    if (/^(?:pass|pwd|pw|passw|kennwort)/.test(k)) return PASSWORD_T;
    if (k === 'pin') return T('PIN', 'lock');
    if (k === 'token') return T('Token', 'key');
    if (k === 'secret') return T('Secret', 'key');
    if (k.startsWith('credential')) return T('Credentials', 'lock');
    if (k === 'apikey') return T('API key', 'key');
    if (k === 'privatekey') return T('Private key', 'key');
    return T('Secret key', 'key');
  }

  // Every secret in s, in the order the finders run. Earlier finders win where
  // matches overlap, so `API_KEY=sk-proj-…` keeps its token prefix visible.
  function scan(s) {
    const taken = [], out = [];
    for (const f of finders) {
      for (const hit of f(s)) {
        if (taken.some(([a, b]) => hit.span[0] < b && a < hit.span[1])) continue;
        taken.push(hit.span);
        out.push(hit);
      }
    }
    return out;
  }

  // Sorted [start, end) ranges of s to blur.
  function find(s) {
    return scan(s).map((h) => h.blur).filter(([a, b]) => b > a).sort((a, b) => a[0] - b[0]);
  }

  // Where the hidden part of a private key starts (just past its BEGIN line),
  // or -1 when text holds none.
  function privateKey(text) {
    const m = PRIVATE_KEY.exec(text);
    return m ? m.index + m[0].length : -1;
  }

  // The kinds of secret in text, each once, in the order they appear:
  // [{ label: 'IBAN', icon: 'bank' }, …].
  function types(text) {
    const hits = scan(text).map((h) => ({ at: h.span[0], type: h.type }));
    const pk = PRIVATE_KEY.exec(text);
    if (pk) hits.push({ at: pk.index, type: T(/OPENSSH/.test(pk[1]) ? 'SSH key' : /PGP/.test(pk[1]) ? 'PGP key' : 'Private key', 'key') });
    const seen = new Set();
    return hits.sort((a, b) => a.at - b.at).map((h) => h.type).filter((t) => !seen.has(t.label) && seen.add(t.label));
  }

  const api = { find, types, privateKey };
  if (typeof module !== 'undefined') module.exports = api;
  else root.StashSecrets = api;
})(typeof window !== 'undefined' ? window : globalThis);
