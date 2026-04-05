# CI Build Time Optimization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reduce release CI build time from ~45 minutes to ~15 minutes by using pre-built binaries, faster linkers, narrower dependencies, and compilation caching.

**Architecture:** Five independent optimizations, each modifying local tooling config. Changes to `.cargo/config.toml`, `Cargo.toml`, `Justfile`, and `.github/workflows/ci.yml`. No Rust source code changes except removing unused tokio features.

**Tech Stack:** Cargo, sccache, mold, lld, just, GitHub Actions

---

### Task 1: Narrow Tokio Features

**Files:**
- Modify: `Cargo.toml:18`

- [ ] **Step 1: Replace tokio "full" with explicit features**

In `Cargo.toml` line 18, replace:

```toml
tokio = { version = "1.50", features = ["full"] }
```

with:

```toml
tokio = { version = "1.50", features = ["rt-multi-thread", "sync", "time", "macros"] }
```

These are the only features used in the codebase:
- `rt-multi-thread` — `tokio::runtime::Builder`, `tokio::spawn`
- `sync` — `RwLock`, `mpsc`, `oneshot`
- `time` — `tokio::time::sleep`
- `macros` — `#[tokio::test]`, `tokio::select!`

Note: `rt` (for `spawn_blocking`, `block_in_place`, `Handle`) is included transitively via `rt-multi-thread`.

- [ ] **Step 2: Verify it compiles**

Run: `cargo check --workspace --all-targets`

Expected: Compiles successfully. If any feature is missing, the compiler will tell you exactly which one (e.g., "feature `fs` is required").

- [ ] **Step 3: Run tests**

Run: `cargo nextest run --workspace`

Expected: All tests pass.

- [ ] **Step 4: Commit**

```bash
git add Cargo.toml
git commit -m "perf(build): narrow tokio features from full to minimal set"
```

---

### Task 2: Reduce Release LTO Cost

**Files:**
- Modify: `Cargo.toml:106-115`

- [ ] **Step 1: Change release profile LTO and codegen-units**

In `Cargo.toml`, replace the `[profile.release]` section (lines 106-115):

```toml
[profile.release]
opt-level = 3
debug = false
debug-assertions = false
overflow-checks = false
lto = "fat"
strip = "symbols"
panic = "abort"
incremental = false
codegen-units = 1
```

with:

```toml
[profile.release]
opt-level = 3
debug = false
debug-assertions = false
overflow-checks = false
lto = "thin"
strip = "symbols"
panic = "abort"
incremental = false
codegen-units = 4
```

Changes: `lto = "fat"` → `"thin"`, `codegen-units = 1` → `4`.

- [ ] **Step 2: Verify release build works**

Run: `cargo build --release -p youtun4-core`

Expected: Compiles successfully. This is a quick check — we don't need to build the full Tauri app.

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml
git commit -m "perf(build): switch to thin LTO and codegen-units=4"
```

---

### Task 3: Configure Faster Linker

**Files:**
- Modify: `.cargo/config.toml`

- [ ] **Step 1: Add per-platform linker configuration**

In `.cargo/config.toml`, add the following after the existing `[target.wasm32-unknown-unknown]` section:

```toml
# Use mold linker on Linux for faster link times
[target.x86_64-unknown-linux-gnu]
linker = "clang"
rustflags = ["-C", "link-arg=-fuse-ld=mold"]

[target.aarch64-unknown-linux-gnu]
linker = "clang"
rustflags = ["-C", "link-arg=-fuse-ld=mold"]

# Use lld linker on macOS for faster link times
[target.x86_64-apple-darwin]
rustflags = ["-C", "link-arg=-fuse-ld=lld"]

