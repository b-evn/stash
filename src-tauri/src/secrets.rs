// Finds secrets in text, so what stash hands to other programs (the MCP server)
// never carries them. A port of ui/secrets.js, which blurs the same things in the
// list; keep the two in step. Offsets here are byte offsets into the text.

use base64::{engine::general_purpose::GeneralPurpose, engine::GeneralPurposeConfig, engine::DecodePaddingMode, Engine as _};
use fancy_regex::Regex as Fancy;
use regex::Regex;
use std::sync::LazyLock;

const TAIL: usize = 4;

#[derive(Debug, Clone, PartialEq)]
pub struct Hit {
    /// The whole match: a token's prefix, a password's name and value.
    pub span: (usize, usize),
    /// The part that is secret, which the list blurs and the MCP server replaces.
    pub blur: (usize, usize),
    pub label: &'static str,
}

const W: &str = "A-Za-z0-9_";

static IBAN: LazyLock<Fancy> =
    LazyLock::new(|| Fancy::new(r"(^|[^A-Z0-9])([A-Z]{2}[0-9]{2}(?: ?[A-Z0-9]{4}){2,7}(?: ?[A-Z0-9]{1,3})?)(?![A-Z0-9])").unwrap());
static CARD: LazyLock<Fancy> = LazyLock::new(|| Fancy::new(r"(^|[^0-9])([2-6](?:[ -]?[0-9]){12,18})(?![0-9])").unwrap());

// Token prefixes, longest first where one starts another.
const TOKENS: &[(&str, &str)] = &[
    (r"sk-ant-(?:api|admin)[0-9][0-9]-", "Anthropic key"),
    (r"sk-(?:proj|svcacct|admin)-", "OpenAI key"),
    (r"sk-or-v1-", "OpenRouter key"),
    (r"sk-", "API key"),
    (r"[sr]k_(?:live|test)_", "Stripe key"),
    (r"whsec_", "Webhook secret"),
    (r"gh[pousr]_|github_pat_", "GitHub token"),
    (r"glpat-", "GitLab token"),
    (r"xox[abeprs]-|xapp-", "Slack token"),
    (r"AKIA|ASIA", "AWS key"),
    (r"AIza", "Google API key"),
    (r"ya29\.", "Google token"),
    (r"npm_", "npm token"),
    (r"pypi-", "PyPI token"),
    (r"hf_", "Hugging Face token"),
    (r"shp(?:at|ca|pa|ss)_", "Shopify token"),
    (r"dop_v1_", "DigitalOcean token"),
    (r"lin_api_", "Linear key"),
    (r"SG\.", "SendGrid key"),
    (r"xai-", "xAI key"),
    (r"gsk_", "Groq key"),
    (r"r8_", "Replicate token"),
    (r"glsa_", "Grafana token"),
    (r"figd_", "Figma token"),
    (r"ntn_", "Notion token"),
    (r"ATATT", "Atlassian token"),
    (r"eyJ", "JWT"),
];
static TOKEN_TYPES: LazyLock<Vec<(Regex, &'static str)>> =
    LazyLock::new(|| TOKENS.iter().map(|(p, l)| (Regex::new(&format!("^(?:{p})$")).unwrap(), *l)).collect());
static TOKEN: LazyLock<Fancy> = LazyLock::new(|| {
    let alts = TOKENS.iter().map(|(p, _)| *p).collect::<Vec<_>>().join("|");
    Fancy::new(&format!(r"(^|[^{W}-])({alts})([{W}.-]{{16,}})")).unwrap()
});
static TELEGRAM: LazyLock<Fancy> =
    LazyLock::new(|| Fancy::new(&format!(r"(^|[^{W}:])([0-9]{{8,10}}:)(AA[{W}-]{{33}})(?![{W}-])")).unwrap());

static URL_PASSWORD: LazyLock<Fancy> = LazyLock::new(|| Fancy::new(&format!(r"(^|//)([^\s:/@]*):([^\s/@]+)@(?=[{W}-])")).unwrap());
static DB_SCHEME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?:postgres(?:ql)?|mysql|mariadb|mongodb(?:\+srv)?|rediss?|amqps?|mssql|sqlserver|clickhouse|cockroachdb|neo4j(?:\+s)?|couchdb)://$").unwrap()
});

