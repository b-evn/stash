// Other forms a clip can be pasted in ("Paste as…"): the text in a screenshot,
// a link without its tracking, reformatted JSON, tidied lines, another case,
// decoded text. Only the forms that change the clip are offered.

use base64::{
    alphabet,
    engine::{general_purpose::GeneralPurpose, DecodePaddingMode, GeneralPurposeConfig},
    Engine as _,
};
use serde::Serialize;

/// Larger clips are only pasted as they are.
const MAX_CHARS: usize = 200_000;
/// How much of the result a choice shows.
const HINT_CHARS: usize = 40;

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Choice {
    /// What `apply` is asked for.
    pub how: &'static str,
    pub label: &'static str,
    /// A glimpse of the result.
    pub hint: String,
    /// Choices of one group are shown together.
    pub group: u8,
}

struct Form {
    how: &'static str,
    label: &'static str,
    group: u8,
    kinds: &'static [&'static str],
    make: fn(&str) -> Option<String>,
}

const PROSE: &[&str] = &["text"];
const WRITTEN: &[&str] = &["text", "code"];

const FORMS: &[Form] = &[
    Form { how: "clean_url", label: "Without tracking", group: 1, kinds: &["link"], make: clean_url },
    Form { how: "url_decode", label: "URL-decoded", group: 1, kinds: &["link", "text", "code", "path"], make: url_decode },
    Form { how: "json_pretty", label: "Pretty JSON", group: 2, kinds: WRITTEN, make: |s| json(s, true) },
    Form { how: "json_compact", label: "Compact JSON", group: 2, kinds: WRITTEN, make: |s| json(s, false) },
    Form { how: "tidy", label: "Trimmed", group: 3, kinds: WRITTEN, make: tidy },
    Form { how: "join", label: "One line", group: 3, kinds: WRITTEN, make: join },
    Form { how: "sort", label: "Sorted lines", group: 3, kinds: WRITTEN, make: sort_lines },
    Form { how: "dedupe", label: "Without repeats", group: 3, kinds: WRITTEN, make: dedupe },
    Form { how: "upper", label: "UPPERCASE", group: 4, kinds: PROSE, make: |s| Some(s.to_uppercase()) },
    Form { how: "lower", label: "lowercase", group: 4, kinds: &["text", "email"], make: |s| Some(s.to_lowercase()) },
    Form { how: "title", label: "Title Case", group: 4, kinds: PROSE, make: title_case },
    Form { how: "camel", label: "camelCase", group: 5, kinds: WRITTEN, make: |s| ident(s, Ident::Camel) },
    Form { how: "snake", label: "snake_case", group: 5, kinds: WRITTEN, make: |s| ident(s, Ident::Snake) },
    Form { how: "kebab", label: "kebab-case", group: 5, kinds: WRITTEN, make: |s| ident(s, Ident::Kebab) },
    Form { how: "base64", label: "Base64-decoded", group: 6, kinds: WRITTEN, make: base64_text },
];

/// The forms this clip can be pasted in, besides as it is.
pub fn choices(kind: &str, text: &str, ocr: &str) -> Vec<Choice> {
    if kind == "image" {
        let ocr = ocr.trim();
        return if ocr.is_empty() { Vec::new() } else { vec![Choice { how: "ocr", label: "Text in image", hint: glimpse(ocr), group: 0 }] };
    }
    if text.len() > MAX_CHARS {
        return Vec::new();
    }
    FORMS
        .iter()
        .filter(|f| f.kinds.contains(&kind))
        .filter_map(|f| {
            let out = (f.make)(text).filter(|o| o != text && !o.trim().is_empty())?;
            Some(Choice { how: f.how, label: f.label, hint: hint(f.how, text, &out), group: f.group })
        })
        .collect()
}

/// The clip in the form `how` names, if it can take that form.
pub fn apply(how: &str, kind: &str, text: &str, ocr: &str) -> Option<String> {
    if how == "ocr" {
        return (kind == "image" && !ocr.trim().is_empty()).then(|| ocr.trim().to_string());
    }
    let form = FORMS.iter().find(|f| f.how == how && f.kinds.contains(&kind))?;
    (text.len() <= MAX_CHARS).then(|| (form.make)(text)).flatten()
}

