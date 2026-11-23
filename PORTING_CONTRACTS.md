# Porting contracts (Phase 0)

Snapshot of the observable contracts between the Electron main process, the renderer, and the PTY backend. The Rust/Tauri port must reproduce these byte-for-byte. `node scripts/verify-porting-contracts.js` fails if either the source or this document drifts.

Line references point at the archived JS implementation now under `legacy/`, not at the port.

## 1. PTY / WebSocket protocol

Source: `legacy/src/classes/terminal.class.js` (client role lines 3-300, server role lines 302-480).

### Ports

- Base PTY is spawned by `legacy/src/_boot.js:243-268` on `settings.port || 3000`.
- Up to 4 extra PTYs for tabs: `basePort = Number(settings.port || 3000) + 2`, i.e. 3002-3005 by default (`legacy/src/_boot.js:276-283`).
- Renderer connects to `ws://<host>:<port>`, host defaults to `127.0.0.1` (`legacy/src/classes/terminal.class.js:176-179`).
- `http_proxy` / `https_proxy` are deleted from the main-process env so the local WebSockets are not proxied (`legacy/src/_boot.js:56-59`).
- WS server rejects a second client (`this.wss.clients.length >= 1` in `verifyClient`, lines 422-432). One renderer per PTY.

### Renderer -> main IPC: `terminal_channel-<port>`

| Message | Payload | Source |
| --- | --- | --- |
| `Renderer startup` | none; registers `e.sender` as the renderer | 152, 435-443 |
| `Resize` | `cols`, `rows` as **zero-padded 3-character strings** (`80` -> `"080"`, `24` -> `"024"`) | 17-27, 444-453 |

On `Renderer startup` the main process immediately replays `New cwd` (or `Fallback cwd` if tracking is disabled).

### Main -> renderer IPC: `terminal_channel-<port>`

| Message | Payload | Source |
| --- | --- | --- |
| `New cwd` | absolute path string | 373, 438 |
| `Fallback cwd` | `opts.cwd \|\| process.env.PWD`; renderer prefixes display with `"FALLBACK \|-- "` | 380, 441, 159-162 |
| `New process` | process name string, or `""` on lookup failure | 394, 400 |

CWD/process poll runs every 1000 ms (lines 366-407):

- Linux CWD: readlink `/proc/${pid}/cwd`.
- macOS CWD: `lsof -a -d cwd -p ${pid} | tail -1 | awk '{ for (i=9; i<=NF; i++) printf "%s ", $i }'`.
- Process (Linux + macOS): `ps -o comm --no-headers --sort=+pid -g ${pid} | tail -1`.
- Unsupported OSes disable tracking and send `Fallback cwd`.

### WebSocket frames (xterm-addon-attach)

- Client -> server: raw bytes from `socket.send(cmd)` (line 281); `writelr` appends `"\r"` (line 285). No JSON envelope.
- Server -> client: raw PTY output via `ws.send(data)` (line 470). No JSON envelope.
- The renderer's `message` listener does not parse frames; it triggers the stdout sound effect, periodic refit, and (only when `experimentalGlobeFeatures` is on) scans for IP literals (lines 193-214).
- PTY spawned with `require("node-pty")`, args `opts.params` or `["--login"]` on non-Windows, `name: opts.env.TERM || "xterm-256color"`, initial size 80x24, `cwd: opts.cwd || process.env.PWD` (lines 304, 409-415).

## 2. IPC channels

| Channel | Direction | Arguments | Handler |
| --- | --- | --- | --- |
| `log` | renderer -> main | `type`, `content` -> `signale[type](content)` | `legacy/src/_boot.js:41-43`; sent at `legacy/src/_renderer.js:152-153` |
| `ttyspawn` | renderer -> main | `"true"` (unused) | `legacy/src/_boot.js:285-330`; sent at `legacy/src/_renderer.js:553` |
| `ttyspawn-reply` | main -> renderer | `"SUCCESS: "+port` or `"ERROR: max number of ttys reached"` | `legacy/src/_boot.js:296, 328`; received once at `legacy/src/_renderer.js:554` |
| `systeminformation-call` | renderer -> main controller | `method`, `id`, `...args` | `legacy/src/_multithread.js:45-61`; sent at `legacy/src/_renderer.js:196` |
| `systeminformation-reply-<id>` | main -> renderer | result value | `legacy/src/_multithread.js:54, 67`; received at `legacy/src/_renderer.js:190` |
| `getThemeOverride` / `setThemeOverride` | request / set | `null` or theme name | `legacy/src/_boot.js:335-343`; `legacy/src/_renderer.js:69-78, 513` |
| `getKbOverride` / `setKbOverride` | request / set | `null` or layout name | `legacy/src/_boot.js:338-346`; `legacy/src/_renderer.js:80-86, 525` |
| `terminal_channel-<port>` | bidirectional | see section 1 | `legacy/src/classes/terminal.class.js` |

