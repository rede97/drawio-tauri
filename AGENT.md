# AGENT.md - AI Assistant Guide for draw.io Tauri

## Project Overview

Draw.io Tauri is a Tauri-based desktop application that wraps the core draw.io diagramming editor (included as a git submodule). It enables creating flowcharts, UML diagrams, and more, with a security-first design that isolates diagram data from the internet.

**Repository:** https://github.com/rede97/drawio-tauri
**License:** Apache 2.0
**Current Version:** 31.1.5

## Quick Reference

```bash
# Clone (MUST be recursive for submodule)
git clone --recursive https://github.com/rede97/drawio-tauri.git

# Install dependencies
npm install

# Sync version before building (required)
npm run sync

# Run application (dev mode)
cargo tauri dev

# Build for release
cargo tauri build
```

## Project Structure

```
drawio-tauri/
├── src-tauri/
│   ├── Cargo.toml             # Rust dependencies
│   ├── tauri.conf.json        # Tauri window/bundle/plugin/updater config
│   ├── build.rs               # Tauri build script
│   ├── capabilities/
│   │   └── default.json       # Permission capabilities for the main window
│   ├── icons/                 # App icons (RGBA PNG, ICO, ICNS)
│   └── src/
│       ├── main.rs            # Entry point (Windows subsystem config)
│       ├── lib.rs             # App assembly: plugins, window, managed state (slim)
│       ├── ipc.rs             # electron_request / electron_message IPC commands
│       ├── close_flow.rs      # Close-confirm handshake (Save/Discard/Cancel)
│       ├── drafts.rs          # Draft/backup file conventions (.$name.dtmp/.bkp)
│       ├── prefs.rs           # Persisted user preferences (prefs.json)
│       ├── fonts.rs           # System font enumeration
│       ├── watcher.rs         # Polling-based file watching
│       ├── update.rs          # Auto-update checks (tauri-plugin-updater)
│       ├── windows.rs         # Editor window creation + bookkeeping (multi-window)
│       ├── export.rs          # Local export pipeline (hidden export3.html renderer)
│       └── electron_bridge.js # Init script: Electron→Tauri IPC bridge for the webapp
├── drawio/                    # Git submodule - core draw.io editor (jgraph/drawio, dev branch)
│   └── src/main/webapp/       # Static webapp served by Tauri webview
├── temp/                      # Local reference clones (gitignored)
│   ├── drawio/                #   jgraph/drawio @ 31.1.5
│   └── drawio-desktop-ref/    #   jgraph/drawio-desktop (official Electron main process)
├── scripts/
│   └── release.ps1            # CI-only release helper: sync version, tag, push (CI signs + publishes)
├── .github/workflows/
│   └── build.yml              # CI/CD: Linux + Windows builds, releases
├── package.json               # Tauri JS plugin deps (@tauri-apps/*)
├── sync.cjs                   # Version sync: drawio/VERSION → package.json, Cargo.toml, tauri.conf.json
├── build/                     # Icon resources
└── doc/
    ├── RELEASE_PROCESS.md
    └── FEATURE_GAP.md         # Unimplemented features vs drawio-desktop, with reasons
```

## Tech Stack

- **Runtime:** Tauri 2.x (Rust backend + system webview)
- **Language:** Rust (backend), JavaScript (draw.io webapp)
- **Build Tool:** cargo tauri (via tauri-cli)
- **Frontend:** Pre-built static draw.io webapp (no bundler needed)
- **Package Manager:** npm (JS deps) + cargo (Rust deps)

## Key Files

| File | Purpose |
|------|---------|
| `src-tauri/src/lib.rs` | App assembly: plugin registration, window creation, managed state wiring |
| `src-tauri/src/ipc.rs` | `electron_request` / `electron_message` IPC command handlers |
| `src-tauri/src/close_flow.rs` | Close-confirm handshake state machine (Save/Discard/Cancel) |
| `src-tauri/src/drafts.rs` | Draft (`.$name.dtmp`) and backup (`.$name.bkp`) file helpers |
| `src-tauri/src/prefs.rs` | Preferences persisted to `prefs.json` |
| `src-tauri/src/fonts.rs` | System font enumeration |
| `src-tauri/src/watcher.rs` | Polling file-watch thread |
| `src-tauri/src/update.rs` | Startup + manual update checks via tauri-plugin-updater |
| `src-tauri/src/windows.rs` | Editor window creation, labels, args routing (multi-window, single-instance) |
| `src-tauri/src/export.rs` | Local export pipeline: hidden export3.html renderer window, print/svg/xml |
| `src-tauri/src/electron_bridge.js` | Init script injected before page load; maps `electron.*` webapp API to Tauri IPC |
| `src-tauri/tauri.conf.json` | Window config, bundle targets, plugin settings, updater endpoint/pubkey, file associations |
| `src-tauri/capabilities/default.json` | Security permissions for the main window |
| `sync.cjs` | Pre-build script that syncs version from `drawio/VERSION` |
| `scripts/release.ps1` | CI-only release helper: sync version, tag `vX.Y.Z`, push (CI signs + publishes) |
| `.github/workflows/build.yml` | CI/CD pipeline for Linux and Windows |

