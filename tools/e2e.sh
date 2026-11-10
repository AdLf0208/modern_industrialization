#!/usr/bin/env bash
# In-game check for Modern Industrialization slice 1: the electric macerator.
#
# The machine is one block: 32,000 FE (3,200 EU at the mod's own 1 EU = 10 FE),
# one item input, four item outputs, and `CrafterComponent`'s tick — 8 EU/t (80
# FE/t) base, 32 EU/t (320 FE/t) at the overclock cap, `eu * duration` EU a run.
#
# The native bundle is compiled into the game. By default the content comes from
# the bundle itself (EMBEDDED=1, the AE2 pattern: `--dev-mod` is NOT passed), and
# CONTROL=1 first launches the same binary with `.horizon-cache` unusable, where
# `modern_industrialization:electric_macerator` must be an unknown block — so the
# "one `.hnmod` installs block and code" claim cannot pass vacuously.
#
# Default run (Overworld, stone floor at y=100):
#  A  a fresh machine reads 0/32,000 FE, 0 FE/t, progress 0, efficiency 0/0;
#  B  control "input, no energy": a stone in the slot makes nothing and is kept;
#  C  control "energy, no input": 10,000 FE stay 10,000 FE for 40 ticks;
#  D  one stone becomes one cobblestone and the buffer loses exactly 2,000 FE
#     (200 EU at 8 EU/t: 25 ticks);
#  E  one iron ore becomes THREE raw iron and the buffer loses exactly 4,000 FE
#     (400 EU, 200 ticks of recipe time at 2 EU/t — the duration is honoured);
#  F  at efficiency 72 the machine runs at 320 FE/t: an ancient debris still
#     costs 1600 EU (16,000 FE) but now in 50 ticks instead of 200;
#  G  a run in progress survives save + quit + relaunch and finishes there:
#     a diamond costs exactly 16,000 FE (1600 EU, 800 ticks) whenever it ends.
#
# SELFTEST=1 flips B's "no energy, no output" assertion (it must FAIL), proving
# the scenario can fail.
set -Eeuo pipefail
export LC_ALL=C
[ -z "${MI_DEBUG:-}" ] || set -x

EMBEDDED=${EMBEDDED:-1}
CONTROL=${CONTROL:-$EMBEDDED}

CLONE=${CAMPAIGN_ROOT:-$(cd "$(dirname "$0")/../../.." && pwd)}
PORT=$CLONE/ports/modern_industrialization
PIN_BIN=$CLONE/ports/_shared/bin/horizon-1d63e0a5
CTL=${CTL:-$PIN_BIN/horizon-ctl}
GAME=${GAME:-$PORT/target/horizon-cargo/fast/horizon-client}
RES=${RES:-${HORIZON_RES:-${HORIZON_RES:-$HOME/.local/horizon-client/26.3}}}
SOCK=${SOCK:-/tmp/opencode/modern_industrialization-e2e.sock}
SELFTEST=${SELFTEST:-0}
KEEP=${KEEP:-0}
WORLD=ModernIndustrializationE2E
CONTROL_WORLD=ModernIndustrializationControl

# The numbers this run asserts (Modern Industrialization 2.5.12's own).
CAPACITY=32000        # MachineTier.LV.getMaxStoredEu() 3200 EU x 10 FE
RATE=80               # 8 EU/t x 10
OVERCLOCK_RATE=320    # 32 EU/t x 10
STONE_FE=2000         # 2 EU/t x 100 ticks = 200 EU
IRON_ORE_FE=4000      # 2 EU/t x 200 ticks = 400 EU
OVERCLOCK_TICKS=72    # the efficiency at which 200 EU reaches 32 EU/t
DIAMOND_FE=16000      # 2 EU/t x 800 ticks = 1600 EU
DEBRIS_FE=16000       # 2 EU/t x 800 ticks = 1600 EU (50 ticks at 32 EU/t)

[ -x "$CTL" ] || { echo "missing horizon-ctl: $CTL" >&2; exit 1; }
[ -x "$GAME" ] || { echo "missing horizon-client: $GAME" >&2; exit 1; }
[ -d "$RES/assets" ] || { echo "missing prepared resources: $RES" >&2; exit 1; }

