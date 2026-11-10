#!/usr/bin/env bash
# Package both artifacts, deterministically:
#  - the content `.hmod` via the pin's own cargo-horizon-mod (build + check), from content/
#  - the source-only `.hnmod` (tools/package.py) which embeds that `.hmod` in
#    `mixins/modern_industrialization/data/` (the AE2 pattern, so one file
#    installs block and code)
# Run inside a build slot (it compiles the packaging tool):
#   ports/tools/build-slot.sh run --job mi-pack --port ports/modern_industrialization \
#       --step package -- env CARGO_INCREMENTAL=0 tools/package.sh
set -euo pipefail

PORT=$(cd "$(dirname "$0")/.." && pwd)
HORIZON=${HORIZON:-${CAMPAIGN_ROOT:-$(cd "$(dirname "$0")/../../.." && pwd)}/ports/_shared/pins/horizon-1d63e0a}
LOGS=$PORT/target/ladder
mkdir -p "$LOGS"
cd "$PORT"

# The content package the bundle embeds must be this build's.
cargo_run() {
    (cd content && cargo run --locked --offline --quiet \
        --manifest-path "$HORIZON/crates/cargo-horizon-mod/Cargo.toml" -- "$@")
}
CARGO_TARGET_DIR=$PORT/target/carry-tools
export CARGO_TARGET_DIR
cargo_run horizon-mod build 2>&1 | tee "$LOGS/package-hmod.log"
cp content/target/horizon-mod/modern_industrialization.hmod content/modern_industrialization.hmod
cargo_run horizon-mod check modern_industrialization.hmod 2>&1 | tee -a "$LOGS/package-hmod.log"
cp content/modern_industrialization.hmod \
    mixins/modern_industrialization/data/modern_industrialization-content.hmod

python3 tools/package.py | tee "$LOGS/package-hnmod.log"
cp target/modern_industrialization-0.1.0.hnmod modern_industrialization.hnmod

echo "== artifacts =="
for f in content/modern_industrialization.hmod modern_industrialization.hnmod; do
    [ -f "$f" ] || { echo "missing $f"; exit 1; }
    size=$(stat -c %s "$f")
    sha=$(sha256sum "$f" | cut -d' ' -f1)
    entries=$(python3 -c "import zipfile,sys; print(len(zipfile.ZipFile(sys.argv[1]).namelist()))" "$f")
    echo "$f size=$size sha256=$sha entries=$entries"
done
