use base64::{engine::general_purpose::STANDARD, Engine as _};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use std::{path::Path, sync::Mutex};

const PREVIEW_CHARS: i64 = 600;

#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub key: String,
    pub kind: String,
    pub text: String,
    pub ocr_text: String,
    /// The formatting the text was copied with, as HTML (CF_HTML on Windows).
    pub html: Option<String>,
    pub data: Option<Vec<u8>>,
    pub thumb: Option<Vec<u8>>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub bytes: i64,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Clip {
    pub id: i64,
    pub kind: String,
    pub text: String,
    pub ocr_text: String,
    pub len: i64,
    pub bytes: i64,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub thumb: Option<String>,
    pub pinned: bool,
    /// Copied with formatting, so it can also be pasted as plain text.
    pub rich: bool,
    pub uses: i64,
    pub created: i64,
    pub last_used: i64,
}

pub struct Payload {
    pub kind: String,
    pub text: String,
    pub data: Option<Vec<u8>>,
    pub html: Option<String>,
}

pub struct Db {
    conn: Mutex<Connection>,
}

impl Db {
    /// Opens the history encrypted with `key` (64 hex digits), see `vault`.
    pub fn open(path: &Path, key: &str) -> rusqlite::Result<Self> {
        let conn = Connection::open(path)?;
        unlock(&conn, key)?;
        Self::init(conn)
    }

