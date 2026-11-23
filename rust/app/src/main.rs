use edex_config::InitializedConfig;
use edex_pty::{PtyConfig, PtyManager, TerminalEvent, TerminalEventKind};
use edex_sysinfo::SysinfoState;
use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::UNIX_EPOCH;
use tauri::{Emitter, EventId, Listener, Manager, Runtime};

const MAX_TEXT_FILE_BYTES: u64 = 1024 * 1024;

struct AppState {
    config: InitializedConfig,
    pty: Arc<PtyManager>,
    sysinfo: SysinfoState,
    theme_override: Mutex<Option<String>>,
    kb_override: Mutex<Option<String>>,
    _listeners: Vec<EventId>,
}

impl Drop for AppState {
    fn drop(&mut self) {
        for port in self.pty.ports() {
            self.pty.close(port);
        }
    }
}

#[tauri::command]
fn config_dir(state: tauri::State<'_, AppState>) -> String {
    state.config.user_data_dir.to_string_lossy().into_owned()
}

#[tauri::command]
fn exit_app(app: tauri::AppHandle) {
    app.exit(0);
}

#[tauri::command]
fn restart_app(app: tauri::AppHandle) {
    app.restart();
}

#[cfg(target_os = "linux")]
fn is_safe_session_token(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.'))
}

#[tauri::command]
fn logout_user() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("shutdown.exe")
            .arg("/L")
            .spawn()
            .map_err(|error| format!("failed to start Windows logout: {error}"))?;
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("osascript")
            .arg("-e")
            .arg("tell application \"System Events\" to log out")
            .spawn()
            .map_err(|error| format!("failed to start macOS logout: {error}"))?;
        return Ok(());
    }

    #[cfg(target_os = "linux")]
    {
        if let Ok(session) = std::env::var("XDG_SESSION_ID") {
            if is_safe_session_token(&session)
                && std::process::Command::new("loginctl")
                    .args(["terminate-session", &session])
                    .spawn()
                    .is_ok()
            {
                return Ok(());
            }
        }

        let user = std::env::var("USER")
            .or_else(|_| std::env::var("LOGNAME"))
            .unwrap_or_default();
        if is_safe_session_token(&user) {
            if std::process::Command::new("loginctl")
                .args(["terminate-user", &user])
                .spawn()
                .is_ok()
            {
                return Ok(());
            }
            if std::process::Command::new("pkill")
                .args(["-KILL", "-u", &user])
                .spawn()
                .is_ok()
            {
                return Ok(());
            }
        }
        Err("unable to start logout: no supported session manager found".to_string())
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        Err("logout is not supported on this platform".to_string())
    }
}

fn settings_json(config: &InitializedConfig) -> Result<serde_json::Value, String> {
    let settings = edex_config::read_settings(&config.settings_path).map_err(|e| e.to_string())?;
    serde_json::to_value(settings).map_err(|e| e.to_string())
}

fn write_settings_json(
    config: &InitializedConfig,
    value: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let settings: edex_config::Settings =
        serde_json::from_value(value).map_err(|e| e.to_string())?;
    edex_config::write_settings(&config.settings_path, &settings).map_err(|e| e.to_string())?;
    settings_json(config)
}

fn verify_settings_persistence(config: &InitializedConfig) -> Result<serde_json::Value, String> {
    let original = edex_config::read_settings(&config.settings_path).map_err(|e| e.to_string())?;
    let mut probe = original.clone();
    probe.audio_volume = if (probe.audio_volume - 0.25).abs() < f64::EPSILON {
        0.75
    } else {
        0.25
    };
    probe.nointro = !probe.nointro;

    let probe_path = config.user_data_dir.join("settings.verify.json");
    edex_config::write_settings(&probe_path, &probe).map_err(|e| e.to_string())?;
    let read_back = edex_config::read_settings(&probe_path).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(&probe_path);

    if read_back != probe {
        return Err("settings round-trip mismatch".to_string());
    }

    let live = edex_config::read_settings(&config.settings_path).map_err(|e| e.to_string())?;
    if live != original {
        return Err("live settings changed during verification".to_string());
    }

    Ok(serde_json::json!({
        "ok": true,
        "path": config.settings_path.to_string_lossy(),
        "probePath": probe_path.to_string_lossy()
    }))
}