fn hint(how: &str, before: &str, after: &str) -> String {
    let count = |s: &str| s.lines().filter(|l| !l.trim().is_empty()).count();
    match how {
        "json_pretty" => format!("{} lines", after.lines().count()),
        "dedupe" => {
            let n = count(before) - count(after);
            format!("{n} line{} fewer", if n == 1 { "" } else { "s" })
        }
        _ => glimpse(after),
    }
}

fn glimpse(text: &str) -> String {
    let line = text.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("");
    match line.char_indices().nth(HINT_CHARS) {
        Some((i, _)) => format!("{}…", &line[..i]),
        None => line.to_string(),
    }
}

fn ending(text: &str) -> &'static str {
    if text.contains("\r\n") { "\r\n" } else { "\n" }
}

fn lines(text: &str) -> impl Iterator<Item = &str> {
    text.split('\n').map(|l| l.trim_end_matches('\r'))
}

/// Without trailing spaces, the blank lines around it, runs of blank lines or
/// the indent every line shares.
fn tidy(text: &str) -> Option<String> {
    let all: Vec<&str> = lines(text).map(str::trim_end).collect();
    let first = all.iter().position(|l| !l.is_empty())?;
    let last = all.iter().rposition(|l| !l.is_empty())?;
    let body = &all[first..=last];
    let lead = |l: &str| l.len() - l.trim_start().len();
    let mut indent: &str = &body[0][..lead(body[0])];
    for l in body.iter().filter(|l| !l.is_empty()) {
        let shared = indent.bytes().zip(l.bytes()).take_while(|(a, b)| a == b).count();
        indent = &indent[..shared.min(lead(l))];
    }
    let mut out: Vec<&str> = Vec::with_capacity(body.len());
    for l in body {
        if l.is_empty() && out.last().is_some_and(|p| p.is_empty()) {
            continue;
        }
        out.push(l.strip_prefix(indent).unwrap_or(l));
    }
    Some(out.join(ending(text)))
}

fn join(text: &str) -> Option<String> {
    (lines(text).filter(|l| !l.trim().is_empty()).count() >= 2).then(|| text.split_whitespace().collect::<Vec<_>>().join(" "))
}

fn sort_lines(text: &str) -> Option<String> {
    let mut all: Vec<&str> = lines(text).map(str::trim_end).filter(|l| !l.is_empty()).collect();
    if all.len() < 2 {
        return None;
    }
    all.sort_by_cached_key(|l| (l.trim_start().to_lowercase(), l.to_string()));
    Some(all.join(ending(text)))
}

/// Each line once, where it first appears; blank lines stay.
fn dedupe(text: &str) -> Option<String> {
    let mut seen = std::collections::HashSet::new();
    let kept: Vec<&str> = lines(text).filter(|l| l.trim().is_empty() || seen.insert(l.trim())).collect();
    Some(kept.join(ending(text)))
}

/// Every word capitalised, the rest of it in lower case.
fn title_case(text: &str) -> Option<String> {
    if !text.trim().contains(char::is_whitespace) {
        return None;
    }
    let mut out = String::with_capacity(text.len());
    let mut start = true;
    for c in text.chars() {
        if c.is_alphanumeric() {
            if start {
                out.extend(c.to_uppercase());
            } else {
                out.extend(c.to_lowercase());
            }
            start = false;
        } else {
            out.push(c);
            // A word starts after a space or an opening bracket or quote; an apostrophe stays inside it.
            let opens = c.is_whitespace() || "([{\"“‘/".contains(c);
            start = opens || (start && !matches!(c, '\'' | '’'));
        }
    }
    Some(out)
}

#[derive(Clone, Copy)]
enum Ident {
    Camel,
    Snake,
    Kebab,
}

