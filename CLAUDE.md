# CLAUDE.md - AI Assistant Guide for draw.io Tauri

## Project Overview

Draw.io Tauri is a Tauri-based desktop application that wraps the core draw.io diagramming editor (included as a git submodule). It enables creating flowcharts, UML diagrams, and more, with a security-first design that isolates diagram data from the internet.

**Repository:** https://github.com/lumilla/drawio-tauri
**License:** Apache 2.0
**Current Version:** 29.7.11

## Quick Reference

```bash
# Clone (MUST be recursive for submodule)
git clone --recursive https://github.com/jgraph/drawio-desktop.git

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
drawio-desktop/
├── src-tauri/
│   ├── Cargo.toml             # Rust dependencies
│   ├── tauri.conf.json        # Tauri window/bundle/plugin config
│   ├── build.rs               # Tauri build script
│   ├── capabilities/
│   │   └── default.json       # Permission capabilities for the main window
│   ├── icons/                 # App icons (RGBA PNG, ICO, ICNS)
│   └── src/
│       ├── main.rs            # Entry point (Windows subsystem config)
│       └── lib.rs             # Tauri setup, plugins, IPC commands
├── drawio/                    # Git submodule - core draw.io editor
│   └── src/main/webapp/       # Static webapp served by Tauri webview
├── .github/workflows/
│   └── build.yml              # CI/CD: Linux + Windows builds, releases
├── package.json               # Tauri JS plugin deps (@tauri-apps/*)
├── sync.cjs                   # Version sync: drawio/VERSION → package.json, Cargo.toml, tauri.conf.json
├── build/                     # Icon resources
└── doc/
    └── RELEASE_PROCESS.md
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
| `src-tauri/src/lib.rs` | Tauri app setup, plugins (dialog, fs, shell), IPC commands |
| `src-tauri/src/main.rs` | Entry point, Windows subsystem configuration |
| `src-tauri/tauri.conf.json` | Window config, bundle targets, plugin settings, file associations |
| `src-tauri/capabilities/default.json` | Security permissions for the main window |
| `sync.cjs` | Pre-build script that syncs version from `drawio/VERSION` |
| `.github/workflows/build.yml` | CI/CD pipeline for Linux and Windows |

## Code Style

- **Rust:** Standard Rust conventions (rustfmt)
- **JavaScript:** ES6 modules, tab indentation, Allman brace style
- **camelCase** for JS variables, **snake_case** for Rust

## Git Conventions

### Branches
- `dev` - Main development branch (PR target)
- `release` - Production releases

### Commit Messages
- Lowercase sentence style without period
- Issue references: `[jgraph/drawio-desktop#XXXX]`

### Version Tags
Format: `v{MAJOR}.{MINOR}.{PATCH}` (e.g., `v29.5.2`)
Tags trigger CI/CD build workflows.

## Build Process

1. **Sync version:** `npm run sync` reads `drawio/VERSION` and updates `package.json`, `Cargo.toml`, `tauri.conf.json`
2. **Install:** `npm ci` for JS deps
3. **Build:** `cargo tauri build` compiles Rust + bundles webapp into platform-specific installers

### Bundle Targets
| Platform | Formats |
|----------|---------|
| Linux | AppImage, .deb |
| Windows | MSI |

## Architecture Notes

### How It Works
Tauri creates a native window with the system's webview (WebKit2GTK on Linux, WebView2 on Windows). The draw.io webapp from `drawio/src/main/webapp/` is served directly into this webview with URL parameters that configure it for desktop/offline mode (`mode=device`).

### Security Model
- Tauri capabilities system restricts what the webview can access
- File system, dialog, and shell permissions declared in `capabilities/default.json`
- No external data transmission of diagram data

### IPC Commands (Rust → JS)
Defined in `src-tauri/src/lib.rs`:
- `read_file_bytes` – Read raw bytes from a file path
- `write_file_bytes` – Write raw bytes to a file path
- `get_app_version` – Return the app version string

### Tauri Plugins
- `tauri-plugin-dialog` – Native file open/save dialogs
- `tauri-plugin-fs` – File system access
- `tauri-plugin-shell` – Open URLs in default browser

## CI/CD Workflow

| Workflow | Trigger | Purpose |
|----------|---------|---------|
| `build.yml` | Push/PR to main/dev, version tags | Build Linux + Windows, create draft release on tag |

## Important Constraints

1. **Recursive clone required** – drawio submodule must be initialized
2. **Run `npm run sync` before building** – Updates version across all config files
3. **Version source of truth** – `drawio/VERSION`, not package.json
4. **Icons must be RGBA PNG** – Tauri requires RGBA format for icons
5. **Node 20+ required** for sync script
6. **Rust stable required** for Tauri compilation
7. **Linux system deps required** – WebKit2GTK, GTK3, etc. (see DEVELOPMENT.md)

## Development Tips

- Use `cargo tauri dev` for hot-reload development
- Tauri dev mode opens the webapp with system webview
- Main process (Rust) logs to console; check terminal for errors
- Use `RUST_BACKTRACE=1` for detailed error traces

## Key Dependencies

### Rust (Cargo.toml)
| Package | Purpose |
|---------|---------|
| `tauri` | Desktop app framework |
| `tauri-plugin-dialog` | Native dialogs |
| `tauri-plugin-fs` | File system access |
| `tauri-plugin-shell` | Shell/URL opening |
| `serde` / `serde_json` | Serialization |

### JavaScript (package.json)
| Package | Purpose |
|---------|---------|
| `@tauri-apps/api` | Tauri JS API |
| `@tauri-apps/plugin-dialog` | Dialog plugin JS bindings |
| `@tauri-apps/plugin-fs` | FS plugin JS bindings |
| `@tauri-apps/plugin-shell` | Shell plugin JS bindings |

## Electron Bridge (Desktop Mode)

The app uses `electron_bridge.js` (injected via `with_initialization_script()`) to make the webapp detect as a desktop app and maps `electron.*` API calls to Tauri IPC. This allows `ElectronApp.js` to load, enabling native file dialogs, menus, and other desktop features.

### IPC Commands (Bridge)

Defined in `src-tauri/src/lib.rs`:

| Command | Purpose |
|---------|---------|
| `electron_request` | Handles `electron.request()` — file I/O, dialogs, file stats, drafts, plugins |
| `electron_message` | Handles `electron.sendMessage()` — fullscreen, zoom, devtools, app events |

### Bridge Architecture

```
drawio webapp (ElectronApp.js)
    → electron.request() / electron.sendMessage()
    → electron_bridge.js (init script, injected before page load)
    → Tauri IPC invoke()
    → Rust electron_request / electron_message handlers
    → Tauri plugins (dialog, fs, shell) / native Rust fs
```

### Unimplemented Features (TODO)

| Feature | Status | Notes |
|---------|--------|-------|
| **Print** | Not implemented | Electron uses `BrowserWindow.webContents.printToPDF()` + hidden window. Tauri/WebView2 has no equivalent. Options: (1) hidden `<iframe>` + SVG → `iframe.contentWindow.print()`, (2) Rust-side `resvg` + `printpdf` for native PDF → system print dialog |
| **Export to PDF/PNG/SVG via server** | Stub (returns error) | Electron spawns hidden BrowserWindow with `export.js`. Tauri needs alternative rendering pipeline |
| **System clipboard (image)** | Implemented | `writeImage`/`readImage` via `navigator.clipboard` + `ClipboardItem` API in bridge |
| **File watching** | Implemented | Polling-based (std::thread, 2s interval), emits `file-watch-changed` events via `window.eval()` |
| **Plugin management** | Not implemented | `installPlugin`/`uninstallPlugin`/`getPluginFile` return error |
| **System font enumeration** | Implemented | Windows: `reg query` fonts registry. Linux: `fc-list`. macOS: `fc-list` + fallback to enumerating `/System/Library/Fonts`, `/Library/Fonts` |
| **Spell check toggle** | Implemented | Persisted to `prefs.json`. WebView2 native spellcheck via MutationObserver overriding draw.io's hardcoded `spellcheck="false"` on text elements |
| **Google Fonts toggle** | Implemented | Persisted to `prefs.json`. URL param `isGoogleFontsEnabled` set dynamically at startup |
| **Store backup toggle** | Stub | `toggleStoreBkp` no-op |
| **Check for updates** | Implemented | `ureq` fetches `api.github.com/repos/rede97/drawio-tauri/releases/latest`, semver comparison, dialog with "Download"→opens release page in browser |
| **VSDX import via IPC** | Not implemented | Electron uses node.js to parse VSDX; Tauri needs Rust-side parser |
| **Command-line file args** | Partial | `args-obj` emitted on startup but file-to-open flow not fully tested |
| **Window zoom (native)** | Via CSS | `zoomIn`/`zoomOut`/`resetZoom` use `document.body.style.zoom` (not native) |

### Prefs persistence

User preferences stored as JSON at `<data_local_dir>/drawio/prefs.json`. Fields:

- `googleFonts` (bool) — controlled by Extras → Google Fonts
- `spellCheck` (bool) — controlled by Extras → Spell Check

Toggling either shows "restart required" alert (webapp-side). New values take effect on next launch.
