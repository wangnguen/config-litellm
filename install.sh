#!/usr/bin/env bash
# Install ccl on macOS or Linux: download the latest release into ~/.ccl/bin
# and add that folder to PATH.
#
#   curl -fsSL https://raw.githubusercontent.com/wangnguen/config-litellm/main/install.sh | bash

set -euo pipefail

REPO="wangnguen/config-litellm"
BIN_DIR="$HOME/.ccl/bin"
# `ccl uninstall` removes exactly this line and the export below it.
MARKER="# ccl - Claude Code LiteLLM"

case "$(uname -s)-$(uname -m)" in
  Linux-x86_64)  TARGET="x86_64-unknown-linux-gnu" ;;
  Darwin-arm64)  TARGET="aarch64-apple-darwin" ;;
  Darwin-x86_64) TARGET="x86_64-apple-darwin" ;;
  *)
    echo "No ccl build for $(uname -s) $(uname -m)." >&2
    echo "Build it instead: cargo install --git https://github.com/$REPO ccl" >&2
    exit 1
    ;;
esac

# A plain release download rather than api.github.com: the API allows 60
# unauthenticated requests an hour per IP, which an office runs out of.
URL="https://github.com/$REPO/releases/latest/download/ccl-$TARGET"

mkdir -p "$BIN_DIR"
echo "Downloading ccl-$TARGET..."
curl -fsSL "$URL" -o "$BIN_DIR/ccl.new"
chmod +x "$BIN_DIR/ccl.new"
mv -f "$BIN_DIR/ccl.new" "$BIN_DIR/ccl"

case "$(basename "${SHELL:-sh}")" in
  zsh)  RC="$HOME/.zshrc" ;;
  bash) RC="$HOME/.bashrc" ;;
  *)    RC="$HOME/.profile" ;;
esac

if grep -qF "$MARKER" "$RC" 2>/dev/null; then
  echo "$BIN_DIR is already in PATH ($RC)."
else
  printf '\n%s\nexport PATH="$PATH:%s"\n' "$MARKER" "$BIN_DIR" >> "$RC"
  echo "Added $BIN_DIR to PATH in $RC. Open a new terminal, or run: source $RC"
fi

echo
echo "Done: $BIN_DIR/ccl"
echo "Open a project folder and run: ccl"