/// An identifier spelled another way: `fooBar`, `foo_bar`, `foo-bar`, `HTTPServer`.
fn ident(text: &str, to: Ident) -> Option<String> {
    let t = text.trim();
    if t.is_empty() || t.chars().count() > 64 || !t.chars().all(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '.')) {
        return None;
    }
    let chars: Vec<char> = t.chars().collect();
    let mut words: Vec<String> = Vec::new();
    let mut cur = String::new();
    for (i, &c) in chars.iter().enumerate() {
        if !c.is_alphanumeric() {
            if !cur.is_empty() {
                words.push(std::mem::take(&mut cur));
            }
            continue;
        }
        if let Some(&p) = i.checked_sub(1).and_then(|j| chars.get(j)).filter(|p| p.is_alphanumeric()) {
            let next_lower = chars.get(i + 1).is_some_and(|n| n.is_lowercase());
            let hump = (p.is_lowercase() || p.is_numeric()) && c.is_uppercase();
            let acronym_end = p.is_uppercase() && c.is_uppercase() && next_lower;
            if hump || acronym_end {
                words.push(std::mem::take(&mut cur));
            }
        }
        cur.push(c);
    }
    if !cur.is_empty() {
        words.push(cur);
    }
    if words.len() < 2 {
        return None;
    }
    let lower: Vec<String> = words.iter().map(|w| w.to_lowercase()).collect();
    Some(match to {
        Ident::Snake => lower.join("_"),
        Ident::Kebab => lower.join("-"),
        Ident::Camel => {
            let mut out = lower[0].clone();
            for w in &lower[1..] {
                let mut c = w.chars();
                if let Some(f) = c.next() {
                    out.extend(f.to_uppercase());
                    out.push_str(c.as_str());
                }
            }
            out
        }
    })
}

/// Query parameters that only say where a click came from.
const TRACKING: &[&str] = &[
    "fbclid", "gclid", "gclsrc", "dclid", "gbraid", "wbraid", "msclkid", "yclid", "twclid", "ttclid", "li_fat_id", "mc_cid", "mc_eid",
    "igshid", "igsh", "_hsenc", "_hsmi", "__hssc", "__hstc", "__hsfp", "hsctatracking", "mkt_tok", "vero_id", "vero_conv",
    "oly_anon_id", "oly_enc_id", "rb_clickid", "s_cid", "_ga", "_gl", "ref_src", "ref_url", "wt_mc", "wt.mc_id", "pk_campaign",
    "pk_kwd", "pk_source", "pk_medium", "pk_content", "mtm_campaign", "mtm_source", "mtm_medium", "mtm_content", "mtm_keyword",
];
/// Hosts whose share links add their own: `si` (YouTube, Spotify) and `feature` (YouTube).
const SHARE_HOSTS: &[&str] = &["youtube.com", "youtu.be", "spotify.com"];

fn clean_url(text: &str) -> Option<String> {
    let t = text.trim();
    if t.contains(char::is_whitespace) {
        return None;
    }
    let host = crate::query::host(t)?.to_ascii_lowercase();
    let shared = SHARE_HOSTS.iter().any(|h| host == *h || host.ends_with(&format!(".{h}")));
    let (main, fragment) = t.split_once('#').map_or((t, None), |(m, f)| (m, Some(f)));
    let (path, query) = main.split_once('?')?;
    let kept: Vec<&str> = query
        .split('&')
        .filter(|p| {
            let key = p.split('=').next().unwrap_or("").to_ascii_lowercase();
            !p.is_empty()
                && !key.starts_with("utm_")
                && !TRACKING.contains(&key.as_str())
                && !(shared && matches!(key.as_str(), "si" | "feature"))
        })
        .collect();
    let mut out = path.to_string();
    if !kept.is_empty() {
        out.push('?');
        out.push_str(&kept.join("&"));
    }
    if let Some(f) = fragment {
        out.push('#');
        out.push_str(f);
    }
    Some(out)
}