    /// A history that lives only as long as the process, for when its key is out of reach.
    pub fn in_memory() -> rusqlite::Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> rusqlite::Result<Self> {
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             CREATE TABLE IF NOT EXISTS entries (
                 id        INTEGER PRIMARY KEY,
                 key       TEXT    NOT NULL UNIQUE,
                 kind      TEXT    NOT NULL,
                 text      TEXT    NOT NULL,
                 ocr_text  TEXT    NOT NULL DEFAULT '',
                 html      TEXT,
                 data      BLOB,
                 thumb     BLOB,
                 width     INTEGER,
                 height    INTEGER,
                 bytes     INTEGER NOT NULL,
                 pinned    INTEGER NOT NULL DEFAULT 0,
                 uses      INTEGER NOT NULL DEFAULT 0,
                 created   INTEGER NOT NULL,
                 last_used INTEGER NOT NULL
             );
             CREATE INDEX IF NOT EXISTS entries_last_used ON entries (last_used);",
        )?;
        add_column(&conn, "ocr_text", "TEXT NOT NULL DEFAULT ''")?;
        add_column(&conn, "html", "TEXT")?;
        let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if version < 1 {
            fold_text_duplicates(&conn)?;
            conn.execute_batch("PRAGMA user_version = 1;")?;
        }
        Ok(Self { conn: Mutex::new(conn) })
    }

    fn conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn add(&self, e: &Entry, now: i64, max_items: usize) -> rusqlite::Result<()> {
        let c = self.conn();
        c.execute(
            "INSERT INTO entries (key, kind, text, ocr_text, html, data, thumb, width, height, bytes, created, last_used)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?11)
             ON CONFLICT(key) DO UPDATE SET last_used = excluded.last_used,
                 text = excluded.text, kind = excluded.kind, bytes = excluded.bytes,
                 html =COALESCE(excluded.html, html),
                 ocr_text = CASE WHEN excluded.ocr_text <> '' THEN excluded.ocr_text ELSE ocr_text END",
            params![e.key, e.kind, e.text, e.ocr_text, e.html, e.data, e.thumb, e.width, e.height, e.bytes, now],
        )?;
        c.execute(
            "DELETE FROM entries WHERE pinned = 0 AND id NOT IN
                 (SELECT id FROM entries WHERE pinned = 0 ORDER BY last_used DESC, id DESC LIMIT ?1)",
            params![max_items as i64],
        )?;
        Ok(())
    }

    pub fn list(&self) -> rusqlite::Result<Vec<Clip>> {
        let c = self.conn();
        let mut stmt = c.prepare(
            "SELECT id, kind, substr(text, 1, ?1), length(text), bytes, width, height, thumb,
                    pinned, uses, created, last_used, ocr_text, html IS NOT NULL
             FROM entries ORDER BY last_used DESC, id DESC",
        )?;
        let rows = stmt.query_map(params![PREVIEW_CHARS], |r| {
            let thumb: Option<Vec<u8>> = r.get(7)?;
            Ok(Clip {
                id: r.get(0)?,
                kind: r.get(1)?,
                text: r.get(2)?,
                len: r.get(3)?,
                bytes: r.get(4)?,
                width: r.get(5)?,
                height: r.get(6)?,
                thumb: thumb.map(|b| {
                    let mime = if b.starts_with(&[0xFF, 0xD8]) { "jpeg" } else { "png" };
                    format!("data:image/{mime};base64,{}", STANDARD.encode(b))
                }),
                pinned: r.get::<_, i64>(8)? != 0,
                rich: r.get(13)?,
                uses: r.get(9)?,
                created: r.get(10)?,
                last_used: r.get(11)?,
                ocr_text: r.get(12)?,
            })
        })?;
        rows.collect()
    }

    pub fn payload(&self, id: i64) -> rusqlite::Result<Option<Payload>> {
        self.conn()
            .query_row("SELECT kind, text, data, html FROM entries WHERE id = ?1", params![id], |r| {
                Ok(Payload { kind: r.get(0)?, text: r.get(1)?, data: r.get(2)?, html: r.get(3)? })
            })
            .optional()
    }

    /// Stores a thumbnail drawn for a file or SVG clip, unless it has one.
    pub fn set_thumb(&self, id: i64, thumb: &[u8], width: u32, height: u32) -> rusqlite::Result<bool> {
        self.conn()
            .execute(
                "UPDATE entries SET thumb = ?2, width = ?3, height = ?4
                 WHERE id = ?1 AND thumb IS NULL AND kind <> 'image'",
                params![id, thumb, width, height],
            )
            .map(|n| n > 0)
    }

    /// Stores the text recognized in an image clip, which arrives after the clip is saved.
    pub fn set_ocr(&self, key: &str, text: &str) -> rusqlite::Result<()> {
        self.conn().execute("UPDATE entries SET ocr_text = ?2 WHERE key = ?1", params![key, text]).map(|_| ())
    }

    pub fn mark_used(&self, id: i64, now: i64) -> rusqlite::Result<()> {
        self.conn()
            .execute("UPDATE entries SET uses = uses + 1, last_used = ?2 WHERE id = ?1", params![id, now])
            .map(|_| ())
    }

    pub fn toggle_pin(&self, id: i64) -> rusqlite::Result<()> {
        self.conn().execute("UPDATE entries SET pinned = 1 - pinned WHERE id = ?1", params![id]).map(|_| ())
    }

    pub fn delete(&self, id: i64) -> rusqlite::Result<()> {
        self.conn().execute("DELETE FROM entries WHERE id = ?1", params![id]).map(|_| ())
    }

    pub fn clear_unpinned(&self) -> rusqlite::Result<()> {
        let c = self.conn();
        c.execute("DELETE FROM entries WHERE pinned = 0", [])?;
        c.execute_batch("VACUUM;")
    }
}

// The key has to be the first thing a connection is given. Asking for the cipher
// afterwards makes sure SQLCipher is what's linked: plain SQLite ignores the key.
pub(crate) fn unlock(conn: &Connection, key: &str) -> rusqlite::Result<()> {
    let refuse = |why: &str| rusqlite::Error::SqliteFailure(rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_MISUSE), Some(why.into()));
    if key.len() != 64 || !key.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(refuse("malformed database key"));
    }
    conn.execute_batch(&format!("PRAGMA key = \"x'{key}'\";"))?;
    let cipher: Option<String> = conn.query_row("PRAGMA cipher_version", [], |r| r.get(0)).optional()?;
    if cipher.is_none() {
        return Err(refuse("SQLite was built without SQLCipher"));
    }
    Ok(())
}

