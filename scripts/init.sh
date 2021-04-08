#!/usr/bin/env bash

set -e

echo "*** Initializing WASM build environment"

if [ -z $CI_PROJECT_NAME ] ; then
#    rustup update nightly
#   rustup update stable
    rustup default nightly-2021-03-04
fi

rustup target add wasm32-unknown-unknown --toolchain nightly-2021-03-04
