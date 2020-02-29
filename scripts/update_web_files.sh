#!/bin/bash
set -euxo pipefail
BRANCH=$1
BUCKET=games.gridbugs.org

# The goal of this script is to update the mime type of the wasm file stored in s3.
# Experimentation suggests that this requires downloading and re-uploading the file
# with the new mime type. Copying the file within s3 doesn't correctly set the mime.
DIR=$(mktemp -d)
trap "rm -rf $DIR" EXIT
pushd $DIR
aws s3 cp s3://$BUCKET/slime99.zip .
unzip $DIR/slime99.zip
cd slime99/$BRANCH
for f in *; do
    if [[ ${f: -5} == ".wasm" ]]; then
        aws s3 cp $f s3://$BUCKET/slime99/$BRANCH/$f --content-type "application/wasm"
    else
        aws s3 cp $f s3://$BUCKET/slime99/$BRANCH/$f
    fi
done