SCRATCH=$(mktemp -d /tmp/opencode/modern_industrialization-home.XXXXXX)
export HORIZON_HOME=$SCRATCH/game
export HORIZON_LAUNCHER_HOME=$SCRATCH/launcher
export XDG_DATA_HOME=$SCRATCH/data
export XDG_CONFIG_HOME=$SCRATCH/config
export HORIZON_NO_AUDIO=1
mkdir -p "$(dirname "$SOCK")" "$XDG_CONFIG_HOME"
rm -f "$SOCK"

ctl() { "$CTL" --socket "$SOCK" "$@"; }
say() { ctl chat "/$1" >/dev/null; }
fail() { echo "FAIL: $*" >&2; ctl logs --lines 60 >&2 || true; exit 1; }
on_error() { echo "FAIL: line $1: '$2' exited $?" >&2; ctl logs --lines 40 >&2 || true; }
trap 'on_error "$LINENO" "$BASH_COMMAND"' ERR

cleanup() {
    local status=$?
    ctl quit >/dev/null 2>&1 || true
    sleep 1
    if [ "$KEEP" = 1 ]; then
        echo "kept scratch: $SCRATCH" >&2
    else
        rm -rf "$SCRATCH" 2>/dev/null || true
    fi
    # Last line of every run: `check-e2e-logs.py` reads it as the verdict.
    echo "EXIT:$status"
}
trap cleanup EXIT

launch() { # launch [WORLD]
    local world=${1:-$WORLD}
    rm -f "$SOCK"
    cd "$SCRATCH"
    local args=(--game "$GAME" --world "$world" --seed 20261105 --creative --headless
                --wait --resources "$RES")
    if [ "$EMBEDDED" = 1 ]; then
        echo "  launch: no --dev-mod; the bundle carries its own content" >&2
    else
        args+=(--dev-mod "$PORT/content")
    fi
    "$CTL" --socket "$SOCK" launch "${args[@]}" >/dev/null
    ctl wait-ready 120
    ctl wait 40
}

stop_game() { ctl quit >/dev/null 2>&1 || true; sleep 2; }

block_name() { ctl block "$1" "$2" "$3" | jq -r '.name // .block // empty'; }

# A stone floor at y=99 with clear air above, in the Overworld, and the player
# standing on it at (0.5, 100, 3.5).
arena() {
    ctl tp 0.5 101 3.5 >/dev/null
    ctl wait 20
    ctl command "fill -8 99 -8 24 99 24 minecraft:stone" >/dev/null
    ctl command "fill -8 100 -8 24 110 24 minecraft:air" >/dev/null
    ctl command "kill @e[type=!player]" >/dev/null
    ctl tp 0.5 100 3.5 >/dev/null
    ctl wait 20
    local y feet floor
    y=$(ctl state | jq -r '.player.position[1] // 0')
    feet=$(block_name 0 100 3)
    floor=$(block_name 0 99 3)
    echo "  arena: y=$y floor(0,99,3)=$floor feet(0,100,3)=$feet"
    awk -v y="$y" 'BEGIN { exit !(y >= 99.9) }' || fail "the player is not on the arena floor (y=$y)"
    [ "$feet" = "minecraft:air" ] || fail "the arena volume is not clear ($feet at 0,100,3)"
    [ "$floor" = "minecraft:stone" ] || fail "the arena floor is missing ($floor at 0,99,3)"
}

place() { # place X Y Z ID
    ctl command "setblock $1 $2 $3 $4" >/dev/null
    ctl wait 4
    local got
    got=$(block_name "$1" "$2" "$3")
    echo "  block($1,$2,$3) = $got"
    [ "$got" = "$4" ] || fail "expected $4 at $1,$2,$3, got '$got'"
}

# The non-null slots of the container at a position, as JSON.
slots() { ctl container "$1" "$2" "$3" | jq -c '[.slots[] | select(. != null)]'; }

# has X Y Z ITEM COUNT: the container holds exactly COUNT of ITEM.
has() {
    local got
    got=$(slots "$1" "$2" "$3")
    jq -e --arg item "$4" --argjson count "$5" \
        '[.[] | select(.item == $item) | .count] | add // 0 | . == $count' <<<"$got" >/dev/null \
        || fail "expected $5 x $4 at $1,$2,$3; got $got"
    echo "  has($1,$2,$3) $4 x$5: ok"
}

