// Short descriptions of large clips, so an AI tool can see what a clip holds
// and read only the part it needs: the shape of JSON, a table's columns, a
// document's headings, a file's functions, a log's errors. Built from fixed
// rules, and every line shown from the clip goes through `secrets::redact`.

use crate::secrets;
use regex::Regex;
use serde_json::{Map, Value};
use std::sync::LazyLock;

const LINE_CHARS: usize = 160;
const MAX_ITEMS: usize = 60;

/// Byte offset where each line starts.
pub fn line_starts(text: &str) -> Vec<usize> {
    std::iter::once(0).chain(text.match_indices('\n').map(|(i, _)| i + 1)).filter(|&i| i < text.len() || i == 0).collect()
}

/// Line `i` (0-based) as a byte range, without its line break.
pub fn line(text: &str, starts: &[usize], i: usize) -> (usize, usize) {
    let a = starts[i];
    let mut b = starts.get(i + 1).map_or(text.len(), |&n| n - 1);
    if b > a && text.as_bytes()[b - 1] == b'\r' {
        b -= 1;
    }
    (a, b.max(a))
}

/// Cuts s to `max` characters, ending in … when it had more.
pub fn cut(s: &str, max: usize) -> String {
    match s.char_indices().nth(max) {
        Some((i, _)) => format!("{}…", s[..i].trim_end()),
        None => s.to_string(),
    }
}

/// s with runs of whitespace as one space.
pub fn squash(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn shown(text: &str, starts: &[usize], i: usize) -> String {
    let (a, b) = line(text, starts, i);
    cut(secrets::redact(text, a, b).trim_end(), LINE_CHARS)
}

pub fn parse_json(text: &str) -> Option<Value> {
    let t = text.trim_start();
    if !(t.starts_with('{') || t.starts_with('[')) {
        return None;
    }
    serde_json::from_str(t).ok()
}

fn scalar(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(_) => "num",
        Value::String(_) => "str",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn key(k: &str) -> String {
    if !k.is_empty() && k.chars().all(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '$')) {
        k.to_string()
    } else {
        format!("{k:?}")
    }
}

/// The union of the keys of the objects in a list, in the order they first
/// appear, each with the first value that isn't null.
fn merged<'a>(items: impl Iterator<Item = &'a Map<String, Value>>) -> Vec<(&'a str, &'a Value)> {
    let mut out: Vec<(&str, &Value)> = Vec::new();
    for o in items.take(50) {
        for (k, v) in o {
            match out.iter_mut().find(|(ok, _)| ok == k) {
                Some(slot) if slot.1.is_null() => slot.1 = v,
                Some(_) => {}
                None => out.push((k, v)),
            }
        }
    }
    out
}

/// The shape of a JSON value: `{users: [1200× {id: num, email: str}], next: str}`.
pub fn shape(v: &Value, depth: usize) -> String {
    let s = match v {
        Value::Array(a) if a.is_empty() => "[]".into(),
        Value::Array(a) => format!("[{}× {}]", a.len(), items(a, depth)),
        Value::Object(o) if o.is_empty() => "{}".into(),
        Value::Object(o) if depth >= 3 => format!("{{…{} keys}}", o.len()),
        Value::Object(o) => fields(o.iter().map(|(k, v)| (k.as_str(), v)).collect(), o.len(), depth),
        _ => scalar(v).into(),
    };
    cut(&s, 400)
}

fn items(a: &[Value], depth: usize) -> String {
    let objs: Vec<&Map<String, Value>> = a.iter().take(50).filter_map(Value::as_object).collect();
    if !objs.is_empty() && objs.len() == a.len().min(50) {
        let m = merged(objs.into_iter());
        let n = m.len();
        return fields(m, n, depth + 1);
    }
    let mut kinds: Vec<String> = Vec::new();
    for v in a.iter().take(50) {
        let s = shape(v, depth + 1);
        if !kinds.contains(&s) {
            kinds.push(s);
        }
    }
    let more = kinds.len() > 3;
    kinds.truncate(3);
    kinds.join("|") + if more { "|…" } else { "" }
}

