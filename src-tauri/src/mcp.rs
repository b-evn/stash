// `stash mcp`: the clipboard history as an MCP server over stdio, so AI tools
// (Claude, Cursor, …) can search and read what the user copied. Off until the
// user turns it on in the settings. Output is plain lines rather than JSON, a
// clip's size is given in tokens, large clips come as an outline first, and
// secrets are replaced by their name wherever text leaves stash.

use crate::{
    capture, config,
    db::{Db, Row},
    outline,
    query::{self, Doc, Ranker, Sort},
    secrets, vault,
};
use serde_json::{json, Value};
use std::{
    borrow::Cow,
    collections::HashSet,
    io::{BufRead, Write},
    path::PathBuf,
};

const PROTOCOLS: &[&str] = &["2025-06-18", "2025-03-26", "2024-11-05"];
const BUDGET: usize = 2000;
const MAX_BUDGET: usize = 20_000;
const PREVIEW: usize = 100;
const IMAGE_MAX: u32 = 1568;
/// Near-copies (most lines shared) are folded into the first one listed.
const SIMILAR: f64 = 0.7;

const QUERY_HELP: &str = "Query: words (all must match; any case unless a word has a capital), \"exact phrase\", 'wholeword, ^startswith, !exclude, either|or, /regex/i. Filters: @text @code @link @path @email @number @color @image @file, @pin, @hour @today @week @month, @since:3d @before:2026-11-01, @site:github.com, @size>2k @size<500 (tokens).";

const INSTRUCTIONS: &str = "stash is the user's clipboard history: everything they copied, newest first. \
search finds clips (one line each: id, kind, age, size in tokens, preview); read returns one clip, whole when small, \
otherwise an outline with line numbers to read next with `at`; grep finds matching lines across clips; copy puts \
text on the user's clipboard. Secrets (passwords, tokens, cards, keys) always show as ‹label›.";

pub struct Server {
    dir: Option<PathBuf>,
    db: Option<Db>,
    client: String,
    /// Set in tests; otherwise the settings decide.
    allow: Option<bool>,
    now: fn() -> i64,
}

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

pub fn data_dir() -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join(crate::APP_ID))
}

/// Serves MCP on stdin and stdout until stdin closes.
pub fn run() -> i32 {
    let mut server = Server { dir: data_dir(), db: None, client: "an AI tool".into(), allow: None, now: now_ms };
    let stdin = std::io::stdin();
    let mut out = std::io::stdout().lock();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<Value>(line) {
            Ok(Value::Array(batch)) => {
                let replies: Vec<Value> = batch.iter().filter_map(|m| server.handle(m)).collect();
                (!replies.is_empty()).then(|| Value::Array(replies))
            }
            Ok(msg) => server.handle(&msg),
            Err(e) => Some(json!({"jsonrpc": "2.0", "id": null, "error": {"code": -32700, "message": e.to_string()}})),
        };
        if let Some(reply) = reply {
            if writeln!(out, "{reply}").and_then(|_| out.flush()).is_err() {
                break;
            }
        }
    }
    0
}

fn tools() -> Value {
    json!([
        {
            "name": "search",
            "title": "Search clipboard history",
            "description": format!("Find clips the user copied. One line per clip: id, kind, age, size in tokens, preview. {QUERY_HELP}"),
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": {"type": "string", "description": "Empty lists the most recent clips."},
                    "sort": {"type": "string", "enum": Sort::NAMES, "description": "Default: relevance when the query has words, else recent."},
                    "limit": {"type": "integer", "minimum": 1, "maximum": 100, "default": 20},
                    "page": {"type": "integer", "minimum": 1, "default": 1}
                }
            },
            "annotations": {"readOnlyHint": true, "openWorldHint": false}
        },
        {
            "name": "read",
            "title": "Read a clip",
            "description": "Read one clip by id. Small clips come whole; large ones as an outline (JSON shape, table columns, headings, definitions, log errors) with line numbers to read next.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": {"type": "integer"},
                    "at": {"type": "string", "description": "What to read: L120-180 (lines), L120 (from line 120), tail, C0-8000 (characters), $.users[0] (JSON path), outline, image (the picture itself)."},
                    "budget": {"type": "integer", "minimum": 100, "maximum": MAX_BUDGET, "default": BUDGET, "description": "Most tokens to return."},
                    "against": {"type": "integer", "description": "Another clip's id: returns a diff from it to this one."}
                },
                "required": ["id"]
            },
            "annotations": {"readOnlyHint": true, "openWorldHint": false}
        },
        {
            "name": "grep",
            "title": "Find lines in clips",
            "description": "Matching lines across clips, newest first, with line numbers. pattern: text (any case unless it has a capital) or /regex/i.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "pattern": {"type": "string"},
                    "query": {"type": "string", "description": "Only clips matching this search query."},
                    "context": {"type": "integer", "minimum": 0, "maximum": 5, "default": 0, "description": "Lines around each match."},
                    "limit": {"type": "integer", "minimum": 1, "maximum": 200, "default": 40, "description": "Most lines to return."}
                },
                "required": ["pattern"]
            },
            "annotations": {"readOnlyHint": true, "openWorldHint": false}
        },
        {
            "name": "copy",
            "title": "Copy to the clipboard",
            "description": "Put text, or a clip by id, on the user's clipboard so they can paste it. It lands in their history too.",
            "inputSchema": {
                "type": "object",
                "properties": {"text": {"type": "string"}, "id": {"type": "integer"}}
            },
            "annotations": {"readOnlyHint": false, "destructiveHint": false, "idempotentHint": true, "openWorldHint": false}
        }
    ])
}

impl Server {
    #[cfg(test)]
    fn for_test(db: Db, allow: bool, now: fn() -> i64) -> Self {
        Server { dir: None, db: Some(db), client: "test".into(), allow: Some(allow), now }
    }

