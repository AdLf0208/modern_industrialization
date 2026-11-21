#!/usr/bin/env bash
# In-game check for Modern Industrialization slice 2: the coke oven multiblock, plus
# slice 1's electric macerator as the regression.
#
# Since the SDK migration (2026-11-18) the mod is one flat `.hmod` that the launcher
# compiles into the game, so this script needs a game the launcher built with it:
#
#   ports/tools/launcher-game.sh build modern_industrialization
#   eval "$(ports/tools/launcher-game.sh env modern_industrialization)"
#   build-slot.sh run --job W1-modern_industrialization --port ports/modern_industrialization \
#       --step e2e -- bash ports/modern_industrialization/tools/e2e.sh
#
# The blocks, their block entities, the recipe tables, the shell and the commands are
# inside the same package; there is no `--dev-mod` leg and no `.horizon-cache`. The
# control is the *same Nightly* without the package (`GAME=$CONTROL_GAME`, everything
# else identical), where `modern_industrialization:coke_oven` is an unknown block and
# the first `setblock` assertion fails.
#
# The coke oven is upstream's shape, block for block: `ShapeTemplate.Builder(BRICKS)
# .add3by3Levels(-1, 1, bricks, hatches).build()` is 25 `minecraft:bricks` in a 3x3x3
# box whose front-centre of the middle layer is the controller, with the interior
# (0,0,1) hollow and the controller's own position removed. Its recipe is the jar's
# `materials/coke_oven/coke.json`: 2 EU/t for 600 ticks (1,200 EU = 12,000 FE at the
# mod's own 1 EU = 10 FE), one coal in, one coke out.
#
# Legs (Overworld, stone floor at y=99, the oven's controller at 8,101,0):
#  A  the controller alone is `formed false, missing 25` and exposes its two slots;
#  B  control — an unformed oven runs nothing and burns nothing over 620 ticks, and
#     keeps the coal in its input slot;
#  C  a 3x3x3 shell of 25 bricks forms it: `formed true, missing 0`, the run reports
#     20 FE/t, and one coal becomes exactly one coke for exactly 12,000 FE in 600
#     ticks (the coal is consumed at the start, the coke appears at the end);
#  D  breaking one brick flips `formed` to false and freezes the run (no energy, no
#     progress, no product over 120 ticks); the macerator standing in the gap does not
#     re-form it (the shell is block-specific); a brick put back re-forms it and the
#     paused run finishes, still for exactly 12,000 FE in all;
#  E..K the macerator: the 32,000 FE buffer, the four outputs, the LV overclock, and a
#     half-finished run surviving save + quit + relaunch.
#
# SELFTEST=1 runs only the coke oven legs with the formed assertion inverted (B must
# FAIL), so the fail-capability is the multiblock's, not the machine's.
set -Eeuo pipefail
export LC_ALL=C
[ -z "${MI_DEBUG:-}" ] || set -x

CLONE=/home/adlf/Desktop/all_the_mods_10_rust_rewrite
PORT=$CLONE/ports/modern_industrialization
PIN_BIN=$CLONE/ports/_shared/bin/horizon-1d63e0a5
CTL=${CTL:-$PIN_BIN/horizon-ctl}
GAME=${GAME:-}
RES=${RES:-${HORIZON_RES:-/home/adlf/Desktop/horizon/.local/horizon-client/26.3}}
SOCK=${SOCK:-/tmp/opencode/modern_industrialization-e2e.sock}
SELFTEST=${SELFTEST:-0}
KEEP=${KEEP:-0}
WORLD=ModernIndustrializationE2E

# The macerator's numbers (Modern Industrialization 2.5.12's own).
CAPACITY=32000        # MachineTier.LV.getMaxStoredEu() 3200 EU x 10 FE
RATE=80               # 8 EU/t x 10
OVERCLOCK_RATE=320    # 32 EU/t x 10
STONE_FE=2000         # 2 EU/t x 100 ticks = 200 EU
IRON_ORE_FE=4000      # 2 EU/t x 200 ticks = 400 EU
OVERCLOCK_TICKS=72    # the efficiency at which 200 EU reaches 32 EU/t
DIAMOND_FE=16000      # 2 EU/t x 800 ticks = 1600 EU
DEBRIS_FE=16000       # 2 EU/t x 800 ticks = 1600 EU (50 ticks at 32 EU/t)