// A name that says what follows is a secret: DB_PASSWORD, "client_secret",
// X-Api-Key, ?access_token. The keyword ends the name (max_tokens and
// secret_name are not secrets); short ones like pass and pin start it too.
static KEY_VALUE: LazyLock<Fancy> = LazyLock::new(|| {
    let name = format!(
        "(?:[{W}.-]*?(passw(?:or)?[dt]|passphrase|passcode|kennwort|secret|token|credentials?|\
         api[_-]?key|(?:access|account|private|secret|encryption|signing|master|client|shared[_-]?access)[_-]?key)|\
         (?:[{W}.-]*?[_.-])?(pass|pwd|pw|pin))\
         (?:[_.-]?(?:key|value|hash|digest|base|b64|base64|code|confirm(?:ation)?|repeat|again|enc(?:rypted)?|[0-9]))?"
    );
    // The value: quoted, or up to a space or the next `name=` in a query or
    // connection string, so `Tr0ub4dor&3` stays whole but `&state=x` is left.
    let value = format!(r#"(?:"([^"\n]{{2,}})"|'([^'\n]{{2,}})'|((?:(?![;,&][ \t]*[{W}.-]+[ \t]*[:=])[^\s"'`])+))"#);
    Fancy::new(&format!(r#"(?i)(^|[^a-z0-9])({name})(["']?[ \t]*(?::=|=>|[:=])[ \t]*){value}"#)).unwrap()
});
static BEARER: LazyLock<Fancy> = LazyLock::new(|| Fancy::new(&format!(r"(?i)(^|[^{W}])((Bearer|Basic|Token) +)([{W}.~+/-]{{8,}}=*)")).unwrap());
static PRIVATE_KEY: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"-----BEGIN ([A-Z0-9 ]*?)PRIVATE KEY(?: BLOCK)?-----").unwrap());

// Values after a secret's name that are not the secret itself: types,
// flags, placeholders and references to where the secret really lives.
static NOT_SECRET: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    [
        r"(?i)^(?:true|false|null|nil|none|undefined|yes|no|string|str|number|int|integer|bool|boolean|any|object|bytes|required|optional|include|omit|same-origin)$",
        r"(?i)^(?:\*+|x+|•+|\.+|\[redacted\])$",
        &format!(r"^(?:\$\{{[^}}]+\}}|\$[A-Z_][A-Z0-9_]*|\{{\{{.*\}}\}}|<[{W} -]*>?|%\([{W}]+\)s|%s)$"),
        r"^(?:process\.env|import\.meta\.env|os\.environ)\b",
        &format!(r"^[A-Za-z_$][{W}$.]*\(.*\)[;,]?$"),
        &format!(
            r"^(?:this|self|req|request|res|config|cfg|settings|options|opts|args|params|props|data|body|form|values|ctx|context|app|user|input|payload|credentials|secrets|auth|session|window|env)\.[{W}$.]+[;,]?$"
        ),
    ]
    .iter()
    .map(|p| Regex::new(p).unwrap())
    .collect()
});

fn not_secret(v: &str) -> bool {
    let v = v.strip_suffix([';', ',']).unwrap_or(v);
    NOT_SECRET.iter().any(|re| re.is_match(v))
}

// Random enough to be a key rather than a word: a digit, or mixed case.
fn random(s: &str) -> bool {
    s.bytes().any(|b| b.is_ascii_digit())
        || (s.chars().skip(1).any(|c| c.is_ascii_uppercase()) && s.chars().any(|c| c.is_ascii_lowercase()))
}

// Byte offsets of the letters and digits in s, so grouping spaces don't count.
fn alnum(s: &str, from: usize) -> Vec<usize> {
    s.char_indices().filter(|(_, c)| c.is_ascii_alphanumeric()).map(|(i, _)| from + i).collect()
}

fn iban_valid(iban: &str) -> bool {
    let s: String = iban.chars().filter(|&c| c != ' ').collect();
    if s.len() < 15 || s.len() > 34 {
        return false;
    }
    let mut r: u32 = 0;
    for c in s[4..].chars().chain(s[..4].chars()) {
        let digits = if c.is_ascii_uppercase() { (c as u32 - 55).to_string() } else { c.to_string() };
        for d in digits.chars() {
            r = (r * 10 + d.to_digit(10).unwrap_or(0)) % 97;
        }
    }
    r == 1
}

fn luhn(digits: &[u8]) -> bool {
    let mut sum = 0;
    for (i, d) in digits.iter().rev().enumerate() {
        let mut d = (d - b'0') as u32;
        if i % 2 == 1 {
            d *= 2;
            if d > 9 {
                d -= 9;
            }
        }
        sum += d;
    }
    sum % 10 == 0
}

