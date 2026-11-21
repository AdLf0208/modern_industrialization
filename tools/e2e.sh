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
#  C  a 3x3x3 shell of 22 bricks and three bronze hatches (item in, item out, fluid
#     in) forms it: `formed true, missing 0, hatches 3`, and each hatch exposes its
#     own storage (one slot each, no item slots in the fluid hatch);
#  D  one coal put into the ITEM INPUT HATCH (not the controller) is pumped into the
#     run, and one coke appears in the ITEM OUTPUT HATCH after exactly 12,000 FE in
#     600 ticks; the coal dust run does the same for coke dust;
#  E  breaking one brick flips `formed` to false and freezes the run (no energy, no
#     progress, no product over 120 ticks); a coil standing in the gap does not re-form
#     it (the shell is block-specific); a brick put back re-forms it and the paused run
#     finishes, still for exactly 12,000 FE in all;
#  F  a hatch in a middle-ring position (HatchFlags.NO_HATCH) does not form the oven;
#  G  `/modern_industrialization recipes` reads the coke oven recipe kind back through
#     `recipes_of_type`: 2 recipes, their ids and the file's `duration: 600`;
#  H  the add-on companions and the matcher's 3x3x4 harness check: 17 bronze-plated
#     bricks + 16 coil positions (two holding a hatch) around a controller form, one
#     coil broken is `missing 1`, and a brick where a coil belongs is still unformed;
#  I..O the macerator: the 32,000 FE buffer, the four outputs, the LV overclock, and a
#     half-finished run surviving save + quit + relaunch.
#
# SELFTEST=1 runs only the coke oven legs A/B with the unformed assertion inverted (B
# must FAIL), so the fail-capability is the multiblock's, not the machine's.
set -Eeuo pipefail
export LC_ALL=C
[ -z "${MI_DEBUG:-}" ] || set -x

CLONE=/home/adlf/Desktop/all_the_mods_10_rust_rewrite
PORT=$CLONE/ports/modern_industrialization
PIN_BIN=$CLONE/ports/_shared/bin/horizon-1d63e0a5
CTL=${CTL:-$PIN_BIN/horizon-ctl}
GAME=${GAME:-}
RES=${RES:-${HORIZON_RES:-/home/adlf/Desktop/horizon/.local/horizon-client/26.3}}
# `/tmp` is a 7.5 GB tmpfs that has been 100 % full; `launcher-game.sh env`
# exports E2E_TMP under `~/.cache/horizon-e2e/scratch`, and `SOCK` follows it.
E2E_TMP=${E2E_TMP:-/tmp/opencode}
SOCK=${SOCK:-$E2E_TMP/modern_industrialization-e2e.sock}
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
OVEN_SHELL=25         # 9 + 7 + 9 members: 22 bricks and 3 bronze hatches
COKE_OVEN_AT="8 101 0"
# The three hatch positions: the floor's -x corner, the roof's +x/+z corner and the
# floor's centre. All are end-layer positions, which is where the jar's HatchFlags
# allow a hatch; the middle ring is brick-only.
HATCH_IN="7 100 0"          # item input hatch, offset [-1,-1,0]
HATCH_OUT="9 102 2"         # item output hatch, offset [1,1,2]
HATCH_FLUID="8 100 0"       # fluid input hatch, offset [0,-1,0]
# The matcher's 3x3x4 harness shape (the add-on's pyrolyse shell), at 20,101,0.
SHAPE_AT="20 101 0"

[ -x "$CTL" ] || { echo "missing horizon-ctl: $CTL" >&2; exit 1; }
[ -n "$GAME" ] || {
    echo "GAME is unset: the SDK package is compiled into the game by the launcher." >&2
    echo "  ports/tools/launcher-game.sh build modern_industrialization" >&2
    echo "  eval \"\$(ports/tools/launcher-game.sh env modern_industrialization)\"" >&2
    exit 1
}
[ -x "$GAME" ] || { echo "missing horizon-client: $GAME" >&2; exit 1; }
[ -d "$RES/assets" ] || { echo "missing prepared resources: $RES" >&2; exit 1; }

mkdir -p "$E2E_TMP"
SCRATCH=$(mktemp -d "$E2E_TMP/modern_industrialization-home.XXXXXX")
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
hatches_of() { sed -nE 's/.*hatches ([0-9]+).*/\1/p' <<<"$1"; }

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