The `systeminformation` controller forks up to 7 cluster workers (`os.cpus().length - 1`, capped at 7) and round-robins calls (legacy/src/_multithread.js:7-42). Calls with more than one argument, or with no workers, run in the main process.

## 3. `systeminformation` surface used by the renderer

`window.si` is a Proxy (`legacy/src/_renderer.js:177-200`): any property access returns a function; the last argument may be a callback, and the function always returns a Promise. Methods called by the ported classes:

`battery`, `blockDevices`, `chassis`, `cpu`, `cpuTemperature`, `currentLoad`, `fsSize`, `mem`, `networkConnections`, `networkInterfaces`, `networkStats`, `processes`, `system`.

## 4. Config files and startup state

Source: `legacy/src/_boot.js:45-167`. All live in Electron's `userData` dir.

`settings.json` defaults (indent 4):

```
shell          win32 ? "powershell.exe" : "bash"
shellArgs      ""
cwd            userData dir
keyboard       "en-US"
theme          "tron"
termFontSize   15
audio          true
audioVolume    1.0
disableFeedbackAudio     false
clockHours     24
pingAddr       "1.1.1.1"
port           3000
nointro        false
nocursor       false
forceFullscreen         true
allowWindowed           false
excludeThreadsFromToplist true
hideDotfiles            false
fsListView              false
experimentalGlobeFeatures false
experimentalFeatures    false
```

`shortcuts.json` defaults (indent 4), array of 14 entries, fields `type`/`trigger`/`action`/`enabled` plus optional `linebreak`:

```
app  Ctrl+Shift+C          COPY          enabled
app  Ctrl+Shift+V          PASTE         enabled
app  Ctrl+Tab              NEXT_TAB      enabled
app  Ctrl+Shift+Tab        PREVIOUS_TAB  enabled
app  Ctrl+X                TAB_X         enabled
app  Ctrl+Shift+S          SETTINGS      enabled
app  Ctrl+Shift+K          SHORTCUTS     enabled
app  Ctrl+Shift+F          FUZZY_SEARCH  enabled
app  Ctrl+Shift+L          FS_LIST_VIEW  enabled
app  Ctrl+Shift+H          FS_DOTFILES   enabled
app  Ctrl+Shift+P          KB_PASSMODE   enabled
app  Ctrl+Shift+I          DEV_DEBUG     disabled
app  Ctrl+Shift+F5         DEV_RELOAD    enabled
shell Ctrl+Shift+Alt+Space neofetch      disabled, linebreak: true
```

`lastWindowState.json`: `{ "useFullscreen": true }` (indent 4).

`versions_log.json`: `{ [version]: { firstSeen: <ms>, lastSeen: <ms> } }` (indent 2); updated on every launch.

Startup also mirrors bundled assets into `userData`: `assets/themes` -> `themes/`, `assets/kb_layouts` -> `keyboards/`, `assets/fonts` -> `fonts/`, overwriting existing files on each launch (`legacy/src/_boot.js:128-153`). Shell is resolved via `which`, and its login environment via `shell-env`, with `TERM=xterm-256color`, `COLORTERM=truecolor`, `TERM_PROGRAM=eDEX-UI`, `TERM_PROGRAM_VERSION=<version>` injected (`legacy/src/_boot.js:223-241`).

`legacy/src/ui.html:5` sets `default-src file: 'unsafe-inline'; img-src file: data:; media-src file:; connect-src ws: file:`. The Tauri port must relax this for its asset protocol and keep `ws:` for the local PTY ports.
