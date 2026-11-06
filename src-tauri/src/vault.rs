// Keeps the history encrypted on disk. The database is SQLCipher (every page
// AES-256 with an HMAC-SHA512, WAL included), and its key is 32 random bytes that
// live in the macOS Keychain or the Windows Credential Manager, never in a file.

use crate::db::Db;
use rusqlite::{params, Connection, ErrorCode};
use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

pub const FILE_NAME: &str = "clips.db";
const SERVICE: &str = "app.stash.clipboard";
const ACCOUNT: &str = "history key";
const PLAIN_HEADER: &[u8] = b"SQLite format 3\0";

/// Where the key is kept; the system keychain, or a fake in tests.
pub trait Keys {
    fn get(&self) -> Result<Option<String>, String>;
    fn set(&self, key: &str) -> Result<(), String>;
}

pub struct Keychain;

impl Keychain {
    fn entry() -> Result<keyring::Entry, String> {
        keyring::Entry::new(SERVICE, ACCOUNT).map_err(|e| e.to_string())
    }
}

impl Keys for Keychain {
    fn get(&self) -> Result<Option<String>, String> {
        match Self::entry()?.get_password() {
            Ok(key) => Ok(Some(key)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    fn set(&self, key: &str) -> Result<(), String> {
        Self::entry()?.set_password(key).map_err(|e| e.to_string())
    }
}

struct Files {
    dir: PathBuf,
    db: PathBuf,
    // The encrypted copy being written, and the plaintext original while it's swapped out.
    encrypting: PathBuf,
    plaintext: PathBuf,
}

impl Files {
    fn new(dir: &Path) -> Self {
        let db = dir.join(FILE_NAME);
        Self { dir: dir.into(), encrypting: suffixed(&db, ".encrypting"), plaintext: suffixed(&db, ".plaintext"), db }
    }
}

fn suffixed(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    name.into()
}

/// Opens the encrypted history in `dir`, first encrypting a plaintext one from an
/// older stash. When that can't be done safely (no access to the key, a file that
/// can't be moved), it fails with nothing on disk lost, and stash keeps history in
/// memory instead.
pub fn open(dir: &Path, keys: &impl Keys) -> Result<Db, String> {
    let f = Files::new(dir);
    finish_interrupted(&f)?;
    let key = match keys.get()? {
        Some(stored) => {
            let key = stored.trim();
            // Not a key stash made: leave it, and any history it opens, alone.
            if !well_formed(key) {
                return Err("the keychain item for the history doesn't hold a key stash can read".into());
            }
            key.to_ascii_lowercase()
        }
        None => {
            let key = new_key()?;
            keys.set(&key)?;
            // Read it back, so a keychain that drops writes can't leave a history nobody can open.
            if keys.get()?.as_deref() != Some(key.as_str()) {
                return Err("the keychain did not keep the key".into());
            }
            // Only now that the new key is safe: a history encrypted with the lost one is kept aside.
            if f.db.exists() && !is_plaintext(&f.db)? {
                set_aside(&f.db, "its key is gone")?;
            }
            key
        }
    };
    if is_plaintext(&f.db)? {
        encrypt(&f, &key).map_err(|e| format!("could not encrypt the history: {e}"))?;
    }
    match Db::open(&f.db, &key) {
        Err(rusqlite::Error::SqliteFailure(e, _)) if e.code == ErrorCode::NotADatabase => {
            set_aside(&f.db, "its key doesn't open it")?;
            Db::open(&f.db, &key).map_err(|e| e.to_string())
        }
        db => db.map_err(|e| e.to_string()),
    }
}

fn well_formed(key: &str) -> bool {
    key.len() == 64 && key.bytes().all(|b| b.is_ascii_hexdigit())
}

fn new_key() -> Result<String, String> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|e| format!("no randomness for a key: {e}"))?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

// Whether the file is an unencrypted SQLite database; a missing or empty one isn't.
// A file that can't be read is an error, not a guess.
fn is_plaintext(path: &Path) -> Result<bool, String> {
    let mut head = [0u8; 16];
    match fs::File::open(path).and_then(|mut f| io::Read::read_exact(&mut f, &mut head)) {
        Ok(()) => Ok(head == PLAIN_HEADER),
        Err(e) if matches!(e.kind(), io::ErrorKind::NotFound | io::ErrorKind::UnexpectedEof) => Ok(false),
        Err(e) => Err(format!("could not read {}: {e}", path.display())),
    }
}

fn sidecars(db: &Path) -> [PathBuf; 3] {
    ["-wal", "-shm", "-journal"].map(|s| suffixed(db, s))
}

fn path_str(path: &Path) -> rusqlite::Result<&str> {
    path.to_str().ok_or_else(|| rusqlite::Error::InvalidPath(path.into()))
}

// Writes an encrypted copy next to the plaintext history, swaps it in, then
// overwrites and deletes the plaintext. Each step can be interrupted: the next
// start finishes the swap or starts over from the untouched original.
fn encrypt(f: &Files, key: &str) -> Result<(), String> {
    remove_with_sidecars(&f.encrypting);
    let export = || -> rusqlite::Result<()> {
        let conn = Connection::open(&f.db)?;
        // The WAL is shredded below, so all of it has to be in the file first.
        let busy: i64 = conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |r| r.get(0))?;
        if busy != 0 {
            return Err(rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_BUSY),
                Some("the history is in use".into()),
            ));
        }
        conn.execute("ATTACH DATABASE ?1 AS secure KEY ?2", params![path_str(&f.encrypting)?, format!("x'{key}'")])?;
        conn.query_row("SELECT sqlcipher_export('secure')", [], |_| Ok(()))?;
        conn.execute_batch("DETACH DATABASE secure;")
    };
    export().map_err(|e| e.to_string())?;
    let swap = || -> io::Result<()> {
        // Windows only flushes a file opened for writing.
        fs::OpenOptions::new().write(true).open(&f.encrypting)?.sync_all()?;
        fs::rename(&f.db, &f.plaintext)?;
        for side in sidecars(&f.db) {
            shred(&side);
        }
        fs::rename(&f.encrypting, &f.db)?;
        // The renames reach the disk before the plaintext is deleted.
        sync_dir(&f.dir)
    };
    swap().map_err(|e| e.to_string())?;
    shred(&f.plaintext);
    Ok(())
}