#[tauri::command]
fn get_settings(state: tauri::State<'_, AppState>) -> Result<serde_json::Value, String> {
    settings_json(&state.config)
}

#[tauri::command]
fn save_settings(
    settings: serde_json::Value,
    state: tauri::State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    write_settings_json(&state.config, settings)
}

#[tauri::command]
fn verify_settings(state: tauri::State<'_, AppState>) -> Result<serde_json::Value, String> {
    verify_settings_persistence(&state.config)
}

#[tauri::command]
fn get_shortcuts(state: tauri::State<'_, AppState>) -> Result<serde_json::Value, String> {
    let shortcuts =
        edex_config::read_shortcuts(&state.config.shortcuts_path).map_err(|e| e.to_string())?;
    serde_json::to_value(shortcuts).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_last_window_state(state: tauri::State<'_, AppState>) -> Result<serde_json::Value, String> {
    let state = edex_config::read_last_window_state(&state.config.last_window_state_path)
        .map_err(|e| e.to_string())?;
    serde_json::to_value(state).map_err(|e| e.to_string())
}

#[tauri::command]
async fn spawn_tty(state: tauri::State<'_, AppState>) -> Result<String, String> {
    let port = state.pty.spawn().await.map_err(|e| e.to_string())?;
    Ok(format!("SUCCESS: {}", port))
}

#[tauri::command]
fn resize_tty(
    port: u16,
    cols: u16,
    rows: u16,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    state
        .pty
        .resize(port, cols, rows)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn systeminformation_call(
    method: String,
    args: Vec<serde_json::Value>,
    state: tauri::State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    state.sysinfo.call(&method, &args)
}

#[tauri::command]
fn close_tty(port: u16, state: tauri::State<'_, AppState>) {
    state.pty.close(port);
}

#[tauri::command]
fn log(kind: String, content: String) {
    println!("[{}] {}", kind, content);
}

#[tauri::command]
fn frontend_event(
    event: String,
    detail: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let path = state.config.user_data_dir.join("frontend.log");
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    let timestamp = std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    writeln!(file, "{} {} {}", timestamp, event, detail).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_theme_override(state: tauri::State<'_, AppState>) -> Option<String> {
    state.theme_override.lock().unwrap().clone()
}

#[tauri::command]
fn set_theme_override(theme: Option<String>, state: tauri::State<'_, AppState>) {
    *state.theme_override.lock().unwrap() = theme;
}

#[tauri::command]
fn get_kb_override(state: tauri::State<'_, AppState>) -> Option<String> {
    state.kb_override.lock().unwrap().clone()
}

#[tauri::command]
fn set_kb_override(layout: Option<String>, state: tauri::State<'_, AppState>) {
    *state.kb_override.lock().unwrap() = layout;
}

#[tauri::command]
fn list_themes(state: tauri::State<'_, AppState>) -> Result<Vec<String>, String> {
    list_json_stems(state.config.user_data_dir.join("themes"))
}

#[tauri::command]
fn list_keyboards(state: tauri::State<'_, AppState>) -> Result<Vec<String>, String> {
    list_json_stems(state.config.user_data_dir.join("keyboards"))
}

#[tauri::command]
fn get_theme(state: tauri::State<'_, AppState>) -> Result<serde_json::Value, String> {
    let settings =
        edex_config::read_settings(&state.config.settings_path).map_err(|e| e.to_string())?;
    let name = resolve_theme_name(
        &settings.theme,
        state.theme_override.lock().unwrap().clone(),
    )?;
    let path = state
        .config
        .user_data_dir
        .join("themes")
        .join(format!("{name}.json"));
    let contents = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&contents).map_err(|e| e.to_string())
}

fn resolve_theme_name(theme: &str, override_name: Option<String>) -> Result<String, String> {
    let name = override_name.unwrap_or_else(|| theme.to_string());
    if name.is_empty() || name.contains('/') || name.contains('\\') || name.contains("..") {
        return Err("invalid theme name".to_string());
    }
    Ok(name)
}

#[tauri::command]
fn list_directory(
    path: Option<String>,
    state: tauri::State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let settings =
        edex_config::read_settings(&state.config.settings_path).map_err(|e| e.to_string())?;
    let path = path
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(&settings.cwd));
    let path_text = path.to_string_lossy().into_owned();
    let mut entries = Vec::new();
    for entry in std::fs::read_dir(&path).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let file_type = entry.file_type().map_err(|e| e.to_string())?;
        let metadata = entry.metadata().map_err(|e| e.to_string())?;
        let modified = metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map(|duration| duration.as_secs())
            .unwrap_or(0);
        entries.push(serde_json::json!({
            "name": entry.file_name().to_string_lossy(),
            "path": entry.path().to_string_lossy(),
            "isDir": file_type.is_dir(),
            "isFile": file_type.is_file(),
            "size": metadata.len(),
            "modified": modified
        }));
    }
    entries.sort_by(|a, b| {
        let a_dir = a["isDir"].as_bool().unwrap_or(false);
        let b_dir = b["isDir"].as_bool().unwrap_or(false);
        b_dir.cmp(&a_dir).then_with(|| {
            a["name"]
                .as_str()
                .unwrap_or_default()
                .cmp(b["name"].as_str().unwrap_or_default())
        })
    });
    let disks = match state.sysinfo.call("fsSize", &[]) {
        Ok(serde_json::Value::Array(disks)) => disks,
        _ => Vec::new(),
    };
    Ok(serde_json::json!({
        "path": path_text,
        "parent": path.parent().map(|parent| parent.to_string_lossy().into_owned()),
        "entries": entries,
        "filesystem": filesystem_for_path(&path_text, &disks)
    }))
}