    fn handle(&mut self, msg: &Value) -> Option<Value> {
        let id = msg.get("id").filter(|v| !v.is_null()).cloned();
        let method = msg.get("method").and_then(Value::as_str).unwrap_or("");
        let params = msg.get("params").cloned().unwrap_or_else(|| json!({}));
        // Notifications (initialized, cancelled) need no answer; replies to requests stash never makes are dropped.
        let id = id?;
        let result = match method {
            "initialize" => Ok(self.initialize(&params)),
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({"tools": tools()})),
            "tools/call" => Ok(self.call(&params)),
            "resources/list" => Ok(json!({"resources": []})),
            "resources/templates/list" => Ok(json!({"resourceTemplates": []})),
            "prompts/list" => Ok(json!({"prompts": []})),
            _ => Err(json!({"code": -32601, "message": format!("stash has no method {method:?}")})),
        };
        Some(match result {
            Ok(r) => json!({"jsonrpc": "2.0", "id": id, "result": r}),
            Err(e) => json!({"jsonrpc": "2.0", "id": id, "error": e}),
        })
    }

    fn initialize(&mut self, params: &Value) -> Value {
        let info = &params["clientInfo"];
        if let Some(name) = info["title"].as_str().or(info["name"].as_str()).filter(|n| !n.trim().is_empty()) {
            self.client = outline::cut(name.trim(), 40);
        }
        let asked = params["protocolVersion"].as_str().unwrap_or("");
        let version = PROTOCOLS.iter().find(|&&v| v == asked).unwrap_or(&PROTOCOLS[0]);
        json!({
            "protocolVersion": version,
            "capabilities": {"tools": {"listChanged": false}},
            "serverInfo": {"name": "stash", "title": "stash clipboard history", "version": env!("CARGO_PKG_VERSION")},
            "instructions": format!("{INSTRUCTIONS} {QUERY_HELP}")
        })
    }

    fn allowed(&self) -> bool {
        self.allow.unwrap_or_else(|| self.dir.as_deref().and_then(config::peek).is_some_and(|c| c.ai_access))
    }

    fn db(&mut self) -> Result<&Db, String> {
        if self.db.is_none() {
            let dir = self.dir.as_deref().ok_or("stash can't find where it keeps its history")?;
            self.db = Some(vault::open_existing(dir)?);
        }
        Ok(self.db.as_ref().unwrap())
    }

    fn call(&mut self, params: &Value) -> Value {
        let name = params["name"].as_str().unwrap_or("");
        let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
        let result = if !self.allowed() {
            Err("The user hasn't let AI tools read their clipboard history. They can turn it on in stash: Settings → AI tools.".into())
        } else {
            let out = match name {
                "search" => self.search(&args).map(text),
                "read" => self.read(&args),
                "grep" => self.grep(&args).map(text),
                "copy" => self.copy(&args).map(text),
                _ => Err(format!("stash has no tool {name:?}; it has search, read, grep and copy")),
            };
            self.log(name, &args);
            out
        };
        match result {
            Ok(content) => json!({"content": content, "isError": false}),
            Err(e) => json!({"content": text(e), "isError": true}),
        }
    }

    // What each request asked for, so the settings can show who read what.
    fn log(&mut self, tool: &str, args: &Value) {
        let s = |k: &str| args[k].as_str().unwrap_or("").to_string();
        let detail = match tool {
            "search" => format!("\"{}\"", s("query")),
            "read" => format!("#{}{}", args["id"], args["at"].as_str().map_or(String::new(), |a| format!(" {a}"))),
            "grep" => format!("\"{}\"{}", s("pattern"), args["query"].as_str().map_or(String::new(), |q| format!(" in \"{q}\""))),
            "copy" => args["id"].as_i64().map_or_else(|| format!("{} characters", s("text").chars().count()), |id| format!("#{id}")),
            _ => return,
        };
        let (now, client) = ((self.now)(), self.client.clone());
        if let Ok(db) = self.db() {
            let _ = db.log_access(now, &client, tool, &outline::cut(&detail, 120));
        }
    }

    fn search(&mut self, args: &Value) -> Result<String, String> {
        let now = (self.now)();
        let qs = args["query"].as_str().unwrap_or("");
        let q = query::parse(qs, now)?;
        let sort = match args["sort"].as_str() {
            Some(s) => Sort::parse(s).ok_or_else(|| format!("sort is one of {}", Sort::NAMES.join(", ")))?,
            None if q.is_empty() => Sort::Recent,
            None => Sort::Relevance,
        };
        let limit = int(args, "limit", 20).clamp(1, 100);
        let page = int(args, "page", 1).max(1);
        let db = self.db()?;
        let mut ranker = Ranker::new(&q, now);
        db.scan(|row| {
            let src = source(&row);
            ranker.add(&doc(&row, &haystack(&row), src.as_deref()));
        })
        .map_err(|e| e.to_string())?;
        let total = ranker.len();
        if total == 0 {
            let all = db.count().unwrap_or(0);
            return Ok(if qs.trim().is_empty() {
                "The history is empty.".into()
            } else {
                format!("No clips match {qs:?} ({all} clips in the history). Try fewer words or filters.")
            });
        }
        let ranked = ranker.finish(sort);

        // Fold near-copies into the first one listed, then cut out the page.
        let want = page * limit;
        struct Group {
            row: Row,
            at: Option<(usize, usize)>,
            lines: Option<HashSet<u64>>,
            similar: Vec<i64>,
        }
        let mut groups: Vec<Group> = Vec::new();
        let mut looked = 0;
        for r in &ranked {
            if groups.len() > want || looked > want * 3 + 50 {
                break;
            }
            looked += 1;
            let Some(row) = db.row(r.id).map_err(|e| e.to_string())? else { continue };
            let lines = signature(&row.text);
            if let Some(sig) = &lines {
                let near = |g: &&mut Group| g.row.kind == row.kind && g.lines.as_ref().is_some_and(|s| jaccard(s, sig) >= SIMILAR);
                if let Some(g) = groups.iter_mut().find(near) {
                    g.similar.push(row.id);
                    continue;
                }
            }
            groups.push(Group { row, at: r.at, lines, similar: Vec::new() });
        }
        let from = (page - 1) * limit;
        if from >= groups.len() {
            return Ok(format!("{total} clips match; page {page} is past the end."));
        }
        let to = want.min(groups.len());
        let more = groups.len() > want || looked < ranked.len();
        let mut out = vec![format!(
            "{total} clip{}{} · by {} · showing {}–{}{}",
            if total == 1 { "" } else { "s" },
            if qs.trim().is_empty() { "" } else if total == 1 { " matches" } else { " match" },
            sort.name(),
            from + 1,
            to,
            if more { format!(" · more: page={}", page + 1) } else { String::new() }
        )];
        out.push("id kind age tokens preview".into());
        for Group { row, at, similar, .. } in &groups[from..to] {
            let hay = haystack(row);
            let mut tags = Vec::new();
            if row.pinned {
                tags.push("pinned".to_string());
            }
            if let Some(h) = source(row).as_deref().and_then(query::host).filter(|_| row.kind != "link") {
                tags.push(h.to_string());
            }
            if !similar.is_empty() {
                let ids: Vec<String> = similar.iter().take(5).map(|i| i.to_string()).collect();
                tags.push(format!("+{} similar: {}", similar.len(), ids.join(", ")));
            }
            let tags = if tags.is_empty() { String::new() } else { format!(" ({})", tags.join(" · ")) };
            out.push(format!(
                "{} {} {} {} {}{tags}",
                row.id,
                row.kind,
                age(now, row.last_used),
                tokens(query::estimate_tokens(hay.len())),
                preview(row, &hay, *at)
            ));
        }
        Ok(out.join("\n"))
    }

    fn read(&mut self, args: &Value) -> Result<Vec<Value>, String> {
        let now = (self.now)();
        let id = args["id"].as_i64().ok_or("read needs the id of a clip; search finds them")?;
        let budget = int(args, "budget", BUDGET).clamp(100, MAX_BUDGET);
        let chars = budget * 4;
        let db = self.db()?;
        let row = db.row(id).map_err(|e| e.to_string())?.ok_or_else(|| format!("There's no clip #{id}; search finds the ids."))?;
        let head = header(&row, now);
        if let Some(other) = args["against"].as_i64() {
            let old = db.row(other).map_err(|e| e.to_string())?.ok_or_else(|| format!("There's no clip #{other}."))?;
            return Ok(text(format!("{head}\n{}", diff(&old, &row, chars))));
        }
        let hay = haystack(&row);
        let at = args["at"].as_str().map(str::trim).filter(|a| !a.is_empty());
        if row.kind == "image" {
            if at == Some("image") {
                let data = db.image_data(id).map_err(|e| e.to_string())?.ok_or("the picture wasn't kept")?;
                let png = fit_image(&data).ok_or("the stored picture can't be read")?;
                return Ok(vec![json!({"type": "text", "text": head}), json!({"type": "image", "data": b64(&png), "mimeType": "image/png"})]);
            }
            if at.is_none() {
                let body = if row.ocr_text.trim().is_empty() {
                    "No text was recognized in it.".to_string()
                } else {
                    format!("text in it:\n{}", slice_lines(&row.ocr_text, 1, None, chars)?)
                };
                return Ok(text(format!("{head}\n{body}\n(at=image shows the picture)")));
            }
        }
        let body = match at {
            None if query::estimate_tokens(hay.len()) <= budget => secrets::redact_all(&hay),
            None | Some("outline") => {
                let mut o = outline::outline(&hay, &row.kind);
                o.push_str(&hints(&hay, chars));
                o
            }
            Some("image") => return Err(format!("#{id} is {}, not an image", row.kind)),
            Some("tail") => tail(&hay, chars),
            Some("full") => slice_lines(&hay, 1, None, chars)?,
            Some(a) if a.starts_with('$') => json_slice(&hay, a, chars)?,
            Some(a) if a.starts_with(['L', 'l']) => {
                let (from, to) = range(&a[1..]).ok_or_else(|| format!("can't read {a:?}; use L120-180 or L120"))?;
                slice_lines(&hay, from.max(1), to, chars)?
            }
            Some(a) if a.starts_with(['C', 'c']) => {
                let (from, to) = range(&a[1..]).ok_or_else(|| format!("can't read {a:?}; use C0-8000"))?;
                char_slice(&hay, from, to, chars)
            }
            Some(a) => return Err(format!("can't read at={a:?}; use L120-180, L120, tail, C0-8000, $.path, outline or image")),
        };
        Ok(text(format!("{head}\n{body}")))
    }

    fn grep(&mut self, args: &Value) -> Result<String, String> {
        const PER_CLIP: usize = 8;
        const COUNT_CAP: usize = 1000;
        let now = (self.now)();
        let pat = args["pattern"].as_str().filter(|p| !p.is_empty()).ok_or("grep needs a pattern")?;
        let re = query::pattern(pat)?;
        let q = query::parse(args["query"].as_str().unwrap_or(""), now)?;
        let context = int(args, "context", 0).min(5);
        let limit = int(args, "limit", 40).clamp(1, 200);
        let db = self.db()?;
        let mut ranker = Ranker::new(&q, now);
        let (mut lines_total, mut clips_total, mut shown) = (0usize, 0usize, 0usize);
        let mut out: Vec<String> = Vec::new();
        let mut seen: std::collections::HashMap<u64, i64> = Default::default();
        db.scan(|row| {
            if lines_total >= COUNT_CAP {
                return;
            }
            let hay = haystack(&row);
            let src = source(&row);
            if !ranker.add(&doc(&row, &hay, src.as_deref())) {
                return;
            }
            let starts = outline::line_starts(&hay);
            let key = secrets::private_key(&hay).map(|(a, b, _)| (a, b));
            let mut hits: Vec<(usize, usize, usize)> = Vec::new();
            for m in re.find_iter(&hay) {
                if m.start() == m.end() {
                    continue;
                }
                let ln = starts.partition_point(|&s| s <= m.start()) - 1;
                if hits.last().is_some_and(|h| h.0 == ln) || secrets::hidden(&hay, m.start(), m.end(), key) {
                    continue;
                }
                hits.push((ln, m.start(), m.end()));
                if lines_total + hits.len() >= COUNT_CAP {
                    break;
                }
            }
            if hits.is_empty() {
                return;
            }
            clips_total += 1;
            lines_total += hits.len();
            if shown >= limit {
                return;
            }
            let title = format!("#{} {} {}", row.id, row.kind, age(now, row.last_used));
            let mut lines: Vec<String> = Vec::new();
            let mut last_printed: Option<usize> = None;
            // A line already shown under another clip (copies of the same log) is only counted.
            let (mut repeats, mut repeat_of) = (0, None);
            let take = hits.len().min(PER_CLIP).min(limit - shown);
            for &(ln, a, b) in &hits[..take] {
                let matched = grep_line(&hay, &starts, ln, Some((a, b)));
                let h = capture::fnv(matched.as_bytes());
                if let Some(&first) = seen.get(&h).filter(|&&first| first != row.id) {
                    repeats += 1;
                    repeat_of.get_or_insert(first);
                    continue;
                }
                seen.insert(h, row.id);
                let lo = ln.saturating_sub(context);
                let hi = (ln + context).min(starts.len() - 1);
                if context > 0 && last_printed.is_some_and(|p| p + 1 < lo) {
                    lines.push("  --".into());
                }
                for i in lo..=hi {
                    if last_printed.is_some_and(|p| i <= p) {
                        continue;
                    }
                    let shown_line = if i == ln { format!("  {}: {matched}", i + 1) } else { format!("  {}- {}", i + 1, grep_line(&hay, &starts, i, None)) };
                    lines.push(shown_line);
                    last_printed = Some(i);
                }
            }
            let first = repeat_of.unwrap_or(0);
            if lines.is_empty() {
                out.push(format!("{title}: the same {} as #{first}", plural(repeats, "line")));
            } else {
                out.push(title);
                out.extend(lines);
                if repeats > 0 {
                    out.push(format!("  (+{} also in #{first})", plural(repeats, "line")));
                }
            }
            shown += take - repeats;
            if hits.len() > take {
                let (next, last) = (hits[take].0 + 1, hits[hits.len() - 1].0 + 1);
                out.push(format!("  … +{} more lines: read {} at=L{next}-{last}", hits.len() - take, row.id));
            }
        })
        .map_err(|e| e.to_string())?;
        if lines_total == 0 {
            return Ok(format!("No lines match {pat:?}."));
        }
        let cap = if lines_total >= COUNT_CAP { "+" } else { "" };
        let head = format!(
            "{lines_total}{cap} lines in {clips_total}{cap} clips{}",
            if shown < lines_total { format!(" · showing {shown}; narrow with query or a tighter pattern") } else { String::new() }
        );
        out.insert(0, head);
        Ok(out.join("\n"))
    }

    fn copy(&mut self, args: &Value) -> Result<String, String> {
        let paste = if cfg!(target_os = "macos") { "⌘V" } else { "Ctrl+V" };
        let mut cb = arboard::Clipboard::new().map_err(|e| e.to_string())?;
        if let Some(t) = args["text"].as_str() {
            if t.is_empty() {
                return Err("copy needs some text, or the id of a clip".into());
            }
            cb.set_text(t).map_err(|e| e.to_string())?;
            return Ok(format!("Copied {} characters. The user can paste them with {paste}.", t.chars().count()));
        }
        let id = args["id"].as_i64().ok_or("copy needs text, or the id of a clip")?;
        let p = self.db()?.payload(id).map_err(|e| e.to_string())?.ok_or_else(|| format!("There's no clip #{id}."))?;
        capture::write(&mut cb, &p.kind, &p.text, p.data.as_deref(), p.html.as_deref())?;
        Ok(format!("Clip #{id} is on the clipboard. The user can paste it with {paste}."))
    }
}