# expect_oven X Y Z FORMED MISSING [HATCHES]: the controller's shell answer, parsed.
expect_oven() { # X Y Z FORMED(true|false) MISSING [HATCHES]
    local line got_formed got_missing got_hatches
    line=$(info "$1" "$2" "$3")
    echo "  oven($1,$2,$3): $line"
    got_formed=$(formed_of "$line")
    got_missing=$(missing_of "$line")
    [ "$got_formed" = "$4" ] || fail "expected formed $4 at $1,$2,$3, got '$got_formed' ($line)"
    [ "$got_missing" = "$5" ] || fail "expected missing $5 at $1,$2,$3, got '$got_missing' ($line)"
    if [ $# -ge 6 ]; then
        got_hatches=$(hatches_of "$line")
        [ "$got_hatches" = "$6" ] || fail "expected hatches $6 at $1,$2,$3, got '$got_hatches' ($line)"
    fi
}

# build_shell X Y Z: upstream's coke oven shape around a controller at X,Y,Z — 25
# members (skipping the controller and the hollow interior (0,0,1)): 22 bricks and
# the three bronze hatches at the positions this script names.
build_shell() { # X Y Z
    local x=$1 y=$2 z=$3 built=0 hatches=0 dx dy dz id px py pz
    for dy in -1 0 1; do
        for dx in -1 0 1; do
            for dz in 0 1 2; do
                if [ "$dx" = 0 ] && [ "$dy" = 0 ] && [ "$dz" != 2 ]; then
                    continue   # the controller (0,0,0) and the interior (0,0,1)
                fi
                px=$((x + dx)); py=$((y + dy)); pz=$((z + dz))
                id=minecraft:bricks
                if [ "$px $py $pz" = "$HATCH_IN" ]; then
                    id=modern_industrialization:bronze_item_input_hatch; hatches=$((hatches + 1))
                elif [ "$px $py $pz" = "$HATCH_OUT" ]; then
                    id=modern_industrialization:bronze_item_output_hatch; hatches=$((hatches + 1))
                elif [ "$px $py $pz" = "$HATCH_FLUID" ]; then
                    id=modern_industrialization:bronze_fluid_input_hatch; hatches=$((hatches + 1))
                fi
                ctl command "setblock $px $py $pz $id" >/dev/null
                built=$((built + 1))
            done
        done
    done
    ctl wait 4
    echo "  shell($x,$y,$z) = $((built - hatches)) bricks + $hatches hatches"
    [ "$built" = "$OVEN_SHELL" ] || fail "the shape built $built members, expected $OVEN_SHELL"
    [ "$hatches" = 3 ] || fail "the shape placed $hatches hatches, expected 3"
}

# ---------------------------------------------------------------------------
# The coke oven multiblock (slice 2)
# ---------------------------------------------------------------------------

# reset_hatch X Y Z ID: empty a hatch by breaking it and putting it back. A
# block entity's contents are lost with the block, and the whole shell still
# standing re-forms the oven at once. The player stands in front of the shell.
reset_hatch() { # X Y Z ID
    ctl command "execute in minecraft:overworld run tp @s 8.5 101 -1.5" >/dev/null
    ctl wait 4
    ctl command "setblock $1 $2 $3 minecraft:air" >/dev/null
    ctl wait 4
    place "$1" "$2" "$3" "$4"
    ctl wait 6
    expect_oven $COKE_OVEN_AT true 0
}

expect_hatch_storage() { # X Y Z SLOTS
    local got
    got=$(slot_count "$1" "$2" "$3")
    echo "  hatch slots($1,$2,$3) = $got"
    [ "$got" = "$4" ] || fail "the hatch at $1,$2,$3 exposes $got item slots, expected $4"
}

oven_legs() {
    echo "== A: the coke oven controller alone is unformed and names the 25 missing members =="
    place $COKE_OVEN_AT modern_industrialization:coke_oven
    # The controller keeps its own two slots as the run's internal buffer; the
    # hatches the shell adds are what automation reaches (D).
    expect_slots $COKE_OVEN_AT 2
    expect_oven $COKE_OVEN_AT false "$OVEN_SHELL" 0

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

    echo "== C: a 3x3x3 shell of 22 bricks and three bronze hatches forms it =="
    # B put a coal in the controller's own slot; empty it so the hatch is the only
    # way in (a block entity's contents go with the block, and the shell re-forms).
    ctl command "execute in minecraft:overworld run tp @s 8.5 101 -1.5" >/dev/null
    ctl wait 4
    ctl break 8 101 0 >/dev/null
    ctl wait 6
    place $COKE_OVEN_AT modern_industrialization:coke_oven
    build_shell $COKE_OVEN_AT
    expect_oven $COKE_OVEN_AT true 0 3
    [ "$(block_name $HATCH_IN)" = "modern_industrialization:bronze_item_input_hatch" ] \
        || fail "the input hatch is not at $HATCH_IN: $(block_name $HATCH_IN)"
    [ "$(block_name $HATCH_OUT)" = "modern_industrialization:bronze_item_output_hatch" ] \
        || fail "the output hatch is not at $HATCH_OUT: $(block_name $HATCH_OUT)"
    [ "$(block_name $HATCH_FLUID)" = "modern_industrialization:bronze_fluid_input_hatch" ] \
        || fail "the fluid hatch is not at $HATCH_FLUID: $(block_name $HATCH_FLUID)"
    expect_hatch_storage $HATCH_IN 1
    expect_hatch_storage $HATCH_OUT 1
    expect_hatch_storage $HATCH_FLUID 0   # a fluid hatch has a tank, not an item slot
    expect_slots $COKE_OVEN_AT 2          # the controller's internal run buffer

    echo "== D: one coal in the ITEM INPUT HATCH becomes one coke in the ITEM OUTPUT HATCH =="
    set_energy $COKE_OVEN_AT 20000
    ctl container $HATCH_IN minecraft:coal 1 >/dev/null
    ctl wait 30
    line=$(info $COKE_OVEN_AT)
    echo "  running: $line"
    [ "$(active_of "$line")" = modern_industrialization:coke_oven_coal_to_coke ] \
        || fail "the hatch's coal did not start the jar's coke recipe: $line"
    [ "$(rate_of "$line")" = "$OVEN_RATE" ] \
        || fail "the steam tier does not report $OVEN_RATE FE/t: $line"
    [ "$(progress_of "$line")" -gt 0 ] || fail "the formed oven has no progress: $line"
    [ "$(progress_of "$line")" -lt 1000 ] || fail "the formed oven finished in 30 ticks: $line"
    has $HATCH_IN minecraft:coal 0     # the pump moved it into the run
    ctl wait 620
    line=$(info $COKE_OVEN_AT)
    echo "  $line"
    [ "$(energy_of "$line")" = $((20000 - OVEN_FE)) ] \
        || fail "the coke run did not cost exactly $OVEN_FE FE: $line"
    [ "$(progress_of "$line")" = 0 ] || fail "the coke run did not finish: $line"
    [ "$(active_of "$line")" = none ] || fail "the coke run did not end: $line"
    has $HATCH_OUT modern_industrialization:coke 1   # the pump moved it out
    has $COKE_OVEN_AT modern_industrialization:coke 0

    echo "== D2: the jar's second coke oven recipe (coal dust), through the hatch =="
    reset_hatch $HATCH_OUT modern_industrialization:bronze_item_output_hatch
    set_energy $COKE_OVEN_AT 20000
    ctl container $HATCH_IN modern_industrialization:coal_dust 1 >/dev/null
    ctl wait 620
    line=$(info $COKE_OVEN_AT)
    echo "  $line"
    [ "$(energy_of "$line")" = $((20000 - OVEN_FE)) ] \
        || fail "the coal-dust run did not cost exactly $OVEN_FE FE: $line"
    [ "$(progress_of "$line")" = 0 ] || fail "the coal-dust run did not finish: $line"
    has $HATCH_OUT modern_industrialization:coke_dust 1

    echo "== D3: the items the package ships and an unknown id =="
    known=$(ctl give modern_industrialization:coke_dust 1)
    echo "  give(coke_dust) = $known"
    jq -e '.ok == true' <<<"$known" >/dev/null || fail "give coke_dust failed: $known"
    unknown=$(ctl give modern_industrialization:no_such_item 1 || true)
    echo "  give(no_such_item) = $unknown"
    if jq -e '.ok == true' <<<"$unknown" >/dev/null 2>&1; then
        fail "an unknown item id was accepted: $unknown"
    fi

    echo "== E: breaking one brick unforms the oven and freezes the run =="
    reset_hatch $HATCH_OUT modern_industrialization:bronze_item_output_hatch
    set_energy $COKE_OVEN_AT 20000
    ctl container $HATCH_IN minecraft:coal 1 >/dev/null
    ctl wait 60
    running=$(info $COKE_OVEN_AT)
    echo "  before the break: $running"
    [ "$(progress_of "$running")" -gt 0 ] || fail "the second run did not start: $running"
    [ "$(progress_of "$running")" -lt 1000 ] || fail "the second run finished too early: $running"
    # A real player break (not `/setblock air`) from behind the shell: the shell must
    # unform on the same `Level.setBlock` the block_changed hook hears.
    ctl command "execute in minecraft:overworld run tp @s 8.5 101 4.5" >/dev/null
    ctl wait 4
    ctl break 7 102 2 >/dev/null
    ctl wait 6
    [ "$(block_name 7 102 2)" = "minecraft:air" ] \
        || fail "the player break did not remove the brick at 7,102,2: $(block_name 7 102 2)"
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
    has $HATCH_OUT modern_industrialization:coke 0
    echo "  a coil in the gap does not re-form it (the shell is block-specific)"
    place 7 102 2 modern_industrialization:cupronickel_coil
    ctl wait 6
    expect_oven $COKE_OVEN_AT false 1
    echo "  a brick put back re-forms it and the paused run finishes"
    place 7 102 2 minecraft:bricks
    ctl wait 6
    expect_oven $COKE_OVEN_AT true 0 3
    ctl wait 620
    line=$(info $COKE_OVEN_AT)
    echo "  resumed: $line"
    [ "$(energy_of "$line")" = $((20000 - OVEN_FE)) ] \
        || fail "the paused ticks cost nothing, so the whole run is $OVEN_FE FE: $line"
    [ "$(progress_of "$line")" = 0 ] || fail "the resumed run did not finish: $line"
    has $HATCH_OUT modern_industrialization:coke 1

    echo "== F: a hatch in a middle-ring position (HatchFlags.NO_HATCH) does not form it =="
    place 9 101 2 modern_industrialization:bronze_item_input_hatch
    ctl wait 6
    expect_oven $COKE_OVEN_AT false 1
    place 9 101 2 minecraft:bricks
    ctl wait 6
    expect_oven $COKE_OVEN_AT true 0 3
}

# ---------------------------------------------------------------------------
# The mod-readable coke recipe kind (slice 3)
# ---------------------------------------------------------------------------

recipes_legs() {
    echo "== G: /modern_industrialization recipes reads the coke oven kind back =="
    local out lines
    out=$(ctl messages 40)
    say "modern_industrialization recipes"
    for _ in $(seq 1 25); do
        ctl wait 4 >/dev/null
        out=$(ctl messages 60)
        if jq -r '.chat[]' <<<"$out" | grep -qx "modern_industrialization:coke_oven: 2 recipes"; then
            break
        fi
    done
    lines=$(jq -r '.chat[]' <<<"$out" | grep -E "^modern_industrialization:coke_oven/coke(_dust)? ")
    echo "$lines" | sed 's/^/  /'
    jq -r '.chat[]' <<<"$out" | grep -qx "modern_industrialization:coke_oven: 2 recipes" \
        || fail "the recipe kind did not list its 2 recipes: $(jq -c .chat <<<"$out")"
    grep -q '"duration":600' <<<"$lines" || fail "the recipe data's duration is not readable: $lines"
    grep -q '"item":"minecraft:coal"' <<<"$lines" || fail "the coke recipe's input is not readable: $lines"
    grep -q '"item":"modern_industrialization:coke"' <<<"$lines" || fail "the coke recipe's output is not readable: $lines"
    grep -q '"item":"modern_industrialization:coke_dust"' <<<"$lines" || fail "the coal-dust recipe is missing: $lines"
}

# ---------------------------------------------------------------------------
# The add-on companions and the matcher's 3x3x4 harness check (slice 3)
# ---------------------------------------------------------------------------

# build_shape_fixture X Y Z: the 3x3x4 shell `/modern_industrialization shape`
# matches — 33 members (the controller and two interior blocks are not members):
# 17 bronze-plated bricks, then 16 coil positions, the two roof corners holding
# hatches. Same order as `runtime::pyrolyse_shape_fixture`.
build_shape_fixture() { # X Y Z
    local x=$1 y=$2 z=$3 index=0 built=0 dx dy dz px py pz id
    for dy in 0 1 2 3; do
        for dz in 0 1 2; do
            for dx in -1 0 1; do
                case "$dx $dy $dz" in
                    "0 0 0"|"0 0 1"|"0 0 2") continue ;;
                esac
                px=$((x + dx)); py=$((y + dy)); pz=$((z + dz))
                id=modern_industrialization:cupronickel_coil
                [ "$index" -lt 17 ] && id=modern_industrialization:bronze_plated_bricks
                [ "$dx $dy $dz" = "-1 3 0" ] && id=modern_industrialization:bronze_item_input_hatch
                [ "$dx $dy $dz" = "1 3 2" ] && id=modern_industrialization:bronze_fluid_input_hatch
                ctl command "setblock $px $py $pz $id" >/dev/null
                index=$((index + 1)); built=$((built + 1))
            done
        done
    done
    ctl wait 6
    echo "  shape fixture($x,$y,$z) = $built members"
    [ "$built" = 33 ] || fail "the 3x3x4 fixture built $built members, expected 33"
}

