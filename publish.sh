#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# PACT publish script
#
# Builds release binaries for Windows, Linux, and macOS and places them in
# ./publish/<platform>/.
#
# Usage:
#   ./publish.sh                # build all three platforms
#   ./publish.sh linux          # build only Linux
#   ./publish.sh windows        # build only Windows
#   ./publish.sh macos          # build only macOS
#
# ── What you need ─────────────────────────────────────────────────────────────
#
#   For all cross-platform builds from Linux:
#     1. rustup  — https://rustup.rs
#     2. Docker  — sudo pacman -S docker && sudo systemctl enable --now docker
#     3. cross   — cargo install cross --git https://github.com/cross-rs/cross
#     4. Add yourself to the docker group and apply it:
#          sudo usermod -aG docker $USER && newgrp docker
#
#   macOS cross-compilation from Linux is not supported via cross (the
#   cross-rs Docker images do not include the Apple SDK).  Build macOS
#   natively on a Mac, or use GitHub Actions (see bottom of this file).
#
# ─────────────────────────────────────────────────────────────────────────────

set -euo pipefail

BINARY_NAME="process_affinity_control_tool"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
OUT_DIR="$SCRIPT_DIR/publish"
WINDOWS_COMPAT_INC="$SCRIPT_DIR/.cross/windows-headers"

# ─── Detect environment ───────────────────────────────────────────────────────

case "$(uname -s)" in
    Linux*)               HOST=linux   ;;
    Darwin*)              HOST=macos   ;;
    MSYS*|MINGW*|CYGWIN*) HOST=windows ;;
    *)                    HOST=unknown ;;
esac

# Resolve cross: check PATH first, then ~/.cargo/bin
CROSS_BIN=""
if   command -v cross            &>/dev/null; then CROSS_BIN="cross"
elif [ -x "$HOME/.cargo/bin/cross" ];          then CROSS_BIN="$HOME/.cargo/bin/cross"
fi

# Docker may be installed but the current session may not have picked up the
# docker group yet (requires logout/login or newgrp docker).
# Try plain access first, then fall back to sg docker.
docker_ok() {
    docker info &>/dev/null 2>&1 && return 0
    sg docker -c "docker info" &>/dev/null 2>&1 && return 0
    return 1
}

have()         { command -v "$1" &>/dev/null; }
have_rustup()  { have rustup && rustup show &>/dev/null 2>&1; }
have_cross()   { [ -n "$CROSS_BIN" ] && docker_ok; }

# Wrapper: run cross, applying sg docker if needed so the socket is accessible
run_cross() {
    if docker info &>/dev/null 2>&1; then
        "$CROSS_BIN" "$@"
    else
        # Current session hasn't applied docker group yet — use sg
        sg docker -c "$(printf '%q ' "$CROSS_BIN" "$@")"
    fi
}

# ─── Helpers ──────────────────────────────────────────────────────────────────

section() {
    echo
    echo "══════════════════════════════════════════════════"
    echo "  $*"
    echo "══════════════════════════════════════════════════"
}
warn() { echo "⚠  $*" >&2; }
info() { echo "   $*"; }
ok()   { echo "✓  $*"; }
fail() { echo "✗  $*" >&2; }

# ─── Per-target build ─────────────────────────────────────────────────────────

ensure_windows_compat_headers() {
    mkdir -p "$WINDOWS_COMPAT_INC"
    cat >"$WINDOWS_COMPAT_INC/BaseTsd.h" <<'EOF'
#ifndef PACT_BASETSD_COMPAT_H
#define PACT_BASETSD_COMPAT_H
#include <basetsd.h>
#endif
EOF
}