# The coke oven's numbers.
OVEN_CAPACITY=32000   # the same LV buffer, 3,200 EU x 10
OVEN_RATE=20          # the steam tier: 2 EU/t x 10
OVEN_FE=12000         # 2 EU/t x 600 ticks = 1,200 EU
OVEN_SHELL=25         # 9 + 7 + 9 positions of minecraft:bricks
COKE_OVEN_AT="8 101 0"

[ -x "$CTL" ] || { echo "missing horizon-ctl: $CTL" >&2; exit 1; }
[ -n "$GAME" ] || {
    echo "GAME is unset: the SDK package is compiled into the game by the launcher." >&2
    echo "  ports/tools/launcher-game.sh build modern_industrialization" >&2
    echo "  eval \"\$(ports/tools/launcher-game.sh env modern_industrialization)\"" >&2
    exit 1
}
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
    echo "  launch: the launcher-built game carries the package; no --dev-mod" >&2
    "$CTL" --socket "$SOCK" launch --game "$GAME" --world "$world" --seed 20261105 --creative \
        --headless --wait --resources "$RES" >/dev/null
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

# How many item slots the block entity at a position exposes to the engine.
slot_count() { ctl container "$1" "$2" "$3" | jq -r '.slots | length'; }

# expect_slots X Y Z COUNT: the block entity exposes exactly COUNT item slots.
# The declared `ModContainer` is not enough: the engine sizes a mod block
# entity's own `items` to it only on a tick that answers `Changed`, so a fresh
# or reloaded machine whose ticks answer `Unchanged` exposes none.
expect_slots() { # X Y Z COUNT
    local got
    got=$(slot_count "$1" "$2" "$3")
    echo "  slots($1,$2,$3) = $got"
    [ "$got" = "$4" ] \
        || fail "the block entity at $1,$2,$3 exposes $got item slots, expected $4: $(ctl container "$1" "$2" "$3" | jq -c '.')"
}

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

# set_efficiency X Y Z TICKS: the harness control that warms the macerator.
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
formed_of() { sed -nE 's/.*formed (true|false).*/\1/p' <<<"$1"; }
missing_of() { sed -nE 's/.*, missing ([0-9]+),.*/\1/p' <<<"$1"; }

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
        formed) got=$(formed_of "$line") ;;
        missing) got=$(missing_of "$line") ;;
        *) fail "unknown field $4" ;;
    esac
    [ "$got" = "$5" ] || fail "expected $4 = $5 at $1,$2,$3, got '$got' ($line)"
}

# expect_oven X Y Z FORMED MISSING: the controller's shell answer, parsed.
expect_oven() { # X Y Z FORMED(true|false) MISSING
    local line got_formed got_missing
    line=$(info "$1" "$2" "$3")
    echo "  oven($1,$2,$3): $line"
    got_formed=$(formed_of "$line")
    got_missing=$(missing_of "$line")
    [ "$got_formed" = "$4" ] || fail "expected formed $4 at $1,$2,$3, got '$got_formed' ($line)"
    [ "$got_missing" = "$5" ] || fail "expected missing $5 at $1,$2,$3, got '$got_missing' ($line)"
}

# build_shell X Y Z: the 25 bricks of upstream's coke oven shape around a controller
# at X,Y,Z, skipping the controller's own position and the hollow interior (0,0,1).
build_shell() { # X Y Z
    local x=$1 y=$2 z=$3 built=0 dx dy dz
    for dy in -1 0 1; do
        for dx in -1 0 1; do
            for dz in 0 1 2; do
                if [ "$dx" = 0 ] && [ "$dy" = 0 ] && [ "$dz" != 2 ]; then
                    continue   # the controller (0,0,0) and the interior (0,0,1)
                fi
                ctl command "setblock $((x + dx)) $((y + dy)) $((z + dz)) minecraft:bricks" >/dev/null
                built=$((built + 1))
            done
        done
    done
    ctl wait 4
    echo "  shell($x,$y,$z) = $built bricks"
    [ "$built" = "$OVEN_SHELL" ] || fail "the shape built $built members, expected $OVEN_SHELL"
}

