#!/bin/bash

# Get project directory
PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$PROJECT_DIR"

# Determine Cargo package name (simple extraction)
# Use grep+sed for macOS compatibility (avoids awk dialect issues).
pkg_name=$(grep -m1 '^[[:space:]]*name[[:space:]]*=' Cargo.toml | sed -E 's/.*"([^"]+)".*/\1/')
if [ -z "$pkg_name" ]; then
    echo "❌ Could not determine package name from Cargo.toml"
    exit 1
fi
BINARY_PATH="$PROJECT_DIR/target/release/$pkg_name"

# Build the project
echo "🔨 Building $pkg_name..."
cargo build --release
if [ $? -ne 0 ]; then
    echo "❌ Build failed"
    exit 1
fi

# 1. Run Interactive Menu (Normal Font)
# This runs in the current terminal, so the font size is readable.
echo "🖥️  Launching Menu..."

# Capture output to a temporary file
MENU_OUTPUT=$(mktemp)
"$BINARY_PATH" menu > "$MENU_OUTPUT"

# Check if menu was cancelled (empty output)
if [ ! -s "$MENU_OUTPUT" ]; then
    echo "❌ Menu cancelled or no output"
    rm "$MENU_OUTPUT"
    exit 0
fi

# Debug: Print raw output to see what's wrong
echo "--- Raw Menu Output ---"
cat "$MENU_OUTPUT"
echo "-----------------------"

# Read variables using grep to avoid sourcing garbage
# ...
VIDEO_PATH=$(grep "__BAD_APPLE_CONFIG__VIDEO_PATH=" "$MENU_OUTPUT" | sed 's/__BAD_APPLE_CONFIG__//' | cut -d'=' -f2-)
AUDIO_PATH=$(grep "__BAD_APPLE_CONFIG__AUDIO_PATH=" "$MENU_OUTPUT" | sed 's/__BAD_APPLE_CONFIG__//' | cut -d'=' -f2-)
RENDER_MODE=$(grep "__BAD_APPLE_CONFIG__RENDER_MODE=" "$MENU_OUTPUT" | sed 's/__BAD_APPLE_CONFIG__//' | cut -d'=' -f2-)
FILL_SCREEN=$(grep "__BAD_APPLE_CONFIG__FILL_SCREEN=" "$MENU_OUTPUT" | sed 's/__BAD_APPLE_CONFIG__//' | cut -d'=' -f2-)
GHOSTTY_ARGS=$(grep "__BAD_APPLE_CONFIG__GHOSTTY_ARGS=" "$MENU_OUTPUT" | sed 's/__BAD_APPLE_CONFIG__//' | cut -d'=' -f2-)

rm "$MENU_OUTPUT"

# Validate variables
if [ -z "$VIDEO_PATH" ]; then
    echo "❌ Error: Failed to capture VIDEO_PATH from menu"
    exit 1
fi

# Debug output
echo "Selected Video: '$VIDEO_PATH'"
echo "Selected Audio: '$AUDIO_PATH'"
echo "Render Mode: '$RENDER_MODE'"
echo "Fill Screen: '$FILL_SCREEN'"
echo "Ghostty Args: '$GHOSTTY_ARGS'"

# Build the command arguments array
# Note: Subcommand is 'play-live', not '--play-live'
ARGS=(play-live --video "$VIDEO_PATH" --mode "$RENDER_MODE")

if [ -n "$AUDIO_PATH" ]; then
    ARGS+=(--audio "$AUDIO_PATH")
fi

if [ "$FILL_SCREEN" = "true" ]; then
    ARGS+=(--fill)
fi

# Find Ghostty binary
GHOSTTY_BIN="ghostty"
if ! command -v ghostty &> /dev/null; then
    if [ -f "/Applications/Ghostty.app/Contents/MacOS/ghostty" ]; then
        GHOSTTY_BIN="/Applications/Ghostty.app/Contents/MacOS/ghostty"
    else
        echo "❌ Ghostty not found. Please install Ghostty or add it to your PATH."
        exit 1
    fi
fi

# Use absolute path to binary to avoid CWD issues
# BINARY_PATH is determined dynamically above from Cargo.toml

# [수정] CMD_ARGS_STR 라인을 삭제합니다. 더 이상 필요 없습니다.

# [수정] GHOSTTY_ARGS 변수가 비어있을 수도 있으니, 
# 셸이 빈 문자열을 인자로 해석하지 않도록 처리합니다.
# `eval`을 사용해 GHOSTTY_ARGS를 안전하게 확장(word-splitting)합니다.
GHOSTTY_CMD=("$GHOSTTY_BIN" --config-file=Gascii.config)
if [ -n "$GHOSTTY_ARGS" ]; then
    # eval을 사용하여 $GHOSTTY_ARGS를 단어 단위로 분리하여 배열에 추가
    eval "GHOSTTY_CMD+=($GHOSTTY_ARGS)"
fi

# [수정] 이것이 새로운 bash -c 스크립트입니다.
# 1. 'BINARY_TO_RUN' 변수에 $1 (프로그램 경로)을 저장합니다.
# 2. 'shift'를 호출하여 $1을 인자 목록($@)에서 "제거"합니다.
# 3. $BINARY_TO_RUN을 나머지 인자들($@)과 함께 실행합니다.
# 4. 프로그램이 끝나면 'echo/read'가 실행됩니다.
GHOSTTY_SCRIPT='
BINARY_TO_RUN="$1"
shift
"$BINARY_TO_RUN" "$@"
echo "Press Enter to exit..."
read
'

echo "🚀 Launching Ghostty..."

# [수정] GHOSTTY_CMD 배열과 스크립트, 인자 배열을 모두 전달합니다.
"${GHOSTTY_CMD[@]}" \
    -e bash -c "$GHOSTTY_SCRIPT" "gascii_shell" "$BINARY_PATH" "${ARGS[@]}"