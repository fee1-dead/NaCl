#!/bin/sh
set -e
pushd nacl
cargo $@
popd
pushd nacl_boot
cargo $@
popd