fn url_decode(text: &str) -> Option<String> {
    let b = text.as_bytes();
    let hex = |c: u8| (c as char).to_digit(16).map(|d| d as u8);
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match (b[i], b.get(i + 1).copied().and_then(hex), b.get(i + 2).copied().and_then(hex)) {
            (b'%', Some(h), Some(l)) => {
                out.push(h << 4 | l);
                i += 3;
            }
            (c, ..) => {
                out.push(c);
                i += 1;
            }
        }
    }
    let s = String::from_utf8(out).ok()?;
    (!s.chars().any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))).then_some(s)
}

/// Base64 that holds readable text, such as a config value or a JWT's middle.
fn base64_text(text: &str) -> Option<String> {
    let t: String = text.split_whitespace().collect();
    if t.len() < 8 || !t.bytes().all(|b| b.is_ascii_alphanumeric() || b"+/=-_".contains(&b)) {
        return None;
    }
    let lenient = GeneralPurposeConfig::new().with_decode_padding_mode(DecodePaddingMode::Indifferent);
    let bytes = [alphabet::STANDARD, alphabet::URL_SAFE].iter().find_map(|a| GeneralPurpose::new(a, lenient).decode(&t).ok())?;
    let s = String::from_utf8(bytes).ok()?;
    let readable = s.chars().any(char::is_alphabetic) && !s.chars().any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'));
    readable.then_some(s)
}