[target.aarch64-apple-darwin]
rustflags = ["-C", "link-arg=-fuse-ld=lld"]
```

Note: Windows is left with the default MSVC linker — `lld` support on Windows/MSVC is less mature.

- [ ] **Step 2: Verify build works on current platform**

Run: `cargo build --workspace`

Expected: Compiles successfully. If the linker isn't installed, you'll get a clear error like "lld: command not found" — install it with `brew install lld` (macOS) or `apt-get install mold` (Linux).

- [ ] **Step 3: Commit**

```bash
git add .cargo/config.toml
git commit -m "perf(build): configure mold/lld linkers for faster linking"
```

---

### Task 4: Pre-built Binary Tool Installer

**Files:**
- Modify: `Justfile`
- Modify: `.gitignore`

- [ ] **Step 1: Add .bin/ to .gitignore**

Append to `.gitignore`:

```
# Pre-built CI tool binaries
.bin/
```

Note: `.bin/` may already be partially ignored if tailwindcss is installed there. Adding the directory pattern ensures all binaries are ignored.

- [ ] **Step 2: Add install-ci-tools recipe to Justfile**

Add the following recipe to `Justfile` after the existing `tailwind-install` recipe:

```just
# Install pre-built CI tool binaries (cached by version)
install-ci-tools:
    #!/usr/bin/env bash
    set -euo pipefail
    mkdir -p .bin

    install_tool() {
        local name="$1" version="$2" url_template="$3" extract_cmd="$4"
        local marker=".bin/.${name}-version"
        if [ -f ".bin/${name}" ] && [ -f "$marker" ] && [ "$(cat "$marker")" = "$version" ]; then
            echo "✓ ${name} ${version} (cached)"
            return
        fi
        echo "↓ Installing ${name} ${version}..."
        local url
        url=$(echo "$url_template" | sed "s|{version}|${version}|g; s|{platform}|${PLATFORM}|g")
        if [ "$extract_cmd" = "direct" ]; then
            curl -sL "$url" -o ".bin/${name}"
        else
            curl -sL "$url" | eval "$extract_cmd"
        fi
        chmod +x ".bin/${name}"
        echo "$version" > "$marker"
        echo "✓ ${name} ${version} installed"
    }

    OS=$(uname -s)
    ARCH=$(uname -m)
    case "${OS}-${ARCH}" in
        Darwin-arm64)  PLATFORM="aarch64-apple-darwin" ;;
        Darwin-x86_64) PLATFORM="x86_64-apple-darwin" ;;
        Linux-x86_64)  PLATFORM="x86_64-unknown-linux-gnu" ;;
        Linux-aarch64) PLATFORM="aarch64-unknown-linux-gnu" ;;
        MINGW*|MSYS*|CYGWIN*)
            case "$ARCH" in
                x86_64) PLATFORM="x86_64-pc-windows-msvc" ;;
                *) echo "Unsupported Windows arch: ${ARCH}" && exit 1 ;;
            esac ;;
        *) echo "Unsupported platform: ${OS}-${ARCH}" && exit 1 ;;
    esac

    TRUNK_VERSION="0.21.14"
    TAURI_CLI_VERSION="2.10.3"
    CARGO_DENY_VERSION="0.19.0"
    SCCACHE_VERSION="0.10.0"

    install_tool "trunk" "$TRUNK_VERSION" \
        "https://github.com/trunk-rs/trunk/releases/download/v{version}/trunk-{platform}.tar.gz" \
        "tar xzf - -C .bin trunk"

    install_tool "cargo-deny" "$CARGO_DENY_VERSION" \
        "https://github.com/EmbarkStudios/cargo-deny/releases/download/{version}/cargo-deny-{version}-{platform}.tar.gz" \
        "tar xzf - --strip-components=1 -C .bin cargo-deny-{version}-{platform}/cargo-deny"

    install_tool "sccache" "$SCCACHE_VERSION" \
        "https://github.com/nickel-org/sccache/releases/download/v{version}/sccache-v{version}-{platform}.tar.gz" \
        "tar xzf - --strip-components=1 -C .bin sccache-v{version}-{platform}/sccache"

    # tauri-cli: use cargo-binstall pattern (GitHub release)
    install_tool "cargo-tauri" "$TAURI_CLI_VERSION" \
        "https://github.com/nickel-org/tauri/releases/download/tauri-cli-v{version}/cargo-tauri-{platform}.tgz" \
        "tar xzf - -C .bin cargo-tauri"

    echo ""
    echo "All tools ready in .bin/"
    echo "Add .bin/ to your PATH: export PATH=\"\$PWD/.bin:\$PATH\""