# ---------------------------------------------------------------------------
# The coke oven multiblock (slice 2)
# ---------------------------------------------------------------------------

# empty_oven: the controller carries one input and one output slot (the item input and
# output hatches upstream puts in the shell), so a run whose product differs from what
# the output slot holds cannot start. A player empties it by breaking the controller and
# putting it back — which the whole shell still standing re-forms at once, so this is
# also the "controller placed last on a waiting shell" check. The player stands in front
# of the controller's face: from behind, the shell's own back wall is what the click hits.
empty_oven() {
    local ox oy oz
    read -r ox oy oz <<<"$COKE_OVEN_AT"
    ctl command "execute in minecraft:overworld run tp @s $ox.5 $oy -1.5" >/dev/null
    ctl wait 4
    ctl break "$ox" "$oy" "$oz" >/dev/null
    ctl wait 6
    [ "$(block_name "$ox" "$oy" "$oz")" = "minecraft:air" ] \
        || fail "the controller did not break: $(block_name "$ox" "$oy" "$oz")"
    place "$ox" "$oy" "$oz" modern_industrialization:coke_oven
    expect_oven "$ox" "$oy" "$oz" true 0
    expect_slots "$ox" "$oy" "$oz" 2
}

oven_legs() {
    echo "== A: the coke oven controller alone is unformed and names the 25 missing members =="
    place $COKE_OVEN_AT modern_industrialization:coke_oven
    # One input, one output: the hatches are not ported, so the controller carries them.
    expect_slots $COKE_OVEN_AT 2
    expect_oven $COKE_OVEN_AT false "$OVEN_SHELL"

    echo "== B: control — an unformed coke oven runs nothing and burns nothing over 620 ticks =="
    set_energy $COKE_OVEN_AT 20000
    ctl container $COKE_OVEN_AT minecraft:coal 1 >/dev/null
    ctl wait 620
    line=$(info $COKE_OVEN_AT)
    echo "  $line"
    if [ "$SELFTEST" = 1 ]; then
        echo "  SELFTEST: expecting the unformed oven to have burned its coal and made coke (this must fail)"
        if [ "$(energy_of "$line")" = 20000 ] && [ "$(progress_of "$line")" = 0 ]; then
            fail "SELFTEST flipped: the unformed oven ran nothing and burned nothing, as it should"
        fi
    fi
    [ "$(energy_of "$line")" = 20000 ] || fail "an unformed oven burned energy: $line"
    [ "$(progress_of "$line")" = 0 ] || fail "an unformed oven has progress: $line"
    [ "$(active_of "$line")" = none ] || fail "an unformed oven started a run: $line"
    has $COKE_OVEN_AT minecraft:coal 1
    has $COKE_OVEN_AT modern_industrialization:coke 0

    echo "== C: a 3x3x3 shell of $OVEN_SHELL bricks forms it; one coal becomes one coke for exactly $OVEN_FE FE =="
    build_shell $COKE_OVEN_AT
    expect_oven $COKE_OVEN_AT true 0
    ctl wait 30
    line=$(info $COKE_OVEN_AT)
    echo "  running: $line"
    [ "$(active_of "$line")" = modern_industrialization:coke_oven_coal_to_coke ] \
        || fail "the formed oven did not start the jar's coke recipe: $line"
    [ "$(rate_of "$line")" = "$OVEN_RATE" ] \
        || fail "the steam tier does not report $OVEN_RATE FE/t: $line"
    progress=$(progress_of "$line")
    [ "${progress:-0}" -gt 0 ] || fail "the formed oven has no progress: $line"
    [ "${progress:-1000}" -lt 1000 ] || fail "the formed oven finished in 30 ticks: $line"
    has $COKE_OVEN_AT minecraft:coal 0
    ctl wait 620
    line=$(info $COKE_OVEN_AT)
    echo "  $line"
    [ "$(energy_of "$line")" = $((20000 - OVEN_FE)) ] \
        || fail "the coke run did not cost exactly $OVEN_FE FE: $line"
    [ "$(progress_of "$line")" = 0 ] || fail "the coke run did not finish: $line"
    [ "$(active_of "$line")" = none ] || fail "the coke run did not end: $line"
    has $COKE_OVEN_AT modern_industrialization:coke 1

    echo "== C2: the jar's second coke oven recipe (coal dust), and the items the package ships =="
    # C's coke is in the output slot; the dust run needs it empty.
    empty_oven
    set_energy 8 101 0 20000
    ctl container 8 101 0 modern_industrialization:coal_dust 1 >/dev/null
    ctl wait 620
    line=$(info 8 101 0)
    echo "  $line"
    [ "$(energy_of "$line")" = $((20000 - OVEN_FE)) ] \
        || fail "the coal-dust run did not cost exactly $OVEN_FE FE: $line"
    [ "$(progress_of "$line")" = 0 ] || fail "the coal-dust run did not finish: $line"
    has 8 101 0 modern_industrialization:coke_dust 1
    known=$(ctl give modern_industrialization:coke_dust 1)
    echo "  give(coke_dust) = $known"
    jq -e '.ok == true' <<<"$known" >/dev/null || fail "give coke_dust failed: $known"
    unknown=$(ctl give modern_industrialization:no_such_item 1 || true)
    echo "  give(no_such_item) = $unknown"
    if jq -e '.ok == true' <<<"$unknown" >/dev/null 2>&1; then
        fail "an unknown item id was accepted: $unknown"
    fi

    echo "== D: breaking one brick unforms the oven and freezes the run =="
    # C2's coke dust is in the output slot and this run makes coke: empty it first.
    empty_oven
    set_energy $COKE_OVEN_AT 20000
    ctl container $COKE_OVEN_AT minecraft:coal 1 >/dev/null
    ctl wait 60
    running=$(info $COKE_OVEN_AT)
    echo "  before the break: $running"
    [ "$(progress_of "$running")" -gt 0 ] || fail "the second run did not start: $running"
    [ "$(progress_of "$running")" -lt 1000 ] || fail "the second run finished too early: $running"
    # A real player break (not `/setblock air`): the shell must unform on the same
    # `Level.setBlock` the block_changed hook hears, whichever way the block went.
    ctl command "execute in minecraft:overworld run tp @s 8.5 101 4.5" >/dev/null
    ctl wait 4
    ctl break 9 102 2 >/dev/null
    ctl wait 6
    [ "$(block_name 9 102 2)" = "minecraft:air" ] \
        || fail "the player break did not remove the brick at 9,102,2: $(block_name 9 102 2)"
    expect_oven $COKE_OVEN_AT false 1
    frozen=$(info $COKE_OVEN_AT)
    frozen_energy=$(energy_of "$frozen")
    frozen_progress=$(progress_of "$frozen")
    ctl wait 120
    line=$(info $COKE_OVEN_AT)
    echo "  after 120 unformed ticks: $line"
    [ "$(energy_of "$line")" = "$frozen_energy" ] \
        || fail "an unformed oven kept burning energy: $line"
    [ "$(progress_of "$line")" = "$frozen_progress" ] \
        || fail "an unformed oven kept making progress: $line"
    has $COKE_OVEN_AT modern_industrialization:coke 0
    echo "  a wrong block in the gap does not re-form it (the shell is block-specific)"
    place 9 102 2 modern_industrialization:electric_macerator
    ctl wait 6
    expect_oven $COKE_OVEN_AT false 1
    echo "  a brick put back re-forms it and the paused run finishes"
    place 9 102 2 minecraft:bricks
    ctl wait 6
    expect_oven $COKE_OVEN_AT true 0
    ctl wait 620
    line=$(info $COKE_OVEN_AT)
    echo "  resumed: $line"
    [ "$(energy_of "$line")" = $((20000 - OVEN_FE)) ] \
        || fail "the paused ticks cost nothing, so the whole run is $OVEN_FE FE: $line"
    [ "$(progress_of "$line")" = 0 ] || fail "the resumed run did not finish: $line"
    has $COKE_OVEN_AT modern_industrialization:coke 1
}