// The card network a number belongs to, by its leading digits and length.
// Numbers no network issues are not cards, however their checksum comes out.
fn network(d: &str) -> Option<&'static str> {
    let n = d.len();
    let p = |len: usize| d.get(..len).and_then(|s| s.parse::<u32>().ok()).unwrap_or(0);
    let ok = |cond: bool, name| if cond { Some(name) } else { None };
    if matches!(p(2), 34 | 37) {
        return ok(n == 15, "Amex card");
    }
    if (300..=305).contains(&p(3)) || matches!(p(2), 36 | 38 | 39) {
        return ok(n >= 14, "Diners card");
    }
    if (3528..=3589).contains(&p(4)) {
        return ok(n >= 16, "JCB card");
    }
    if d.starts_with('4') {
        return ok([13, 16, 19].contains(&n), "Visa card");
    }
    if (51..=55).contains(&p(2)) || (2220..=2720).contains(&p(4)) {
        return ok(n == 16, "Mastercard");
    }
    if p(4) == 6011 || (644..=649).contains(&p(3)) || p(2) == 65 {
        return ok(n >= 16, "Discover card");
    }
    if p(2) == 62 {
        return ok(n >= 16, "UnionPay card");
    }
    if matches!(p(2), 50 | 56 | 57 | 58) || d.starts_with('6') {
        return ok(n >= 12, "Maestro card");
    }
    None
}

fn basic_auth(value: &str) -> bool {
    const LENIENT: GeneralPurpose = GeneralPurpose::new(
        &base64::alphabet::STANDARD,
        GeneralPurposeConfig::new().with_decode_padding_mode(DecodePaddingMode::Indifferent),
    );
    LENIENT.decode(value).map(|b| b.contains(&b':')).unwrap_or(false)
}

// What a secret's name says it holds: DB_PASSWORD a password, X-Api-Key a key.
fn name_type(keyword: &str) -> &'static str {
    let k: String = keyword.to_lowercase().chars().filter(|&c| c != '_' && c != '-').collect();
    if ["pass", "pwd", "pw", "passw", "kennwort"].iter().any(|p| k.starts_with(p)) {
        return "Password";
    }
    match k.as_str() {
        "pin" => "PIN",
        "token" => "Token",
        "secret" => "Secret",
        "apikey" => "API key",
        "privatekey" => "Private key",
        _ if k.starts_with("credential") => "Credentials",
        _ => "Secret key",
    }
}

fn captures<'t>(re: &Fancy, s: &'t str) -> Vec<fancy_regex::Captures<'t>> {
    re.captures_iter(s).filter_map(Result::ok).collect()
}

type Finder = fn(&str, &mut Vec<Hit>);

fn iban(s: &str, out: &mut Vec<Hit>) {
    for m in captures(&IBAN, s) {
        let g = m.get(2).unwrap();
        if !iban_valid(g.as_str()) {
            continue;
        }
        let pos = alnum(g.as_str(), g.start());
        out.push(Hit { span: (g.start(), g.end()), blur: (pos[4], pos[pos.len() - TAIL]), label: "IBAN" });
    }
}

fn card(s: &str, out: &mut Vec<Hit>) {
    for m in captures(&CARD, s) {
        let g = m.get(2).unwrap();
        let digits: String = g.as_str().chars().filter(char::is_ascii_digit).collect();
        let Some(net) = network(&digits) else { continue };
        if !luhn(digits.as_bytes()) {
            continue;
        }
        let pos = alnum(g.as_str(), g.start());
        out.push(Hit { span: (g.start(), g.end()), blur: (g.start(), pos[pos.len() - TAIL]), label: net });
    }
}

fn token(s: &str, out: &mut Vec<Hit>) {
    for m in captures(&TOKEN, s) {
        let (prefix, rest) = (m.get(2).unwrap(), m.get(3).unwrap());
        let body = rest.as_str().trim_end_matches(['.', '-']);
        if !random(body) {
            continue;
        }
        let end = rest.start() + body.len();
        let label = TOKEN_TYPES.iter().find(|(re, _)| re.is_match(prefix.as_str())).map_or("API key", |(_, l)| *l);
        out.push(Hit { span: (prefix.start(), end), blur: (rest.start(), end.saturating_sub(TAIL).max(rest.start())), label });
    }
    for m in captures(&TELEGRAM, s) {
        let (lead, body) = (m.get(2).unwrap(), m.get(3).unwrap());
        out.push(Hit { span: (lead.start(), body.end()), blur: (body.start(), body.end() - TAIL), label: "Telegram token" });
    }
}

