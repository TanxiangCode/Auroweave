#!/usr/bin/env bash
# Download sing-box 1.13.14 binary for macOS (Universal) / Linux
# Author: TanXiang

set -e

VERSION="1.13.14"
OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
ARCH="$(uname -m)"

if [ "$OS" = "darwin" ]; then
    PLATFORM="darwin-universal"
    TARGET_DIR="./src-tauri/sidecar-bin/macos-universal"
elif [ "$OS" = "linux" ]; then
    if [ "$ARCH" = "x86_64" ]; then
        PLATFORM="linux-amd64"
    elif [ "$ARCH" = "aarch64" ]; then
        PLATFORM="linux-arm64"
    else
        echo "Unsupported architecture: $ARCH"
        exit 1
    fi
    TARGET_DIR="./src-tauri/sidecar-bin/linux-$ARCH"
else
    echo "Unsupported OS: $OS"
    exit 1
fi

TAR_NAME="sing-box-${VERSION}-${PLATFORM}.tar.gz"
URL="https://github.com/SagerNet/sing-box/releases/download/v${VERSION}/${TAR_NAME}"
TEMP_DIR="$(mktemp -d)"

echo "Downloading sing-box $VERSION for $PLATFORM from $URL ..."
curl -sSL "$URL" -o "${TEMP_DIR}/${TAR_NAME}"

echo "Extracting sing-box..."
tar -xzf "${TEMP_DIR}/${TAR_NAME}" -C "$TEMP_DIR"

mkdir -p "$TARGET_DIR"
SINGBOX_BIN=$(find "$TEMP_DIR" -type f -name "sing-box")

if [ -n "$SINGBOX_BIN" ]; then
    cp "$SINGBOX_BIN" "${TARGET_DIR}/sing-box-${VERSION}"
    chmod +x "${TARGET_DIR}/sing-box-${VERSION}"
    echo "Successfully installed sing-box to ${TARGET_DIR}/sing-box-${VERSION}"
else
    echo "Error: sing-box binary not found in archive."
    exit 1
fi

rm -rf "$TEMP_DIR"