fn fields(pairs: Vec<(&str, &Value)>, total: usize, depth: usize) -> String {
    let mut parts: Vec<String> = pairs.iter().take(12).map(|(k, v)| format!("{}: {}", key(k), shape(v, depth + 1))).collect();
    if total > 12 {
        parts.push(format!("…+{}", total - 12));
    }
    format!("{{{}}}", parts.join(", "))
}

/// A one-line shape for a preview, keys only below the top level.
pub fn brief(v: &Value) -> String {
    fn inner(v: &Value) -> String {
        match v {
            Value::Object(o) => {
                let keys: Vec<String> = o.keys().take(6).map(|k| key(k)).collect();
                format!("{{{}{}}}", keys.join(", "), if o.len() > 6 { ", …" } else { "" })
            }
            Value::Array(a) if a.is_empty() => "[]".into(),
            Value::Array(a) => format!("[{}× {}]", a.len(), a.first().map_or(String::new(), inner)),
            _ => scalar(v).into(),
        }
    }
    let s = match v {
        Value::Object(o) => {
            let parts: Vec<String> = o.iter().take(8).map(|(k, v)| format!("{}: {}", key(k), inner(v))).collect();
            format!("{{{}{}}}", parts.join(", "), if o.len() > 8 { ", …" } else { "" })
        }
        _ => inner(v),
    };
    cut(&s, 120)
}

fn json_outline(v: &Value) -> String {
    match v {
        Value::Object(o) => {
            let mut out = vec![format!("json object, {} keys:", o.len())];
            for (k, v) in o.iter().take(MAX_ITEMS) {
                out.push(format!("  {}: {}", key(k), shape(v, 1)));
            }
            if o.len() > MAX_ITEMS {
                out.push(format!("  …+{} keys", o.len() - MAX_ITEMS));
            }
            out.join("\n")
        }
        Value::Array(_) => format!("json array: {}", shape(v, 0)),
        _ => format!("json {}", scalar(v)),
    }
}

fn json_lines(text: &str, starts: &[usize]) -> Option<String> {
    if starts.len() < 2 {
        return None;
    }
    let mut objs = Vec::new();
    for i in 0..starts.len() {
        let (a, b) = line(text, starts, i);
        let l = text[a..b].trim();
        if l.is_empty() {
            continue;
        }
        match serde_json::from_str::<Value>(l) {
            Ok(Value::Object(o)) => {
                if objs.len() < 50 {
                    objs.push(o);
                }
            }
            _ => return None,
        }
    }
    let n = merged(objs.iter()).len();
    Some(format!("json lines, {} records, each {}", starts.len(), fields(merged(objs.iter()), n, 1)))
}

// Delimiters outside quotes in a line.
fn delims(l: &str, d: char) -> usize {
    let mut quoted = false;
    l.chars()
        .filter(|&c| {
            if c == '"' {
                quoted = !quoted;
            }
            c == d && !quoted
        })
        .count()
}

fn table(text: &str, starts: &[usize]) -> Option<String> {
    let lines: Vec<usize> = (0..starts.len()).filter(|&i| line(text, starts, i).1 > line(text, starts, i).0).collect();
    if lines.len() < 3 {
        return None;
    }
    let head: Vec<&str> = lines.iter().take(6).map(|&i| {
        let (a, b) = line(text, starts, i);
        &text[a..b]
    })
    .collect();
    let (d, n) = ['\t', ',', ';', '|']
        .iter()
        .map(|&d| (d, delims(head[0], d)))
        .filter(|&(d, n)| n >= 1 && head.iter().all(|l| delims(l, d) == n))
        .max_by_key(|&(_, n)| n)?;
    let name = match d {
        '\t' => "tab",
        ',' => "comma",
        ';' => "semicolon",
        _ => "pipe",
    };
    let mut out = vec![format!("table, {} rows × {} columns ({name}-separated)", lines.len() - 1, n + 1)];
    out.push(format!("  header: {}", shown(text, starts, lines[0])));
    for &i in lines.iter().skip(1).take(3) {
        out.push(format!("  L{} {}", i + 1, shown(text, starts, i)));
    }
    Some(out.join("\n"))
}

