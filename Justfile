# Default recipe: list all available recipes
default:
    @just --list

# One-time setup after cloning: install the git hooks (pre-commit and commit-msg)
setup:
    prek install

# Run every pre-commit hook against the whole repo (same as the CI hooks job)
hooks:
    prek run --all-files

# Format all code with cargo fmt
fmt:
    cargo fmt --all

# Check formatting without applying changes
fmt-check:
    cargo fmt --all -- --check

# Run clippy on the entire workspace (same command as CI; lint levels live in Cargo.toml)
clippy:
    cargo clippy --workspace --all-targets --all-features -- -D warnings

# Run clippy and auto-fix where possible
clippy-fix:
    cargo clippy --workspace --all-targets --all-features --fix --allow-dirty --allow-staged -- -D warnings

# Build the workspace in debug mode
build:
    cargo build --workspace --all-targets

# Build the workspace in release mode
build-release:
    cargo build --workspace --release

# Run all tests with nextest
test:
    cargo nextest run --workspace

# Run all tests with standard cargo test (includes doctests)
test-doc:
    cargo test --workspace --doc

# Run a specific test by name
test-one NAME:
    cargo nextest run --workspace -E 'test({{ NAME }})'

# Check the workspace compiles without producing binaries
check:
    cargo check --workspace --all-targets

# Generate and open documentation
doc:
    cargo doc --workspace --no-deps --open

# Check docs build without warnings
doc-check:
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --quiet

# Run cargo audit for known vulnerabilities
audit:
    cargo audit

# Run cargo deny for license and advisory checks
deny:
    cargo deny check

# Detect unused dependencies
machete:
    cargo machete

# Run code coverage with llvm-cov
coverage:
    cargo llvm-cov --workspace nextest

# Run code coverage and generate an HTML report
coverage-html:
    cargo llvm-cov --workspace nextest --html
    @echo "Report at target/llvm-cov/html/index.html"

# Clean build artifacts
clean:
    cargo clean

# Run the full CI-style check suite
ci: fmt-check clippy test doc-check deny audit machete

# Check, lint, and test (quick local iteration)
dev: check clippy test

