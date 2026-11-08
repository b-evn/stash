// The query language AI tools search the history with, and how matches are
// ranked. A superset of the popup's (ui/search.js): words, 'exact, ^start,
// !without and @kind filters work the same, plus phrases, either-or, regexes,
// dates, sites and sizes. Matching is literal rather than loose, since an agent
// types whole words, and ranking is BM25 nudged by how recently and how often
// a clip was used.

use crate::secrets;
use regex::{Regex, RegexBuilder};

const DAY: i64 = 86_400_000;
/// Matches counted per term and clip; more say nothing new about relevance.
const TF_CAP: u32 = 64;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Sort {
    Relevance,
    Recent,
    Oldest,
    Frequent,
    Largest,
    Smallest,
}

impl Sort {
    pub const NAMES: &[&str] = &["relevance", "recent", "oldest", "frequent", "largest", "smallest"];

    pub fn parse(s: &str) -> Option<Sort> {
        Some(match s.trim().to_ascii_lowercase().as_str() {
            "relevance" | "best" | "score" => Sort::Relevance,
            "recent" | "newest" | "new" => Sort::Recent,
            "oldest" | "old" => Sort::Oldest,
            "frequent" | "used" | "popular" => Sort::Frequent,
            "largest" | "longest" | "biggest" | "size" => Sort::Largest,
            "smallest" | "shortest" => Sort::Smallest,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        Sort::NAMES[self as usize]
    }
}

pub struct Term {
    pub re: Regex,
}

#[derive(Default)]
pub struct Query {
    pub terms: Vec<Term>,
    pub not: Vec<Regex>,
    pub kinds: Vec<&'static str>,
    pub pinned: bool,
    pub since: Option<i64>,
    pub before: Option<i64>,
    pub sites: Vec<String>,
    pub min_tokens: Option<usize>,
    pub max_tokens: Option<usize>,
    /// The bare words in order, as one phrase: clips holding them together rank higher.
    pub phrase: Option<Regex>,
}

impl Query {
    pub fn is_empty(&self) -> bool {
        self.terms.is_empty()
    }
}

fn kind(word: &str) -> Option<&'static str> {
    Some(match word {
        "link" | "links" | "url" | "urls" => "link",
        "code" => "code",
        "path" | "paths" | "folder" => "path",
        "color" | "colour" | "colors" | "colours" => "color",
        "number" | "num" | "numbers" => "number",
        "email" | "mail" | "emails" => "email",
        "text" => "text",
        "image" | "images" | "img" | "screenshot" | "screenshots" => "image",
        "file" | "files" => "file",
        _ => return None,
    })
}

/// Splits on spaces, keeping "quoted phrases" and /regexes with spaces/ (after
/// any ! ' ^ in front) together.
fn tokens(q: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    let open_regex = |cur: &str| {
        let body = cur.trim_start_matches(['!', '\'', '^']);
        body.starts_with('/') && body.matches('/').count() == 1
    };
    for c in q.chars() {
        if c == '"' && (quoted || cur.chars().all(|p| matches!(p, '!' | '\'' | '^'))) {
            quoted = !quoted;
            cur.push(c);
        } else if c.is_whitespace() && !quoted && !open_regex(&cur) {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
        } else {
            cur.push(c);
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Sub,
    Word,
    Start,
}

fn build(src: &str, insensitive: bool) -> Result<Regex, String> {
    RegexBuilder::new(src)
        .case_insensitive(insensitive)
        .size_limit(1 << 20)
        .dfa_size_limit(1 << 22)
        .build()
        .map_err(|e| format!("bad pattern {src:?}: {}", e.to_string().lines().last().unwrap_or("")))
}

/// A pattern as the query and grep take it: `/regex/` (with `i` after it for
/// any case), or literal text that ignores case unless it has a capital.
pub fn pattern(body: &str) -> Result<Regex, String> {
    compile(body, Mode::Sub)
}

fn compile(body: &str, mode: Mode) -> Result<Regex, String> {
    if let Some(rest) = body.strip_prefix('/') {
        if let Some(end) = rest.rfind('/') {
            let flags = &rest[end + 1..];
            if flags.chars().all(|c| c == 'i') {
                return build(&rest[..end], !flags.is_empty());
            }
        }
    }
    let body = body.strip_prefix('"').map(|b| b.strip_suffix('"').unwrap_or(b)).unwrap_or(body);
    if body.is_empty() {
        return Err("empty term".into());
    }
    let insensitive = !body.chars().any(char::is_uppercase);
    let literal = |s: &str| s.split_whitespace().map(regex::escape).collect::<Vec<_>>().join(r"\s+");
    let alts: Vec<String> = if body.contains('|') && !body.contains(char::is_whitespace) {
        body.split('|').filter(|s| !s.is_empty()).map(literal).collect()
    } else {
        vec![literal(body)]
    };
    let word_char = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric() || c == '_');
    let alts: Vec<String> = alts
        .into_iter()
        .zip(if body.contains('|') { body.split('|').filter(|s| !s.is_empty()).collect::<Vec<_>>() } else { vec![body] })
        .map(|(re, raw)| match mode {
            Mode::Word => {
                let (a, b) = (word_char(raw.chars().next()), word_char(raw.chars().last()));
                format!("{}{re}{}", if a { r"\b" } else { "" }, if b { r"\b" } else { "" })
            }
            _ => re,
        })
        .collect();
    let joined = format!("(?:{})", alts.join("|"));
    build(&if mode == Mode::Start { format!(r"\A\s*{joined}") } else { joined }, insensitive)
}

/// When `@since:` and `@before:` mean: 3d, 12h, 30m, 2w, 6mo, 1y ago, or a
/// date (2026-11-01, 2026-11), taken as midnight UTC.
fn when(v: &str, now: i64) -> Option<i64> {
    let v = v.trim();
    if let Some(n) = v.find(|c: char| !c.is_ascii_digit()).filter(|&i| i > 0) {
        let (num, unit) = v.split_at(n);
        if let Ok(num) = num.parse::<i64>() {
            let ms = match unit {
                "m" | "min" => 60_000,
                "h" => 3_600_000,
                "d" => DAY,
                "w" => 7 * DAY,
                "mo" => 30 * DAY,
                "y" => 365 * DAY,
                _ => 0,
            };
            if ms > 0 {
                return Some(now - num * ms);
            }
        }
    }
    let parts: Vec<i64> = v.split('-').map(|p| p.parse().ok()).collect::<Option<_>>()?;
    let (y, m, d) = match parts[..] {
        [y] => (y, 1, 1),
        [y, m] => (y, m, 1),
        [y, m, d] => (y, m, d),
        _ => return None,
    };
    if !(1970..=9999).contains(&y) || !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    Some(days_from_civil(y, m, d) * DAY)
}

// Days since 1970-01-01 (Howard Hinnant's algorithm).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn size(v: &str) -> Option<usize> {
    let v = v.trim().to_ascii_lowercase();
    let (num, mul) = match v.strip_suffix('k') {
        Some(n) => (n, 1000.0),
        None => match v.strip_suffix('m') {
            Some(n) => (n, 1_000_000.0),
            None => (v.as_str(), 1.0),
        },
    };
    num.parse::<f64>().ok().map(|n| (n * mul) as usize)
}

pub fn parse(q: &str, now: i64) -> Result<Query, String> {
    let mut p = Query::default();
    let mut words = Vec::new();
    for tok in tokens(q) {
        if let Some(rest) = tok.strip_prefix('@') {
            let lower = rest.to_ascii_lowercase();
            let (key, val) = lower.split_once(':').map_or((lower.as_str(), ""), |(k, v)| (k, v));
            let at = |v: &str| when(v, now).ok_or_else(|| format!("can't read the date in {tok:?}; use 3d, 12h, 2w or 2026-11-01"));
            match key {
                "pin" | "pinned" => p.pinned = true,
                "hour" => p.since = Some(now - 3_600_000),
                "today" => p.since = Some(now - DAY),
                "week" => p.since = Some(now - 7 * DAY),
                "month" => p.since = Some(now - 30 * DAY),
                "year" => p.since = Some(now - 365 * DAY),
                "since" | "after" if !val.is_empty() => p.since = Some(at(val)?),
                "before" | "until" if !val.is_empty() => p.before = Some(at(val)?),
                "site" | "from" | "domain" if !val.is_empty() => p.sites.push(val.trim_start_matches("www.").to_string()),
                _ if key.starts_with("size") || key.starts_with("tokens") => {
                    let op = key.trim_start_matches("size").trim_start_matches("tokens");
                    let op = if op.is_empty() { val } else { op };
                    let (lt, n) = match op.strip_prefix('<') {
                        Some(n) => (true, n),
                        None => (false, op.strip_prefix('>').unwrap_or(op)),
                    };
                    let n = size(n.trim_start_matches('=')).ok_or_else(|| format!("can't read the size in {tok:?}; use @size>2k or @size<500"))?;
                    if lt {
                        p.max_tokens = Some(n);
                    } else {
                        p.min_tokens = Some(n);
                    }
                }
                _ => match kind(key) {
                    Some(k) if val.is_empty() => p.kinds.push(k),
                    _ => p.terms.push(Term { re: compile(&tok, Mode::Sub)? }),
                },
            }
        } else if let Some(rest) = tok.strip_prefix('!').filter(|r| !r.is_empty()) {
            p.not.push(compile(rest, Mode::Sub)?);
        } else if let Some(rest) = tok.strip_prefix('\'').filter(|r| !r.is_empty()) {
            p.terms.push(Term { re: compile(rest, Mode::Word)? });
        } else if let Some(rest) = tok.strip_prefix('^').filter(|r| !r.is_empty()) {
            p.terms.push(Term { re: compile(rest, Mode::Start)? });
        } else {
            let bare = !tok.starts_with('/') && !tok.starts_with('"') && !tok.contains('|');
            if bare {
                words.push(tok.clone());
            }
            p.terms.push(Term { re: compile(&tok, Mode::Sub)? });
        }
    }
    if words.len() >= 2 {
        p.phrase = compile(&words.join(" "), Mode::Sub).ok();
    }
    Ok(p)
}

/// The host a clip came from, for `@site:`: the page it was copied from, or
/// the link itself.
pub fn host(url: &str) -> Option<&str> {
    let rest = url.split_once("://")?.1;
    let host = rest.split(['/', '?', '#']).next()?;
    let host = host.rsplit_once('@').map_or(host, |(_, h)| h);
    let host = host.split(':').next()?;
    (!host.is_empty()).then(|| host.trim_start_matches("www."))
}

pub fn estimate_tokens(chars: usize) -> usize {
    chars.div_ceil(4)
}

/// One clip as the ranker sees it.
pub struct Doc<'a> {
    pub id: i64,
    pub hay: &'a str,
    pub kind: &'a str,
    pub site: Option<&'a str>,
    pub pinned: bool,
    pub uses: i64,
    pub created: i64,
    pub last_used: i64,
    pub tokens: usize,
}