fn text(s: impl Into<String>) -> Vec<Value> {
    vec![json!({"type": "text", "text": s.into()})]
}

fn int(args: &Value, key: &str, default: usize) -> usize {
    args[key].as_u64().map_or(default, |n| n as usize)
}

fn b64(data: &[u8]) -> String {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.encode(data)
}

/// What a clip is searched by: its text, and the text recognized in it.
fn haystack(row: &Row) -> Cow<'_, str> {
    match (row.text.is_empty(), row.ocr_text.is_empty()) {
        (_, true) => Cow::Borrowed(&row.text),
        (true, false) => Cow::Borrowed(&row.ocr_text),
        _ => Cow::Owned(format!("{}\n{}", row.text, row.ocr_text)),
    }
}

/// The page a clip was copied from: Windows notes it in the copied formatting,
/// and a link is its own source.
fn source(row: &Row) -> Option<String> {
    let from_html = row.html_head.as_deref().and_then(|h| {
        h.lines().take(12).find_map(|l| l.strip_prefix("SourceURL:")).map(|u| u.trim().to_string()).filter(|u| !u.is_empty())
    });
    from_html.or_else(|| (row.kind == "link").then(|| row.text.trim().to_string()))
}

fn doc<'a>(row: &'a Row, hay: &'a str, src: Option<&'a str>) -> Doc<'a> {
    Doc {
        id: row.id,
        hay,
        kind: &row.kind,
        site: src.and_then(query::host),
        pinned: row.pinned,
        uses: row.uses,
        created: row.created,
        last_used: row.last_used,
        tokens: query::estimate_tokens(hay.len()),
    }
}