# ---------------------------------------------------------------------------
# The electric macerator (slice 1, the regression)
# ---------------------------------------------------------------------------

macerator_legs() {
    echo "== E: a fresh machine is empty and idle =="
    place 0 100 0 modern_industrialization:electric_macerator
    # The declared container (one input, four outputs) must be visible to the
    # engine on the very first tick, not only once something has changed.
    expect_slots 0 100 0 5
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

    echo "== F: control — input, no energy, nothing happens =="
    place 0 100 4 modern_industrialization:electric_macerator
    ctl container 0 100 4 minecraft:stone 1 >/dev/null
    ctl wait 40
    line=$(info 0 100 4)
    echo "  $line"
    [ "$(active_of "$line")" = none ] || fail "a machine with no energy started a run: $line"
    [ "$(progress_of "$line")" = 0 ] || fail "a machine with no energy has progress: $line"
    [ "$(energy_of "$line")" = 0 ] || fail "a machine with no energy stored FE: $line"
    has 0 100 4 minecraft:stone 1

    echo "== G: control — energy, no input, nothing happens =="
    place 4 100 0 modern_industrialization:electric_macerator
    set_energy 4 100 0 10000
    ctl wait 40
    expect_info 4 100 0 energy 10000
    expect_info 4 100 0 active none

    echo "== H: one stone becomes one cobblestone for exactly $STONE_FE FE =="
    set_energy 0 100 0 10000
    ctl container 0 100 0 minecraft:stone 1 >/dev/null
    ctl wait 40
    line=$(info 0 100 0)
    echo "  $line"
    [ "$(energy_of "$line")" = $((10000 - STONE_FE)) ] \
        || fail "the stone run did not cost exactly $STONE_FE FE: $line"
    [ "$(progress_of "$line")" = 0 ] || fail "the stone run did not finish: $line"
    has 0 100 0 minecraft:cobblestone 1

    echo "== I: one iron ore becomes three raw iron for exactly $IRON_ORE_FE FE =="
    ctl container 0 100 0 minecraft:iron_ore 1 >/dev/null
    ctl wait 70
    line=$(info 0 100 0)
    echo "  $line"
    [ "$(energy_of "$line")" = $((10000 - STONE_FE - IRON_ORE_FE)) ] \
        || fail "the iron-ore run did not cost exactly $IRON_ORE_FE FE: $line"
    has 0 100 0 minecraft:raw_iron 3
    has 0 100 0 minecraft:cobblestone 1

    echo "== J: at efficiency $OVERCLOCK_TICKS the machine runs at $OVERCLOCK_RATE FE/t =="
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

    echo "== K: a run in progress survives save, quit and relaunch =="
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
    # A reloaded machine gets its five slots back too (the saved list keeps only
    # the occupied ones, so the first tick after the load has to grow it again).
    expect_slots 0 100 0 5
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
}