## Code Style

- **Rust:** Standard Rust conventions (rustfmt)
- **JavaScript:** ES6 modules, tab indentation, Allman brace style (match draw.io webapp style in the bridge)
- **camelCase** for JS variables, **snake_case** for Rust

## Git Conventions

### Branches
- `dev` - Main development branch (PR target)
- `release` - Production releases

### Commit Messages
- Lowercase sentence style without period
- Issue references: `[jgraph/drawio-desktop#XXXX]`

### Version Tags
Format: `v{MAJOR}.{MINOR}.{PATCH}` (e.g., `v31.1.5`)
Tags trigger CI/CD build workflows.

## Build Process

1. **Sync version:** `npm run sync` reads `drawio/VERSION` and updates `package.json`, `Cargo.toml`, `tauri.conf.json`
2. **Install:** `npm ci` for JS deps
3. **Build:** `cargo tauri build` compiles Rust + bundles webapp into platform-specific installers
   (also runs `npm run sync` via `beforeBuildCommand`)

### Bundle Targets
| Platform | Formats |
|----------|---------|
| Windows | NSIS (.exe), MSI |
| Linux | AppImage, .deb |

## Architecture Notes

### How It Works
Tauri creates a native window with the system's webview (WebKit2GTK on Linux, WebView2 on Windows). The draw.io webapp from `drawio/src/main/webapp/` is served directly into this webview with URL parameters that configure it for desktop/offline mode (`mode=device`).

### Security Model
- Tauri capabilities system restricts what the webview can access
- File system, dialog, and shell permissions declared in `capabilities/default.json`
- No external data transmission of diagram data

### IPC Commands (Rust → JS)
Defined in `src-tauri/src/ipc.rs`:
- `read_file_bytes` – Read raw bytes from a file path
- `write_file_bytes` – Write raw bytes to a file path
- `getapp_version` – Return the app version string
- `electron_request` – Handles `electron.request()` from the webapp
- `electron_message` – Handles `electron.sendMessage()` from the webapp

### Tauri Plugins
- `tauri-plugin-dialog` – Native file open/save/message dialogs
- `tauri-plugin-fs` – File system access
- `tauri-plugin-shell` – Open URLs in default browser
- `tauri-plugin-opener` – Open URLs/files with system handler
- `tauri-plugin-updater` – Signed auto-update checks + install (see below)
- `tauri-plugin-window-state` – Persist/restore window size and position
- `tauri-plugin-single-instance` – Forward second-launch file args to a new window

## Auto-Update

Implemented with `tauri-plugin-updater` (minisign-signed artifacts):

- **Startup check**: silent background check ~5s after launch (matches official behavior when `disableUpdate=0`)
- **Manual check**: Help → Check for Updates (`checkForUpdates` channel) always reports the outcome
- **Flow**: fetch `latest.json` from the latest GitHub release → newer version → confirm dialog → download + verify signature → install (NSIS passive mode) → offer restart (`tauri::process::restart`)

### Configuration

| Item | Location |
|------|----------|
| Endpoint | `tauri.conf.json` → `plugins.updater.endpoints` → `.../releases/latest/download/latest.json` |
| Public key | `tauri.conf.json` → `plugins.updater.pubkey` |
| Private key | **GitHub repo secret `TAURI_SIGNING_PRIVATE_KEY` only** — CI-only by design; GitHub secrets are write-only (no local copy exists). If the secret is lost, existing installs can never auto-update again |
| Signing | CI injects `bundle.createUpdaterArtifacts: true` via `--config` at build time, so local dev builds never need the key |

### Releasing (CI-only)

```powershell
powershell -ExecutionPolicy Bypass -File scripts\release.ps1 -Push
```