// Where in the text a pattern matches outside any secret, and how often it
// matches there. A match that touches a hidden secret doesn't count, so a
// search can't be used to check a guess at a password. Only matches up to the
// first visible one are checked, which keeps common words cheap; the ones
// after it may include a secret and only nudge the score.
fn visible(re: &Regex, hay: &str, key: &mut Option<Option<(usize, usize)>>) -> (u32, Option<(usize, usize)>) {
    let mut tf = 0;
    let mut first = None;
    for m in re.find_iter(hay) {
        if m.start() == m.end() {
            continue;
        }
        if first.is_none() {
            let pk = *key.get_or_insert_with(|| secrets::private_key(hay).map(|(a, b, _)| (a, b)));
            if secrets::hidden(hay, m.start(), m.end(), pk) {
                continue;
            }
        }
        tf += 1;
        if first.is_none() {
            first = Some((m.start(), m.end()));
        }
        if tf >= TF_CAP {
            break;
        }
    }
    (tf, first)
}

struct Cand {
    id: i64,
    tf: Vec<u32>,
    first: Vec<Option<(usize, usize)>>,
    early: Vec<bool>,
    phrase: bool,
    tokens: usize,
    pinned: bool,
    uses: i64,
    created: i64,
    last_used: i64,
}

pub struct Ranked {
    pub id: i64,
    pub score: f64,
    /// The match to show in a preview: of the rarest word that matched.
    pub at: Option<(usize, usize)>,
}