/// The lines of a multi-line clip, to tell near-copies of it.
fn signature(text: &str) -> Option<HashSet<u64>> {
    let lines: HashSet<u64> =
        text.lines().map(str::trim).filter(|l| !l.is_empty()).take(5000).map(|l| capture::fnv(l.as_bytes())).collect();
    (lines.len() >= 3).then_some(lines)
}

fn jaccard(a: &HashSet<u64>, b: &HashSet<u64>) -> f64 {
    let both = a.intersection(b).count() as f64;
    both / ((a.len() + b.len()) as f64 - both)
}

pub fn age(now: i64, at: i64) -> String {
    let s = (now - at).max(0) / 1000;
    match s {
        0..60 => "now".into(),
        60..3600 => format!("{}m", s / 60),
        3600..86_400 => format!("{}h", s / 3600),
        86_400..1_209_600 => format!("{}d", s / 86_400),
        1_209_600..5_184_000 => format!("{}w", s / 604_800),
        5_184_000..63_072_000 => format!("{}mo", s / 2_592_000),
        _ => format!("{}y", s / 31_536_000),
    }
}

fn tokens(n: usize) -> String {
    match n {
        0..1000 => n.to_string(),
        1000..10_000 => format!("{:.1}k", n as f64 / 1000.0),
        _ => format!("{}k", (n + 500) / 1000),
    }
}