The script syncs the version, then tags `vX.Y.Z` and pushes it. The tag push triggers the CI pipeline: build → sign NSIS updater artifacts → generate `latest.json` → publish the GitHub release. Clients auto-update on their next startup after the release is published.

## Electron Bridge (Desktop Mode)

The app uses `electron_bridge.js` (injected via `with_initialization_script()`) to make the webapp detect as a desktop app and maps `electron.*` API calls to Tauri IPC. This allows `ElectronApp.js` (in the webapp) to load, enabling native file dialogs, file watching, drafts, backups, and other desktop features.

### Bridge Architecture

```
drawio webapp (ElectronApp.js)
    → electron.request() / electron.sendMessage()
    → electron_bridge.js (init script, injected before page load)
    → Tauri IPC invoke()
    → Rust electron_request / electron_message handlers
    → Tauri plugins (dialog, fs, shell, opener) / native Rust fs
```

### Implemented `electron.request()` Actions

| Action | Notes |
|--------|-------|
| `getDocumentsFolder` | OS documents directory |
| `dirname` | Parent directory of a path |
| `openExternal` | Open URL via tauri-plugin-opener |
| `showSaveDialog` / `showOpenDialog` | Native dialogs; filters/paths/multi-select mapped |
| `readFile` / `writeFile` | utf-8 / base64 encodings |
| `saveFile` | Writes file, creates `.$name.bkp` backup first when `storeBkp` pref enabled and file exists; removes legacy `~$name.bkp` after success; returns file stat |
| `fileStat` / `isFileWritable` | size/mtime/birthtime/isFile/isDirectory, readonly check |
| `getFileDrafts` | Scans sibling `.$name.dtmp` / `.$name_N.dtmp` drafts (ports legacy `~$` prefix); entries include `data` content |
| `saveDraft` | Writes draft beside the file as `.$name[.dtmp|_N.dtmp]`, sets hidden attribute on Windows, returns draft path |
| `getBkpFile` | Reads `.$name.bkp` (fallback `~$name.bkp`); returns `{data, created, modified, path}` or `null` |
| `deleteFile` | Remove file if it exists |
| `watchFile` / `unwatchFile` | Polling-based file watching (2s interval, std::thread) |
| `getLocalFonts` | Windows: registry query. Linux: `fc-list`. macOS: `fc-list` + font dir fallback |
| `clipboardAction` | Handled JS-side in the bridge via `navigator.clipboard` (readText/writeText/readImage/writeImage) |
| `isPluginsEnabled` | Returns `true` |
| `exit` | File → Exit; closes the window through the normal close-confirm flow |
| `getPluginFile` / `installPlugin` / `uninstallPlugin` | Stub (returns error) — plugin management not implemented |
| `checkFileExists` | Joins `pathParts`, returns `{exists, path}` |
| `windowAction` | minimize/maximize/unmaximize/isMaximized; `close` goes through the close-confirm flow |
| `isFullscreen` | Returns current fullscreen state |

### Implemented `electron.sendMessage()` Channels

| Channel | Notes |
|---------|-------|
| `toggleFullscreen` | Native fullscreen toggle |
| `openDevTools` | Debug builds only |
| `zoomIn` / `zoomOut` / `resetZoom` | CSS `document.body.style.zoom` (not native) |
| `app-load-finished` | Emits `args-obj` (command-line file args) to the webapp; overrides Help → Website link to fork homepage |
| `checkForUpdates` | Signed update check via tauri-plugin-updater (see Auto-Update section) |
| `toggleGoogleFonts` / `toggleSpellCheck` | Persisted to `prefs.json`; take effect on next launch |
| `toggleStoreBkp` | Persisted to `prefs.json`; affects subsequent saves immediately |
| `newfile` | Opens a new editor window (multi-window; args go to the first window only) |
| `isModified-result` / `saveAndClose-result` / `draftRemoved` | Close-confirm handshake (see below) |

### Close-Confirm Handshake (mirrors drawio-desktop)

```
window close / File → Exit
  → Rust: prevent_close, generate unique id, eval __tauriCloseCheck(id)
  → webapp 'isModified' listener replies sendMessage('isModified-result', {uniqueId, isModified, draftPath})
  → Rust validates uniqueId against pending close id:
      not modified → destroy window
      modified → 3-button dialog (Save / Discard Changes / Cancel):
        Save    → eval __tauriSaveAndClose(id)
                  → webapp saves (Save As dialog for new files)
                  → sendMessage('saveAndClose-result', {uniqueId, success})
                  → success: destroy; failure/cancel: stay open
        Discard → draftPath != null ? delete draft file directly
                                    : eval __tauriRemoveDraft() → webapp 'removeDraft'
                                              → sendMessage('draftRemoved') → destroy
        Cancel  → abort close, clear pending id
```

