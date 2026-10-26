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
    pub uses: i64,
    pub created: i64,
    pub last_used: i64,
}

pub struct Payload {
    pub kind: String,
    pub text: String,
    pub data: Option<Vec<u8>>,
}

pub struct Db {
    conn: Mutex<Connection>,
}

impl Db {
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        Self::init(Connection::open(path)?)
    }

    #[cfg(test)]
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
        let has_ocr: i64 = conn.query_row(
            "SELECT count(*) FROM pragma_table_info('entries') WHERE name = 'ocr_text'",
            [],
            |r| r.get(0),
        )?;
        if has_ocr == 0 {
            conn.execute("ALTER TABLE entries ADD COLUMN ocr_text TEXT NOT NULL DEFAULT ''", [])?;
        }
        Ok(Self { conn: Mutex::new(conn) })
    }

    fn conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn add(&self, e: &Entry, now: i64, max_items: usize) -> rusqlite::Result<()> {
        let c = self.conn();
        c.execute(
            "INSERT INTO entries (key, kind, text, ocr_text, data, thumb, width, height, bytes, created, last_used)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)
             ON CONFLICT(key) DO UPDATE SET last_used = excluded.last_used,
                 ocr_text = CASE WHEN excluded.ocr_text <> '' THEN excluded.ocr_text ELSE ocr_text END",
            params![e.key, e.kind, e.text, e.ocr_text, e.data, e.thumb, e.width, e.height, e.bytes, now],
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
                    pinned, uses, created, last_used, ocr_text
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
            .query_row("SELECT kind, text, data FROM entries WHERE id = ?1", params![id], |r| {
                Ok(Payload { kind: r.get(0)?, text: r.get(1)?, data: r.get(2)? })
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

#[cfg(test)]
mod tests {
    use super::*;

    fn text(t: &str) -> Entry {
        Entry {
            key: format!("t:{t}"),
            kind: "text".into(),
            text: t.into(),
            ocr_text: String::new(),
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