fn plural(n: usize, one: &str) -> String {
    format!("{n} {one}{}", if n == 1 { "" } else { "s" })
}

fn preview(row: &Row, hay: &str, at: Option<(usize, usize)>) -> String {
    let around = |hay: &str, (a, b): (usize, usize)| {
        let lo = secrets::floor_boundary(hay, a.saturating_sub(40));
        let hi = secrets::ceil_boundary(hay, b + 90);
        let lo = if lo > 0 { hay[lo..a].find(char::is_whitespace).map_or(lo, |i| lo + i) } else { lo };
        let s = outline::squash(&secrets::redact(hay, lo, hi));
        let s = outline::cut(&s, PREVIEW);
        format!("{}{s}{}", if lo > 0 { "…" } else { "" }, if hi < hay.len() && !s.ends_with('…') { "…" } else { "" })
    };
    let start = |hay: &str| {
        let lead = hay.len() - hay.trim_start().len();
        outline::cut(&outline::squash(&secrets::redact(hay, lead, secrets::ceil_boundary(hay, lead + 600))), PREVIEW)
    };
    match row.kind.as_str() {
        "image" => {
            let dims = format!("{}×{}", row.width.unwrap_or(0), row.height.unwrap_or(0));
            if row.ocr_text.trim().is_empty() {
                return dims;
            }
            // In an image the haystack is its recognized text.
            let t = at.map_or_else(|| start(hay), |at| around(hay, at));
            format!("{dims} text: {t}")
        }
        "file" => {
            let names: Vec<&str> = row.text.lines().map(|p| p.rsplit(['/', '\\']).next().unwrap_or(p)).collect();
            format!("{}: {}", plural(names.len(), "file"), outline::cut(&names.join(", "), PREVIEW))
        }
        _ => match at {
            Some(at) => around(hay, at),
            None => match (hay.len() < 2_000_000).then(|| outline::parse_json(hay)).flatten() {
                Some(v) => outline::brief(&v),
                None => start(hay),
            },
        },
    }
}

fn header(row: &Row, now: i64) -> String {
    let hay = haystack(row);
    let mut parts = vec![format!("#{}", row.id)];
    match row.kind.as_str() {
        "image" => {
            parts.push(format!("image {}×{}", row.width.unwrap_or(0), row.height.unwrap_or(0)));
            if !row.ocr_text.trim().is_empty() {
                parts.push(format!("~{} tokens of text", tokens(query::estimate_tokens(row.ocr_text.len()))));
            }
        }
        "file" => parts.push(plural(row.text.lines().count(), "file")),
        k => {
            parts.push(k.to_string());
            parts.push(plural(outline::line_starts(&hay).len(), "line"));
            parts.push(format!("~{} tokens", tokens(query::estimate_tokens(hay.len()))));
        }
    }
    if now - row.created > 60_000 && row.last_used - row.created > 60_000 {
        parts.push(format!("first copied {} ago", age(now, row.created)));
        parts.push(format!("last used {} ago", age(now, row.last_used)));
    } else {
        parts.push(format!("copied {} ago", age(now, row.created)));
    }
    if row.uses > 0 {
        parts.push(format!("pasted {}×", row.uses));
    }
    if row.pinned {
        parts.push("pinned".into());
    }
    if let Some(src) = source(row).filter(|_| row.kind != "link") {
        parts.push(format!("from {}", outline::cut(&src, 100)));
    }
    parts.join(" · ")
}

fn hints(text: &str, chars: usize) -> String {
    let starts = outline::line_starts(text);
    let mut n = 0;
    let mut used = 0;
    while n < starts.len() {
        let (a, b) = outline::line(text, &starts, n);
        used += b - a + 1;
        if used > chars && n > 0 {
            break;
        }
        n += 1;
    }
    let json = if outline::parse_json(text).is_some() { " · at=$.key" } else { "" };
    if starts.len() == 1 {
        return format!("\nmore: at=C0-{chars} · at=tail{json}");
    }
    format!("\nmore: at=L1-{n} · at=tail · at=L<from>-<to>{json}")
}

// "120-180", "120", "120-" as (from, to).
fn range(s: &str) -> Option<(usize, Option<usize>)> {
    let s = s.trim();
    match s.split_once('-') {
        Some((a, "")) => Some((a.trim().parse().ok()?, None)),
        Some((a, b)) => {
            let (a, b) = (a.trim().parse().ok()?, b.trim().parse().ok()?);
            (b >= a).then_some((a, Some(b)))
        }
        None => Some((s.parse().ok()?, None)),
    }
}

/// Lines `from` to `to` (1-based, inclusive), as many as fit in `chars`.
fn slice_lines(text: &str, from: usize, to: Option<usize>, chars: usize) -> Result<String, String> {
    let starts = outline::line_starts(text);
    let total = starts.len();
    if from > total {
        return Err(format!("it has {}", plural(total, "line")));
    }
    let end = to.unwrap_or(total).min(total);
    let (first, _) = outline::line(text, &starts, from - 1);
    let mut last = from - 1;
    let mut used = 0;
    for i in from - 1..end {
        let (a, b) = outline::line(text, &starts, i);
        if used + (b - a + 1) > chars {
            if i == from - 1 {
                // One line longer than the budget: give its start, then go on by characters.
                let cut = secrets::floor_boundary(text, a + chars);
                return Ok(format!(
                    "{}\n[L{from} is {} characters; showing the first {} · next: at=C{cut}-{}]",
                    secrets::redact(text, a, cut),
                    b - a,
                    cut - a,
                    cut + chars
                ));
            }
            break;
        }
        used += b - a + 1;
        last = i;
    }
    let (_, stop) = outline::line(text, &starts, last);
    let shown = last + 1;
    let footer = if shown < end || (to.is_none() && shown < total) {
        let next_to = (shown + 1 + (shown - from)).min(total);
        format!("[L{from}-{shown} of {total} · next: at=L{}-{next_to}]", shown + 1)
    } else {
        format!("[L{from}-{shown} of {total}]")
    };
    Ok(format!("{}\n{footer}", secrets::redact(text, first, stop)))
}