Rust→JS push functions exposed by the bridge: `__tauriCloseCheck(id)`, `__tauriSaveAndClose(id)`, `__tauriRemoveDraft()`, `__tauriFileChanged(payload)`, `__tauriArgsObj(payload)`, `__tauriRender(args)`, `__tauriGetSvgData()`, `__tauriExportReply(channel, data)`.

### Multi-Window & Single Instance

- File → New Window (`newfile`) creates another editor window (`win-N` labels); every window gets the bridge script and its own close-confirm handshake (`CloseState` keyed by window label)
- File-watch events route to the owning window; watches of destroyed windows are dropped
- Command-line file args go to the first window; additional windows start with the splash
- Single instance (mirrors drawio-desktop): a second launch — e.g. double-clicking another `.drawio` — spawns a new window in the running instance and forwards the file via queued `args-obj`

### Local Export Pipeline (print/svg/xml)

Mirrors drawio-desktop's `exportDiagram()` with a hidden `export-N` window loading `export3.html`:

```
webapp sendMessage('export', args)
  → Rust spawns hidden renderer window (same bridge script)
  → renderer signals 'export-renderer-ready' → Rust evals __tauriRender(args)
  → export.js renders pages, replies 'render-finished' (bounds-checked)
  → print: window shown, @page CSS + body zoom injected, window.print(),
           'print-after' (afterprint) ends the job
    svg:   __tauriGetSvgData() → 'svg-data' reply
    xml:   'xml-data' reply (renderer pushes on its own)
  → Rust forwards 'export-success'/'export-error' to the requesting window
```

Print approximates Electron's `printToPDF` options with CSS: `@page size` ≈ preferCSSPageSize, `body zoom` ≈ scaleFactor = 100/pageScale [jgraph/drawio#5540]. png/jpg/pdf **file** output needs webview capture/printToPDF equivalents Tauri does not expose; those formats still return an error.

### File Conventions (mirror drawio-desktop)

| Convention | Meaning |
|-----------|---------|
| `.$name.dtmp`, `.$name_N.dtmp` | Autosave drafts beside the original file (hidden on Windows) |
| `.$name.bkp` | Backup of the last good save, written before each overwrite (pref-gated) |
| `~$name.dtmp` / `~$name.bkp` | Legacy prefixes; ported/cleaned up automatically |

### Prefs persistence

User preferences stored as JSON at `<data_local_dir>/drawio/prefs.json`. Fields:

- `googleFonts` (bool, default false) — Extras → Google Fonts
- `spellCheck` (bool, default false) — Extras → Spell Check
- `storeBkp` (bool, default **true**) — Extras → Store Backup Files

Google Fonts / Spell Check toggles show a "restart required" alert (webapp-side); new values take effect on next launch. `storeBkp` affects subsequent saves immediately.

### Bridge init script (`electron_bridge.js`)

Injected via `with_initialization_script()` before webapp loads. Besides the core Electron→Tauri IPC bridge, it also:

- **Desktop detection**: Spoofs `navigator.userAgent` with ` electron/tauri draw.io/<version>` (version injected from `CARGO_PKG_VERSION` at startup via `__DRAWIO_VERSION__` placeholder) and sets `window.process.versions.electron`
- **Startup splash**: Clears stale `.draft_*` entries from IndexedDB on launch, ensuring the "create new / open existing" splash dialog always appears instead of auto-restoring a previous unsaved draft
- **System fonts**: Sets `window.DRAWIO_CONFIG = { enableLocalFonts: true }` so the webapp calls `getLocalFonts` and populates the font name input's autocomplete datalist (note: system fonts appear as typing suggestions, not in the font family dropdown)
- **Spellcheck**: When `enableSpellCheck=1` in URL params, a MutationObserver forces `spellcheck="true"` on all text inputs/editable elements, overriding draw.io's hardcoded `spellcheck="false"`

### Feature Status vs Official drawio-desktop