# info X Y Z: this machine's status line, after a fresh reply arrives (the t=
# field makes each reply unique).
info() { # info X Y Z
    local out line before
    before=$(ctl messages 20 | jq -r '.chat[]' | grep -E " at $1,$2,$3:" | head -1 || true)
    say "modern_industrialization info $1 $2 $3"
    for _ in $(seq 1 25); do
        ctl wait 4 >/dev/null
        out=$(ctl messages 20)
        line=$(jq -r '.chat[]' <<<"$out" | grep -E " at $1,$2,$3:|No block entity" | head -1 || true)
        if [ -n "$line" ] && [ "$line" != "$before" ]; then
            echo "$line"
            return
        fi
    done
    echo "NO-INFO $(jq -c '{chat,action_bar}' <<<"$out")"
}

# set_energy X Y Z AMOUNT: the harness control `/modern_industrialization energy`.
set_energy() { # X Y Z AMOUNT
    local want=$4 out
    say "modern_industrialization energy $want $1 $2 $3"
    for _ in $(seq 1 20); do
        ctl wait 2 >/dev/null
        out=$(ctl messages 20 | jq -r '.chat[]' || true)
        if grep -q "Energy set to $want FE." <<<"$out"; then
            echo "  set_energy($1,$2,$3) = $want FE"
            return
        fi
    done
    fail "could not set $1,$2,$3 to $want FE"
}

# set_efficiency X Y Z TICKS: the harness control that warms the machine.
set_efficiency() { # X Y Z TICKS
    local want=$4 out
    say "modern_industrialization efficiency $want $1 $2 $3"
    for _ in $(seq 1 20); do
        ctl wait 2 >/dev/null
        out=$(ctl messages 20 | jq -r '.chat[]' || true)
        if grep -q "Efficiency set to $want ticks." <<<"$out"; then
            echo "  set_efficiency($1,$2,$3) = $want"
            return
        fi
    done
    fail "could not set the efficiency of $1,$2,$3 to $want"
}

energy_of() { sed -nE 's/.* at [-0-9]+,[-0-9]+,[-0-9]+: ([0-9]+)\/[0-9]+ FE.*/\1/p' <<<"$1"; }
progress_of() { sed -nE 's/.*progress ([0-9]+)\/1000.*/\1/p' <<<"$1"; }
rate_of() { sed -nE 's/.*, ([0-9]+) FE\/t,.*/\1/p' <<<"$1"; }
efficiency_of() { sed -nE 's/.*efficiency ([0-9]+)\/[0-9]+.*/\1/p' <<<"$1"; }
active_of() { sed -nE 's/.*active ([^,]*), t=.*/\1/p' <<<"$1"; }

expect_info() { # expect_info X Y Z FIELD WANT
    local line got
    line=$(info "$1" "$2" "$3")
    echo "  info($1,$2,$3): $line"
    case "$4" in
        energy) got=$(energy_of "$line") ;;
        progress) got=$(progress_of "$line") ;;
        rate) got=$(rate_of "$line") ;;
        efficiency) got=$(efficiency_of "$line") ;;
        active) got=$(active_of "$line") ;;
        *) fail "unknown field $4" ;;
    esac
    [ "$got" = "$5" ] || fail "expected $4 = $5 at $1,$2,$3, got '$got' ($line)"
}

expect_not_zero_energy() { # the counterfactual that SELFTEST flips
    local line energy
    line=$(info "$1" "$2" "$3")
    energy=$(energy_of "$line")
    echo "  $line"
    if [ "$SELFTEST" = 1 ]; then
        echo "  SELFTEST: expecting a machine with no energy to have processed its input (this must fail)"
        [ "${energy:-0}" -gt 0 ] || fail "SELFTEST flipped: the machine with no energy made nothing, as it should"
    fi
    [ "${energy:-0}" = 0 ] || fail "a machine with no energy stored FE: $line"
}