echo "== launch (Overworld arena at y=100, the launcher-built game carrying the package) =="
launch
arena

if [ "$SELFTEST" = 1 ]; then
    echo "== SELFTEST: the macerator legs are skipped; the coke oven's formed assertion runs inverted =="
else
    macerator_legs
fi
oven_legs

if [ "$SELFTEST" = 1 ]; then
    echo "SELFTEST finished without failing (this is a bug in the test)"
    exit 1
fi
echo "PASS: the coke oven is a 3x3x3 shell of 25 bricks around its controller: unformed it runs nothing and burns nothing, formed it reports 20 FE/t and one coal becomes one coke for exactly 12,000 FE (1,200 EU in 600 ticks), breaking one brick flips formed to false and freezes the run, a macerator in the gap does not re-form it, and a brick put back resumes the run for the recipe's own energy; the electric macerator still holds 32,000 FE (3,200 EU), runs a stone at $RATE FE/t for exactly $STONE_FE FE into one cobblestone, an iron ore for $IRON_ORE_FE FE into three raw iron, reaches $OVERCLOCK_RATE FE/t at efficiency $OVERCLOCK_TICKS while an ancient debris still costs 1600 EU (16,000 FE), makes nothing without energy and burns nothing without an input, and a half-finished run survives save and relaunch; one .hmod carries the blocks, the data and the behaviour"
