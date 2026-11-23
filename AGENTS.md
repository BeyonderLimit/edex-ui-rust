# AGENTS.md

## Project shape
- Canonical app is the Rust/Tauri workspace at the repo root: `rust/core` (config defaults/mirroring), `rust/pty` (portable-pty/WebSocket bridge), `rust/sysinfo` (systeminformation-call implementation), and `rust/app` (Tauri 2 binary with frontend in `rust/app/frontend`).
- Legacy Electron app (archived Oct 2021) is under `legacy/` (`legacy/src`, `legacy/package.json`, `legacy/package-lock.json`). Kept for reference only; root package files no longer drive the app.
- Rust port scope is Core MVP: terminal tabs, settings/themes, CPU/RAM/network/process monitors. Deferred: globe, PDF/media, on-screen keyboard, GeoIP.
- `PORTING_CONTRACTS.md` records the PTY/IPC/config contracts the port must preserve; `node scripts/verify-porting-contracts.js` checks them against `legacy/src`. The Tauri frontend provides `window.si` and `window.ipcRenderer` compatibility shims in `rust/app/frontend/main.js`.

## Commands
- Rust: run all Cargo commands from the repo root: `cargo test --workspace`, `cargo build -p edex-ui`, `cargo run -p edex-ui`. With `cargo-tauri` 2.x installed, build a Linux `.deb` via `cargo tauri build --config rust/app/tauri.conf.json --bundles deb --ci`; output lands in `target/release/bundle/deb/`.
- Tauri needs `libwebkit2gtk-4.1-dev` (plus appindicator/rsvg/xdo) on Linux. Icons in `rust/app/icons/` are generated RGBA PNGs — the repo's palette-based `media/linuxIcons/32x32.png` is rejected by `tauri-build`.
- Verify from the repo root with `cargo test --workspace`, `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `node scripts/verify-porting-contracts.js`.
- The Tauri frontend mirrors the legacy three-column cockpit: left system modules, center xterm terminals, right network modules, and a collapsible debug panel for settings/shortcuts/metrics/process table.
- `rust/app/frontend/vendor/xterm/` contains vendored `@xterm/xterm` 6.0.0 and `@xterm/addon-attach` 0.12.0 UMD builds with their MIT licenses; the Tauri CSP allows them as `self` assets.
- CI runs format/lint/contract/test checks on Linux, then builds `.deb`, `.dmg`, and `.msi` artifacts on Linux/macOS/Windows runners.

## Legacy Electron (reference only)
- Two dependency trees: `legacy/package.json` holds build/dev tooling; `legacy/src/package.json` holds app runtime deps (xterm, node-pty, systeminformation, etc.). Both must be installed for the legacy app to run.
- Main-process entrypoint is `legacy/src/_boot.js`; renderer is `legacy/src/_renderer.js` loaded via `legacy/src/ui.html`; UI pieces live in `legacy/src/classes/*.class.js`.
- `legacy/src/_multithread.js` forks cluster workers that proxy `systeminformation` calls from the renderer. The renderer BrowserWindow uses `nodeIntegration: true` and `contextIsolation: false`, so renderer code uses `require()` directly.
- Legacy dev run: from `legacy/`, `npm run install-linux` (or `install-windows`) then `npm run start`. The `install-*` scripts install BOTH package trees and run `electron-rebuild -f -w node-pty`.
- Legacy build: from `legacy/`, plain `npm install`, then `npm run prebuild-<os>`, then `npm run build-<os>`. electron-builder packages `prebuild-src`, not `src`; output goes to `legacy/dist/`.
- `npm test` in `legacy/` is a Snyk dependency scan, not a unit suite; it requires a global `snyk` CLI plus network access.
- File icons are generated from the `file-icons/*` git submodules by root `file-icons-generator.js` into committed files `legacy/src/assets/icons/file-icons.json` and `legacy/src/assets/misc/file-icons-match.js`. Refresh with `node file-icons-generator.js` from the root.
- `legacy/prebuild-minify.js` minifies JS/CSS but deliberately skips `file-icons-match.js` (issue #446) and all JSON except `icons.json`, so themes, keyboard layouts, and `grid.json` stay human-readable.
- Settings/shortcuts/window-state defaults are written into Electron's `userData` dir by `_boot.js` on first launch; themes, keyboards, and fonts are loaded from both `legacy/src/assets/*` and that user config dir.
- `legacy/prebuild-src/` and `legacy/dist/` are gitignored artifacts. Never edit source there.