```

Note: The exact download URLs for tauri-cli and sccache will need to be verified against the actual GitHub release assets during implementation. The patterns above follow common conventions but may need adjustment.

- [ ] **Step 3: Verify the recipe works**

Run: `just install-ci-tools`

Expected: Downloads binaries to `.bin/`, shows "✓" for each tool. Run again — should show "(cached)" for all tools.

- [ ] **Step 4: Commit**

```bash
git add Justfile .gitignore
git commit -m "perf(build): add pre-built binary installer for CI tools"
```

---

### Task 5: Configure sccache

**Files:**
- Modify: `.cargo/config.toml`
- Modify: `.github/workflows/ci.yml`

- [ ] **Step 1: Add sccache as rustc wrapper**

In `.cargo/config.toml`, add at the top before any `[target.*]` sections:

```toml
# Use sccache to cache compiled artifacts (install via `just install-ci-tools`)
# Comment out if sccache is not installed locally
[build]
rustc-wrapper = "sccache"
```

- [ ] **Step 2: Verify local build works with sccache**

First install sccache:

Run: `just install-ci-tools && export PATH="$PWD/.bin:$PATH"`

Then build:

Run: `cargo build --workspace`

Expected: Compiles successfully. Check cache stats:

Run: `sccache --show-stats`

Expected: Shows cache hits/misses.

- [ ] **Step 3: Update CI workflow to use sccache**

In `.github/workflows/ci.yml`, add the sccache setup step to each job that compiles Rust code (clippy, build, coverage, build-wasm, docs, msrv). Add these steps after "Install Rust" and before "Setup Rust cache":

```yaml
      - name: Install sccache
        uses: mozilla-actions/sccache-action@v0.0.7

      - name: Configure sccache
        run: echo "RUSTC_WRAPPER=sccache" >> "$GITHUB_ENV"
```

Also add to the `env` section at the top of the workflow:

```yaml
env:
  CARGO_TERM_COLOR: always
  RUST_TOOLCHAIN: 1.94.0
  RUSTFLAGS: -D warnings
  SCCACHE_GHA_ENABLED: "true"
```

- [ ] **Step 4: Update CI workflow to use pre-built binaries**

In the `build-wasm` job, replace:

```yaml
      - name: Install trunk
        run: cargo install trunk --locked
```

with:

```yaml
      - name: Cache CI tools
        uses: actions/cache@v4
        with:
          path: .bin
          key: ci-tools-${{ runner.os }}-${{ runner.arch }}-trunk-0.21.14

      - name: Install trunk
        run: |
          just install-ci-tools
          echo "$PWD/.bin" >> "$GITHUB_PATH"
```

In the `deps` job, replace:

```yaml
      - name: Install cargo-deny
        run: cargo install cargo-deny --locked

      - name: Install cargo-machete
        run: cargo install cargo-machete --locked
```

with:

```yaml
      - name: Cache CI tools
        uses: actions/cache@v4
        with:
          path: .bin
          key: ci-tools-${{ runner.os }}-${{ runner.arch }}-deny-0.19.0

      - name: Install CI tools
        run: |
          just install-ci-tools
          echo "$PWD/.bin" >> "$GITHUB_PATH"
```

In the `coverage` job, replace:

```yaml
      - name: Install cargo-tarpaulin
        run: cargo install cargo-tarpaulin --locked
```

with:

```yaml
      - name: Cache CI tools
        uses: actions/cache@v4
        with:
          path: .bin
          key: ci-tools-${{ runner.os }}-${{ runner.arch }}-tarpaulin-0.32.7

      - name: Install CI tools
        run: |
          just install-ci-tools
          echo "$PWD/.bin" >> "$GITHUB_PATH"
```

- [ ] **Step 5: Verify CI workflow syntax**

Run: `actionlint .github/workflows/ci.yml`

Expected: No errors.

- [ ] **Step 6: Commit**

```bash
git add .cargo/config.toml .github/workflows/ci.yml
git commit -m "perf(build): add sccache and pre-built binaries to CI"
```
