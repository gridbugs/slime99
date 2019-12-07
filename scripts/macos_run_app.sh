#!/bin/bash
#
# Tiny bash script to invoke the game's binary from within
# the app directory structure.
#
# I have no idea why this is necessary, but without it the
# game doesn't start when you run the app, despite the
# binary starting fine when run directly.
#
# Replacing this script with the binary and running the
# app with `open -a <APP>` gives the error:
#
# LSOpenURLsWithRole() failed for the application APP with error -10810.
#
# Nobody seems to agree on what this error means.
set -euxo pipefail
DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
$DIR/app
