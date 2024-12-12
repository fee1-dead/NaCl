#!/usr/bin/env sh
set -e
pushd nacl
cargo $@ --target x86_64-unknown-none
popd
pushd nacl_boot
cargo $@
popd