/// Rekeys text clips with `capture::text_key` and merges the ones that now share
/// a key, keeping the newest copy with the earliest date, total uses and any pin.
fn fold_text_duplicates(conn: &Connection) -> rusqlite::Result<()> {
    let rows: Vec<(i64, String, String)> = {
        let mut stmt =
            conn.prepare("SELECT id, key, text FROM entries WHERE key LIKE 't:%' ORDER BY last_used DESC, id DESC")?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
        rows.collect::<rusqlite::Result<_>>()?
    };
    let mut groups: Vec<(String, Vec<(i64, String)>)> = Vec::new();
    let mut index = std::collections::HashMap::new();
    for (id, old, text) in rows {
        let key = crate::capture::text_key(&text);
        let i = *index.entry(key.clone()).or_insert_with(|| {
            groups.push((key, Vec::new()));
            groups.len() - 1
        });
        groups[i].1.push((id, old));
    }
    let tx = conn.unchecked_transaction()?;
    for (key, rows) in &groups {
        let (keep, old) = &rows[0];
        for (id, _) in &rows[1..] {
            tx.execute(
                "UPDATE entries SET
                     pinned  = max(pinned, (SELECT pinned FROM entries WHERE id = ?2)),
                     uses    = uses + (SELECT uses FROM entries WHERE id = ?2),
                     created = min(created, (SELECT created FROM entries WHERE id = ?2)),
                     html    = COALESCE(html, (SELECT html FROM entries WHERE id = ?2))
                 WHERE id = ?1",
                params![keep, id],
            )?;
            tx.execute("DELETE FROM entries WHERE id = ?1", params![id])?;
        }
        if old != key {
            tx.execute("UPDATE entries SET key = ?2 WHERE id = ?1", params![keep, key])?;
        }
    }
    tx.commit()
}