fn url_password(s: &str, out: &mut Vec<Hit>) {
    for m in captures(&URL_PASSWORD, s) {
        let (user, pass) = (m.get(2).unwrap(), m.get(3).unwrap());
        if not_secret(pass.as_str()) {
            continue;
        }
        let at = user.start();
        let before = &s[floor_boundary(s, at.saturating_sub(24))..at];
        let label = if DB_SCHEME.is_match(before) { "Database URL" } else { "URL password" };
        out.push(Hit { span: (at, pass.end()), blur: (pass.start(), pass.end()), label });
    }
}

fn key_value(s: &str, out: &mut Vec<Hit>) {
    for m in captures(&KEY_VALUE, s) {
        let bare = m.get(8);
        let Some(value) = m.get(6).or(m.get(7)).or(bare) else { continue };
        if not_secret(value.as_str()) || (bare.is_some() && value.as_str().chars().count() < 3) {
            continue;
        }
        let keyword = m.get(3).or(m.get(4)).map_or("", |g| g.as_str());
        let at = m.get(2).unwrap().start();
        out.push(Hit { span: (at, value.end()), blur: (value.start(), value.end()), label: name_type(keyword) });
    }
}

fn bearer(s: &str, out: &mut Vec<Hit>) {
    for m in captures(&BEARER, s) {
        let (lead, scheme, value) = (m.get(2).unwrap(), m.get(3).unwrap().as_str().to_lowercase(), m.get(4).unwrap());
        let ok = if scheme == "basic" { basic_auth(value.as_str()) } else { random(value.as_str()) };
        if !ok {
            continue;
        }
        let label = match scheme.as_str() {
            "basic" => "Basic auth",
            "bearer" => "Bearer token",
            _ => "Token",
        };
        out.push(Hit { span: (lead.start(), value.end()), blur: (value.start(), value.end()), label });
    }
}

const FINDERS: &[Finder] = &[iban, card, token, url_password, key_value, bearer];

// Something every secret above has in it, found by a plain regex in one fast
// pass: the finders' lookarounds are slow, so they only run on lines this
// finds. It must match wherever any finder could; matching more is only slower.
static PREFILTER: LazyLock<Regex> = LazyLock::new(|| {
    let alts = TOKENS.iter().map(|(p, _)| *p).collect::<Vec<_>>().join("|");
    Regex::new(&format!(
        "[A-Z]{{2}}[0-9]{{2}}\
         |[2-6](?:[ -]?[0-9]){{12}}\
         |(?:{alts})[{W}.-]{{16}}\
         |[0-9]{{8}}:AA\
         |:[^\\s/@]+@\
         |(?i:pass|pwd|pw|pin|secret|token|credential|kennwort|key)[{W}.-]*[\"']?[ \\t]*(?::=|=>|[:=])\
         |(?i:bearer|basic|token) +[{W}.~+/-]{{8}}"
    ))
    .unwrap()
});

/// Longest stretch of one line the finders look at around a likely secret.
const LONG_LINE: usize = 4096;

/// Every secret in s, in the order the finders run. Earlier finders win where
/// matches overlap, so `API_KEY=sk-proj-…` keeps its token prefix visible.
/// No secret spans a line break, so only lines the prefilter finds are read.
pub fn scan(s: &str) -> Vec<Hit> {
    let mut windows: Vec<(usize, usize)> = Vec::new();
    for m in PREFILTER.find_iter(s) {
        let start = s[..m.start()].rfind('\n').map_or(0, |i| i + 1);
        let end = s[m.end()..].find('\n').map_or(s.len(), |i| m.end() + i);
        let lo = floor_boundary(s, start.max(m.start().saturating_sub(LONG_LINE / 2)));
        let hi = ceil_boundary(s, end.min(m.end() + LONG_LINE / 2));
        match windows.last_mut() {
            Some(w) if lo <= w.1 => w.1 = w.1.max(hi),
            _ => windows.push((lo, hi)),
        }
    }
    let mut out = Vec::new();
    for (lo, hi) in windows {
        out.extend(scan_lines(&s[lo..hi]).into_iter().map(|h| Hit {
            span: (lo + h.span.0, lo + h.span.1),
            blur: (lo + h.blur.0, lo + h.blur.1),
            label: h.label,
        }));
    }
    out
}