build_target() {
    local TARGET="$1"    # Rust triple
    local PLATFORM="$2"  # linux | windows | macos
    local EXT="$3"       # "" or ".exe"

    section "Building for $PLATFORM ($TARGET)"

    mkdir -p "$OUT_DIR/$PLATFORM"

    # ── Choose build strategy ─────────────────────────────────────────────────

    local TOOL

    if [ "$HOST" = "$PLATFORM" ]; then
        TOOL=cargo
        info "Native build"

    elif have_cross; then
        TOOL=cross
        info "Using 'cross' via Docker"

    elif have_rustup; then
        TOOL=cargo
        rustup target add "$TARGET" 2>/dev/null || true
        info "Using plain 'cargo' (install Docker + cross for reliable cross-compilation)"

    else
        fail "rustup not found and not on native host.  Install rustup first:"
        echo  "    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
        return 1
    fi

    # ── Ensure Rust target stdlib is present ──────────────────────────────────
    if have_rustup; then
        rustup target add "$TARGET" 2>/dev/null || true
    fi

    # ── Features / release flags ──────────────────────────────────────────────
    # hwlocality-sys needs to build hwloc from source when cross-compiling
    # because pkg-config can't find a target-platform hwloc on the host.
    local FEATURES=""
    if [ "$HOST" != "$PLATFORM" ]; then
        FEATURES="--features hwlocality/vendored"
    fi

    # Publish builds should not carry debug info in the executable.
    local RELEASE_CONFIG="--config profile.release.debug=false"

    # ── Run ───────────────────────────────────────────────────────────────────
    cd "$SCRIPT_DIR"
    if [ "$PLATFORM" = "windows" ] && [ "$TARGET" = "x86_64-pc-windows-gnu" ]; then
        ensure_windows_compat_headers
    fi

    if [ "$TOOL" = cross ]; then
        # Use an isolated target dir so cross never picks up host-compiled
        # build scripts (which require the host glibc, not the container's).
        local CROSS_TARGET_DIR="$SCRIPT_DIR/target/cross-$PLATFORM"
        mkdir -p "$CROSS_TARGET_DIR"
         # shellcheck disable=SC2086
          if [ "$PLATFORM" = "windows" ] && [ "$TARGET" = "x86_64-pc-windows-gnu" ]; then
              local WIN_C_INCLUDE="/project/.cross/windows-headers"
              HWLOC_SYS_USE_VENDORED=1 \
              RUSTFLAGS="-C target-feature=+crt-static${RUSTFLAGS:+ $RUSTFLAGS}" \
              WINDRES="x86_64-w64-mingw32-windres" \
              C_INCLUDE_PATH="$WIN_C_INCLUDE${C_INCLUDE_PATH:+:$C_INCLUDE_PATH}" \
              CPLUS_INCLUDE_PATH="$WIN_C_INCLUDE${CPLUS_INCLUDE_PATH:+:$CPLUS_INCLUDE_PATH}" \
              run_cross build --release --target "$TARGET" $RELEASE_CONFIG \
                   --target-dir "$CROSS_TARGET_DIR" $FEATURES
          else
              run_cross build --release --target "$TARGET" $RELEASE_CONFIG \
                   --target-dir "$CROSS_TARGET_DIR" $FEATURES
          fi
    else
        # shellcheck disable=SC2086
        if [ "$PLATFORM" = "windows" ] && [ "$TARGET" = "x86_64-pc-windows-gnu" ]; then
            HWLOC_SYS_USE_VENDORED=1 \
            RUSTFLAGS="-C target-feature=+crt-static${RUSTFLAGS:+ $RUSTFLAGS}" \
            WINDRES="x86_64-w64-mingw32-windres" \
            CFLAGS_x86_64_pc_windows_gnu="-I$WINDOWS_COMPAT_INC ${CFLAGS_x86_64_pc_windows_gnu:-}" \
            CPPFLAGS_x86_64_pc_windows_gnu="-I$WINDOWS_COMPAT_INC ${CPPFLAGS_x86_64_pc_windows_gnu:-}" \
            cargo build --release --target "$TARGET" $RELEASE_CONFIG $FEATURES
        else
            cargo build --release --target "$TARGET" $RELEASE_CONFIG $FEATURES
        fi
    fi

    # ── Copy output ───────────────────────────────────────────────────────────
    local SRC
    if [ "$TOOL" = cross ]; then
        SRC="$SCRIPT_DIR/target/cross-$PLATFORM/$TARGET/release/${BINARY_NAME}${EXT}"
    else
        SRC="$SCRIPT_DIR/target/$TARGET/release/${BINARY_NAME}${EXT}"
    fi
    if [ ! -f "$SRC" ]; then
        fail "Expected binary not found: $SRC"
        return 1
    fi

    if [ "$EXT" != ".exe" ] && have strip; then
        strip "$SRC"
        info "Stripped binary"
    fi

    local FINAL="$OUT_DIR/$PLATFORM/${BINARY_NAME}${EXT}"
    cp "$SRC" "$FINAL"

    local SIZE
    SIZE=$(du -sh "$FINAL" | cut -f1)
    ok "Output: $FINAL  ($SIZE)"
}