# EMBEDDED=1: the bundle must have written its content out at startup, byte for
# byte the built `content/modern_industrialization.hmod`.
embedded_content_check() {
    local cache=$SCRATCH/.horizon-cache/modern_industrialization-embedded-content.hmod
    [ -f "$cache" ] || fail "the bundle did not write $cache"
    cmp -s "$cache" "$PORT/content/modern_industrialization.hmod" \
        || fail "the embedded content differs from content/modern_industrialization.hmod"
    echo "  embedded content: $(stat -c %s "$cache") B, byte-identical to content/modern_industrialization.hmod"
}

# CONTROL=1: the same binary with `.horizon-cache` made unusable — the bundle
# cannot write its content, so the block ids must be unknown.
control_leg() {
    echo "== control: the same binary without its embedded content =="
    rm -rf "$SCRATCH/.horizon-cache"
    : > "$SCRATCH/.horizon-cache"
    launch "$CONTROL_WORLD"
    local got
    ctl command "setblock 0 100 0 modern_industrialization:electric_macerator" >/dev/null
    ctl wait 4
    got=$(block_name 0 100 0)
    echo "  without the embedded content, block(0,100,0) = '${got:-air}'"
    [ "$got" != "modern_industrialization:electric_macerator" ] \
        || fail "the block registered without the embedded content: the check is vacuous"
    [ -f "$SCRATCH/.horizon-cache" ] || fail "the control's .horizon-cache was replaced: the helper wrote its content"
    stop_game
    rm -f "$SCRATCH/.horizon-cache"
}

echo "== launch (Overworld arena at y=100, one native bundle) =="
if [ "$CONTROL" = 1 ]; then control_leg; fi
launch
arena

echo "== A: a fresh machine is empty and idle =="
place 0 100 0 modern_industrialization:electric_macerator
line=$(info 0 100 0)
echo "  $line"
[ "$(energy_of "$line")" = 0 ] || fail "a fresh machine stored FE: $line"
[ "$(progress_of "$line")" = 0 ] || fail "a fresh machine has progress: $line"
[ "$(rate_of "$line")" = 0 ] || fail "a fresh machine reports a rate: $line"
[ "$(efficiency_of "$line")" = 0 ] || fail "a fresh machine is warm: $line"
[ "$(active_of "$line")" = none ] || fail "a fresh machine has a recipe: $line"
case "$line" in
    *"/$CAPACITY FE"*) ;;
    *) fail "the buffer is not $CAPACITY FE: $line" ;;
esac

echo "== B: control — input, no energy, nothing happens =="
place 0 100 4 modern_industrialization:electric_macerator
ctl container 0 100 4 minecraft:stone 1 >/dev/null
ctl wait 40
line=$(info 0 100 4)
echo "  $line"
[ "$(active_of "$line")" = none ] || fail "a machine with no energy started a run: $line"
[ "$(progress_of "$line")" = 0 ] || fail "a machine with no energy has progress: $line"
expect_not_zero_energy 0 100 4
has 0 100 4 minecraft:stone 1

echo "== C: control — energy, no input, nothing happens =="
place 4 100 0 modern_industrialization:electric_macerator
set_energy 4 100 0 10000
ctl wait 40
expect_info 4 100 0 energy 10000
expect_info 4 100 0 active none

echo "== D: one stone becomes one cobblestone for exactly $STONE_FE FE =="
set_energy 0 100 0 10000
ctl container 0 100 0 minecraft:stone 1 >/dev/null
ctl wait 40
line=$(info 0 100 0)
echo "  $line"
[ "$(energy_of "$line")" = $((10000 - STONE_FE)) ] \
    || fail "the stone run did not cost exactly $STONE_FE FE: $line"
[ "$(progress_of "$line")" = 0 ] || fail "the stone run did not finish: $line"
has 0 100 0 minecraft:cobblestone 1

echo "== E: one iron ore becomes three raw iron for exactly $IRON_ORE_FE FE =="
ctl container 0 100 0 minecraft:iron_ore 1 >/dev/null
ctl wait 70
line=$(info 0 100 0)
echo "  $line"
[ "$(energy_of "$line")" = $((10000 - STONE_FE - IRON_ORE_FE)) ] \
    || fail "the iron-ore run did not cost exactly $IRON_ORE_FE FE: $line"