fn scan_lines(s: &str) -> Vec<Hit> {
    let mut out: Vec<Hit> = Vec::new();
    for f in FINDERS {
        let mut found = Vec::new();
        f(s, &mut found);
        for hit in found {
            if out.iter().any(|h| hit.span.0 < h.span.1 && h.span.0 < hit.span.1) {
                continue;
            }
            out.push(hit);
        }
    }
    out
}

#[cfg(test)]
/// Sorted [start, end) ranges of s to blur.
pub fn find(s: &str) -> Vec<(usize, usize)> {
    let mut v: Vec<_> = scan(s).into_iter().map(|h| h.blur).filter(|(a, b)| b > a).collect();
    v.sort();
    v
}

/// The private key in text: where its hidden part starts (just past the BEGIN
/// line), where it ends (the END line, or the end of the text) and its name.
pub fn private_key(text: &str) -> Option<(usize, usize, &'static str)> {
    let m = PRIVATE_KEY.captures(text)?;
    let all = m.get(0).unwrap();
    let kind = m.get(1).map_or("", |g| g.as_str());
    let label = if kind.contains("OPENSSH") { "SSH key" } else if kind.contains("PGP") { "PGP key" } else { "Private key" };
    let end = text[all.end()..].find("-----END ").map_or(text.len(), |i| all.end() + i);
    Some((all.end(), end, label))
}

#[cfg(test)]
/// The kinds of secret in text, each once, in the order they appear.
pub fn types(text: &str) -> Vec<&'static str> {
    let mut hits: Vec<(usize, &str)> = scan(text).into_iter().map(|h| (h.span.0, h.label)).collect();
    if let Some((at, _, label)) = private_key(text) {
        hits.push((at, label));
    }
    hits.sort_by_key(|h| h.0);
    let mut seen = Vec::new();
    for (_, l) in hits {
        if !seen.contains(&l) {
            seen.push(l);
        }
    }
    seen
}