shape_at() { # X Y Z -> the reply line
    local before out line
    before=$(ctl messages 20 | jq -r '.chat[]' | grep -E "^pyrolyse shape at $1,$2,$3:" | head -1 || true)
    say "modern_industrialization shape $1 $2 $3"
    for _ in $(seq 1 25); do
        ctl wait 4 >/dev/null
        out=$(ctl messages 30)
        line=$(jq -r '.chat[]' <<<"$out" | grep -E "^pyrolyse shape at $1,$2,$3:" | head -1 || true)
        if [ -n "$line" ] && [ "$line" != "$before" ]; then
            echo "$line"
            return
        fi
    done
    echo "NO-SHAPE $(jq -c . <<<"$out")"
}

expect_shape() { # X Y Z FORMED MISSING HATCHES
    local read_line got_formed got_missing got_hatches
    read_line=$(shape_at "$1" "$2" "$3")
    echo "  shape($1,$2,$3): $read_line"
    got_formed=$(formed_of "$read_line")
    got_missing=$(missing_of "$read_line")
    got_hatches=$(hatches_of "$read_line")
    [ "$got_formed" = "$4" ] || fail "expected the 3x3x4 shape formed $4, got '$got_formed' ($read_line)"
    [ "$got_missing" = "$5" ] || fail "expected the 3x3x4 shape missing $5, got '$got_missing' ($read_line)"
    [ "$got_hatches" = "$6" ] || fail "expected the 3x3x4 shape hatches $6, got '$got_hatches' ($read_line)"
}

