#!/bin/bash
set -euxo pipefail

BRANCH=$1

pushd web

npm install

npm run build -- --mode production

TMP=$(mktemp -d)
trap "rm -rf $TMP" EXIT

rm -rf slime99
mkdir slime99

mv dist slime99/$BRANCH

zip -r $TMP/slime99.zip slime99
rm -rf slime99

aws s3 cp $TMP/slime99.zip s3://games.gridbugs.org/slime99.zip
