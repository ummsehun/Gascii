#!/bin/bash

# Gascii Installer for Linux & macOS

REPO="eleulleuso/Gascii"
INSTALL_DIR="/usr/local/bin"
BINARY_NAME="bad_apple"

echo "🚀 Installing Gascii..."

# Detect OS
OS="$(uname -s)"
case "$OS" in
    Linux*)     OS_TYPE="linux";;
    Darwin*)    OS_TYPE="macos";;
    *)          echo "❌ Unsupported OS: $OS"; exit 1;;
esac

echo "Detected OS: $OS_TYPE"

# Determine latest release URL (using GitHub API is better, but simple URL construction for now)
# We assume the release tag matches the version or is 'latest'
# Since we can't easily guess the tag without API, we'll try to use the 'latest' release endpoint redirection
# or just ask the user to provide a version if this fails.
# Actually, GitHub releases have a 'latest' download URL format:
# https://github.com/<user>/<repo>/releases/latest/download/<asset>

DOWNLOAD_URL="https://github.com/$REPO/releases/latest/download/bad_apple-$OS_TYPE"

if [ "$OS_TYPE" == "windows" ]; then
    DOWNLOAD_URL="${DOWNLOAD_URL}.exe"
fi

echo "⬇️  Downloading from: $DOWNLOAD_URL"

# Create temp file
TMP_FILE=$(mktemp)

# Download
if command -v curl >/dev/null 2>&1; then
    curl -L -o "$TMP_FILE" "$DOWNLOAD_URL"
elif command -v wget >/dev/null 2>&1; then
    wget -O "$TMP_FILE" "$DOWNLOAD_URL"
else
    echo "❌ curl or wget is required."
    exit 1
fi

if [ ! -s "$TMP_FILE" ]; then
    echo "❌ Download failed or file is empty."
    rm "$TMP_FILE"
    exit 1
fi

# Install
echo "📦 Installing to $INSTALL_DIR..."
chmod +x "$TMP_FILE"

if [ -w "$INSTALL_DIR" ]; then
    mv "$TMP_FILE" "$INSTALL_DIR/$BINARY_NAME"
else
    echo "sudo access required to move binary to $INSTALL_DIR"
    if ! sudo mv "$TMP_FILE" "$INSTALL_DIR/$BINARY_NAME"; then
        echo "❌ Failed to move binary. Please check your permissions."
        rm "$TMP_FILE"
        exit 1
    fi
fi

echo "✅ Installation complete! Run '$BINARY_NAME' to start."
