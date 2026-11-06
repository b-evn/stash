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
    db: PathBuf,
    // The encrypted copy being written, and the plaintext original while it's swapped out.
    encrypting: PathBuf,
    plaintext: PathBuf,
}

impl Files {
    fn new(dir: &Path) -> Self {
        Self {
            db: dir.join(FILE_NAME),
            encrypting: dir.join(format!("{FILE_NAME}.encrypting")),
            plaintext: dir.join(format!("{FILE_NAME}.plaintext")),
        }
    }
}

/// Opens the encrypted history in `dir`, first encrypting a plaintext one from an
/// older stash. Fails only when the key can't be read or stored; the history on
/// disk is then left as it is.
pub fn open(dir: &Path, keys: &impl Keys) -> Result<Db, String> {
    let f = Files::new(dir);
    finish_interrupted(&f);
    let key = match keys.get()? {
        Some(key) if well_formed(&key) => key,
        _ => {
            if f.db.exists() && !is_plaintext(&f.db) {
                set_aside(&f.db, "its key is gone");
            }
            let key = new_key()?;
            keys.set(&key)?;
            // Read it back, so a keychain that drops writes can't leave a history nobody can open.
            if keys.get()?.as_deref() != Some(key.as_str()) {
                return Err("the keychain did not keep the key".into());
            }
            key
        }
    };
    if is_plaintext(&f.db) {
        encrypt(&f, &key).map_err(|e| format!("could not encrypt the history: {e}"))?;
    }
    match Db::open(&f.db, &key) {
        Err(rusqlite::Error::SqliteFailure(e, _)) if e.code == ErrorCode::NotADatabase => {
            set_aside(&f.db, "its key doesn't open it");
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

fn is_plaintext(path: &Path) -> bool {
    let mut head = [0u8; 16];
    fs::File::open(path).and_then(|mut f| io::Read::read_exact(&mut f, &mut head)).is_ok() && head == PLAIN_HEADER
}

fn sidecars(db: &Path) -> [PathBuf; 3] {
    ["-wal", "-shm", "-journal"].map(|s| PathBuf::from(format!("{}{s}", db.display())))
}

// Writes an encrypted copy next to the plaintext history, swaps it in, then
// overwrites and deletes the plaintext. Each step can be interrupted: the next
// start finishes the swap or starts over from the untouched original.
fn encrypt(f: &Files, key: &str) -> Result<(), String> {
    remove_with_sidecars(&f.encrypting);
    let export = || -> rusqlite::Result<()> {
        let conn = Connection::open(&f.db)?;
        conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
        conn.execute("ATTACH DATABASE ?1 AS secure KEY ?2", params![f.encrypting.to_string_lossy(), format!("x'{key}'")])?;
        conn.query_row("SELECT sqlcipher_export('secure')", [], |_| Ok(()))?;
        conn.execute_batch("DETACH DATABASE secure;")
    };
    export().map_err(|e| e.to_string())?;
    let swap = || -> io::Result<()> {
        fs::File::open(&f.encrypting)?.sync_all()?;
        fs::rename(&f.db, &f.plaintext)?;
        for side in sidecars(&f.db) {
            shred(&side);
        }
        fs::rename(&f.encrypting, &f.db)
    };
    swap().map_err(|e| e.to_string())?;
    shred(&f.plaintext);
    Ok(())
}

fn finish_interrupted(f: &Files) {
    if f.plaintext.exists() {
        if f.db.exists() {
            shred(&f.plaintext);
        } else if let Err(e) = fs::rename(&f.plaintext, &f.db) {
            eprintln!("stash: could not restore {}: {e}", f.plaintext.display());
        }
    }
    remove_with_sidecars(&f.encrypting);
}

fn remove_with_sidecars(path: &Path) {
    let _ = fs::remove_file(path);
    for side in sidecars(path) {
        let _ = fs::remove_file(side);
    }
}

// Keeps a history stash can't decrypt, renamed, instead of deleting it.
fn set_aside(db: &Path, why: &str) {
    let to = PathBuf::from(format!("{}.unreadable-{}", db.display(), crate::now_ms()));
    eprintln!("stash: starting a new history, {why}; the old one is kept as {}", to.display());
    let _ = fs::rename(db, &to);
    for (side, suffix) in sidecars(db).iter().zip(["-wal", "-shm", "-journal"]) {
        let _ = fs::rename(side, format!("{}{suffix}", to.display()));
    }
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
        assert!(!is_plaintext(&file) && !file_contains(&file, "hunter2"));
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
        assert!(is_plaintext(&file));
        let keys = Fake::default();
        let clips = open(&dir, &keys).unwrap().list().unwrap();
        assert_eq!((clips[0].text.as_str(), clips[0].pinned), ("DB_PASSWORD=hunter2-very-secret", true));
        assert!(!is_plaintext(&file) && !file_contains(&file, "hunter2"));
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
        assert!(!f.plaintext.exists() && !f.encrypting.exists() && !is_plaintext(&f.db));
    }

    #[test]
    fn a_history_without_its_key_is_kept_aside() {
        let dir = scratch("lost");
        open(&dir, &Fake::default()).unwrap().add(&secret(), 1, 10).unwrap();
        let fresh = Fake::default();
        assert!(open(&dir, &fresh).unwrap().list().unwrap().is_empty());
        let kept = fs::read_dir(&dir).unwrap().filter(|e| e.as_ref().unwrap().file_name().to_string_lossy().contains("unreadable")).count();
        assert!(kept >= 1);
    }

    #[test]
    fn a_locked_keychain_touches_nothing() {
        let dir = scratch("locked");
        plaintext_history(&dir);
        assert!(open(&dir, &Locked).is_err());
        assert!(is_plaintext(&dir.join(FILE_NAME)));
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