fn add_column(conn: &Connection, name: &str, decl: &str) -> rusqlite::Result<()> {
    let has: i64 =
        conn.query_row("SELECT count(*) FROM pragma_table_info('entries') WHERE name = ?1", params![name], |r| r.get(0))?;
    if has == 0 {
        conn.execute_batch(&format!("ALTER TABLE entries ADD COLUMN {name} {decl}"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(t: &str) -> Entry {
        Entry {
            key: format!("t:{t}"),
            kind: "text".into(),
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

    #[test]
    fn duplicates_bump_instead_of_stacking() {
        let db = Db::in_memory().unwrap();
        db.add(&text("a"), 1, 10).unwrap();
        db.add(&text("b"), 2, 10).unwrap();
        db.add(&text("a"), 3, 10).unwrap();
        let l = db.list().unwrap();
        assert_eq!(l.iter().map(|c| c.text.as_str()).collect::<Vec<_>>(), ["a", "b"]);
        assert_eq!((l[0].created, l[0].last_used), (1, 3));
    }

    #[test]
    fn near_copies_count_as_the_same_clip() {
        let key = crate::capture::text_key;
        assert_eq!(key("foo"), key("foo\n"));
        assert_eq!(key("a\r\nb"), key("a\nb  \n"));
        assert_eq!(key("\n\n    indented"), key("    indented"));
        assert_ne!(key("    indented"), key("indented"));
        assert_ne!(key("Foo"), key("foo"));
        let db = Db::in_memory().unwrap();
        let entry = |t: &str| Entry { key: key(t), ..text(t) };
        db.add(&entry("foo"), 1, 10).unwrap();
        db.add(&entry("bar"), 2, 10).unwrap();
        db.add(&entry("foo\r\n"), 3, 10).unwrap();
        let l = db.list().unwrap();
        assert_eq!(l.iter().map(|c| c.text.as_str()).collect::<Vec<_>>(), ["foo\r\n", "bar"]);
    }

    #[test]
    fn existing_near_copies_are_folded_once() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE entries (
                 id INTEGER PRIMARY KEY, key TEXT NOT NULL UNIQUE, kind TEXT NOT NULL, text TEXT NOT NULL,
                 ocr_text TEXT NOT NULL DEFAULT '', html TEXT, data BLOB, thumb BLOB, width INTEGER, height INTEGER,
                 bytes INTEGER NOT NULL, pinned INTEGER NOT NULL DEFAULT 0, uses INTEGER NOT NULL DEFAULT 0,
                 created INTEGER NOT NULL, last_used INTEGER NOT NULL);
             INSERT INTO entries (key, kind, text, bytes, pinned, uses, created, last_used) VALUES
                 ('t:old1', 'text', 'foo', 3, 1, 2, 1, 1),
                 ('t:old2', 'text', 'bar', 3, 0, 0, 2, 2),
                 ('t:old3', 'text', 'foo' || char(10), 4, 0, 3, 3, 3);",
        )
        .unwrap();
        let db = Db::init(conn).unwrap();
        let l = db.list().unwrap();
        assert_eq!(l.iter().map(|c| c.text.as_str()).collect::<Vec<_>>(), ["foo\n", "bar"]);
        assert_eq!((l[0].pinned, l[0].uses, l[0].created, l[0].last_used), (true, 5, 1, 3));
        let foo = Entry { key: crate::capture::text_key("foo"), ..text("foo") };
        db.add(&foo, 4, 10).unwrap();
        assert_eq!(db.list().unwrap().len(), 2);
    }

    #[test]
    fn prune_keeps_pinned_and_newest() {
        let db = Db::in_memory().unwrap();
        db.add(&text("old-pinned"), 1, 2).unwrap();
        db.toggle_pin(db.list().unwrap()[0].id).unwrap();
        for (i, t) in ["a", "b", "c", "d"].iter().enumerate() {
            db.add(&text(t), 10 + i as i64, 2).unwrap();
        }
        let texts: Vec<_> = db.list().unwrap().into_iter().map(|c| c.text).collect();
        assert_eq!(texts, ["d", "c", "old-pinned"]);
    }

    #[test]
    fn preview_is_truncated_but_payload_is_whole() {
        let db = Db::in_memory().unwrap();
        db.add(&text(&"x".repeat(2000)), 1, 10).unwrap();
        let c = &db.list().unwrap()[0];
        assert_eq!((c.text.chars().count(), c.len), (600, 2000));
        assert_eq!(db.payload(c.id).unwrap().unwrap().text.len(), 2000);
    }

    #[test]
    fn images_keep_data_and_expose_a_thumbnail() {
        let db = Db::in_memory().unwrap();
        let img = Entry {
            key: "img:1".into(),
            kind: "image".into(),
            text: String::new(),
            ocr_text: "invoice total $4,200".into(),
            html: None,
            data: Some(vec![1, 2, 3]),
            thumb: Some(vec![9]),
            width: Some(640),
            height: Some(480),
            bytes: 3,
        };
        db.add(&img, 1, 10).unwrap();
        let c = &db.list().unwrap()[0];
        assert_eq!(c.thumb.as_deref(), Some("data:image/png;base64,CQ=="));
        assert_eq!((c.width, c.height), (Some(640), Some(480)));
        assert_eq!(c.ocr_text, "invoice total $4,200");
        let p = db.payload(c.id).unwrap().unwrap();
        assert_eq!((p.kind.as_str(), p.data), ("image", Some(vec![1, 2, 3])));
    }

    #[test]
    fn recopying_an_image_backfills_ocr() {
        let db = Db::in_memory().unwrap();
        let mut img = Entry {
            key: "img:2".into(),
            kind: "image".into(),
            text: String::new(),
            ocr_text: String::new(),
            html: None,
            data: Some(vec![1]),
            thumb: None,
            width: Some(1),
            height: Some(1),
            bytes: 1,
        };
        db.add(&img, 1, 10).unwrap();
        img.ocr_text = "receipt".into();
        db.add(&img, 2, 10).unwrap();
        let l = db.list().unwrap();
        assert_eq!(l.len(), 1);
        assert_eq!((l[0].ocr_text.as_str(), l[0].last_used), ("receipt", 2));
    }

    #[test]
    fn recognized_text_arrives_after_the_image() {
        let db = Db::in_memory().unwrap();
        let img = Entry {
            key: "i:1".into(),
            kind: "image".into(),
            text: String::new(),
            ocr_text: String::new(),
            html: None,
            data: Some(vec![1]),
            thumb: None,
            width: Some(1),
            height: Some(1),
            bytes: 1,
        };
        db.add(&img, 1, 10).unwrap();
        assert_eq!(db.list().unwrap()[0].ocr_text, "");
        db.set_ocr("i:1", "receipt").unwrap();
        db.add(&img, 2, 10).unwrap();
        let l = db.list().unwrap();
        assert_eq!((l[0].ocr_text.as_str(), l[0].last_used), ("receipt", 2));
    }

    #[test]
    fn old_databases_gain_an_ocr_column() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE entries (
                 id        INTEGER PRIMARY KEY,
                 key       TEXT    NOT NULL UNIQUE,
                 kind      TEXT    NOT NULL,
                 text      TEXT    NOT NULL,
                 data      BLOB,
                 thumb     BLOB,
                 width     INTEGER,
                 height    INTEGER,
                 bytes     INTEGER NOT NULL,
                 pinned    INTEGER NOT NULL DEFAULT 0,
                 uses      INTEGER NOT NULL DEFAULT 0,
                 created   INTEGER NOT NULL,
                 last_used INTEGER NOT NULL
             );
             INSERT INTO entries (key, kind, text, bytes, created, last_used)
             VALUES ('t:old', 'text', 'old', 3, 1, 1);",
        )
        .unwrap();
        let db = Db::init(conn).unwrap();
        let old = db.list().unwrap();
        assert_eq!((old[0].text.as_str(), old[0].ocr_text.as_str()), ("old", ""));
        db.add(&text("new"), 2, 10).unwrap();
        assert!(db.list().unwrap().iter().any(|c| c.text == "new"));
    }

    #[test]
    fn drawn_thumbs_are_stored_once_with_their_type() {
        let db = Db::in_memory().unwrap();
        db.add(&Entry { kind: "file".into(), ..text("/a/logo.jpg") }, 1, 10).unwrap();
        let id = db.list().unwrap()[0].id;
        assert!(db.set_thumb(id, &[0xFF, 0xD8, 0xFF, 1], 640, 480).unwrap());
        assert!(!db.set_thumb(id, b"\x89PNG", 1, 1).unwrap());
        let c = &db.list().unwrap()[0];
        assert!(c.thumb.as_deref().unwrap().starts_with("data:image/jpeg;base64,"));
        assert_eq!((c.width, c.height), (Some(640), Some(480)));
    }

    #[test]
    fn formatting_is_kept_until_a_copy_brings_new_formatting() {
        let db = Db::in_memory().unwrap();
        let html = |db: &Db| db.payload(db.list().unwrap()[0].id).unwrap().unwrap().html;
        db.add(&Entry { html: Some("<b>hi</b>".into()), ..text("hi") }, 1, 10).unwrap();
        assert!(db.list().unwrap()[0].rich);
        // The same words copied from somewhere plain, or read while the formatting couldn't be.
        db.add(&text("hi"), 2, 10).unwrap();
        assert_eq!(html(&db).as_deref(), Some("<b>hi</b>"));
        db.add(&Entry { html: Some("<i>hi</i>".into()), ..text("hi") }, 3, 10).unwrap();
        assert_eq!(html(&db).as_deref(), Some("<i>hi</i>"));
    }

    #[test]
    fn use_pin_delete_and_clear() {
        let db = Db::in_memory().unwrap();
        db.add(&text("keep"), 1, 10).unwrap();
        db.add(&text("drop"), 2, 10).unwrap();
        let keep = db.list().unwrap().into_iter().find(|c| c.text == "keep").unwrap().id;
        db.toggle_pin(keep).unwrap();
        db.mark_used(keep, 5).unwrap();
        db.clear_unpinned().unwrap();
        let l = db.list().unwrap();
        assert_eq!(l.len(), 1);
        assert!(l[0].pinned && l[0].uses == 1 && l[0].last_used == 5);
        db.delete(keep).unwrap();
        assert!(db.list().unwrap().is_empty());
    }
}