pub fn floor_boundary(s: &str, mut i: usize) -> usize {
    if i >= s.len() {
        return s.len();
    }
    while !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

pub fn ceil_boundary(s: &str, mut i: usize) -> usize {
    if i >= s.len() {
        return s.len();
    }
    while !s.is_char_boundary(i) {
        i += 1;
    }
    i
}

/// How far around a slice stash looks for secrets, so one the slice cuts in
/// half is still recognised by its whole.
const CONTEXT: usize = 512;

/// `text[start..end]` with every secret in it replaced by `‹label›`. Secrets that
/// cross the slice's edges are hidden too. Offsets are snapped to characters.
pub fn redact(text: &str, start: usize, end: usize) -> String {
    let (start, end) = (floor_boundary(text, start), ceil_boundary(text, end.max(start)));
    let lo = floor_boundary(text, start.saturating_sub(CONTEXT));
    let hi = ceil_boundary(text, end + CONTEXT);
    let mut spans: Vec<(usize, usize, &str)> =
        scan(&text[lo..hi]).into_iter().filter(|h| h.blur.1 > h.blur.0).map(|h| (lo + h.blur.0, lo + h.blur.1, h.label)).collect();
    if let Some((a, b, label)) = private_key(text) {
        spans.retain(|s| s.1 <= a || s.0 >= b);
        spans.push((a, b, label));
    }
    spans.sort_by_key(|s| s.0);
    let mut out = String::with_capacity(end - start);
    let mut at = start;
    for (a, b, label) in spans {
        if b <= at || a >= end {
            continue;
        }
        out.push_str(&text[at..a.max(at)]);
        out.push('‹');
        out.push_str(label);
        out.push('›');
        at = b.min(end);
    }
    out.push_str(&text[at..end]);
    out
}

/// Whether `text[a..b]` touches a secret. `key` is the private key's range, if
/// the text holds one (see `private_key`), so callers can look it up once.
pub fn hidden(text: &str, a: usize, b: usize, key: Option<(usize, usize)>) -> bool {
    if key.is_some_and(|(ka, kb)| a < kb && ka < b) {
        return true;
    }
    let lo = floor_boundary(text, a.saturating_sub(CONTEXT));
    let hi = ceil_boundary(text, b + CONTEXT);
    scan(&text[lo..hi]).iter().any(|h| h.blur.1 > h.blur.0 && a < lo + h.blur.1 && lo + h.blur.0 < b)
}

pub fn redact_all(text: &str) -> String {
    redact(text, 0, text.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    // s with its blurred ranges as •, to compare against what stays visible.
    fn show(s: &str) -> String {
        let blur = find(s);
        s.char_indices().map(|(i, c)| if blur.iter().any(|&(a, b)| i >= a && i < b) { '•' } else { c }).collect()
    }
    fn b(n: usize) -> String {
        "•".repeat(n)
    }

    #[test]
    fn same_blur_as_the_list() {
        assert_eq!(show("DE89370400440532013000"), format!("DE89{}3000", b(14)));
        assert_eq!(show("IBAN: DE89 3704 0044 0532 0130 00"), format!("IBAN: DE89 {}30 00", b(17)));
        assert_eq!(show("DE00370400440532013000"), "DE00370400440532013000");
        assert_eq!(show("4111 1111 1111 1111"), format!("{}1111", b(15)));
        assert_eq!(show("card 5555-5555-5555-4444 exp 12/29"), format!("card {}4444 exp 12/29", b(15)));
        assert_eq!(show("4111 1111 1111 1112"), "4111 1111 1111 1112");
        assert_eq!(show("1793023865319"), "1793023865319");
        assert_eq!(show("sk-ant-api03-abcdefghijklmnopqrstuvwxyz0123"), format!("sk-ant-api03-{}0123", b(26)));
        assert_eq!(show("ghp_aBcDeFgHiJkLmNoPqRsTuVwXyZ012345"), format!("ghp_{}2345", b(28)));
        assert_eq!(show("key AKIAIOSFODNN7EXAMPLE."), format!("key AKIA{}MPLE.", b(12)));
        assert_eq!(show("eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxIn0.c2lnbmF0dXJl"), format!("eyJ{}dXJl", b(42)));
        assert_eq!(show("ask-me-anything"), "ask-me-anything");
        assert_eq!(show("task-abcdefghijklmnopqrstuvwxyz"), "task-abcdefghijklmnopqrstuvwxyz");
        assert_eq!(show("DB_PASSWORD=hunter22"), format!("DB_PASSWORD={}", b(8)));
        assert_eq!(show(r#""client_secret": "s3cr3t-value","#), format!(r#""client_secret": "{}","#, b(12)));
        assert_eq!(show("example.com/cb?access_token=abc123&state=x"), format!("example.com/cb?access_token={}&state=x", b(6)));
        assert_eq!(show("OPENAI_API_KEY=sk-proj-4bG7kLm2Qx9Rt5Vw8Yz1Np3s"), format!("OPENAI_API_KEY=sk-proj-{}Np3s", b(20)));
        assert_eq!(show("postgres://app:p4ssw0rd@db.internal:5432/main"), format!("postgres://app:{}@db.internal:5432/main", b(8)));
        assert_eq!(show("app:p4ssw0rd@db.internal"), format!("app:{}@db.internal", b(8)));
        assert_eq!(show("Authorization: Bearer abcdef0123456789"), format!("Authorization: Bearer {}", b(16)));
        assert_eq!(show("redis://:p4ss@cache:6379"), format!("redis://:{}@cache:6379", b(4)));
        assert_eq!(show("Authorization: Basic dXNlcjpodW50ZXIy"), format!("Authorization: Basic {}", b(16)));
        assert_eq!(show("Password: Tr0ub4dor&3"), format!("Password: {}", b(11)));
        assert_eq!(show(r#"password: "correct horse battery staple""#), format!(r#"password: "{}""#, b(28)));
        assert_eq!(
            show("Server=db;User Id=sa;Password=S3cr3t!;Encrypt=true"),
            format!("Server=db;User Id=sa;Password={};Encrypt=true", b(7))
        );
        assert_eq!(show("'password' => 'hunter22',"), format!("'password' => '{}',", b(8)));
        assert_eq!(show(r#"password := "hunter22""#), format!(r#"password := "{}""#, b(8)));
        assert_eq!(show("Passwort: Sommer2024!"), format!("Passwort: {}", b(11)));
        assert_eq!(show("PIN: 4711"), format!("PIN: {}", b(4)));
        assert_eq!(show("DB_PASS=hunter22"), format!("DB_PASS={}", b(8)));
        assert_eq!(show("X-Api-Key: 9f8e7d6c5b4a"), format!("X-Api-Key: {}", b(12)));
        assert_eq!(show("bot 123456789:AAHdqTcvCH1vGWJxfSeofSAs0K5PALDsaw0 here"), format!("bot 123456789:{}saw0 here", b(31)));
        assert_eq!(show("2223 0000 4840 0011"), format!("{}0011", b(15)));
    }

    #[test]
    fn ordinary_text_is_left_alone() {
        for s in [
            "Meeting moved to Thursday 14:00",
            "billing@acme-industries.com",
            "https://github.com/anthropics",
            "const total = items.reduce((sum, i) => sum + i.price, 0);",
            "call +49 30 1234567",
            "#18181b",
            "max_tokens: 4096",
            "token_type: bearer",
            "secret_name: prod-db",
            "passwordless: true",
            "bypass: true",
            "spin=3",
            "credentials: 'include',",
            "password: string;",
            "password=${DB_PASS}",
            "password: process.env.DB_PASSWORD,",
            "token: this.token,",
            "const pw = getPassword();",
            "password: ********",
            "password: <your password>",
            "PASSWORD=\nUSER=bob",
            "postgres://user:${PASSWORD}@db/x",
            "Basic understanding of things",
            "Bearer authentication is common",
            "xai-grok-vision-beta-model",
            "2000 0000 0000 0008",
        ] {
            assert_eq!(find(s), vec![], "{s}");
        }
    }

    #[test]
    fn each_secret_says_what_it_is() {
        assert_eq!(types("DE89370400440532013000"), ["IBAN"]);
        assert_eq!(types("4111 1111 1111 1111"), ["Visa card"]);
        assert_eq!(types("5555-5555-5555-4444"), ["Mastercard"]);
        assert_eq!(types("3782 822463 10005"), ["Amex card"]);
        assert_eq!(types("ghp_aBcDeFgHiJkLmNoPqRsTuVwXyZ012345"), ["GitHub token"]);
        assert_eq!(types(concat!("sk_", "live_51NzQ8vKd2Lr7XbW4eTfYp9Hm")), ["Stripe key"]);
        assert_eq!(types("AKIAIOSFODNN7EXAMPLE"), ["AWS key"]);
        assert_eq!(types("DB_PASSWORD=hunter22"), ["Password"]);
        assert_eq!(types("postgres://app:p4ssw0rd@db:5432/main"), ["Database URL"]);
        assert_eq!(types("https://bob:p4ssw0rd@example.com"), ["URL password"]);
        assert_eq!(types("Authorization: Bearer abcdef0123456789"), ["Bearer token"]);
        assert_eq!(types("-----BEGIN OPENSSH PRIVATE KEY-----\nb3Bl\n-----END OPENSSH PRIVATE KEY-----"), ["SSH key"]);
        assert_eq!(
            types(concat!(
                "STRIPE_SECRET_KEY=sk_",
                "live_51NzQ8vKd2Lr7XbW4eTfYp9Hm\nDATABASE_URL=postgres://app:Tr0ub4dor@db/main\nREDIS_PASSWORD=x9y8z7"
            )),
            ["Stripe key", "Database URL", "Password"]
        );
        assert_eq!(types("Meeting moved to Thursday 14:00"), Vec::<&str>::new());
    }

    #[test]
    fn redaction_replaces_secrets_with_their_name() {
        assert_eq!(redact_all("DB_PASSWORD=hunter22 ok"), "DB_PASSWORD=‹Password› ok");
        assert_eq!(redact_all("ghp_aBcDeFgHiJkLmNoPqRsTuVwXyZ012345"), "ghp_‹GitHub token›2345");
        let key = "x\n-----BEGIN OPENSSH PRIVATE KEY-----\nb3BlbnNzaC1rZXk\n-----END OPENSSH PRIVATE KEY-----\ny";
        assert_eq!(redact_all(key), "x\n-----BEGIN OPENSSH PRIVATE KEY-----‹SSH key›-----END OPENSSH PRIVATE KEY-----\ny");
        // A slice that cuts a secret in half still hides its half.
        let s = "note: DB_PASSWORD=hunter22";
        assert_eq!(redact(s, 0, 22), "note: DB_PASSWORD=‹Password›");
        assert_eq!(redact(s, 22, s.len()), "‹Password›");
        // Offsets inside a character widen to take it whole.
        assert_eq!(redact("héllo wörld", 2, 9), "éllo wö");
    }
}
