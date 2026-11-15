use serde::{Deserialize, Serialize};
use std::{
    fs, io,
    path::{Path, PathBuf},
    time::SystemTime,
};

pub const FILE_NAME: &str = "config.json";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Config {
    pub hotkey: String,
    pub max_items: usize,
    pub max_clip_chars: usize,
    pub ignore_sensitive: bool,
    pub capture_images: bool,
    pub capture_files: bool,
    pub ocr_images: bool,
    pub max_image_megapixels: u32,
    pub capture_screenshots: bool,
    /// Lets AI tools search and read the history through `stash mcp`.
    pub ai_access: bool,
    /// Apps whose copies are never kept, by name ("Slack") or id ("com.tinyspeck.slackmacgap", "slack.exe").
    pub ignore_apps: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            hotkey: "Ctrl+Alt+V".into(),
            max_items: 2000,
            max_clip_chars: 100_000,
            ignore_sensitive: true,
            capture_images: true,
            capture_files: true,
            ocr_images: true,
            max_image_megapixels: 40,
            capture_screenshots: true,
            ai_access: false,
            ignore_apps: Vec::new(),
        }
    }
}

impl Config {
    /// Whether copies from this app are left out, matching its name or id in any case.
    pub fn ignores_app(&self, id: &str, name: &str) -> bool {
        self.ignore_apps.iter().map(|a| a.trim()).any(|a| a.eq_ignore_ascii_case(id) || a.to_lowercase() == name.to_lowercase())
    }
}

/// The saved settings, without writing defaults when there are none.
pub fn peek(dir: &Path) -> Option<Config> {
    serde_json::from_str(&fs::read_to_string(dir.join(FILE_NAME)).ok()?).ok()
}

pub fn load(dir: &Path) -> Config {
    let path = dir.join(FILE_NAME);
    match fs::read_to_string(&path) {
        Ok(s) => serde_json::from_str(&s).unwrap_or_else(|e| {
            eprintln!("stash: ignoring invalid {}: {e}", path.display());
            Config::default()
        }),
        Err(_) => {
            let cfg = Config::default();
            let _ = save(dir, &cfg);
            cfg
        }
    }
}

pub fn save(dir: &Path, cfg: &Config) -> io::Result<()> {
    let text = serde_json::to_string_pretty(cfg).map_err(io::Error::other)?;
    fs::write(dir.join(FILE_NAME), text)
}

type Stamp = Option<(SystemTime, u64)>;

pub struct Watch {
    path: PathBuf,
    seen: Stamp,
}

impl Watch {
    pub fn new(dir: &Path) -> Self {
        let path = dir.join(FILE_NAME);
        let seen = stamp(&path);
        Self { path, seen }
    }

    pub fn poll(&mut self) -> Option<Config> {
        let now = stamp(&self.path);
        if now == self.seen {
            return None;
        }
        self.seen = now;
        let text = fs::read_to_string(&self.path).ok()?;
        match serde_json::from_str(&text) {
            Ok(cfg) => Some(cfg),
            Err(e) => {
                eprintln!("stash: keeping the current settings, {} is invalid: {e}", self.path.display());
                None
            }
        }
    }
}

fn stamp(path: &Path) -> Stamp {
    let meta = fs::metadata(path).ok()?;
    Some((meta.modified().ok()?, meta.len()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("stash-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn first_run_writes_defaults() {
        let dir = scratch("first");
        assert_eq!(load(&dir), Config::default());
        assert!(dir.join(FILE_NAME).exists());
    }

    #[test]
    fn watch_reports_only_valid_changes() {
        let dir = scratch("watch");
        let file = dir.join(FILE_NAME);
        load(&dir);
        let mut watch = Watch::new(&dir);
        assert_eq!(watch.poll(), None);

        fs::write(&file, r#"{ "max_items": 5 }"#).unwrap();
        let cfg = watch.poll().unwrap();
        assert_eq!((cfg.max_items, cfg.hotkey.as_str()), (5, "Ctrl+Alt+V"));
        assert_eq!(watch.poll(), None);

        fs::write(&file, "{ nope").unwrap();
        assert_eq!(watch.poll(), None);

        fs::write(&file, r#"{ "hotkey": "Ctrl+Shift+V" }"#).unwrap();
        assert_eq!(watch.poll().unwrap().hotkey, "Ctrl+Shift+V");

        fs::remove_file(&file).unwrap();
        assert_eq!(watch.poll(), None);
    }

    #[test]
    fn ignored_apps_match_by_name_or_id() {
        let cfg = Config { ignore_apps: vec!["slack".into(), "com.agilebits.onepassword7".into()], ..Config::default() };
        assert!(cfg.ignores_app("com.tinyspeck.slackmacgap", "Slack"));
        assert!(cfg.ignores_app("com.agilebits.onepassword7", "1Password 7"));
        assert!(!cfg.ignores_app("com.apple.Safari", "Safari"));
        assert!(!Config::default().ignores_app("slack.exe", "Slack"));
    }

    #[test]
    fn save_round_trips() {
        let dir = scratch("save");
        let cfg = Config { max_items: 500, capture_files: false, ..Config::default() };
        save(&dir, &cfg).unwrap();
        assert_eq!(load(&dir), cfg);
    }
}
