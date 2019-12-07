#!/bin/bash
set -euxo pipefail

echo $MODE
echo $APP_NAME

TMP=$(mktemp -d)
DMG_DIR=$TMP/$APP_NAME
APP_BIN_DIR=$DMG_DIR/$APP_NAME.app/Contents/MacOS
mkdir -p $APP_BIN_DIR
cp target/$MODE/rip_graphical $APP_BIN_DIR/app
cp scripts/macos_run_app.sh $APP_BIN_DIR/$APP_NAME
ln -s /Applications $DMG_DIR/Applications
hdiutil create $APP_NAME.dmg -srcfolder $DMG_DIR
