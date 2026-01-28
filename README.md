# MP3YouTube

A desktop/mobile app for managing MP3 playlists from YouTube. Built with Tauri and Rust.

## Features

- **Device Detection**: Automatically detect USB-mounted MP3 players
- **Playlist Management**: Create, delete, and manage playlists locally
- **YouTube Integration**: Download playlists from YouTube as MP3 files
- **Device Sync**: Sync playlists to connected MP3 devices
- **Cross-Platform**: Works on desktop (Windows, macOS, Linux) and mobile (iOS, Android)

## Technology Stack

- **Framework**: Tauri 2.x (Rust-based cross-platform app framework)
- **Frontend**: Leptos (Rust-based reactive web framework)
- **Backend**: Pure Rust modules for device detection, file management, and YouTube downloading

## Project Structure

```
mp3youtube/
├── crates/
│   ├── mp3youtube-core/     # Core library (device detection, playlist management, YouTube)
│   └── mp3youtube-ui/       # Leptos UI components
├── src-tauri/               # Tauri application
│   ├── src/
│   │   ├── main.rs          # Application entry point
│   │   └── commands.rs      # Tauri commands (IPC handlers)
│   └── tauri.conf.json      # Tauri configuration
├── dist/                    # Frontend build output
└── Cargo.toml               # Workspace configuration
```

## Development

### Prerequisites

- Rust stable (1.93+)
- Tauri CLI (`cargo install tauri-cli --version "^2"`)

### Building

```bash
# Check the project compiles
cargo check

# Run tests
cargo test --workspace

# Run clippy
cargo clippy --workspace

# Build the application
cargo tauri build
```

### Running in Development

```bash
cargo tauri dev
```

## Configuration

### Clippy Settings

The project uses strict Clippy settings:
- `unwrap_used = "deny"` - No bare unwraps
- `unsafe_code = "forbid"` - No unsafe code
- Pedantic and nursery lints enabled as warnings

### Code Coverage

Configured for `cargo tarpaulin` with 90% coverage target:

```bash
cargo tarpaulin --config tarpaulin.toml
```

## License

MIT