fn read_text_file_at(path: &Path) -> Result<serde_json::Value, String> {
    let metadata = std::fs::metadata(path).map_err(|e| e.to_string())?;
    if !metadata.is_file() {
        return Err("not a file".to_string());
    }
    let file = File::open(path).map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    file.take(MAX_TEXT_FILE_BYTES)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.contains(&0) {
        return Err("binary file preview is not supported".to_string());
    }
    let truncated = metadata.len() > bytes.len() as u64;
    Ok(serde_json::json!({
        "path": path.to_string_lossy(),
        "size": metadata.len(),
        "truncated": truncated,
        "content": String::from_utf8_lossy(&bytes)
    }))
}

fn write_text_file_at(path: &Path, content: &str) -> Result<(), String> {
    if content.len() as u64 > MAX_TEXT_FILE_BYTES {
        return Err("file is too large to save".to_string());
    }
    if path.is_dir() {
        return Err("not a file".to_string());
    }
    std::fs::write(path, content).map_err(|e| e.to_string())
}

#[tauri::command]
fn read_text_file(path: String) -> Result<serde_json::Value, String> {
    read_text_file_at(&PathBuf::from(path))
}

#[tauri::command]
fn write_text_file(path: String, content: String) -> Result<serde_json::Value, String> {
    write_text_file_at(&PathBuf::from(&path), &content)?;
    Ok(serde_json::json!({ "path": path }))
}

fn filesystem_for_path<'a>(
    path: &str,
    disks: &'a [serde_json::Value],
) -> Option<&'a serde_json::Value> {
    let mut best: Option<&serde_json::Value> = None;
    let mut best_len = 0;
    for disk in disks {
        let mount = disk["fs"].as_str().unwrap_or_default();
        if path.starts_with(mount) && mount.len() > best_len {
            best = Some(disk);
            best_len = mount.len();
        }
    }
    best
}