fn tail(text: &str, chars: usize) -> String {
    let starts = outline::line_starts(text);
    let total = starts.len();
    let mut first = total;
    let mut used = 0;
    while first > 0 {
        let (a, b) = outline::line(text, &starts, first - 1);
        if used + (b - a + 1) > chars {
            break;
        }
        used += b - a + 1;
        first -= 1;
    }
    if first == total {
        // The last line alone is over the budget: its end, by characters.
        let lo = secrets::ceil_boundary(text, text.len().saturating_sub(chars));
        return format!("{}\n[last {} characters · earlier: at=C{}-{lo}]", secrets::redact(text, lo, text.len()), text.len() - lo, lo.saturating_sub(chars));
    }
    let body = secrets::redact(text, starts[first], text.len());
    let earlier = if first > 0 { format!(" · earlier: at=L{}-{first}", first.saturating_sub(total - first).max(1)) } else { String::new() };
    format!("{}\n[L{}-{total} of {total}{earlier}]", body.trim_end_matches('\n'), first + 1)
}

fn char_slice(text: &str, from: usize, to: Option<usize>, chars: usize) -> String {
    let a = secrets::floor_boundary(text, from);
    let b = secrets::floor_boundary(text, to.unwrap_or(a + chars).min(a + chars));
    let next = if b < text.len() { format!(" · next: at=C{b}-{}", b + chars) } else { String::new() };
    format!("{}\n[C{a}-{b} of {}{next}]", secrets::redact(text, a, b), text.len())
}

fn json_slice(text: &str, path: &str, chars: usize) -> Result<String, String> {
    let v = outline::parse_json(text).ok_or("it isn't JSON, so it has no paths; use L or C ranges")?;
    let at = outline::json_at(&v, path)?;
    let pretty = serde_json::to_string_pretty(at).map_err(|e| e.to_string())?;
    let shape = format!("{path}: {}", outline::shape(at, 0));
    if pretty.len() <= chars {
        return Ok(format!("{shape}\n{}", secrets::redact_all(&pretty)));
    }
    let cut = secrets::floor_boundary(&pretty, chars);
    let cut = pretty[..cut].rfind('\n').unwrap_or(cut);
    let hint = if at.is_array() { format!("{path}[0]") } else { format!("{path}.<key>") };
    Ok(format!(
        "{shape}\n{}\n[first {} of {} characters · narrow the path, e.g. {hint}]",
        secrets::redact(&pretty, 0, cut),
        cut,
        pretty.len()
    ))
}

fn diff(old: &Row, new: &Row, chars: usize) -> String {
    // Both end in a line break, so a last line isn't reported as changed for lacking one.
    let ended = |t: String| if t.ends_with('\n') { t } else { t + "\n" };
    let (a, b) = (ended(secrets::redact_all(&haystack(old))), ended(secrets::redact_all(&haystack(new))));
    let d = similar::TextDiff::from_lines(&a, &b);
    let (mut added, mut removed) = (0, 0);
    for c in d.iter_all_changes() {
        match c.tag() {
            similar::ChangeTag::Insert => added += 1,
            similar::ChangeTag::Delete => removed += 1,
            similar::ChangeTag::Equal => {}
        }
    }
    if added + removed == 0 {
        return format!("Same text as #{}.", old.id);
    }
    let body = d.unified_diff().context_radius(2).header(&format!("#{}", old.id), &format!("#{}", new.id)).to_string();
    let summary = format!("diff from #{}: +{added} −{removed} lines", old.id);
    if body.len() <= chars {
        return format!("{summary}\n{body}");
    }
    let cut = secrets::floor_boundary(&body, chars);
    let cut = body[..cut].rfind('\n').unwrap_or(cut);
    format!("{summary}\n{}\n[diff cut at {cut} of {} characters; read L ranges of both to see the rest]", &body[..cut], body.len())
}

fn grep_line(hay: &str, starts: &[usize], i: usize, hit: Option<(usize, usize)>) -> String {
    let (a, b) = outline::line(hay, starts, i);
    if b - a <= 200 {
        return secrets::redact(hay, a, b).trim_end().to_string();
    }
    let (ma, mb) = hit.unwrap_or((a, a));
    let lo = secrets::floor_boundary(hay, ma.saturating_sub(80).max(a));
    let hi = secrets::ceil_boundary(hay, (mb + 100).min(b));
    format!("{}{}{}", if lo > a { "…" } else { "" }, secrets::redact(hay, lo, hi), if hi < b { "…" } else { "" })
}

