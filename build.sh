#!/usr/bin/env bash
set -o errexit
set -o pipefail

TARGET_LINUX=x86_64-unknown-linux-gnu
TARGET_WIN=x86_64-pc-windows-gnu
BIN_NAME=$(grep name Cargo.toml | sed -e 's/.* = "\(.*\)"/\1/')
VERSION=$(grep version Cargo.toml | sed -e 's/.* = "\(.*\)"/\1/')

USAGE="Build script for $BIN_NAME.

USAGE
    build.sh <target>

TARGETS
    build_linux       cargo release build for 64bit linux
    build_win         cargo release build for 64bit windows
    debug_linux       cargo build for 64bit linux
    debug_windows     cargo build for 64bit windows
    check             start a 'cargo watch' job with check
    install           run cargo install to cargo's bin dir
    publish           publish to crates.io
    release           create a new github release with both linux and win binaries
"

gh_release() {
    gh release create "v${VERSION}" --title "Release ${VERSION}" target/${TARGET_WIN}/release/${BIN_NAME}.exe target/${TARGET_LINUX}/release/${BIN_NAME}
	git fetch --tags origin
}

build_linux(){
    cargo build --release --target=$TARGET_LINUX 
}

build_win(){
    cargo build --release --target=$TARGET_WIN 
}


case $1 in
    build) build_linux && build_win ;;
    build_linux) build_linux ;;
    build_win) build_win ;;
    debug_linux) cargo build  --target=$TARGET_LINUX ;;
    debug_win) cargo build  --target=$TARGET_WIN ;;
    check) cargo watch -x check ;;
    install) cargo install --path . --force ;;
    publish) cargo publish ;;
    name) echo $BIN_NAME ;; 
    release) build_linux && build_win && gh_release ;;
    *) 
        echo "$USAGE" ;;
esac
