#!/bin/bash
set -euxo pipefail

echo $MODE
echo $APP_NAME
echo $DMG_NAME

TMP=$(mktemp -d)
DMG_DIR=$TMP/$APP_NAME
APP_BIN_DIR=$DMG_DIR/$APP_NAME.app/Contents/MacOS
mkdir -p $APP_BIN_DIR
cp -v extras/macos-dmg/* $DMG_DIR
cp -v target/$MODE/rip_graphical $APP_BIN_DIR/app
cp -v scripts/macos_run_app.sh $APP_BIN_DIR/$APP_NAME
ln -s /Applications $DMG_DIR/Applications
hdiutil create $DMG_NAME -srcfolder $DMG_DIR