has 0 100 0 minecraft:raw_iron 3
has 0 100 0 minecraft:cobblestone 1

echo "== F: at efficiency $OVERCLOCK_TICKS the machine runs at $OVERCLOCK_RATE FE/t =="
# An ancient debris is 800 ticks at 2 EU/t (1600 EU = 16,000 FE), so it is still
# running when the rate is read, and 1600 EU at 32 EU/t is 50 ticks.
set_efficiency 0 100 0 $OVERCLOCK_TICKS
expect_info 0 100 0 efficiency $OVERCLOCK_TICKS
set_energy 0 100 0 20000
ctl container 0 100 0 minecraft:ancient_debris 1 >/dev/null
ctl wait 3
line=$(info 0 100 0)
echo "  $line"
[ "$(rate_of "$line")" = $OVERCLOCK_RATE ] \
    || fail "the overclocked machine does not report $OVERCLOCK_RATE FE/t: $line"
[ "$(progress_of "$line")" -gt 0 ] || fail "the overclocked run did not start: $line"
ctl wait 70
line=$(info 0 100 0)
echo "  $line"
[ "$(energy_of "$line")" = $((20000 - DEBRIS_FE)) ] \
    || fail "the overclocked run did not cost exactly $DEBRIS_FE FE: $line"
[ "$(progress_of "$line")" = 0 ] || fail "the overclocked run did not finish: $line"
has 0 100 0 minecraft:netherite_scrap 2

echo "== G: a run in progress survives save, quit and relaunch =="
# A diamond is 800 ticks of recipe time at 2 EU/t (1600 EU = 16,000 FE), long
# enough that the save and the relaunch cannot finish it.
set_efficiency 0 100 0 0
set_energy 0 100 0 20000
ctl container 0 100 0 minecraft:diamond 1 >/dev/null
ctl wait 20
before=$(info 0 100 0)
echo "  before save: $before"
before_progress=$(progress_of "$before")
before_energy=$(energy_of "$before")
[ "${before_progress:-0}" -gt 0 ] || fail "the run did not start: $before"
[ "${before_progress:-0}" -lt 1000 ] || fail "the run finished before the save: $before"
[ "${before_energy:-0}" -lt 20000 ] || fail "the run spent no energy: $before"
ctl save | sed 's/^/  /'
stop_game
launch "$WORLD"
ctl command "execute in minecraft:overworld run tp @s 0.5 100 3.5" >/dev/null
ctl wait 40 >/dev/null
after=$(info 0 100 0)
echo "  after relaunch: $after"
[ "$(progress_of "$after")" -gt 0 ] || fail "the saved run was lost: $after"
[ "$(progress_of "$after")" -lt 1000 ] || fail "the saved run finished during the relaunch: $after"
[ "$(active_of "$after")" = "modern_industrialization:diamond_to_diamond_dust" ] \
    || fail "the saved recipe was lost: $after"
ctl wait 850
final=$(info 0 100 0)
echo "  resumed to: $final"
[ "$(energy_of "$final")" = $((20000 - DIAMOND_FE)) ] \
    || fail "the resumed run did not cost exactly $DIAMOND_FE FE: $final"
[ "$(progress_of "$final")" = 0 ] || fail "the resumed run did not finish: $final"
has 0 100 0 modern_industrialization:diamond_dust 1

if [ "$EMBEDDED" = 1 ]; then embedded_content_check; fi

if [ "$SELFTEST" = 1 ]; then
    echo "SELFTEST finished without failing (this is a bug in the test)"
    exit 1
fi
echo "PASS: the electric macerator holds 32,000 FE (3,200 EU), takes one input and gives up to four outputs, runs a stone at $RATE FE/t for exactly $STONE_FE FE (200 EU) into one cobblestone, an iron ore for exactly $IRON_ORE_FE FE (400 EU) into three raw iron, reaches $OVERCLOCK_RATE FE/t at efficiency $OVERCLOCK_TICKS while an ancient debris still costs 1600 EU (16,000 FE), makes nothing without energy and burns nothing without an input, and a half-finished run survives save and relaunch; one .hnmod installs both the block and the behaviour"