fn list_json_stems(dir: PathBuf) -> Result<Vec<String>, String> {
    let mut names = Vec::new();
    for entry in std::fs::read_dir(dir).map_err(|error| error.to_string())? {
        let path = entry.map_err(|error| error.to_string())?.path();
        if path.is_file()
            && path.extension().and_then(|extension| extension.to_str()) == Some("json")
        {
            if let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) {
                names.push(stem.to_string());
            }
        }
    }
    names.sort();
    Ok(names)
}

fn bundled_assets_dir<R: Runtime>(app: &tauri::App<R>) -> PathBuf {
    let from_resource = app
        .path()
        .resource_dir()
        .map(|path| path.join("frontend/assets"))
        .unwrap_or_default();
    if from_resource.is_dir() {
        return from_resource;
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("frontend/assets")
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .setup(|app| {
            let user_data_dir = app.path().app_data_dir()?;
            let assets_dir = bundled_assets_dir(app);
            let version = app.package_info().version.to_string();
            let initialized =
                edex_config::initialize(&user_data_dir, &assets_dir, &version, cfg!(windows))?;
            let settings = edex_config::read_settings(&initialized.settings_path)?;
            let shell = which::which(&settings.shell)?;
            let cwd = PathBuf::from(&settings.cwd);
            if !cwd.is_dir() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("Configured cwd path does not exist: {}", cwd.display()),
                )
                .into());
            }

            let pty_config = PtyConfig {
                shell: shell.to_string_lossy().into_owned(),
                shell_args: settings.shell_args.clone(),
                cwd,
                base_port: settings.port,
                version: version.clone(),
                extra_env: BTreeMap::new(),
            };
            let app_handle = app.handle().clone();
            let emit = Arc::new(move |event: TerminalEvent| {
                let payload = match event.kind {
                    TerminalEventKind::NewCwd(path) => serde_json::json!(["New cwd", path]),
                    TerminalEventKind::FallbackCwd(path) => {
                        serde_json::json!(["Fallback cwd", path])
                    }
                    TerminalEventKind::NewProcess(name) => serde_json::json!(["New process", name]),
                };
                let channel = format!("terminal_channel-{}", event.port);
                let _ = app_handle.emit(channel.as_str(), payload);
            });
            let pty = Arc::new(PtyManager::new(pty_config, emit));

            let mut listeners = Vec::new();
            for port in pty.ports() {
                let pty = pty.clone();
                listeners.push(
                    app.listen(format!("terminal_channel-{}", port), move |event| {
                        let args: Vec<serde_json::Value> =
                            match serde_json::from_str(event.payload()) {
                                Ok(args) => args,
                                Err(_) => return,
                            };
                        match args.first().and_then(|value| value.as_str()) {
                            Some("Renderer startup") => pty.renderer_started(port),
                            Some("Resize") => {
                                let cols = args
                                    .get(1)
                                    .and_then(|value| value.as_str())
                                    .and_then(|value| value.parse::<u16>().ok());
                                let rows = args
                                    .get(2)
                                    .and_then(|value| value.as_str())
                                    .and_then(|value| value.parse::<u16>().ok());
                                if let (Some(cols), Some(rows)) = (cols, rows) {
                                    let _ = pty.resize(port, cols, rows);
                                }
                            }
                            _ => {}
                        }
                    }),
                );
            }

            let sysinfo = SysinfoState::new().map_err(std::io::Error::other)?;
            app.manage(AppState {
                config: initialized,
                pty,
                sysinfo,
                theme_override: Mutex::new(None),
                kb_override: Mutex::new(None),
                _listeners: listeners,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            config_dir,
            get_settings,
            save_settings,
            get_shortcuts,
            get_last_window_state,
            spawn_tty,
            resize_tty,
            close_tty,
            systeminformation_call,
            log,
            get_theme_override,
            set_theme_override,
            get_kb_override,
            set_kb_override,
            list_themes,
            list_keyboards,
            get_theme,
            list_directory,
            read_text_file,
            write_text_file,
            exit_app,
            restart_app,
            logout_user,
            verify_settings,
            frontend_event
        ])
        .run(tauri::generate_context!())
        .expect("failed to start eDEX-UI");
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn test_config(root: &Path) -> InitializedConfig {
        let user_data_dir = root.join("userData");
        std::fs::create_dir_all(&user_data_dir).unwrap();
        let settings_path = user_data_dir.join("settings.json");
        edex_config::write_settings(
            &settings_path,
            &edex_config::Settings::defaults(&user_data_dir, false),
        )
        .unwrap();
        InitializedConfig {
            user_data_dir: user_data_dir.clone(),
            settings_path,
            shortcuts_path: user_data_dir.join("shortcuts.json"),
            last_window_state_path: user_data_dir.join("lastWindowState.json"),
            versions_log_path: user_data_dir.join("versions_log.json"),
        }
    }

    #[test]
    fn settings_commands_round_trip_persisted_values() {
        let dir = tempfile::tempdir().unwrap();
        let config = test_config(dir.path());

        let mut settings = settings_json(&config).unwrap();
        settings["theme"] = json!("blade");
        settings["shellArgs"] = json!("-l");
        settings["hideDotfiles"] = json!(true);
        let saved = write_settings_json(&config, settings.clone()).unwrap();

        assert_eq!(saved["theme"], "blade");
        assert_eq!(settings_json(&config).unwrap(), settings);
        assert_eq!(
            edex_config::read_settings(&config.settings_path)
                .unwrap()
                .shell_args,
            "-l"
        );
        assert_eq!(
            verify_settings_persistence(&config).unwrap()["ok"],
            json!(true)
        );
    }

    #[test]
    fn tauri_config_enables_global_api() {
        let config: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        assert_eq!(config["app"]["withGlobalTauri"], json!(true));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn session_tokens_are_validated() {
        assert!(is_safe_session_token("2"));
        assert!(is_safe_session_token("sam_1"));
        assert!(!is_safe_session_token(""));
        assert!(!is_safe_session_token("sam;rm -rf /"));
    }

    #[test]
    fn list_json_stems_returns_only_json_stems() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("alpha.json"), "{}").unwrap();
        std::fs::write(dir.path().join("beta.JSON"), "{}").unwrap();
        std::fs::write(dir.path().join("gamma.txt"), "x").unwrap();
        std::fs::create_dir(dir.path().join("delta.json")).unwrap();

        let names = list_json_stems(dir.path().to_path_buf()).unwrap();

        assert_eq!(names, vec!["alpha"]);
    }

    #[test]
    fn list_json_stems_errors_for_missing_directory() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("missing");

        assert!(list_json_stems(missing).is_err());
    }

    #[test]
    fn resolve_theme_name_prefers_override() {
        assert_eq!(
            resolve_theme_name("apollo", Some("tron".to_string())).unwrap(),
            "tron"
        );
    }

    #[test]
    fn resolve_theme_name_rejects_traversal() {
        assert_eq!(
            resolve_theme_name("../secret", None),
            Err("invalid theme name".to_string())
        );
    }

    #[test]
    fn filesystem_for_path_prefers_longest_mount() {
        let disks = vec![
            serde_json::json!({ "fs": "/", "size": 1 }),
            serde_json::json!({ "fs": "/home", "size": 2 }),
        ];

        let selected = filesystem_for_path("/home/sam", &disks).unwrap();

        assert_eq!(selected["size"], 2);
    }

    #[test]
    fn read_text_file_at_returns_text_preview() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("note.txt");
        std::fs::write(&path, "hello\nworld").unwrap();

        let preview = read_text_file_at(&path).unwrap();

        assert_eq!(preview["content"], "hello\nworld");
        assert_eq!(preview["size"], 11);
        assert_eq!(preview["truncated"], false);
    }

    #[test]
    fn read_text_file_at_rejects_binary_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("data.bin");
        std::fs::write(&path, [0, 1, 2]).unwrap();

        assert!(read_text_file_at(&path).is_err());
    }

    #[test]
    fn write_text_file_at_overwrites_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("note.txt");
        std::fs::write(&path, "old").unwrap();

        write_text_file_at(&path, "new").unwrap();

        assert_eq!(std::fs::read_to_string(&path).unwrap(), "new");
    }
}
