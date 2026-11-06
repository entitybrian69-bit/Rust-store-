#!/usr/bin/env bash
# Builds volt_lapse.hmod with the real, pinned Horizon toolchain: the host unit
# tests, the legacy sandbox scripts in tests/, then `cargo horizon-mod build`
# (wasm32-unknown-unknown -> componentized -> packed into the .hmod) and
# `cargo horizon-mod check`.
#
# Nothing here is hand-zipped. Every line is also written to build.log, which CI
# commits back to the branch so compiler errors are readable from anywhere.
# Re-exec under bash when invoked as `sh build.sh`: the log capture needs bash.
if [ -z "${BASH_VERSION:-}" ]; then exec bash "$0" "$@"; fi
set -euo pipefail
cd "$(dirname "$0")"
LOG=${LOG:-build.log}
exec > >(tee "$LOG") 2>&1

# The last Horizon main commit that still had the WebAssembly server runtime
# (2026-10-29T04:37:12Z). The commit that removed it is
# e7614d2197ebe9ffdaa75e1bbb432dc0ce855e70.
REV=62a01db8f006b422e6db24e34dcee05ad41760ba
TOOLCHAIN=1.99.1
export RUSTUP_TOOLCHAIN="$TOOLCHAIN"

artifact=target/horizon-mod/volt_lapse.hmod
rm -f "$artifact"
made_tmp=0
if [ -n "${TOOL_DIR:-}" ]; then
    tool_dir=$TOOL_DIR
    mkdir -p "$tool_dir"
else
    tool_dir=$(mktemp -d)
    made_tmp=1
fi
passed=0
cleanup() {
    if [ "$passed" -ne 1 ]; then rm -f "$artifact"; fi
    if [ "$made_tmp" -eq 1 ]; then rm -rf "$tool_dir"; fi
}
trap cleanup EXIT
trap 'exit 1' HUP INT TERM

rustup toolchain install "$TOOLCHAIN" --profile minimal --target wasm32-unknown-unknown

# Pinned to the same revision as the SDK, so the packer and the ABI are the ones
# this mod was written for. TOOL_DIR lets CI cache the built binary.
if [ -x "$tool_dir/bin/cargo-horizon-mod" ]; then
    echo "== reusing cargo-horizon-mod already in $tool_dir =="
else
    cargo install --git https://github.com/horizon-rust/horizon --rev "$REV" --locked \
        --root "$tool_dir" cargo-horizon-mod
fi
PATH="$tool_dir/bin:$PATH"
export PATH

echo "== host unit tests (settings schema, recording format, camera paths) =="
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
