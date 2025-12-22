#!/bin/bash

set -e

# 1. Check or install cargo-bundle
if ! command -v cargo-bundle &> /dev/null; then
    echo "Installing cargo-bundle..."
    cargo install cargo-bundle
fi

# 2. Build and bundle the .app
echo "Building and bundling the .app..."
cargo bundle --release

# 3. Find the generated .app file
APP_PATH=$(find target/release/bundle/osx -name "*.app" -maxdepth 1 | head -n 1)

if [ -z "$APP_PATH" ]; then
    echo "Error: Generated .app file not found"
    exit 1
fi

APP_NAME=$(basename "$APP_PATH")
VOL_NAME="${APP_NAME%.app}"
DMG_NAME="${VOL_NAME}.dmg"
DIST_DIR="target/distrib"

echo "Found app: $APP_PATH"

# 4. Prepare DMG staging directory
STAGE_DIR=$(mktemp -d)
echo "Preparing temporary directory: $STAGE_DIR"

cp -R "$APP_PATH" "$STAGE_DIR/"
ln -s /Applications "$STAGE_DIR/Applications"

# 5. Create DMG
mkdir -p "$DIST_DIR"
OUT_PATH="$DIST_DIR/$DMG_NAME"

echo "Creating DMG: $OUT_PATH"
hdiutil create -volname "$VOL_NAME" -srcfolder "$STAGE_DIR" -ov -format UDZO "$OUT_PATH"

# 6. Clean up
rm -rf "$STAGE_DIR"

echo "Done! DMG file located at: $OUT_PATH"
