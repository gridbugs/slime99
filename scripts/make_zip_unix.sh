#!/bin/bash
set -euxo pipefail

echo $MODE
echo $ZIP_NAME

TMP=$(mktemp -d)
mkdir $TMP/$ZIP_NAME
cp -v target/$MODE/rip_graphical $TMP/$ZIP_NAME/rip-graphic
cp -v target/$MODE/rip_ansi_terminal $TMP/$ZIP_NAME/rip-terminal
if [ -f target/$MODE/rip_graphical_opengl ]; then
  cp -v target/$MODE/rip_graphical_opengl $TMP/$ZIP_NAME/rip-graphic-opengl
fi

cp -v extras/unix/* $TMP/$ZIP_NAME

pushd $TMP
zip $ZIP_NAME.zip $ZIP_NAME/*
popd
mv $TMP/$ZIP_NAME.zip .
