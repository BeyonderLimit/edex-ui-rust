# Quickstart

This guide gets the current Rust/Tauri eDEX-UI running from source and packaged as a Linux `.deb`.

## 1. Install prerequisites

### All platforms

- Rust stable with Cargo: <https://rustup.rs/>
- Node.js 22+ (only required for the porting-contract check)
- `cargo-tauri` 2.x (only required for packaging)

```bash
cargo install tauri-cli --version '^2' --locked
```

### Linux system dependencies

On Debian/Ubuntu:

```bash
sudo apt-get update
sudo apt-get install -y libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev patchelf
```

## 2. Run from source

Run every command from the repository root:

```bash
git clone <your-fork-or-repo-url> edex-ui
cd edex-ui
cargo run -p edex-ui
```

The first launch creates and mirrors the app config into Tauri's application data directory. The path is shown in the bottom status line. The generated files include `settings.json`, `shortcuts.json`, themes, keyboard layouts, and window state.

The window opens fullscreen and borderless. Use the `EXIT` button in the shell controls, `Ctrl+Q`, or your desktop's window-close shortcut such as `Alt+F4` to exit. `LOGOUT` (and **Log out** in settings) ends the current desktop user session after confirmation. Settings also include **Reload UI**, **Restart app**, and **Exit app** actions.

On Linux, logout uses `loginctl terminate-session` for `XDG_SESSION_ID`, then `loginctl terminate-user`, with a `pkill -KILL -u <user>` fallback for environments without logind. On Windows it runs `shutdown.exe /L`; on macOS it asks System Events to log out.

## 3. First-run walkthrough

1. Watch or skip the boot intro. To skip it on future launches, open `SETTINGS` and set **Intro** to `SKIP`.
2. Wait for the first terminal tab to connect. Tabs are limited to five and can be switched with `Ctrl+Tab`, `Ctrl+Shift+Tab`, or `Ctrl+1` through `Ctrl+5`.
3. Open `SETTINGS` with the shell control button or `Ctrl+Shift+S` to change theme, keyboard layout, CWD, shell arguments, audio, boot intro, cursor visibility, and filesystem view options. Changes apply as soon as a control changes and are saved automatically; `Save settings` is an explicit confirmation, and `Verify settings` performs a write/read round-trip and reports the active settings path.
4. Open `SHORTCUTS` with `Ctrl+Shift+K` to inspect the configured shortcut JSON.
5. Open the process table by clicking `PROC` in the shell controls or the toplist module.
6. Use `Ctrl+Shift+F` to search files in the current terminal CWD, then press `Enter` to insert the selected path into the active terminal.
7. Click a file in the filesystem panel to open it in the inline text editor, then use **Save to disk** to write changes.
8. Watch the left column: **CPU** shows current load as a percent and a rolling graph, **RAM** shows active/total memory plus swap and a 0–100% bar, **SYSINFO** shows host, OS, kernel, uptime, and date, and **TOP PROCESSES** shows the highest CPU/memory processes. These values refresh about every 1.5 seconds.

The debug panel (`Ctrl+Shift+I`) includes a **Metrics** JSON view. If a system data source fails, its name and error appear in the `errors` array while the other modules keep updating.

## 4. Useful shortcuts

| Shortcut | Action |
| --- | --- |
| `Ctrl+Shift+S` | Settings modal |
| `Ctrl+Shift+K` | Shortcuts modal |
| `Ctrl+Shift+I` | Debug panel |
| `Ctrl+Shift+F` | Fuzzy cwd file finder |
| `Ctrl+Shift+L` | Toggle filesystem list view |
| `Ctrl+Shift+H` | Toggle dotfiles |
| `Ctrl+Tab` / `Ctrl+Shift+Tab` | Next/previous terminal tab |
| `Ctrl+1` … `Ctrl+5` | Focus terminal tab |
| `Ctrl+Shift+C` | Copy terminal selection |
| `Ctrl+Shift+V` | Paste clipboard into terminal |
| `Ctrl+Q` | Exit eDEX-UI |
| `Escape` | Close the open modal |

Shortcuts are loaded from the generated `shortcuts.json` file and can be edited there.

## 5. Verify the build

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
node scripts/verify-porting-contracts.js
```

## 6. Build a Linux package

```bash
cargo tauri build --config rust/app/tauri.conf.json --bundles deb --ci
```

The installer is written to `target/release/bundle/deb/`, for example:

```text
target/release/bundle/deb/eDEX-UI_2.2.8_amd64.deb
```

Install it with:

```bash
sudo apt install ./target/release/bundle/deb/*.deb
```

CI also builds `.dmg` on macOS and `.msi` on Windows; those bundles are normally produced on their respective host runners.

## 7. Troubleshooting

- `webkit2gtk-4.1` or appindicator errors: install the Linux system dependencies above.
- `cargo tauri` not found: install the Tauri 2 CLI with the command in step 1.
- Contract check fails: the script compares the Rust port against `legacy/src`; inspect the failing check output before changing `PORTING_CONTRACTS.md`.
- Buttons or keyboard shortcuts do nothing and status/metrics stay at `loading…`: confirm `app.withGlobalTauri` is `true` in `rust/app/tauri.conf.json`. Without it, `window.__TAURI__` is undefined and the renderer script stops before attaching listeners. A healthy startup appends `script-start`, `app-start`, `settings-verify`, and `metrics-values` to `frontend.log` in the config directory shown in the status line.
- Settings appear not to save: click **Verify settings** in the settings modal. It writes a probe file next to `settings.json`, reads it back, and leaves the live settings unchanged; an error names the failing step or path.
- The intro is too slow: set **Intro** to `SKIP` in settings or edit `nointro` in `settings.json`.
- Audio does not play: confirm **Audio** is `ON`, **Feedback audio** is not muted for feedback sounds, and your desktop allows WebView audio playback.
- CPU/RAM/SYSINFO stay blank: open the debug panel with `Ctrl+Shift+I` and check **Metrics**. A non-empty `errors` array names the failed systeminformation source; other modules continue updating independently. On Linux, verify `/proc` and `/sys` are readable by the user running eDEX-UI.

## 8. More details

- [README.md](README.md) — project overview and build summary
- [AGENTS.md](AGENTS.md) — repository layout and verification commands
- [PORTING_CONTRACTS.md](PORTING_CONTRACTS.md) — PTY, IPC, and config contracts
- `legacy/` — archived Electron implementation, reference only