fn headings(text: &str, starts: &[usize]) -> Option<String> {
    let mut fenced = false;
    let mut found = Vec::new();
    for i in 0..starts.len() {
        let (a, b) = line(text, starts, i);
        let l = &text[a..b];
        if l.trim_start().starts_with("```") {
            fenced = !fenced;
            continue;
        }
        let hashes = l.len() - l.trim_start_matches('#').len();
        if !fenced && (1..=6).contains(&hashes) && l[hashes..].starts_with(' ') {
            found.push(i);
        }
    }
    if found.len() < 2 {
        return None;
    }
    let mut out = vec![format!("markdown, {} lines, {} headings:", starts.len(), found.len())];
    out.extend(found.iter().take(MAX_ITEMS).map(|&i| format!("  L{} {}", i + 1, shown(text, starts, i))));
    if found.len() > MAX_ITEMS {
        out.push(format!("  …+{} headings", found.len() - MAX_ITEMS));
    }
    Some(out.join("\n"))
}

static SYMBOL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^\s*(?:(?:export|default|pub(?:\([^)]*\))?|public|private|protected|internal|static|async|abstract|final|override|unsafe|extern|inline|virtual|open|sealed|data)\s+)*(?:def|fn|func|function|class|struct|enum|trait|impl|interface|type|module|mod|namespace|object|record|protocol|extension|macro_rules!|CREATE\s+(?:TABLE|VIEW|INDEX|FUNCTION))\b|^\s*(?:export\s+)?(?:const|let|var)\s+[\w$]+\s*=\s*(?:async\s*)?(?:\([^)]*\)\s*=>|function\b|[\w$]+\s*=>)",
    )
    .unwrap()
});

fn symbols(text: &str, starts: &[usize], must: bool) -> Option<String> {
    let found: Vec<usize> = (0..starts.len())
        .filter(|&i| {
            let (a, b) = line(text, starts, i);
            b - a < 400 && SYMBOL.is_match(&text[a..b])
        })
        .collect();
    if found.len() < if must { 1 } else { 3 } {
        return None;
    }
    let mut out = vec![format!("code, {} lines, {} definitions:", starts.len(), found.len())];
    out.extend(found.iter().take(MAX_ITEMS).map(|&i| format!("  L{} {}", i + 1, shown(text, starts, i).trim_start())));
    if found.len() > MAX_ITEMS {
        out.push(format!("  …+{} more", found.len() - MAX_ITEMS));
    }
    Some(out.join("\n"))
}

static PROBLEM: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:error|err|fatal|panic(?:ked)?|exception|traceback|failed|failure|critical|warn(?:ing)?)\b").unwrap()
});
static WARNING: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\bwarn(?:ing)?\b").unwrap());
static LOG_LINE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)^\s*(?:\[?\d{1,4}[-:/.T ]\d|\[?(?:trace|debug|info|warn|warning|error|fatal|critical)\b|\s+at\s|\s+File "|thread '|E\d{4} |[IWEF]\d{4} )"#).unwrap()
});
static NOISE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"0x[0-9a-fA-F]+|\d+").unwrap());

