#!/bin/bash
set -euxo pipefail
BRANCH=$1
REVISION=$2
BUCKET=games.gridbugs.org
EXISTING_OBJECT=rip/$BRANCH/app.$REVISION.wrong-mime.wasm
NEW_OBJECT=rip/$BRANCH/app.$REVISION.wasm

# The goal of this script is to update the mime type of the wasm file stored in s3.
# Experimentation suggests that this requires downloading and re-uploading the file
# with the new mime type. Copying the file within s3 doesn't correctly set the mime.
DIR=$(mktemp -d)
trap "rm -rf $DIR" EXIT
aws s3 cp s3://$BUCKET/$EXISTING_OBJECT $DIR/tmp.wasm
aws s3 cp $DIR/tmp.wasm s3://$BUCKET/$NEW_OBJECT --content-type "application/wasm"