/// Collects the clips a query matches, then ranks them. Every clip that passes
/// the filters counts towards how rare each word is.
pub struct Ranker<'q> {
    q: &'q Query,
    now: i64,
    n: usize,
    total_tokens: f64,
    df: Vec<usize>,
    cands: Vec<Cand>,
}

impl<'q> Ranker<'q> {
    pub fn new(q: &'q Query, now: i64) -> Self {
        Ranker { q, now, n: 0, total_tokens: 0.0, df: vec![0; q.terms.len()], cands: Vec::new() }
    }

    fn passes(&self, d: &Doc) -> bool {
        let q = self.q;
        (q.kinds.is_empty() || q.kinds.contains(&d.kind))
            && (!q.pinned || d.pinned)
            && q.since.is_none_or(|t| d.last_used >= t)
            && q.before.is_none_or(|t| d.last_used < t)
            && q.min_tokens.is_none_or(|n| d.tokens >= n)
            && q.max_tokens.is_none_or(|n| d.tokens <= n)
            && (q.sites.is_empty() || d.site.is_some_and(|h| q.sites.iter().any(|s| h == s || h.ends_with(&format!(".{s}")))))
    }

    /// Whether the clip matches; matching clips are kept for ranking.
    pub fn add(&mut self, d: &Doc) -> bool {
        if !self.passes(d) {
            return false;
        }
        self.n += 1;
        self.total_tokens += d.tokens.max(1) as f64;
        let mut key = None;
        let mut tf = Vec::with_capacity(self.q.terms.len());
        let mut first = Vec::with_capacity(self.q.terms.len());
        for (i, t) in self.q.terms.iter().enumerate() {
            let (n, at) = visible(&t.re, d.hay, &mut key);
            if n > 0 {
                self.df[i] += 1;
            }
            tf.push(n);
            first.push(at);
        }
        if tf.contains(&0) || self.q.not.iter().any(|re| visible(re, d.hay, &mut key).0 > 0) {
            return false;
        }
        let head = secrets::ceil_boundary(d.hay, 240);
        let early = first.iter().map(|f| f.is_some_and(|(a, _)| a < head)).collect();
        let phrase = self.q.phrase.as_ref().is_some_and(|re| visible(re, d.hay, &mut key).0 > 0);
        self.cands.push(Cand {
            id: d.id,
            tf,
            first,
            early,
            phrase,
            tokens: d.tokens,
            pinned: d.pinned,
            uses: d.uses,
            created: d.created,
            last_used: d.last_used,
        });
        true
    }

