## Setup

### Prerequisites

- Node.js
- Rust and Cargo
- Tauri CLI : `cargo install tauri-cli` or via `cargo binstall tauri-cli`
- Additionally on system libraries for WebKit2GTK, see Tauri docs

### Clone
Drawio itself is a submodule, thus we need to clone recursively to build

```bash
git clone --recursive https://github.com/jgraph/drawio-desktop.git
cd drawio-desktop
```

### Install & Run

```bash
npm install
npm run sync          # Sync version from drawio/VERSION
cargo tauri dev       # Launch in dev mode
```

### Build for release

```bash
npm run sync
cargo tauri build     # Produces platform-specific installers
# Note: Windows installers can only really be made in Windows
```

Built artifacts are placed under `src-tauri/target/release/bundle/`.

## CI/CD

The GitHub Actions workflow (`.github/workflows/build.yml`) builds for Linux
(Ubuntu 22.04, deb + AppImage) and Windows (MSI) on every push/PR. Tagged
pushes (`v*`) automatically create a draft GitHub Release with all artifacts.

The workflow is stolen from lumilla/klecks-tauri