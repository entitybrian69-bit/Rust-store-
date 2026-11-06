#!/usr/bin/env sh
# Builds quick_place.hmod with the real, pinned Horizon toolchain, runs the host
# unit tests, the legacy sandbox scripts in tests/, and the registry check.
#
# Nothing here is hand-zipped: `cargo horizon-mod build` compiles the crate to
# wasm32-unknown-unknown, componentizes it and packs the .hmod with Horizon's own
# `horizon-mods` packer.
set -eu
cd "$(dirname "$0")"

# The last Horizon main commit that still had the WebAssembly server runtime
# (2026-10-29T04:37:12Z). The commit that removed it is e7614d2197ebe9ffdaa75e1bbb432dc0ce855e70.
REV=62a01db8f006b422e6db24e34dcee05ad41760ba
TOOLCHAIN=1.99.1
export RUSTUP_TOOLCHAIN="$TOOLCHAIN"

artifact=target/horizon-mod/quick_place.hmod
rm -f "$artifact"
tool_dir=${TOOL_DIR:-$(mktemp -d)}
passed=0
cleanup() {
    if [ "$passed" -ne 1 ]; then rm -f "$artifact"; fi
    rm -rf "$tool_dir"
}
trap cleanup EXIT
trap 'exit 1' HUP INT TERM

rustup toolchain install "$TOOLCHAIN" --profile minimal --target wasm32-unknown-unknown

# The tool is pinned to the same revision as the SDK, so the packer and the ABI are
# the ones this mod was written for.
cargo install --git https://github.com/horizon-rust/horizon --rev "$REV" --locked \
    --root "$tool_dir" cargo-horizon-mod
PATH="$tool_dir/bin:$PATH"
export PATH

echo "== host unit tests (pure planner + config parser) =="
cargo test

echo "== build the .hmod (wasm32 component, packed by horizon-mods) =="
cargo horizon-mod build

echo "== legacy sandbox scripts (tests/*.toml) against the built .hmod =="
cargo horizon-mod test --hmod "$artifact"

echo "== registry check =="
cargo horizon-mod check "$artifact"

test -s "$artifact"
passed=1
echo
echo "Built and tested: $artifact"
sha256sum "$artifact" 2>/dev/null || shasum -a 256 "$artifact"
wc -c < "$artifact"