# Cut a release: bump versions everywhere, commit, and tag (then push to trigger the pipeline)
release VERSION:
    #!/usr/bin/env bash
    set -euo pipefail
    V="{{ VERSION }}"
    V="${V#v}"
    if ! [[ "$V" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
        echo "Invalid version '$V' (expected X.Y.Z or vX.Y.Z)" && exit 1
    fi
    if [ -n "$(git status --porcelain)" ]; then
        echo "Working tree is not clean; commit or stash first." && exit 1
    fi
    if git rev-parse -q --verify "refs/tags/v$V" >/dev/null; then
        echo "Tag v$V already exists." && exit 1
    fi
    # Workspace version (inherited by all crates via version.workspace = true)
    sed -i.bak -E "s/^version = \"[^\"]+\"$/version = \"$V\"/" Cargo.toml
    # Internal dependency pin
    sed -i.bak -E "s/^youtun4-core = \{ version = \"[^\"]+\"/youtun4-core = { version = \"$V\"/" Cargo.toml
    # Tauri bundle version — names the released packages (.dmg/.msi/.deb/.AppImage)
    sed -i.bak -E "s/\"version\": \"[^\"]+\"/\"version\": \"$V\"/" src-tauri/tauri.conf.json
    rm -f Cargo.toml.bak src-tauri/tauri.conf.json.bak
    cargo update --workspace --quiet
    git add Cargo.toml Cargo.lock src-tauri/tauri.conf.json
    git commit -m "chore(release): v$V"
    git tag -a "v$V" -m "Youtun4 v$V"
    echo "Release v$V committed and tagged."
    echo "Push it to trigger the release pipeline:  git push origin main v$V"

# Update dependencies
update:
    cargo update

# Show the dependency tree
tree:
    cargo tree --workspace

# Run typos checker
typos:
    typos

# Install zizmor (GitHub Actions security linter) locally via cargo
zizmor-install:
    cargo install zizmor --locked

# Run zizmor against all workflow files
zizmor:
    #!/usr/bin/env bash
    set -euo pipefail
    if ! command -v zizmor >/dev/null 2>&1; then
        echo "zizmor is not installed. Run 'just zizmor-install' first."
        exit 1
    fi
    zizmor .github/workflows/*.yml

# Format TOML files with taplo
taplo:
    taplo format

# Download and install pre-built CI tool binaries to .bin/
install-ci-tools:
    #!/usr/bin/env bash
    set -euo pipefail
    mkdir -p .bin

    OS=$(uname -s)
    ARCH=$(uname -m)

    # Normalise arch: aarch64 → arm64 for our internal labels
    case "${OS}-${ARCH}" in
        Darwin-arm64|Darwin-aarch64)  OS_LABEL="darwin";  ARCH_LABEL="aarch64" ;;
        Darwin-x86_64)                OS_LABEL="darwin";  ARCH_LABEL="x86_64"  ;;
        Linux-aarch64)                OS_LABEL="linux";   ARCH_LABEL="aarch64" ;;
        Linux-x86_64)                 OS_LABEL="linux";   ARCH_LABEL="x86_64"  ;;
        *) echo "Unsupported platform: ${OS}-${ARCH}" && exit 1 ;;
    esac

    # ── trunk ────────────────────────────────────────────────────────────────
    TRUNK_VERSION="0.22.1"
    TRUNK_MARKER=".bin/.trunk-${TRUNK_VERSION}"
    if [ -f "${TRUNK_MARKER}" ]; then
        echo "trunk ${TRUNK_VERSION}: cached"
    else
        case "${OS_LABEL}-${ARCH_LABEL}" in
            darwin-aarch64) TRUNK_TRIPLE="aarch64-apple-darwin"      ;;
            darwin-x86_64)  TRUNK_TRIPLE="x86_64-apple-darwin"       ;;
            linux-aarch64)  TRUNK_TRIPLE="aarch64-unknown-linux-musl" ;;
            linux-x86_64)   TRUNK_TRIPLE="x86_64-unknown-linux-musl"  ;;
        esac
        TRUNK_ARCHIVE="trunk-${TRUNK_TRIPLE}.tar.gz"
        TRUNK_URL="https://github.com/trunk-rs/trunk/releases/download/v${TRUNK_VERSION}/${TRUNK_ARCHIVE}"
        echo "Downloading trunk ${TRUNK_VERSION} for ${TRUNK_TRIPLE}..."
        curl -sL "${TRUNK_URL}" | tar -xzf - -C .bin trunk
        chmod +x .bin/trunk
        touch "${TRUNK_MARKER}"
        echo "Installed .bin/trunk (${TRUNK_VERSION})"
    fi

    # ── sccache ──────────────────────────────────────────────────────────────
    SCCACHE_VERSION="0.19.0"
    SCCACHE_MARKER=".bin/.sccache-${SCCACHE_VERSION}"
    if [ -f "${SCCACHE_MARKER}" ]; then
        echo "sccache ${SCCACHE_VERSION}: cached"
    else
        case "${OS_LABEL}-${ARCH_LABEL}" in
            darwin-aarch64) SCCACHE_TRIPLE="aarch64-apple-darwin"      ;;
            darwin-x86_64)  SCCACHE_TRIPLE="x86_64-apple-darwin"       ;;
            linux-aarch64)  SCCACHE_TRIPLE="aarch64-unknown-linux-musl" ;;
            linux-x86_64)   SCCACHE_TRIPLE="x86_64-unknown-linux-musl"  ;;
        esac
        SCCACHE_ARCHIVE="sccache-v${SCCACHE_VERSION}-${SCCACHE_TRIPLE}.tar.gz"
        SCCACHE_URL="https://github.com/mozilla/sccache/releases/download/v${SCCACHE_VERSION}/${SCCACHE_ARCHIVE}"
        SCCACHE_SUBDIR="sccache-v${SCCACHE_VERSION}-${SCCACHE_TRIPLE}"
        echo "Downloading sccache ${SCCACHE_VERSION} for ${SCCACHE_TRIPLE}..."
        curl -sL "${SCCACHE_URL}" | tar -xzf - -C .bin --strip-components=1 "${SCCACHE_SUBDIR}/sccache"
        chmod +x .bin/sccache
        touch "${SCCACHE_MARKER}"
        echo "Installed .bin/sccache (${SCCACHE_VERSION})"
    fi