    pub fn len(&self) -> usize {
        self.cands.len()
    }

    pub fn finish(self, sort: Sort) -> Vec<Ranked> {
        let (k1, b) = (1.2, 0.75);
        let n = self.n.max(1) as f64;
        let avg = (self.total_tokens / n).max(1.0);
        let idf: Vec<f64> = self.df.iter().map(|&df| (1.0 + (n - df as f64 + 0.5) / (df as f64 + 0.5)).ln()).collect();
        let rarest = (0..idf.len()).max_by(|&a, &b| idf[a].total_cmp(&idf[b]));
        let now = self.now;
        let mut out: Vec<(Ranked, Cand)> = self
            .cands
            .into_iter()
            .map(|c| {
                let len = c.tokens.max(1) as f64;
                let mut score = 0.0;
                for (i, &tf) in c.tf.iter().enumerate() {
                    let tf = tf as f64;
                    let mut s = idf[i] * tf * (k1 + 1.0) / (tf + k1 * (1.0 - b + b * len / avg));
                    if c.early[i] {
                        s *= 1.3;
                    }
                    score += s;
                }
                if c.phrase {
                    score *= 1.3;
                }
                let age_days = (now - c.last_used).max(0) as f64 / DAY as f64;
                let prior = 1.0 + 0.4 * (-age_days / 7.0).exp() + 0.08 * (1.0 + c.uses.max(0) as f64).ln() + if c.pinned { 0.15 } else { 0.0 };
                let at = rarest.and_then(|r| c.first[r]).or_else(|| c.first.iter().flatten().next().copied());
                (Ranked { id: c.id, score: score * prior, at }, c)
            })
            .collect();
        let sort = if sort == Sort::Relevance && idf.is_empty() { Sort::Recent } else { sort };
        out.sort_by(|(ra, a), (rb, b)| {
            let recent = b.last_used.cmp(&a.last_used).then(b.id.cmp(&a.id));
            match sort {
                Sort::Relevance => rb.score.total_cmp(&ra.score).then(recent),
                Sort::Recent => recent,
                Sort::Oldest => a.created.cmp(&b.created).then(a.id.cmp(&b.id)),
                Sort::Frequent => b.uses.cmp(&a.uses).then(recent),
                Sort::Largest => b.tokens.cmp(&a.tokens).then(recent),
                Sort::Smallest => a.tokens.cmp(&b.tokens).then(recent),
            }
        });
        out.into_iter().map(|(r, _)| r).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_800_000_000_000;

    fn doc<'a>(id: i64, hay: &'a str, kind: &'a str, age_days: i64) -> Doc<'a> {
        Doc {
            id,
            hay,
            kind,
            site: None,
            pinned: false,
            uses: 0,
            created: NOW - age_days * DAY,
            last_used: NOW - age_days * DAY,
            tokens: estimate_tokens(hay.len()),
        }
    }

    fn ids(q: &str, docs: &[Doc], sort: Sort) -> Vec<i64> {
        let q = parse(q, NOW).unwrap();
        let mut r = Ranker::new(&q, NOW);
        for d in docs {
            r.add(d);
        }
        r.finish(sort).into_iter().map(|r| r.id).collect()
    }

    #[test]
    fn words_must_all_match_and_ignore_case_unless_capitalised() {
        let docs = [doc(1, "Parse the entry", "text", 0), doc(2, "parse only", "text", 0), doc(3, "PARSE ENTRY", "text", 0)];
        assert_eq!(ids("parse entry", &docs, Sort::Recent), [3, 1]);
        assert_eq!(ids("Parse", &docs, Sort::Recent), [1]);
        assert_eq!(ids("parse !only", &docs, Sort::Recent), [3, 1]);
        assert_eq!(ids("only|ENTRY", &docs, Sort::Recent), [3, 2]);
        assert_eq!(ids("only|entry", &docs, Sort::Recent), [3, 2, 1]);
    }

    #[test]
    fn phrases_whole_words_starts_and_regexes() {
        let docs = [doc(1, "the quick  brown fox", "text", 0), doc(2, "brown quick", "text", 0), doc(3, "quickly", "text", 0)];
        assert_eq!(ids("\"quick brown\"", &docs, Sort::Recent), [1]);
        assert_eq!(ids("'quick", &docs, Sort::Recent), [2, 1]);
        assert_eq!(ids("^quick", &docs, Sort::Recent), [3]);
        assert_eq!(ids("/qu[a-z]+ly/", &docs, Sort::Recent), [3]);
        assert_eq!(ids("/QUICK/i !brown", &docs, Sort::Recent), [3]);
        assert_eq!(ids("/quick\\s+brown/ fox", &docs, Sort::Recent), [1]);
        assert_eq!(ids("/quick brown/", &docs, Sort::Recent), Vec::<i64>::new());
    }

    #[test]
    fn filters_narrow_by_kind_age_site_and_size() {
        let mut docs = vec![doc(1, "fn main() {}", "code", 0), doc(2, "old note", "text", 40), doc(3, "https://github.com/x", "link", 2)];
        docs[2].site = host("https://github.com/x");
        let big = "word ".repeat(4000);
        docs.push(doc(4, &big, "text", 1));
        assert_eq!(ids("@code", &docs, Sort::Recent), [1]);
        assert_eq!(ids("@week", &docs, Sort::Recent), [1, 4, 3]);
        assert_eq!(ids("@before:30d", &docs, Sort::Recent), [2]);
        assert_eq!(ids("@since:2020-01-01 @text", &docs, Sort::Recent), [4, 2]);
        assert_eq!(ids("@site:github.com", &docs, Sort::Recent), [3]);
        assert_eq!(ids("@size>2k", &docs, Sort::Recent), [4]);
        assert_eq!(ids("@size<10", &docs, Sort::Recent), [1, 3, 2]);
        assert!(parse("@since:soon", NOW).is_err());
    }

    #[test]
    fn rare_words_and_early_matches_rank_first() {
        let filler = "lorem ipsum ".repeat(200);
        let late = format!("{filler} invoice");
        let docs = [
            doc(1, "invoice total", "text", 3),
            doc(2, &late, "text", 0),
            doc(3, "total total total", "text", 0),
            doc(4, "the invoice total due", "text", 30),
        ];
        let r = ids("invoice total", &docs, Sort::Relevance);
        assert_eq!(r[0], 1, "{r:?}");
        assert!(!r.contains(&2) && !r.contains(&3));
        // Without words, relevance falls back to recent.
        assert_eq!(ids("@text", &docs, Sort::Relevance), [3, 2, 1, 4]);
        assert_eq!(ids("@text", &docs, Sort::Largest)[0], 2);
        assert_eq!(ids("@text", &docs, Sort::Oldest)[0], 4);
    }

    #[test]
    fn matches_inside_secrets_dont_count() {
        let docs = [doc(1, "DB_PASSWORD=hunter22\nuser=bob", "text", 0), doc(2, "hunter22 is a film", "text", 0)];
        assert_eq!(ids("hunter22", &docs, Sort::Recent), [2]);
        assert_eq!(ids("DB_PASSWORD", &docs, Sort::Recent), [1]);
        assert_eq!(ids("/PASSWORD=h/", &docs, Sort::Recent), Vec::<i64>::new());
        assert_eq!(ids("user !hunter22", &docs, Sort::Recent), [1]);
    }

    #[test]
    fn hosts() {
        assert_eq!(host("https://www.github.com/a?b"), Some("github.com"));
        assert_eq!(host("postgres://u:p@db.internal:5432/x"), Some("db.internal"));
        assert_eq!(host("not a url"), None);
        assert_eq!(when("2026-11-01", 0), Some(days_from_civil(2026, 11, 1) * DAY));
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(days_from_civil(2000, 3, 1), 11_017);
    }
}
