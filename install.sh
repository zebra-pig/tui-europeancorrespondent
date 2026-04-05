#!/bin/sh
# Install and run The European Correspondent TUI
# Usage: curl -fsSL https://raw.githubusercontent.com/zebra-pig/tui-europeancorrespondent/main/install.sh | sh
set -e

REPO="zebra-pig/tui-europeancorrespondent"
BINARY="tui-europeancorrespondent"
INSTALL_DIR="${TMPDIR:-/tmp}/european-correspondent"

# Detect platform
OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
    Darwin)         OS_TAG="apple-darwin"; EXT="" ;;
    Linux)          OS_TAG="unknown-linux-gnu"; EXT="" ;;
    MINGW*|MSYS*)   OS_TAG="pc-windows-msvc"; EXT=".exe" ;;
    *)              echo "Unsupported OS: $OS"; exit 1 ;;
esac

case "$ARCH" in
    x86_64|amd64)   ARCH_TAG="x86_64" ;;
    arm64|aarch64)  ARCH_TAG="aarch64" ;;
    *)              echo "Unsupported architecture: $ARCH"; exit 1 ;;
esac

TARGET="${ARCH_TAG}-${OS_TAG}"
ASSET_NAME="${BINARY}-${TARGET}${EXT}"
RELEASE_URL="https://github.com/${REPO}/releases/latest/download/${ASSET_NAME}"

mkdir -p "$INSTALL_DIR"
BIN_PATH="${INSTALL_DIR}/${BINARY}${EXT}"

echo "The European Correspondent - Terminal Edition"
echo "=============================================="
echo ""
echo "Platform: ${OS} ${ARCH}"
echo ""

# Download binary
if command -v curl >/dev/null 2>&1; then
    curl -fSL --progress-bar "$RELEASE_URL" -o "$BIN_PATH"
elif command -v wget >/dev/null 2>&1; then
    wget -q --show-progress "$RELEASE_URL" -O "$BIN_PATH"
else
    echo "Error: curl or wget required"
    exit 1
fi

chmod +x "$BIN_PATH" 2>/dev/null || true

echo ""
echo "Running..."
echo ""

# Redirect stdin from /dev/tty so the TUI can read keyboard input
# (when run via curl | sh, stdin is the pipe, not the terminal)
exec "$BIN_PATH" "$@" </dev/tty
