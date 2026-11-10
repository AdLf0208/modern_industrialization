#!/usr/bin/env bash
# Proves the built content package (modern_industrialization.hmod) loads on a
# real game: the shared, unmodded 1d63e0a5 game with `--dev-mod <the .hmod>` must
# register modern_industrialization:electric_macerator and its items. A second
# launch WITHOUT `--dev-mod` is the negative control, so the check cannot pass
# vacuously. (The e2e proves the other half: one `.hnmod` installs the same
# content from `.horizon-cache` with no `--dev-mod` at all.)
set -euo pipefail
export LC_ALL=C
# Line-buffered evidence: every line goes to stderr (unbuffered when redirected).
exec 1>&2

CLONE=${CAMPAIGN_ROOT:-$(cd "$(dirname "$0")/../../.." && pwd)}
PIN_BIN=$CLONE/ports/_shared/bin/horizon-1d63e0a5
CTL=${CTL:-$PIN_BIN/horizon-ctl}
GAME=${GAME:-$PIN_BIN/horizon-client}
HMOD=${HMOD:-$CLONE/ports/modern_industrialization/content/modern_industrialization.hmod}
RES=${RES:-${HORIZON_RES:-${HORIZON_RES:-$HOME/.local/horizon-client/26.3}}}
SOCK=${SOCK:-/tmp/opencode/modern_industrialization-hmod.sock}

[ -x "$CTL" ] || { echo "missing horizon-ctl: $CTL" >&2; exit 1; }
[ -x "$GAME" ] || { echo "missing horizon-client: $GAME" >&2; exit 1; }
[ -f "$HMOD" ] || { echo "missing package: $HMOD" >&2; exit 1; }
[ -d "$RES/assets" ] || { echo "missing prepared resources: $RES" >&2; exit 1; }

SCRATCH=$(mktemp -d /tmp/opencode/modern_industrialization-hmod-home.XXXXXX)
export HORIZON_HOME=$SCRATCH/game
export HORIZON_LAUNCHER_HOME=$SCRATCH/launcher
export XDG_DATA_HOME=$SCRATCH/data
export XDG_CONFIG_HOME=$SCRATCH/config
export HORIZON_NO_AUDIO=1
mkdir -p "$(dirname "$SOCK")" "$XDG_CONFIG_HOME"
rm -f "$SOCK"

ctl() { "$CTL" --socket "$SOCK" "$@"; }
fail() { echo "FAIL: $*" >&2; exit 1; }

cleanup() {
    local status=$?
    ctl quit >/dev/null 2>&1 || true
    sleep 1
    if [ "${KEEP:-0}" = 1 ]; then echo "kept scratch: $SCRATCH" >&2; else rm -rf "$SCRATCH" 2>/dev/null || true; fi
    echo "EXIT:$status"
    exit "$status"
}
trap cleanup EXIT

launch() { # launch WORLD [--dev-mod PATH]
    local world=$1; shift
    rm -f "$SOCK"
    cd "$SCRATCH"
    "$CTL" --socket "$SOCK" launch \
        --game "$GAME" --world "$world" --seed 20261105 --creative --headless --wait \
        --resources "$RES" "$@" >/dev/null 2>>"$SCRATCH/game.log"
    ctl wait-ready 120
    ctl wait 20
}

echo "== launch WITH the package (--dev-mod $HMOD) =="
launch ModernIndustrializationHmod --dev-mod "$HMOD"
ctl command "setblock 0 64 0 modern_industrialization:electric_macerator" >/dev/null
ctl wait 4
block=$(ctl block 0 64 0 | jq -r '.name // .block // empty')
echo "block 0,64,0 = $block"
[ "$block" = "modern_industrialization:electric_macerator" ] || fail "the package did not register the electric macerator"
ctl give modern_industrialization:electric_macerator 1 >/dev/null
ctl wait 6
ctl inventory | jq -e '[.hotbar[] | select(.item == "modern_industrialization:electric_macerator")] | length >= 1' >/dev/null \
    || fail "the package did not register the electric macerator item: $(ctl inventory | jq -c '{hotbar,main}')"
echo "items: $(ctl inventory | jq -c '[.hotbar[] | select(.item == "modern_industrialization:electric_macerator") | .item]')"
ctl quit >/dev/null 2>&1 || true
sleep 2

echo "== negative control: the SAME ids without --dev-mod =="
launch ModernIndustrializationHmodControl
out=$(ctl command "setblock 0 64 0 modern_industrialization:electric_macerator")
echo "setblock reply: $out"
if grep -q 'Changed the block' <<<"$out"; then
    fail "the unmodded game accepted modern_industrialization:electric_macerator (the check is vacuous)"
fi
block=$(ctl block 0 64 0 | jq -r '.name // .block // empty')
echo "control block 0,64,0 = $block"
[ "$block" != "modern_industrialization:electric_macerator" ] || fail "the control block is the electric macerator"
ctl give modern_industrialization:electric_macerator 1 >/dev/null 2>&1 || true
ctl wait 6
ctl inventory | jq -e '[.hotbar[] | select(.item == "modern_industrialization:electric_macerator")] | length == 0' >/dev/null \
    || fail "the unmodded game gave a heat generator item"

echo "PASS: modern_industrialization.hmod registered modern_industrialization:electric_macerator and its item on a real game, and the same id is unknown without the package"