// Brings the files back to a known state after an interrupted migration, or fails
// without touching anything when that can't be done.
fn finish_interrupted(f: &Files) -> Result<(), String> {
    if f.plaintext.exists() {
        if !f.db.exists() {
            // Stopped between the two renames: the original goes back.
            fs::rename(&f.plaintext, &f.db).map_err(|e| format!("could not restore the history: {e}"))?;
        } else if is_plaintext(&f.db)? {
            // An older stash started a new plaintext history in between: keep both, as one.
            let merged = merge_plaintext(&f.db, &f.plaintext).map_err(|e| format!("could not merge two plaintext histories: {e}"))?;
            if merged {
                shred(&f.plaintext);
            } else {
                // The newer one never got as far as holding clips: the original replaces it.
                put_back(f)?;
            }
        } else {
            let len = fs::metadata(&f.db).map_err(|e| format!("could not read {}: {e}", f.db.display()))?.len();
            match len {
                // Created by something else after the swap stopped, and holding nothing.
                0 => put_back(f)?,
                1..16 => return Err(format!("{} is neither the history nor its encrypted copy", f.db.display())),
                // Stopped after the swap: the encrypted copy is in place.
                _ => shred(&f.plaintext),
            }
        }
    }
    if f.encrypting.exists() {
        if f.db.exists() {
            remove_with_sidecars(&f.encrypting);
        } else {
            // The swap was done but its last rename never reached the disk: the copy is the history.
            fs::rename(&f.encrypting, &f.db).map_err(|e| format!("could not restore the history: {e}"))?;
        }
    }
    Ok(())
}

// Replaces a clips.db that holds nothing with the original history.
fn put_back(f: &Files) -> Result<(), String> {
    remove_with_sidecars(&f.db);
    fs::rename(&f.plaintext, &f.db).map_err(|e| format!("could not restore the history: {e}"))
}

// Adds the clips of the plaintext history `older` that `db` doesn't have, in the
// columns both know (an older stash has fewer). False when `db` has no history table.
fn merge_plaintext(db: &Path, older: &Path) -> rusqlite::Result<bool> {
    let conn = Connection::open(db)?;
    conn.execute("ATTACH DATABASE ?1 AS older", params![path_str(older)?])?;
    let columns = |schema: &str| -> rusqlite::Result<Vec<String>> {
        let mut stmt = conn.prepare("SELECT name FROM pragma_table_info('entries', ?1) WHERE name <> 'id'")?;
        let names = stmt.query_map(params![schema], |r| r.get(0))?.collect();
        names
    };
    let ours = columns("main")?;
    if ours.is_empty() {
        conn.execute_batch("DETACH DATABASE older;")?;
        return Ok(false);
    }
    let shared: Vec<String> =
        columns("older")?.into_iter().filter(|c| ours.contains(c)).map(|c| format!("\"{}\"", c.replace('"', "\"\""))).collect();
    if !shared.is_empty() {
        let list = shared.join(", ");
        conn.execute_batch(&format!("INSERT OR IGNORE INTO main.entries ({list}) SELECT {list} FROM older.entries;"))?;
    }
    conn.execute_batch("DETACH DATABASE older;")?;
    Ok(true)
}

