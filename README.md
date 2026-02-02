<p align="center">
  <img src="src-tauri/icons/icon.svg" alt="Youtun4 Logo" width="128" height="128">
</p>

# Youtun4

[![CI](https://github.com/vincentserpoul/youtun4/actions/workflows/ci.yml/badge.svg)](https://github.com/vincentserpoul/youtun4/actions/workflows/ci.yml)
[![Security](https://github.com/vincentserpoul/youtun4/actions/workflows/security.yml/badge.svg)](https://github.com/vincentserpoul/youtun4/actions/workflows/security.yml)
[![codecov](https://codecov.io/gh/vincentserpoul/youtun4/graph/badge.svg?token=91PBO9UKNN)](https://codecov.io/gh/vincentserpoul/youtun4)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-1.93%2B-orange.svg)](https://www.rust-lang.org/)
[![Tauri](https://img.shields.io/badge/tauri-2.x-blue.svg)](https://tauri.app/)

A desktop app for managing MP3 playlists from YouTube. Built with Tauri and Rust.

## Features

- **Device Detection**: Automatically detect USB-mounted MP3 players
- **Playlist Management**: Create, delete, and manage playlists locally
- **YouTube Integration**: Download playlists from YouTube as MP3 files
- **Device Sync**: Sync playlists to connected MP3 devices
- **Cross-Platform**: Works on Windows, macOS, and Linux

## Technology Stack

- **Framework**: Tauri 2.x (Rust-based cross-platform app framework)
- **Frontend**: Leptos (Rust-based reactive web framework compiled to WASM)
- **Backend**: Pure Rust modules for device detection, file management, and YouTube downloading

## Code Quality

| Metric         | Status                                                                                                                                                                      |
| -------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Build          | [![CI](https://github.com/vincentserpoul/youtun4/actions/workflows/ci.yml/badge.svg)](https://github.com/vincentserpoul/youtun4/actions/workflows/ci.yml)                   |
| Security Audit | [![Security](https://github.com/vincentserpoul/youtun4/actions/workflows/security.yml/badge.svg)](https://github.com/vincentserpoul/youtun4/actions/workflows/security.yml) |
| Code Coverage  | [![codecov](https://codecov.io/gh/vincentserpoul/youtun4/branch/main/graph/badge.svg)](https://codecov.io/gh/vincentserpoul/youtun4)                                        |
| Clippy         | Zero warnings policy with pedantic lints                                                                                                                                    |
| Unsafe Code    | Forbidden via `#![forbid(unsafe_code)]`                                                                                                                                     |

## Project Structure

```text
youtun4/
├── crates/
│   ├── youtun4-core/        # Core library (device detection, playlist management, YouTube)
│   └── youtun4-ui/          # Leptos UI components (WASM)
├── src-tauri/               # Tauri application
│   ├── src/
│   │   ├── main.rs          # Application entry point
│   │   └── commands/        # Tauri commands (IPC handlers)
│   └── tauri.conf.json      # Tauri configuration
├── dist/                    # Frontend build output
└── Cargo.toml               # Workspace configuration
```

## Development

### Prerequisites

- Rust stable (1.93+)
- Tauri CLI: `cargo install tauri-cli`
- Trunk (for WASM builds): `cargo install trunk`
- WASM target: `rustup target add wasm32-unknown-unknown`

### Building

```bash
# Check the project compiles
cargo check

# Run tests
cargo test --workspace

# Run clippy with all lints
cargo clippy --all-targets --all-features

# Build the frontend
trunk build --release

# Build the full application
cargo tauri build
```

### Running in Development

```bash
cargo tauri dev
```

### Code Coverage

Generate coverage reports locally:

```bash
# Install tarpaulin
cargo install cargo-tarpaulin

# Generate coverage report
cargo tarpaulin --workspace --out Html --output-dir coverage
```

## Configuration

### Clippy Settings

The project uses strict Clippy settings defined in `Cargo.toml`:

- `unsafe_code = "forbid"` - No unsafe code allowed
- `unwrap_used = "deny"` - No bare `.unwrap()` calls
- `expect_used = "warn"` - Prefer proper error handling
- Pedantic and nursery lints enabled

### CI/CD

The project uses GitHub Actions for:

- **CI Pipeline** (`ci.yml`): Format check, clippy, build & test on all platforms
- **Security Pipeline** (`security.yml`): Dependency audit, license check, secret detection
- **Release Pipeline** (`release.yml`): Automated builds for Windows, macOS, and Linux

## Releases

Pre-built binaries are available on the [Releases](https://github.com/vincentserpoul/youtun4/releases) page:

| Platform | Format                                           |
| -------- | ------------------------------------------------ |
| Windows  | `.msi`, `.exe` (NSIS)                            |
| macOS    | `.dmg` (Universal binary: Intel + Apple Silicon) |
| Linux    | `.deb`, `.rpm`, `.AppImage`                      |

## License

MIT