/// JSON laid out again without touching a value: numbers and strings keep
/// their exact spelling, keys their order.
fn json(text: &str, pretty: bool) -> Option<String> {
    let t = text.trim();
    if !t.starts_with(['{', '[']) || serde_json::from_str::<serde::de::IgnoredAny>(t).is_err() {
        return None;
    }
    let mut out = String::with_capacity(t.len() * if pretty { 2 } else { 1 });
    let mut depth = 0usize;
    let newline = |out: &mut String, depth: usize| {
        out.push('\n');
        out.extend(std::iter::repeat_n("  ", depth));
    };
    let mut chars = t.chars().peekable();
    let (mut in_str, mut escaped) = (false, false);
    while let Some(c) = chars.next() {
        if in_str {
            out.push(c);
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_str = false;
            }
            continue;
        }
        match c {
            '"' => {
                in_str = true;
                out.push(c);
            }
            '{' | '[' => {
                out.push(c);
                while chars.peek().is_some_and(|n| n.is_whitespace()) {
                    chars.next();
                }
                if chars.peek().is_some_and(|&n| n == if c == '{' { '}' } else { ']' }) {
                    out.push(chars.next().unwrap_or(c));
                } else {
                    depth += 1;
                    if pretty {
                        newline(&mut out, depth);
                    }
                }
            }
            '}' | ']' => {
                depth = depth.saturating_sub(1);
                if pretty {
                    newline(&mut out, depth);
                }
                out.push(c);
            }
            ',' => {
                out.push(c);
                if pretty {
                    newline(&mut out, depth);
                }
            }
            ':' => out.push_str(if pretty { ": " } else { ":" }),
            c if c.is_whitespace() => {}
            c => out.push(c),
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hows(kind: &str, text: &str) -> Vec<&'static str> {
        choices(kind, text, "").into_iter().map(|c| c.how).collect()
    }

    fn get(how: &str, kind: &str, text: &str) -> String {
        apply(how, kind, text, "").unwrap_or_else(|| panic!("{how} of {text:?}"))
    }

    #[test]
    fn only_forms_that_change_the_clip_are_offered() {
        assert_eq!(hows("text", "hello world"), ["upper", "title"]);
        assert_eq!(hows("text", "HELLO"), ["lower"]);
        assert!(hows("text", "fooBar").contains(&"snake"));
        assert!(!hows("text", "foo_bar").contains(&"snake"));
        assert!(hows("image", "").is_empty());
        assert_eq!(choices("image", "", "  Total: 42 €\nThanks "), [Choice { how: "ocr", label: "Text in image", hint: "Total: 42 €".into(), group: 0 }]);
        assert_eq!(apply("ocr", "image", "", " Total \n"), Some("Total".into()));
        assert_eq!(apply("upper", "code", "x", ""), None, "case is for prose");
        assert!(hows("text", &"x".repeat(MAX_CHARS + 1)).is_empty());
    }

    #[test]
    fn links_lose_their_tracking() {
        let url = "https://shop.example/p/42?utm_source=mail&id=7&fbclid=abc&UTM_Medium=x#reviews";
        assert_eq!(get("clean_url", "link", url), "https://shop.example/p/42?id=7#reviews");
        assert_eq!(get("clean_url", "link", "https://youtu.be/dQw4?si=Xy12&t=42"), "https://youtu.be/dQw4?t=42");
        assert_eq!(get("clean_url", "link", "https://example.com/?utm_campaign=a"), "https://example.com/");
        assert_eq!(get("clean_url", "link", "https://example.com/search?si=1"), "https://example.com/search?si=1");
        assert!(!hows("link", "https://example.com/a?id=1").contains(&"clean_url"));
        assert_eq!(get("url_decode", "link", "https://de.wikipedia.org/wiki/Stra%C3%9Fe"), "https://de.wikipedia.org/wiki/Straße");
        assert_eq!(url_decode("100%25 sure, 50% off"), Some("100% sure, 50% off".into()));
        assert_eq!(url_decode("%FF"), None);
    }

    #[test]
    fn json_is_laid_out_without_touching_values() {
        let src = r#"{"b": 12345678901234567890, "a":[1, {}, [] , "x, y: {z}"], "e\"": 1.50}"#;
        let pretty = get("json_pretty", "code", src);
        assert_eq!(pretty, "{\n  \"b\": 12345678901234567890,\n  \"a\": [\n    1,\n    {},\n    [],\n    \"x, y: {z}\"\n  ],\n  \"e\\\"\": 1.50\n}");
        assert_eq!(get("json_compact", "code", &pretty), r#"{"b":12345678901234567890,"a":[1,{},[],"x, y: {z}"],"e\"":1.50}"#);
        assert_eq!(choices("code", src, "")[0].hint, "10 lines");
        assert_eq!(json("{nope}", true), None);
        assert_eq!(json("42", true), None);
    }

    #[test]
    fn lines_are_trimmed_joined_sorted_and_deduped() {
        assert_eq!(get("tidy", "code", "\n\n    fn a() {  \n        b()\n\n\n\n    }\n  \n"), "fn a() {\n    b()\n\n}");
        assert_eq!(get("tidy", "text", "\t a\r\n\t b \r\n"), "a\r\nb");
        assert_eq!(get("join", "text", "Dear Sam,\n  the invoice\nis attached."), "Dear Sam, the invoice is attached.");
        assert_eq!(join("one line"), None);
        assert_eq!(get("sort", "text", "pear\nApple\n\nbanana\n"), "Apple\nbanana\npear");
        assert_eq!(get("dedupe", "text", "a\nb\n a\n\nb\nc"), "a\nb\n\nc");
        assert_eq!(choices("text", "a\nb\na", "").iter().find(|c| c.how == "dedupe").map(|c| c.hint.as_str()), Some("1 line fewer"));
    }

    #[test]
    fn case_and_identifiers() {
        assert_eq!(get("title", "text", "the quick (brown) fox's den"), "The Quick (Brown) Fox's Den");
        assert_eq!(get("upper", "text", "straße"), "STRASSE");
        assert_eq!(get("snake", "code", "parseHTTPResponse2Fast"), "parse_http_response2_fast");
        assert_eq!(get("camel", "code", "user-id"), "userId");
        assert_eq!(get("kebab", "text", "UserProfile.Name"), "user-profile-name");
        assert_eq!(ident("hello world", Ident::Snake), None, "prose is not an identifier");
        assert_eq!(ident("word", Ident::Snake), None);
    }

    #[test]
    fn base64_only_when_it_reads_as_text() {
        assert_eq!(get("base64", "text", "aGVsbG8gd29ybGQ="), "hello world");
        assert_eq!(base64_text("eyJzdWIiOiI0MiJ9"), Some(r#"{"sub":"42"}"#.into()));
        assert_eq!(base64_text("Passwords"), None);
        assert_eq!(base64_text("abcdefgh"), None);
        assert_eq!(base64_text("short"), None);
    }
}