fn log(text: &str, starts: &[usize]) -> Option<String> {
    if starts.len() < 10 {
        return None;
    }
    let sample: Vec<usize> = (0..starts.len()).filter(|&i| line(text, starts, i).1 > line(text, starts, i).0).take(40).collect();
    let loggy = sample
        .iter()
        .filter(|&&i| {
            let (a, b) = line(text, starts, i);
            LOG_LINE.is_match(&text[a..b])
        })
        .count();
    if loggy * 10 < sample.len() * 3 {
        return None;
    }
    // The same message with other numbers in it counts once.
    let mut problems: Vec<(String, usize, usize)> = Vec::new();
    let (mut errors, mut warnings) = (0, 0);
    for i in 0..starts.len() {
        let (a, b) = line(text, starts, i);
        let l = &text[a..b];
        if b - a > 2000 || !PROBLEM.is_match(l) {
            continue;
        }
        if WARNING.is_match(l) {
            warnings += 1;
        } else {
            errors += 1;
        }
        let norm = NOISE.replace_all(l, "#").into_owned();
        match problems.iter_mut().find(|p| p.0 == norm) {
            Some(p) => p.2 += 1,
            None => problems.push((norm, i, 1)),
        }
    }
    let mut out = vec![format!("log, {} lines, {errors} errors, {warnings} warnings", starts.len())];
    if !problems.is_empty() {
        out.push("problems (first time each):".into());
        for (_, i, n) in problems.iter().take(20) {
            let times = if *n > 1 { format!(" ×{n}") } else { String::new() };
            out.push(format!("  L{}{times} {}", i + 1, shown(text, starts, *i).trim_start()));
        }
        if problems.len() > 20 {
            out.push(format!("  …+{} other messages", problems.len() - 20));
        }
    }
    out.push("tail:".into());
    for i in starts.len().saturating_sub(5)..starts.len() {
        out.push(format!("  L{} {}", i + 1, shown(text, starts, i)));
    }
    Some(out.join("\n"))
}

fn prose(text: &str, starts: &[usize]) -> String {
    let words = text.split_whitespace().count();
    let mut out = vec![format!("text, {} lines, {words} words, starts:", starts.len())];
    let mut shown_n = 0;
    for i in 0..starts.len() {
        let (a, b) = line(text, starts, i);
        if text[a..b].trim().is_empty() {
            continue;
        }
        out.push(format!("  L{} {}", i + 1, shown(text, starts, i)));
        shown_n += 1;
        if shown_n == 12 {
            break;
        }
    }
    out.join("\n")
}

/// What a clip holds, in a few dozen lines at most.
pub fn outline(text: &str, kind: &str) -> String {
    let starts = line_starts(text);
    if kind == "file" {
        let mut out = vec![format!("{} files:", starts.len())];
        out.extend((0..starts.len().min(MAX_ITEMS)).map(|i| format!("  {}", shown(text, &starts, i))));
        return out.join("\n");
    }
    if let Some(v) = parse_json(text) {
        return json_outline(&v);
    }
    json_lines(text, &starts)
        .or_else(|| table(text, &starts))
        .or_else(|| log(text, &starts))
        .or_else(|| headings(text, &starts))
        .or_else(|| symbols(text, &starts, kind == "code"))
        .unwrap_or_else(|| prose(text, &starts))
}

/// The value at a JSON path such as `$.users[3].email` or `$["a b"][0]`.
pub fn json_at<'v>(v: &'v Value, path: &str) -> Result<&'v Value, String> {
    let mut rest = path.trim().strip_prefix('$').unwrap_or(path.trim());
    let mut cur = v;
    let mut walked = String::from("$");
    while !rest.is_empty() {
        let step;
        if let Some(r) = rest.strip_prefix('.') {
            let end = r.find(['.', '[']).unwrap_or(r.len());
            step = r[..end].to_string();
            rest = &r[end..];
            cur = cur.get(&step).ok_or_else(|| missing(cur, &walked, &step))?;
            walked.push('.');
            walked.push_str(&step);
        } else if let Some(r) = rest.strip_prefix('[') {
            let end = r.find(']').ok_or("unclosed [ in path")?;
            let inner = r[..end].trim();
            rest = &r[end + 1..];
            if let Some(name) = inner.strip_prefix('"').and_then(|s| s.strip_suffix('"')) {
                cur = cur.get(name).ok_or_else(|| missing(cur, &walked, name))?;
            } else {
                let idx: i64 = inner.parse().map_err(|_| format!("bad index [{inner}]"))?;
                let len = cur.as_array().map_or(0, Vec::len) as i64;
                let i = if idx < 0 { len + idx } else { idx };
                cur = cur.get(i.max(0) as usize).ok_or_else(|| format!("{walked} has {len} items, no [{idx}]"))?;
            }
            walked.push_str(&format!("[{inner}]"));
        } else {
            return Err(format!("can't read the path at {rest:?}; use $.key[0].key"));
        }
    }
    Ok(cur)
}

