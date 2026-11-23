use serde::{Deserialize, Serialize};
use serde_json::ser::{PrettyFormatter, Serializer};
use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub shell: String,
    pub shell_args: String,
    pub cwd: String,
    pub keyboard: String,
    pub theme: String,
    pub term_font_size: u32,
    pub audio: bool,
    pub audio_volume: f64,
    pub disable_feedback_audio: bool,
    pub clock_hours: u32,
    pub ping_addr: String,
    pub port: u16,
    pub nointro: bool,
    pub nocursor: bool,
    pub force_fullscreen: bool,
    pub allow_windowed: bool,
    pub exclude_threads_from_toplist: bool,
    pub hide_dotfiles: bool,
    pub fs_list_view: bool,
    pub experimental_globe_features: bool,
    pub experimental_features: bool,
}

impl Settings {
    pub fn defaults(user_data_dir: &Path, is_windows: bool) -> Self {
        Self {
            shell: if is_windows { "powershell.exe" } else { "bash" }.to_string(),
            shell_args: String::new(),
            cwd: user_data_dir.to_string_lossy().into_owned(),
            keyboard: "en-US".to_string(),
            theme: "tron".to_string(),
            term_font_size: 15,
            audio: true,
            audio_volume: 1.0,
            disable_feedback_audio: false,
            clock_hours: 24,
            ping_addr: "1.1.1.1".to_string(),
            port: 3000,
            nointro: false,
            nocursor: false,
            force_fullscreen: true,
            allow_windowed: false,
            exclude_threads_from_toplist: true,
            hide_dotfiles: false,
            fs_list_view: false,
            experimental_globe_features: false,
            experimental_features: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Shortcut {
    #[serde(rename = "type")]
    pub kind: String,
    pub trigger: String,
    pub action: String,
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub linebreak: Option<bool>,
}

pub fn default_shortcuts() -> Vec<Shortcut> {
    let app = |trigger: &str, action: &str, enabled: bool| Shortcut {
        kind: "app".to_string(),
        trigger: trigger.to_string(),
        action: action.to_string(),
        enabled,
        linebreak: None,
    };

    vec![
        app("Ctrl+Shift+C", "COPY", true),
        app("Ctrl+Shift+V", "PASTE", true),
        app("Ctrl+Tab", "NEXT_TAB", true),
        app("Ctrl+Shift+Tab", "PREVIOUS_TAB", true),
        app("Ctrl+X", "TAB_X", true),
        app("Ctrl+Shift+S", "SETTINGS", true),
        app("Ctrl+Shift+K", "SHORTCUTS", true),
        app("Ctrl+Shift+F", "FUZZY_SEARCH", true),
        app("Ctrl+Shift+L", "FS_LIST_VIEW", true),
        app("Ctrl+Shift+H", "FS_DOTFILES", true),
        app("Ctrl+Shift+P", "KB_PASSMODE", true),
        app("Ctrl+Shift+I", "DEV_DEBUG", false),
        app("Ctrl+Shift+F5", "DEV_RELOAD", true),
        Shortcut {
            kind: "shell".to_string(),
            trigger: "Ctrl+Shift+Alt+Space".to_string(),
            action: "neofetch".to_string(),
            enabled: false,
            linebreak: Some(true),
        },
    ]
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LastWindowState {
    pub use_fullscreen: bool,
}

impl Default for LastWindowState {
    fn default() -> Self {
        Self {
            use_fullscreen: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionRecord {
    pub first_seen: u64,
    pub last_seen: u64,
}

pub type VersionsLog = BTreeMap<String, VersionRecord>;

#[derive(Debug)]
pub enum ConfigError {
    Io(std::io::Error),
    Json(serde_json::Error),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "config I/O error: {}", e),
            Self::Json(e) => write!(f, "config JSON error: {}", e),
        }
    }
}

impl std::error::Error for ConfigError {}

impl From<std::io::Error> for ConfigError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<serde_json::Error> for ConfigError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

#[derive(Debug, Clone)]
pub struct InitializedConfig {
    pub user_data_dir: PathBuf,
    pub settings_path: PathBuf,
    pub shortcuts_path: PathBuf,
    pub last_window_state_path: PathBuf,
    pub versions_log_path: PathBuf,
}

fn write_json_pretty<T: Serialize>(
    path: &Path,
    value: &T,
    indent: &[u8],
) -> Result<(), ConfigError> {
    let mut buf = Vec::new();
    let formatter = PrettyFormatter::with_indent(indent);
    let mut serializer = Serializer::with_formatter(&mut buf, formatter);
    value.serialize(&mut serializer)?;
    let mut file = fs::File::create(path)?;
    file.write_all(&buf)?;
    Ok(())
}

pub fn read_settings(path: &Path) -> Result<Settings, ConfigError> {
    Ok(serde_json::from_slice(&fs::read(path)?)?)
}

pub fn write_settings(path: &Path, settings: &Settings) -> Result<(), ConfigError> {
    write_json_pretty(path, settings, b"    ")
}

pub fn read_shortcuts(path: &Path) -> Result<Vec<Shortcut>, ConfigError> {
    Ok(serde_json::from_slice(&fs::read(path)?)?)
}

pub fn write_shortcuts(path: &Path, shortcuts: &[Shortcut]) -> Result<(), ConfigError> {
    write_json_pretty(path, &shortcuts, b"    ")
}

pub fn read_last_window_state(path: &Path) -> Result<LastWindowState, ConfigError> {
    Ok(serde_json::from_slice(&fs::read(path)?)?)
}

pub fn write_last_window_state(path: &Path, state: &LastWindowState) -> Result<(), ConfigError> {
    write_json_pretty(path, state, b"    ")
}

const ASSET_MIRRORS: [(&str, &str); 3] = [
    ("themes", "themes"),
    ("kb_layouts", "keyboards"),
    ("fonts", "fonts"),
];

fn mirror_assets(assets_dir: &Path, user_data_dir: &Path) -> Result<(), ConfigError> {
    for (source, target) in ASSET_MIRRORS {
        let source_dir = assets_dir.join(source);
        let target_dir = user_data_dir.join(target);
        fs::create_dir_all(&target_dir)?;
        for entry in fs::read_dir(&source_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                fs::copy(entry.path(), target_dir.join(entry.file_name()))?;
            }
        }
    }
    Ok(())
}

fn update_versions_log(path: &Path, version: &str) -> Result<(), ConfigError> {
    let mut log: VersionsLog = if path.exists() {
        serde_json::from_slice(&fs::read(path)?)?
    } else {
        BTreeMap::new()
    };

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    log.entry(version.to_string())
        .and_modify(|record| record.last_seen = now)
        .or_insert(VersionRecord {
            first_seen: now,
            last_seen: now,
        });

    write_json_pretty(path, &log, b"  ")
}

pub fn initialize(
    user_data_dir: &Path,
    assets_dir: &Path,
    version: &str,
    is_windows: bool,
) -> Result<InitializedConfig, ConfigError> {
    fs::create_dir_all(user_data_dir)?;

    let settings_path = user_data_dir.join("settings.json");
    let shortcuts_path = user_data_dir.join("shortcuts.json");
    let last_window_state_path = user_data_dir.join("lastWindowState.json");
    let versions_log_path = user_data_dir.join("versions_log.json");

    if !settings_path.exists() {
        write_settings(
            &settings_path,
            &Settings::defaults(user_data_dir, is_windows),
        )?;
    }
    if !shortcuts_path.exists() {
        write_shortcuts(&shortcuts_path, &default_shortcuts())?;
    }
    if !last_window_state_path.exists() {
        write_last_window_state(&last_window_state_path, &LastWindowState::default())?;
    }

    mirror_assets(assets_dir, user_data_dir)?;
    update_versions_log(&versions_log_path, version)?;

    Ok(InitializedConfig {
        user_data_dir: user_data_dir.to_path_buf(),
        settings_path,
        shortcuts_path,
        last_window_state_path,
        versions_log_path,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn bundled_assets(root: &Path) -> PathBuf {
        for (dir, files) in [
            ("themes", vec!["tron.json"]),
            ("kb_layouts", vec!["en-US.json"]),
            ("fonts", vec!["Fira Mono.woff2"]),
        ] {
            let dir = root.join(dir);
            fs::create_dir_all(&dir).unwrap();
            for file in files {
                fs::write(dir.join(file), b"asset").unwrap();
            }
        }
        root.to_path_buf()
    }

    #[test]
    fn settings_defaults_match_contract() {
        let value =
            serde_json::to_value(Settings::defaults(Path::new("/data/edex"), false)).unwrap();
        assert_eq!(
            value,
            json!({
                "shell": "bash",
                "shellArgs": "",
                "cwd": "/data/edex",
                "keyboard": "en-US",
                "theme": "tron",
                "termFontSize": 15,
                "audio": true,
                "audioVolume": 1.0,
                "disableFeedbackAudio": false,
                "clockHours": 24,
                "pingAddr": "1.1.1.1",
                "port": 3000,
                "nointro": false,
                "nocursor": false,
                "forceFullscreen": true,
                "allowWindowed": false,
                "excludeThreadsFromToplist": true,
                "hideDotfiles": false,
                "fsListView": false,
                "experimentalGlobeFeatures": false,
                "experimentalFeatures": false
            })
        );
        assert_eq!(
            Settings::defaults(Path::new("/data"), true).shell,
            "powershell.exe"
        );
    }

    #[test]
    fn shortcut_defaults_match_contract() {
        let shortcuts = default_shortcuts();
        assert_eq!(shortcuts.len(), 14);
        let value = serde_json::to_value(&shortcuts).unwrap();
        assert_eq!(
            value[0],
            json!({"type": "app", "trigger": "Ctrl+Shift+C", "action": "COPY", "enabled": true})
        );
        assert_eq!(
            value[11],
            json!({"type": "app", "trigger": "Ctrl+Shift+I", "action": "DEV_DEBUG", "enabled": false})
        );
        assert_eq!(
            value[13],
            json!({"type": "shell", "trigger": "Ctrl+Shift+Alt+Space", "action": "neofetch", "enabled": false, "linebreak": true})
        );
    }

    #[test]
    fn initialize_seeds_mirrors_and_preserves_existing_files() {
        let temp = tempfile::tempdir().unwrap();
        let assets = bundled_assets(&temp.path().join("assets"));
        let user_data = temp.path().join("userData");

        let config = initialize(&user_data, &assets, "2.2.8", false).unwrap();

        assert!(config.settings_path.exists());
        assert!(config.shortcuts_path.exists());
        assert!(config.last_window_state_path.exists());
        assert!(config.versions_log_path.exists());
        assert!(user_data.join("themes").join("tron.json").exists());
        assert!(user_data.join("keyboards").join("en-US.json").exists());
        assert!(user_data.join("fonts").join("Fira Mono.woff2").exists());

        let raw = fs::read_to_string(&config.settings_path).unwrap();
        assert!(raw.starts_with("{\n    \"shell\""));
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&raw).unwrap()["theme"],
            "tron"
        );

        let mut settings = read_settings(&config.settings_path).unwrap();
        settings.theme = "blade".to_string();
        write_settings(&config.settings_path, &settings).unwrap();

        initialize(&user_data, &assets, "2.2.8", false).unwrap();
        assert_eq!(read_settings(&config.settings_path).unwrap().theme, "blade");
    }

    #[test]
    fn versions_log_tracks_first_and_last_seen() {
        let temp = tempfile::tempdir().unwrap();
        let assets = bundled_assets(&temp.path().join("assets"));
        let user_data = temp.path().join("userData");

        let config = initialize(&user_data, &assets, "2.2.8", false).unwrap();
        let log: VersionsLog =
            serde_json::from_slice(&fs::read(&config.versions_log_path).unwrap()).unwrap();
        let first = log["2.2.8"].first_seen;
        assert_eq!(first, log["2.2.8"].last_seen);

        initialize(&user_data, &assets, "2.2.9", false).unwrap();
        let log: VersionsLog =
            serde_json::from_slice(&fs::read(&config.versions_log_path).unwrap()).unwrap();
        assert_eq!(log["2.2.8"].first_seen, first);
        assert!(log.contains_key("2.2.9"));
    }
}
