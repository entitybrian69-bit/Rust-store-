#!/usr/bin/env sh
set -eu
cd "$(dirname "$0")"
REV=62a01db8f006b422e6db24e34dcee05ad41760ba
export RUSTUP_TOOLCHAIN=1.99.1
artifact=target/horizon-mod/quick_place.hmod
rm -f "$artifact"
tool_dir=$(mktemp -d)
passed=0
cleanup() {
    if [ "$passed" -ne 1 ]; then rm -f "$artifact"; fi
    rm -rf "$tool_dir"
}
trap cleanup EXIT
trap 'exit 1' HUP INT TERM
rustup toolchain install "$RUSTUP_TOOLCHAIN" --profile minimal --target wasm32-unknown-unknown
# Isolate the executable so an unrelated installed version cannot be used.
cargo install --git https://github.com/horizon-rust/horizon --rev "$REV" --locked --root "$tool_dir" cargo-horizon-mod
PATH="$tool_dir/bin:$PATH"
export PATH
cargo test
cargo horizon-mod build
cargo horizon-mod test --hmod "$artifact"
cargo horizon-mod check "$artifact"
test -s "$artifact"
passed=1
printf '\nBuilt and passed tests: %s\n' "$artifact"
