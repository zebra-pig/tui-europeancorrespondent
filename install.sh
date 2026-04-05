#!/bin/sh
# Install The European Correspondent TUI permanently
# Usage: curl -fsSL https://raw.githubusercontent.com/zebra-pig/tui-europeancorrespondent/main/install.sh | sh
set -e

REPO="zebra-pig/tui-europeancorrespondent"
BINARY="tui-europeancorrespondent"
INSTALL_DIR="${HOME}/.local/bin"

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
BIN_PATH="${INSTALL_DIR}/tec${EXT}"

echo "The European Correspondent - Terminal Edition"
echo "=============================================="
echo ""
echo "Installing to: ${BIN_PATH}"
echo ""

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
echo "Installed successfully!"
echo ""
echo "Run with: tec"
echo ""

# Add ~/.local/bin to PATH hint if not already there
case ":$PATH:" in
    *":${INSTALL_DIR}:"*) ;;
    *) echo "Note: Add ${INSTALL_DIR} to your PATH if not already:"
       echo "  export PATH=\"${INSTALL_DIR}:\$PATH\""
       echo "" ;;
esac