/// The picture at most IMAGE_MAX on its long side, as PNG.
fn fit_image(png: &[u8]) -> Option<Vec<u8>> {
    let (w, h, rgba) = capture::decode_png(png)?;
    if w.max(h) <= IMAGE_MAX {
        return Some(png.to_vec());
    }
    let (tw, th, small) = capture::thumbnail(w, h, &rgba, IMAGE_MAX);
    capture::encode_png(tw, th, &small)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Entry;

    const NOW: i64 = 1_800_000_000_000;
    const MIN: i64 = 60_000;

    fn now() -> i64 {
        NOW
    }

    fn entry(t: &str) -> Entry {
        Entry {
            key: capture::text_key(t),
            kind: crate::kind::detect(t).as_str().into(),
            text: t.into(),
            ocr_text: String::new(),
            html: None,
            data: None,
            thumb: None,
            width: None,
            height: None,
            bytes: t.len() as i64,
        }
    }

    fn server(clips: &[(&str, i64)]) -> Server {
        let db = Db::in_memory().unwrap();
        for (t, ago) in clips {
            db.add(&entry(t), NOW - ago * MIN, 1000).unwrap();
        }
        Server::for_test(db, true, now)
    }

    fn call(s: &mut Server, tool: &str, args: Value) -> (String, bool) {
        let r = s.handle(&json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": tool, "arguments": args}})).unwrap();
        let res = &r["result"];
        (res["content"][0]["text"].as_str().unwrap_or("").to_string(), res["isError"].as_bool().unwrap())
    }

    #[test]
    fn speaks_mcp() {
        let mut s = server(&[]);
        let r = s
            .handle(&json!({"jsonrpc": "2.0", "id": 0, "method": "initialize", "params": {"protocolVersion": "2025-03-26", "clientInfo": {"name": "claude-code"}}}))
            .unwrap();
        assert_eq!(r["result"]["protocolVersion"], "2025-03-26");
        assert_eq!(s.client, "claude-code");
        assert!(s.handle(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"})).is_none());
        let r = s.handle(&json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"})).unwrap();
        let names: Vec<&str> = r["result"]["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert_eq!(names, ["search", "read", "grep", "copy"]);
        let r = s.handle(&json!({"jsonrpc": "2.0", "id": 3, "method": "nope"})).unwrap();
        assert_eq!(r["error"]["code"], -32601);
    }

    #[test]
    fn off_until_the_user_allows_it() {
        let mut s = server(&[("hello", 1)]);
        s.allow = Some(false);
        let (t, err) = call(&mut s, "search", json!({}));
        assert!(err && t.contains("Settings → AI tools"), "{t}");
    }

    #[test]
    fn search_lists_one_line_per_clip() {
        let mut s = server(&[
            ("the quarterly invoice is attached", 300),
            ("https://github.com/b-evn/stash/issues/12", 120),
            ("DB_PASSWORD=hunter22", 30),
            ("invoice #4211 due 30 Nov", 5),
        ]);
        let (t, err) = call(&mut s, "search", json!({"query": "invoice"}));
        assert!(!err);
        let lines: Vec<&str> = t.lines().collect();
        assert!(lines[0].starts_with("2 clips match · by relevance · showing 1–2"), "{t}");
        assert_eq!(lines[1], "id kind age tokens preview");
        assert!(lines[2].starts_with("4 ") && lines[2].ends_with(" 5m 6 invoice #4211 due 30 Nov"), "{t}");
        assert!(lines[3].starts_with("1 ") && lines[3].contains(" 5h "), "{t}");

        let (t, _) = call(&mut s, "search", json!({}));
        assert!(t.lines().any(|l| l.starts_with("3 ") && l.ends_with(" 30m 5 DB_PASSWORD=‹Password›")), "{t}");
        assert!(t.contains("2 link 2h 10 https://github.com/b-evn/stash/issues/12"), "{t}");
        let (t, _) = call(&mut s, "search", json!({"query": "hunter22"}));
        assert!(t.starts_with("No clips match"), "{t}");
        let (t, _) = call(&mut s, "search", json!({"query": "@link", "limit": 1}));
        assert!(t.starts_with("1 clip matches"), "{t}");
        let (t, _) = call(&mut s, "search", json!({}));
        assert!(t.starts_with("4 clips · by recent"), "{t}");
        let (t, err) = call(&mut s, "search", json!({"sort": "sideways"}));
        assert!(err && t.contains("relevance, recent"), "{t}");
    }

    #[test]
    fn near_copies_fold_into_one_line() {
        let base: Vec<String> = (0..20).map(|i| format!("line {i} of the config")).collect();
        let a = base.join("\n");
        let b = format!("{a}\nline 20 of the config");
        let mut s = server(&[(&a, 10), (&b, 5), ("other", 1)]);
        let (t, _) = call(&mut s, "search", json!({}));
        assert!(t.contains("(+1 similar: 1)"), "{t}");
        assert_eq!(t.lines().count(), 4, "{t}");
    }

    #[test]
    fn read_gives_small_clips_whole_and_large_ones_as_an_outline() {
        let users: Vec<String> = (0..500).map(|i| format!(r#"{{"id":{i},"email":"u{i}@x.io","password":"pw{i}secret"}}"#)).collect();
        let json_clip = format!(r#"{{"users":[{}],"next":"abc"}}"#, users.join(","));
        let log: String = (1..=3000).map(|i| format!("2026-11-08 10:00:{:02} INFO tick {i}\n", i % 60)).collect::<String>() + "2026-11-08 11:00:00 ERROR boom";
        let mut s = server(&[("short note", 1), (&json_clip, 2), (&log, 3)]);

        let (t, _) = call(&mut s, "read", json!({"id": 1}));
        assert!(t.starts_with("#1 · text · 1 line · ~3 tokens · copied 1m ago\nshort note"), "{t}");

        let (t, _) = call(&mut s, "read", json!({"id": 2}));
        assert!(t.contains("json object, 2 keys:") && t.contains("users: [500× {id: num, email: str, password: str}]"), "{t}");
        assert!(t.contains("more: at=C0-8000 · at=tail · at=$.key"), "{t}");
        let (t, _) = call(&mut s, "read", json!({"id": 2, "at": "$.users[7]"}));
        assert!(t.contains("\"email\": \"u7@x.io\"") && t.contains("\"password\": \"‹Password›\"") && !t.contains("pw7secret"), "{t}");
        let (t, _) = call(&mut s, "read", json!({"id": 2, "at": "$.users", "budget": 200}));
        assert!(t.contains("narrow the path, e.g. $.users[0]"), "{t}");

        let (t, _) = call(&mut s, "read", json!({"id": 3}));
        assert!(t.contains("log, 3001 lines, 1 errors, 0 warnings") && t.contains("L3001 2026-11-08 11:00:00 ERROR boom"), "{t}");
        let (t, _) = call(&mut s, "read", json!({"id": 3, "at": "L10-12"}));
        assert!(t.ends_with("INFO tick 12\n[L10-12 of 3001]"), "{t}");
        let (t, _) = call(&mut s, "read", json!({"id": 3, "at": "L100", "budget": 100}));
        assert!(t.contains("[L100-") && t.contains("next: at=L"), "{t}");
        let (t, _) = call(&mut s, "read", json!({"id": 3, "at": "tail", "budget": 100}));
        assert!(t.contains("ERROR boom\n[L") && t.contains("of 3001 · earlier"), "{t}");
        let (t, err) = call(&mut s, "read", json!({"id": 9}));
        assert!(err && t.contains("no clip #9"), "{t}");
    }

    #[test]
    fn read_diffs_two_versions() {
        let a = "alpha\nbeta\ngamma\ndelta";
        let b = "alpha\nbeta\nGAMMA\ndelta\nepsilon";
        let mut s = server(&[(a, 10), (b, 5)]);
        let (t, _) = call(&mut s, "read", json!({"id": 2, "against": 1}));
        assert!(t.contains("diff from #1: +2 −1 lines") && t.contains("-gamma\n+GAMMA") && t.contains("+epsilon"), "{t}");
    }

    #[test]
    fn grep_finds_lines_and_skips_secrets() {
        let config = "host=db\nDB_PASSWORD=hunter22\nport=5432\nuser=hunter22-fan";
        let mut s = server(&[(config, 10), ("hunter22 the film", 5)]);
        let (t, _) = call(&mut s, "grep", json!({"pattern": "hunter22"}));
        assert!(t.starts_with("2 lines in 2 clips"), "{t}");
        assert!(t.contains("#2 text 5m\n  1: hunter22 the film"), "{t}");
        assert!(t.contains("#1 ") && t.contains("  4: user=hunter22-fan") && !t.contains("2: DB_PASSWORD"), "{t}");
        let (t, _) = call(&mut s, "grep", json!({"pattern": "port", "context": 1}));
        assert!(t.contains("  2- DB_PASSWORD=‹Password›\n  3: port=5432\n  4- user=hunter22-fan"), "{t}");
        let (t, _) = call(&mut s, "grep", json!({"pattern": "/^host/", "query": "@week"}));
        assert!(t.contains("1: host=db"), "{t}");
    }

    #[test]
    fn grep_folds_lines_other_clips_already_showed() {
        let log = "boot\nERROR disk full\nok";
        let mut s = server(&[(log, 10), (&format!("{log}\nmore"), 5)]);
        let (t, _) = call(&mut s, "grep", json!({"pattern": "error"}));
        assert!(t.contains("#2 text 5m\n  2: ERROR disk full\n#1 text 10m: the same 1 line as #2"), "{t}");
    }

    #[test]
    fn requests_are_logged() {
        let mut s = server(&[("hello", 1)]);
        call(&mut s, "search", json!({"query": "hel"}));
        call(&mut s, "read", json!({"id": 1, "at": "L1"}));
        let log = s.db.as_ref().unwrap().access_log(5).unwrap();
        assert_eq!(log.iter().map(|a| (a.tool.as_str(), a.detail.as_str())).collect::<Vec<_>>(), [("read", "#1 L1"), ("search", "\"hel\"")]);
    }

    #[test]
    fn small_helpers() {
        assert_eq!(age(NOW, NOW - 90_000), "1m");
        assert_eq!(age(NOW, NOW - 3 * 86_400_000), "3d");
        assert_eq!(age(NOW, NOW - 40 * 86_400_000), "5w");
        assert_eq!(tokens(820), "820");
        assert_eq!(tokens(4140), "4.1k");
        assert_eq!(tokens(38_400), "38k");
        assert_eq!(range("120-180"), Some((120, Some(180))));
        assert_eq!(range("120"), Some((120, None)));
        assert_eq!(range("9-3"), None);
        let a: HashSet<u64> = [1, 2, 3, 4].into();
        let b: HashSet<u64> = [1, 2, 3, 5].into();
        assert!((jaccard(&a, &b) - 0.6).abs() < 1e-9);
    }

    // A big history: 2,000 clips, 20 of them 100k characters. Run with
    // `cargo test --release --lib big_history -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn big_history() {
        let db = Db::in_memory().unwrap();
        let words = ["alpha", "invoice", "connection", "parse", "deploy", "the", "token", "server", "client", "reset"];
        let mut n = 0;
        let mut add = |t: String, db: &Db| {
            n += 1;
            db.add(&Entry { key: format!("t:{n}"), ..entry(&t) }, NOW - n * MIN, 5000).unwrap();
        };
        for i in 0..1900 {
            let t: Vec<&str> = (0..30).map(|k| words[(i * 7 + k * 3) % words.len()]).collect();
            add(format!("note {i}: {}", t.join(" ")), &db);
        }
        for i in 0..80 {
            add((0..150).map(|k| format!("fn parse_{i}_{k}(raw: &str) -> Result<Entry> {{ todo!() }}\n")).collect(), &db);
        }
        for i in 0..20 {
            let big: String = (0..2500)
                .map(|k| match k % 500 {
                    0 => format!("2026-11-08 10:{:02}:00 ERROR db: connection reset by peer ({i})\n", k % 60),
                    1 => "DB_PASSWORD=hunter22 api_key=sk-proj-4bG7kLm2Qx9Rt5Vw8Yz1Np3s\n".to_string(),
                    _ => format!("2026-11-08 10:{:02}:00 INFO request {k} served in 12 ms\n", k % 60),
                })
                .collect();
            add(big, &db);
        }
        let mut s = Server::for_test(db, true, now);
        let time = |s: &mut Server, tool: &str, args: Value| {
            let t = std::time::Instant::now();
            let (out, err) = call(s, tool, args.clone());
            let ms = t.elapsed().as_millis();
            println!("{ms:>5} ms  {tool} {args}  → {} chars, ~{} tokens{}", out.len(), out.len() / 4, if err { " (error)" } else { "" });
            if std::env::var_os("STASH_SHOW").is_some() {
                println!("{out}\n");
            }
            assert!(!err, "{out}");
            out
        };
        time(&mut s, "search", json!({}));
        time(&mut s, "search", json!({"query": "connection reset"}));
        time(&mut s, "search", json!({"query": "the"}));
        time(&mut s, "search", json!({"query": "@code parse_7"}));
        time(&mut s, "search", json!({"query": "/conn\\w+ reset/ @size>10k"}));
        let out = time(&mut s, "search", json!({"query": "hunter22"}));
        assert!(out.starts_with("No clips match"), "{out}");
        time(&mut s, "grep", json!({"pattern": "ERROR"}));
        let out = time(&mut s, "grep", json!({"pattern": "api_key"}));
        assert!(!out.contains("4bG7"), "{out}");
        time(&mut s, "read", json!({"id": 2000}));
        time(&mut s, "read", json!({"id": 2000, "at": "L490-520"}));
        time(&mut s, "read", json!({"id": 2000, "against": 1999}));
    }
}