shape_legs() {
    local sx sy sz
    read -r sx sy sz <<<"$SHAPE_AT"
    echo "== H: the add-on companions and the 3x3x4 matcher check =="
    echo "  the companion blocks are this package's and readable"
    place 20 100 6 modern_industrialization:bronze_plated_bricks
    place 20 100 7 modern_industrialization:cupronickel_coil
    place 20 100 8 modern_industrialization:kanthal_coil
    build_shape_fixture $SHAPE_AT
    expect_shape $SHAPE_AT true 0 2
    echo "  one coil broken is missing 1"
    ctl command "setblock $((sx + 1)) $((sy + 2)) $((sz + 0)) minecraft:air" >/dev/null
    ctl wait 6
    expect_shape $SHAPE_AT false 1 2
    echo "  a brick where a coil belongs is still unformed"
    ctl command "setblock $((sx + 1)) $((sy + 2)) $((sz + 0)) modern_industrialization:bronze_plated_bricks" >/dev/null
    ctl wait 6
    expect_shape $SHAPE_AT false 1 2
    echo "  the coil back re-forms it"
    ctl command "setblock $((sx + 1)) $((sy + 2)) $((sz + 0)) modern_industrialization:cupronickel_coil" >/dev/null
    ctl wait 6
    expect_shape $SHAPE_AT true 0 2
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
    echo "== SELFTEST: the macerator legs are skipped; the coke oven's unformed assertion runs inverted =="
else
    macerator_legs
fi
oven_legs
if [ "$SELFTEST" = 0 ]; then
    recipes_legs
    shape_legs
fi

if [ "$SELFTEST" = 1 ]; then
    echo "SELFTEST finished without failing (this is a bug in the test)"
    exit 1
fi
echo "PASS: the coke oven's 3x3x3 shell of 22 bricks and three bronze hatches forms around its controller (formed true, missing 0, hatches 3): the input hatch feeds the run and the output hatch gives one coke for exactly 12,000 FE, the coal-dust recipe does the same, breaking one brick flips formed to false and freezes the run, a coil in the gap does not re-form it, a brick put back resumes the run, a hatch in a middle-ring position is refused, and /modern_industrialization recipes reads the coke oven kind's 2 data recipes back; the add-on's 3x3x4 shell of 17 bronze-plated bricks and 16 coil positions (two holding hatches) matches, one coil broken is missing 1 and a brick where a coil belongs is unformed; the electric macerator still holds 32,000 FE (3,200 EU), runs a stone at $RATE FE/t for exactly $STONE_FE FE into one cobblestone, an iron ore for $IRON_ORE_FE FE into three raw iron, reaches $OVERCLOCK_RATE FE/t at efficiency $OVERCLOCK_TICKS while an ancient debris still costs 1600 EU (16,000 FE), makes nothing without energy and burns nothing without an input, and a half-finished run survives save and relaunch; one .hmod carries the blocks, the data and the behaviour"