# ─── Main ─────────────────────────────────────────────────────────────────────

if [ $# -eq 0 ]; then
    TARGETS=(linux windows macos)
else
    TARGETS=("$@")
fi

mkdir -p "$OUT_DIR"

section "Environment"
info "Host OS  : $HOST"
info "rustup   : $(have_rustup && echo yes || echo no)"
info "cross    : $([ -n "$CROSS_BIN" ] && echo "$CROSS_BIN" || echo "not found")"
info "Docker   : $(docker_ok && echo "yes (accessible)" || echo "not accessible — run: newgrp docker")"

FAILED=()

for P in "${TARGETS[@]}"; do
    case "$P" in
        linux)
            build_target "x86_64-unknown-linux-gnu" "linux"   ""     || FAILED+=(linux)
            ;;
        windows)
            if [ "$HOST" = "windows" ]; then
                build_target "x86_64-pc-windows-msvc" "windows" ".exe" || FAILED+=(windows)
            else
                warn "Cross-building Windows from Linux/macOS uses GNU toolchain (x86_64-pc-windows-gnu), which can differ from native Windows MSVC builds."
                build_target "x86_64-pc-windows-gnu"  "windows" ".exe" || FAILED+=(windows)
            fi
            ;;
        macos)
            if [ "$HOST" = "macos" ]; then
                build_target "x86_64-apple-darwin" "macos" "" || FAILED+=(macos)
            else
                section "Building for macos (x86_64-apple-darwin)"
                fail "macOS cross-compilation is not supported."
                echo  "  The cross-rs Docker images do not include the Apple SDK."
                echo  "  Options:"
                echo  "    A) Run ./publish.sh macos on a Mac."
                echo  "    B) Use GitHub Actions (free macOS runners — see bottom of this script)."
                FAILED+=(macos)
            fi
            ;;
        *)
            warn "Unknown platform '$P' — valid options: linux windows macos"
            FAILED+=("$P")
            ;;
    esac
done

# ─── Summary ──────────────────────────────────────────────────────────────────

section "Summary"

for P in linux windows macos; do
    if [[ " ${TARGETS[*]} " == *" $P "* ]]; then
        if [[ " ${FAILED[*]} " == *" $P "* ]]; then
            fail "$P — FAILED"
        else
            ok "$P — $(ls -sh "$OUT_DIR/$P/"* 2>/dev/null | awk '{print $1, $2}' | tr '\n' '  ')"
        fi
    fi
done

echo

if [ ${#FAILED[@]} -gt 0 ]; then
    exit 1
fi

# ─── GitHub Actions (recommended for macOS + all platforms) ───────────────────
# .github/workflows/release.yml:
#
# on: [push]
# jobs:
#   build:
#     strategy:
#       matrix:
#         include:
#           - os: ubuntu-latest   target: x86_64-unknown-linux-gnu   ext: ""
#           - os: windows-latest  target: x86_64-pc-windows-msvc     ext: ".exe"
#           - os: macos-latest    target: x86_64-apple-darwin        ext: ""
#     runs-on: ${{ matrix.os }}
#     steps:
#       - uses: actions/checkout@v4
#       - uses: dtolnay/rust-toolchain@stable
#         with: { targets: "${{ matrix.target }}" }
#       - run: cargo build --release --target ${{ matrix.target }}
#       - uses: actions/upload-artifact@v4
#         with:
#           name: pact-${{ matrix.os }}
#           path: target/${{ matrix.target }}/release/process_affinity_control_tool${{ matrix.ext }}