fn remove_with_sidecars(path: &Path) {
    let _ = fs::remove_file(path);
    for side in sidecars(path) {
        let _ = fs::remove_file(side);
    }
}

// Keeps a history stash can't decrypt, renamed, instead of deleting it.
fn set_aside(db: &Path, why: &str) -> Result<(), String> {
    let to = suffixed(db, &format!(".unreadable-{}", crate::now_ms()));
    fs::rename(db, &to).map_err(|e| format!("could not set aside {}: {e}", db.display()))?;
    for suffix in ["-wal", "-shm", "-journal"] {
        let _ = fs::rename(suffixed(db, suffix), suffixed(&to, suffix));
    }
    eprintln!("stash: starting a new history, {why}; the old one is kept as {}", to.display());
    Ok(())
}

// Makes renames in the folder durable. Windows has no way to flush a folder.
fn sync_dir(dir: &Path) -> io::Result<()> {
    #[cfg(unix)]
    fs::File::open(dir)?.sync_all()?;
    #[cfg(not(unix))]
    let _ = dir;
    Ok(())
}

// Overwrites a plaintext file before deleting it. On SSDs and copy-on-write file
// systems (APFS) the old blocks may survive anyway, so this is a best effort.
fn shred(path: &Path) {
    if let Ok(len) = fs::metadata(path).map(|m| m.len()) {
        if let Ok(mut file) = fs::OpenOptions::new().write(true).open(path) {
            let zeros = vec![0u8; 64 * 1024];
            let mut left = len;
            while left > 0 {
                let n = left.min(zeros.len() as u64) as usize;
                if file.write_all(&zeros[..n]).is_err() {
                    break;
                }
                left -= n as u64;
            }
            let _ = file.sync_all();
        }
    }
    let _ = fs::remove_file(path);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Entry;
    use std::cell::RefCell;

    #[derive(Default)]
    struct Fake(RefCell<Option<String>>);

    impl Keys for Fake {
        fn get(&self) -> Result<Option<String>, String> {
            Ok(self.0.borrow().clone())
        }
        fn set(&self, key: &str) -> Result<(), String> {
            *self.0.borrow_mut() = Some(key.into());
            Ok(())
        }
    }

    impl Fake {
        fn holding(key: &str) -> Self {
            Self(RefCell::new(Some(key.into())))
        }
    }

    struct Locked;

    impl Keys for Locked {
        fn get(&self) -> Result<Option<String>, String> {
            Err("denied".into())
        }
        fn set(&self, _: &str) -> Result<(), String> {
            Err("denied".into())
        }
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("stash-vault-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn secret() -> Entry {
        Entry {
            key: "t:1".into(),
            kind: "text".into(),
            text: "DB_PASSWORD=hunter2-very-secret".into(),
            ocr_text: String::new(),
            html: None,
            data: None,
            thumb: None,
            width: None,
            height: None,
            bytes: 31,
        }
    }

    // Reads keys but can't store one, like a keychain that refuses writes.
    struct Empty;

    impl Keys for Empty {
        fn get(&self) -> Result<Option<String>, String> {
            Ok(None)
        }
        fn set(&self, _: &str) -> Result<(), String> {
            Err("denied".into())
        }
    }

    fn names(dir: &Path) -> Vec<String> {
        let mut names: Vec<_> = fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name().into_string().unwrap()).collect();
        names.sort();
        names
    }

    fn file_contains(path: &Path, needle: &str) -> bool {
        fs::read(path).unwrap().windows(needle.len()).any(|w| w == needle.as_bytes())
    }

    // Writes a history the way stash did before it was encrypted.
    fn plaintext_history(dir: &Path) {
        let conn = Connection::open(dir.join(FILE_NAME)).unwrap();
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             CREATE TABLE entries (
                 id INTEGER PRIMARY KEY, key TEXT NOT NULL UNIQUE, kind TEXT NOT NULL, text TEXT NOT NULL,
                 ocr_text TEXT NOT NULL DEFAULT '', data BLOB, thumb BLOB, width INTEGER, height INTEGER,
                 bytes INTEGER NOT NULL, pinned INTEGER NOT NULL DEFAULT 0, uses INTEGER NOT NULL DEFAULT 0,
                 created INTEGER NOT NULL, last_used INTEGER NOT NULL
             );
             INSERT INTO entries (key, kind, text, bytes, pinned, created, last_used)
             VALUES ('t:old', 'text', 'DB_PASSWORD=hunter2-very-secret', 31, 1, 1, 1);",
        )
        .unwrap();
    }

    #[test]
    fn a_new_history_is_encrypted_with_a_stored_key() {
        let dir = scratch("new");
        let keys = Fake::default();
        let db = open(&dir, &keys).unwrap();
        db.add(&secret(), 1, 10).unwrap();
        drop(db);
        let key = keys.0.borrow().clone().unwrap();
        assert!(well_formed(&key));
        let file = dir.join(FILE_NAME);
        assert!(!is_plaintext(&file).unwrap() && !file_contains(&file, "hunter2"));
        assert_eq!(open(&dir, &keys).unwrap().list().unwrap()[0].text, "DB_PASSWORD=hunter2-very-secret");
        // Without the key it's noise, to SQLCipher and to plain SQLite alike.
        let raw = Connection::open(&file).unwrap();
        assert!(raw.query_row("SELECT count(*) FROM entries", [], |r| r.get::<_, i64>(0)).is_err());
    }

    #[test]
    fn an_old_plaintext_history_is_encrypted_in_place() {
        let dir = scratch("migrate");
        plaintext_history(&dir);
        let file = dir.join(FILE_NAME);
        assert!(is_plaintext(&file).unwrap());
        let keys = Fake::default();
        let clips = open(&dir, &keys).unwrap().list().unwrap();
        assert_eq!((clips[0].text.as_str(), clips[0].pinned), ("DB_PASSWORD=hunter2-very-secret", true));
        assert!(!is_plaintext(&file).unwrap() && !file_contains(&file, "hunter2"));
        let left: Vec<_> = fs::read_dir(&dir).unwrap().map(|e| e.unwrap().file_name().into_string().unwrap()).collect();
        assert!(left.iter().all(|n| n == "clips.db" || n == "clips.db-wal" || n == "clips.db-shm"), "{left:?}");
        for side in sidecars(&file) {
            assert!(!side.exists() || !file_contains(&side, "hunter2"));
        }
    }

    #[test]
    fn an_interrupted_migration_resumes() {
        let dir = scratch("resume");
        plaintext_history(&dir);
        let f = Files::new(&dir);
        // Stopped after the original was swapped out, before the copy was swapped in.
        fs::write(&f.encrypting, b"half written").unwrap();
        fs::rename(&f.db, &f.plaintext).unwrap();
        let clips = open(&dir, &Fake::default()).unwrap().list().unwrap();
        assert_eq!(clips.len(), 1);
        assert!(!f.plaintext.exists() && !f.encrypting.exists() && !is_plaintext(&f.db).unwrap());
    }

    #[cfg(unix)]
    #[test]
    fn a_failed_restore_touches_nothing() {
        use std::os::unix::fs::PermissionsExt;
        let dir = scratch("stuck");
        plaintext_history(&dir);
        let f = Files::new(&dir);
        // Stopped between the two renames, and on the next two starts the folder can't be changed.
        fs::rename(&f.db, &f.plaintext).unwrap();
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o555)).unwrap();
        let starts = [open(&dir, &Fake::default()).is_err(), open(&dir, &Fake::default()).is_err()];
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(starts, [true, true]);
        assert!(!f.db.exists() && file_contains(&f.plaintext, "hunter2"));
        // Once it can, the original goes back and is encrypted.
        assert!(open(&dir, &Fake::default()).unwrap().list().unwrap()[0].text.contains("hunter2"));
    }

    #[test]
    fn a_lost_last_rename_is_recovered() {
        let dir = scratch("lost-rename");
        let keys = Fake::default();
        open(&dir, &keys).unwrap().add(&secret(), 1, 10).unwrap();
        let f = Files::new(&dir);
        // The plaintext's deletion reached the disk, the copy's rename into place didn't.
        fs::rename(&f.db, &f.encrypting).unwrap();
        assert_eq!(open(&dir, &keys).unwrap().list().unwrap().len(), 1);
        assert!(!f.encrypting.exists());
    }

    #[test]
    fn two_plaintext_histories_become_one() {
        let dir = scratch("downgrade");
        plaintext_history(&dir);
        let f = Files::new(&dir);
        // Stopped mid-swap, then an older stash started a new plaintext history.
        fs::rename(&f.db, &f.plaintext).unwrap();
        plaintext_history(&dir);
        Connection::open(&f.db).unwrap().execute_batch("UPDATE entries SET key = 't:new', text = 'newer';").unwrap();
        let mut texts: Vec<_> = open(&dir, &Fake::default()).unwrap().list().unwrap().into_iter().map(|c| c.text).collect();
        texts.sort();
        assert_eq!(texts, ["DB_PASSWORD=hunter2-very-secret", "newer"]);
        assert!(names(&dir).iter().all(|n| !n.contains("plaintext")), "{:?}", names(&dir));
        assert!(!is_plaintext(&f.db).unwrap() && !file_contains(&f.db, "hunter2"));
    }

    #[test]
    fn an_empty_or_tableless_newer_file_gives_way_to_the_original() {
        for (name, newer) in [("empty", None), ("tableless", Some("PRAGMA journal_mode = WAL;"))] {
            let dir = scratch(name);
            plaintext_history(&dir);
            let f = Files::new(&dir);
            fs::rename(&f.db, &f.plaintext).unwrap();
            match newer {
                None => fs::write(&f.db, b"").unwrap(),
                Some(sql) => Connection::open(&f.db).unwrap().execute_batch(sql).unwrap(),
            }
            let clips = open(&dir, &Fake::default()).unwrap().list().unwrap();
            assert_eq!(clips.len(), 1, "{name}");
            assert!(!f.plaintext.exists() && !is_plaintext(&f.db).unwrap(), "{name}");
        }
    }

    #[test]
    fn a_history_without_its_key_is_kept_aside() {
        let dir = scratch("lost");
        open(&dir, &Fake::default()).unwrap().add(&secret(), 1, 10).unwrap();
        assert!(open(&dir, &Fake::default()).unwrap().list().unwrap().is_empty());
        let kept = names(&dir).into_iter().filter(|n| n.starts_with("clips.db.unreadable-") && !n.ends_with("-wal") && !n.ends_with("-shm"));
        assert_eq!(kept.count(), 1);
    }

    #[test]
    fn a_history_with_another_key_is_kept_aside() {
        let dir = scratch("wrong");
        open(&dir, &Fake::default()).unwrap().add(&secret(), 1, 10).unwrap();
        let other = Fake::holding(&new_key().unwrap());
        assert!(open(&dir, &other).unwrap().list().unwrap().is_empty());
        assert!(names(&dir).iter().any(|n| n.starts_with("clips.db.unreadable-")));
    }

    #[test]
    fn nothing_is_set_aside_until_a_new_key_is_stored() {
        let dir = scratch("unstored");
        open(&dir, &Fake::default()).unwrap().add(&secret(), 1, 10).unwrap();
        assert!(open(&dir, &Empty).is_err());
        assert!(names(&dir).iter().all(|n| !n.contains("unreadable")));
    }

    #[test]
    fn a_key_stash_cant_read_is_left_alone() {
        let dir = scratch("odd-key");
        let keys = Fake::default();
        open(&dir, &keys).unwrap().add(&secret(), 1, 10).unwrap();
        let key = keys.0.borrow().clone().unwrap();
        // Spacing and case around a real key are fine.
        assert_eq!(open(&dir, &Fake::holding(&format!(" {}\n", key.to_uppercase()))).unwrap().list().unwrap().len(), 1);
        let odd = Fake::holding("not a key");
        assert!(open(&dir, &odd).is_err());
        assert_eq!(odd.0.borrow().as_deref(), Some("not a key"));
        assert_eq!(open(&dir, &keys).unwrap().list().unwrap().len(), 1);
    }

    #[test]
    fn a_locked_keychain_touches_nothing() {
        let dir = scratch("locked");
        plaintext_history(&dir);
        assert!(open(&dir, &Locked).is_err());
        assert!(is_plaintext(&dir.join(FILE_NAME)).unwrap());
        assert_eq!(names(&dir), ["clips.db"]);
    }

    // Touches the real keychain, under a name of its own: cargo test --lib -- --ignored
    #[test]
    #[ignore]
    fn the_system_keychain_keeps_the_key() {
        let entry = || keyring::Entry::new("app.stash.clipboard.test", ACCOUNT).unwrap();
        let key = new_key().unwrap();
        entry().set_password(&key).unwrap();
        // A fresh entry sees it, so this is the keychain and not keyring's in-memory mock.
        assert_eq!(entry().get_password().unwrap(), key);
        entry().delete_credential().unwrap();
        assert!(matches!(entry().get_password(), Err(keyring::Error::NoEntry)));
    }

    #[test]
    fn keys_are_random_and_whole() {
        let (a, b) = (new_key().unwrap(), new_key().unwrap());
        assert!(well_formed(&a) && well_formed(&b) && a != b);
        assert!(crate::db::unlock(&Connection::open_in_memory().unwrap(), "x'00'").is_err());
    }
}