| Feature | Status | Notes |
|---------|--------|-------|
| File open/save dialogs, file I/O, stat | Implemented | Full parity |
| Autosave drafts (`.$name.dtmp`) | Implemented | Official sibling-file convention |
| Backup files (`.$name.bkp`) + recovery (`getBkpFile`) | Implemented | Since 31.1.5 sync |
| Close-confirm handshake (Save/Discard/Cancel) | Implemented | uniqueId-validated, matches official flow |
| File → Exit | Implemented | Since 31.1.5 sync |
| File watching | Implemented | Polling-based (2s), `file-watch-changed` events |
| System font enumeration | Implemented | Registry / fc-list |
| Spell check toggle | Implemented | MutationObserver override |
| Google Fonts toggle | Implemented | URL param at startup |
| Store backup toggle | Implemented | Since 31.1.5 sync |
| Check for updates | Implemented | tauri-plugin-updater, signed artifacts from GitHub releases |
| Command-line file args | Implemented | `args-obj` emitted on `app-load-finished` |
| Multi-window (File → New Window) | Implemented | `win-N` labels, per-window close handshake |
| Single instance | Implemented | Second launch forwards file to a new window |
| Print | Implemented | Hidden export3.html renderer + system print dialog |
| Window size/position memory | Implemented | tauri-plugin-window-state, restores on launch |
| Window zoom | Via CSS | Not native webContents zoom |
| **Export to PDF/PNG/SVG via local pipeline** | Partial | print/svg/xml local via export3.html renderer; png/jpg/pdf file output still stub (needs webview capture) |
| **Plugin management** | Not implemented | `installPlugin`/`uninstallPlugin`/`getPluginFile` return error |
| **Native application menu** | Not implemented | Webapp renders its own menu bar (sufficient) |
| **VSDX import via IPC** | Not implemented | Electron uses node.js to parse VSDX; needs Rust-side parser |
| `windowAction` / `isFullscreen` / `checkFileExists` | Not implemented | Present in official main but not called by webapp 31.1.5 |

> Detailed gap analysis with reasons and possible approaches: [doc/FEATURE_GAP.md](doc/FEATURE_GAP.md)

## CI/CD Workflow

| Workflow | Trigger | Purpose |
|----------|---------|---------|
| `build.yml` | Push/PR to main/dev, version tags | Build Linux + Windows, create draft release on tag |

## Important Constraints

1. **Recursive clone required** – drawio submodule must be initialized (`git submodule update --init --remote drawio` tracks upstream dev)
2. **Run `npm run sync` before building** – Updates version across all config files
3. **Version source of truth** – `drawio/VERSION`, not package.json
4. **Icons must be RGBA PNG** – Tauri requires RGBA format for icons
5. **Node 20+ required** for sync script
6. **Rust stable required** for Tauri compilation
7. **Linux system deps required** – WebKit2GTK, GTK3, etc. (see DEVELOPMENT.md)
8. **Keep IPC surface aligned with webapp** – When bumping the drawio submodule, diff `drawio/src/main/webapp/js/diagramly/ElectronApp.js` actions/messages against `src-tauri/src/ipc.rs` and `electron_bridge.js`; use `temp/drawio-desktop-ref/src/main/electron.js` (official Electron main) as the behavioral reference
9. **Updater private key lives only in the GitHub secret** – `TAURI_SIGNING_PRIVATE_KEY` is the single copy (CI-only policy); secrets are write-only, so if it is deleted, existing installs can never auto-update again

## Development Tips

- Use `cargo tauri dev` for hot-reload development
- Tauri dev mode opens the webapp with system webview
- Main process (Rust) logs to console; check terminal for errors
- Use `RUST_BACKTRACE=1` for detailed error traces
- Draft/backup files are hidden siblings of the saved file (`.$*` / `~$*`); enable "show hidden files" to inspect them

## Key Dependencies

### Rust (Cargo.toml)
| Package | Purpose |
|---------|---------|
| `tauri` | Desktop app framework |
| `tauri-plugin-dialog` | Native dialogs |
| `tauri-plugin-fs` | File system access |
| `tauri-plugin-shell` | Shell/URL opening |
| `tauri-plugin-opener` | Open URLs with system handler |
| `tauri-plugin-updater` | Signed auto-update |
| `tauri-plugin-window-state` | Window size/position persistence |
| `tauri-plugin-single-instance` | Single instance + second-launch forwarding |
| `serde` / `serde_json` | Serialization |
| `dirs` | OS standard directories |
| `base64` | File encoding support |

### JavaScript (package.json)
| Package | Purpose |
|---------|---------|
| `@tauri-apps/api` | Tauri JS API |
| `@tauri-apps/plugin-dialog` | Dialog plugin JS bindings |
| `@tauri-apps/plugin-fs` | FS plugin JS bindings |
| `@tauri-apps/plugin-shell` | Shell plugin JS bindings |
