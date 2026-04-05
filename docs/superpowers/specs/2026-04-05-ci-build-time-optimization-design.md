# CI Build Time Optimization — Design Spec

## Problem

Release CI builds take ~45 minutes, triggered on tag pushes to main. Breakdown:
- ~10 min installing trunk (cargo install from source)
- ~10 min installing tauri-cli (cargo install from source)
- ~20 min actual compilation (805 crates, fat LTO, codegen-units=1)
- Rest is negligible

## Goal

Reduce release CI build time to ~15 minutes (cold) and ~10 minutes (warm). All optimizations should be local tooling changes testable without GitHub Actions.

## Changes

### 1. Pre-built Binary Downloads with Version Caching

**Files:** `Justfile`, `.cargo/config.toml`

Replace `cargo install` for CLI tools with direct binary downloads from GitHub releases. Each tool is downloaded to `.bin/` (already used for tailwindcss) with version tracking.

**Tools to convert:**
- `trunk` — download from `https://github.com/trunk-rs/trunk/releases`
- `tauri-cli` — download from `https://github.com/nickel-org/cargo-binstall` or use `cargo-binstall` to install pre-built binaries
- `cargo-deny` — download from `https://github.com/EmbarkStudios/cargo-deny/releases`
- `cargo-machete` — download from `https://github.com/bnjbvr/cargo-machete/releases`
- `cargo-tarpaulin` — download from `https://github.com/xd009642/tarpaulin/releases`

**Version caching strategy:**
- Store downloaded binaries in `.bin/` with a version marker file (e.g., `.bin/.trunk-version` containing "0.21.14")
- On install, check if marker matches desired version — skip download if already present
- In CI, cache `.bin/` directory keyed by a hash of tool versions
- Add `.bin/` to `.gitignore` (already likely there for tailwindcss)

**Justfile recipe:**

```just
# Install CI tool binaries (skip if already cached at correct version)
install-ci-tools:
    #!/usr/bin/env bash
    set -euo pipefail

    install_tool() {
        local name="$1" version="$2" url="$3"
        local marker=".bin/.${name}-version"
        if [ -f ".bin/${name}" ] && [ -f "$marker" ] && [ "$(cat "$marker")" = "$version" ]; then
            echo "${name} ${version} already installed, skipping"
            return
        fi
        echo "Installing ${name} ${version}..."
        curl -sL "$url" | tar xz -C .bin/ "${name}" 2>/dev/null || \
        curl -sL "$url" -o ".bin/${name}"
        chmod +x ".bin/${name}"
        echo "$version" > "$marker"
    }

    mkdir -p .bin
    # Platform detection and download for each tool
    # (exact URLs determined at implementation time based on release assets)
```

**Estimated savings:** ~20 minutes (10 min trunk + 10 min tauri-cli + minor for other tools)

### 2. Faster Linker

**Files:** `.cargo/config.toml`

Configure per-platform linker overrides:

```toml
[target.x86_64-unknown-linux-gnu]
linker = "clang"
rustflags = ["-C", "link-arg=-fuse-ld=mold"]

[target.aarch64-unknown-linux-gnu]
linker = "clang"
rustflags = ["-C", "link-arg=-fuse-ld=mold"]

[target.x86_64-apple-darwin]
rustflags = ["-C", "link-arg=-fuse-ld=lld"]

[target.aarch64-apple-darwin]
rustflags = ["-C", "link-arg=-fuse-ld=lld"]
```

CI workflows need to install `mold` on Linux (`apt-get install mold`). macOS uses `lld`. Windows is left with the default MSVC linker — `lld` support on Windows/MSVC is less mature and not worth the risk.

**Estimated savings:** ~1-2 minutes per platform build

### 3. Narrow Tokio Features

**Files:** `Cargo.toml` (workspace root)

Audit actual tokio usage across the codebase and replace `features = ["full"]` with only the features used. Expected minimal set for a desktop app:

```toml
tokio = { version = "1.50", features = ["rt-multi-thread", "macros", "sync", "time", "fs", "io-util"] }
```

The exact feature list will be determined by auditing `use tokio::` imports across the codebase.

**Estimated savings:** ~1-2 minutes (fewer crates to compile: signal, process, net sub-crates removed)

### 4. Reduce Release LTO Cost

**Files:** `Cargo.toml` (workspace root, `[profile.release]` section)

Change from:
```toml
[profile.release]
lto = "fat"
codegen-units = 1
```

To:
```toml
[profile.release]
lto = "thin"
codegen-units = 4
```

Thin LTO provides ~95% of fat LTO's optimization benefit but runs faster due to parallelization. `codegen-units = 4` allows the compiler to split work across cores.

**Estimated savings:** ~3-5 minutes

### 5. sccache Integration

**Files:** `.cargo/config.toml`, `Justfile`, CI workflow

Set up `sccache` as the Rust compiler wrapper to cache compiled artifacts:

```toml
# .cargo/config.toml
[build]
rustc-wrapper = "sccache"
```

**Local usage:** Install sccache, uses local disk cache by default.

**CI usage:** The CI workflow uses sccache backed by GitHub Actions cache:
- Set `SCCACHE_GHA_ENABLED=true` environment variable
- Use `Mozilla-Actions/sccache-action` to configure
- Cache is shared across CI runs, keyed by Cargo.lock hash

**Justfile recipe:**
```just
# Install sccache if not present
install-sccache:
    #!/usr/bin/env bash
    if command -v sccache >/dev/null 2>&1; then
        echo "sccache already installed"
    else
        echo "Installing sccache..."
        # Download pre-built binary similar to other tools
    fi
```

**Estimated savings:** ~5-10 minutes on warm builds (dependencies unchanged), ~0 on first cold build

## Expected Impact

| Scenario | Before | After |
|----------|--------|-------|
| Cold build (no cache) | ~45 min | ~15-20 min |
| Warm build (sccache hit) | ~45 min | ~8-12 min |
| Local dev build | ~3-5 min | ~2-3 min |

## Scope

- `Cargo.toml` — tokio features, release profile
- `.cargo/config.toml` — linker config, sccache wrapper
- `Justfile` — tool install recipes
- `.github/workflows/ci.yml` — use new install recipes, add sccache setup
- `.gitignore` — ensure `.bin/` is ignored