fn missing(v: &Value, at: &str, key: &str) -> String {
    match v.as_object() {
        Some(o) => {
            let keys: Vec<&str> = o.keys().take(20).map(String::as_str).collect();
            format!("{at} has no key {key:?}; it has: {}", keys.join(", "))
        }
        None => format!("{at} is {}, not an object", scalar(v)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_is_described_by_its_shape() {
        let users: Vec<String> = (0..1200).map(|i| format!(r#"{{"id":{i},"email":"u{i}@x.io","plan":null}}"#)).collect();
        let text = format!(r#"{{"users":[{}],"next_cursor":"abc","meta":{{"page":1}}}}"#, users.join(","));
        let o = outline(&text, "text");
        assert!(o.starts_with("json object, 3 keys:"), "{o}");
        assert!(o.contains("users: [1200× {id: num, email: str, plan: null}]"), "{o}");
        assert!(o.contains("meta: {page: num}"), "{o}");
        assert_eq!(brief(&parse_json(&text).unwrap()), "{users: [1200× {id, email, plan}], next_cursor: str, meta: {page}}");
        let v = parse_json(&text).unwrap();
        assert_eq!(json_at(&v, "$.users[3].email").unwrap().as_str(), Some("u3@x.io"));
        assert_eq!(json_at(&v, "$.users[-1].id").unwrap().as_i64(), Some(1199));
        assert!(json_at(&v, "$.nope").unwrap_err().contains("users, next_cursor, meta"));
    }

    #[test]
    fn tables_logs_headings_and_code() {
        let csv = "name,age,city\nann,31,Berlin\nbob,42,Paris\ncid,25,Rome\n";
        let o = outline(csv, "text");
        assert!(o.starts_with("table, 3 rows × 3 columns (comma-separated)"), "{o}");
        assert!(o.contains("header: name,age,city"));

        let mut log_text = String::new();
        for i in 0..50 {
            log_text.push_str(&format!("2026-11-08 10:{:02}:00 INFO request {i} ok\n", i));
        }
        log_text.push_str("2026-11-08 11:00:00 ERROR db: connection reset after 31 ms\n");
        log_text.push_str("2026-11-08 11:00:01 ERROR db: connection reset after 45 ms\n");
        log_text.push_str("2026-11-08 11:00:02 WARN slow query\n");
        let o = outline(&log_text, "text");
        assert!(o.starts_with("log, 53 lines, 2 errors, 1 warnings"), "{o}");
        assert!(o.contains("L51 ×2 2026-11-08 11:00:00 ERROR db: connection reset"), "{o}");

        let md = "# Title\nintro\n## Install\n```\n# not a heading\n```\n## Use\n";
        let o = outline(md, "text");
        assert!(o.contains("L1 # Title") && o.contains("L3 ## Install") && o.contains("L7 ## Use") && !o.contains("not a heading"), "{o}");

        let code = "use x;\n\npub fn parse(raw: &str) -> Entry {\n    todo!()\n}\n\nstruct Entry;\nimpl Entry {}\n";
        let o = outline(code, "code");
        assert!(o.contains("L3 pub fn parse(raw: &str) -> Entry {") && o.contains("L7 struct Entry;"), "{o}");
    }

    #[test]
    fn outlines_hide_secrets() {
        let text = "intro\nDB_PASSWORD=hunter22\nmore\n";
        let o = outline(text, "text");
        assert!(o.contains("DB_PASSWORD=‹Password›") && !o.contains("hunter22"), "{o}");
    }

    #[test]
    fn lines_split_cleanly() {
        let t = "a\r\nbb\n\nc";
        let s = line_starts(t);
        assert_eq!(s, [0, 3, 6, 7]);
        assert_eq!((0..4).map(|i| line(t, &s, i)).map(|(a, b)| &t[a..b]).collect::<Vec<_>>(), ["a", "bb", "", "c"]);
        assert_eq!(line_starts("x\n"), [0]);
        assert_eq!(cut("abcdef", 3), "abc…");
    }
}
