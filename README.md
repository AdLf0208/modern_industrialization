# Modern Industrialization for Horizon — the electric macerator, the coke oven and its hatches

A port of [Modern Industrialization](https://github.com/AztechMC/Modern-Industrialization) 2.5.12
(MIT): the first machine of its electric age and the first of its multiblocks.

`modern_industrialization:electric_macerator`:

- a **32,000 FE buffer** (upstream's 3,200 EU at the mod's own 1 EU = 10 FE), filled from any face,
  drained by nothing (`EnergyComponent.buildInsertable` is insert-only);
- **one item input** slot and **four item output** slots, reachable by hoppers and pipes;
- upstream's `CrafterComponent` tick: a run costs `eu * duration` EU, runs at the LV tier's
  `max(8, eu)` EU/t, and an earned overclock tick adds `totalEu / 600` EU/t up to 32 EU/t, so an
  overclocked run is faster, never cheaper;
- **33 of the jar's own macerator recipes** (the `vanilla_recipes/macerator/` set and the material
  recipes over vanilla items) — `src/runtime.rs`'s `RECIPES` table, generated from the pack jar by
  `tools/carry_recipes.py`.

`modern_industrialization:coke_oven`, upstream's first multiblock:

- a **3×3×3 shell of 25 bricks around the controller**, exactly the shape the mod builds
  (`ShapeTemplate.Builder(BRICKS).add3by3Levels(-1, 1, bricks, hatches).build()`: a full 3×3 floor,
  a middle ring with a hollow interior, a full 3×3 roof, and the controller's own position removed);
  the shell is matched on block changes and the controller reports `formed` and how many members
  are missing;
- the jar's own coke recipe: **1 coal → 1 coke at 2 EU/t for 600 ticks** (1,200 EU = 12,000 FE) —
  and it only runs while the shell is whole. Break one brick and the run pauses, exactly where it
  was and without burning anything; put the brick back and it resumes;
- the **four bronze hatches** — `bronze_item_input_hatch`, `bronze_item_output_hatch`,
  `bronze_fluid_input_hatch`, `bronze_fluid_output_hatch` — each a block a shell position may hold
  where the jar's `HatchFlags` allow it (the floor and roof layers; the middle ring is brick-only).
  An item hatch holds one slot, a fluid hatch a 4,000 mB tank;
- the **controller aggregates the hatches**: a run's input is pumped from the item input hatch into
  the controller and its product is pumped out into the item output hatch, so a hopper or a pipe
  feeds and drains the multiblock through the shell. The controller keeps the two internal slots as
  its run buffer (upstream has none there; `notes.md` records the deviation) and models the steam's
  1 mB = 1 EU as FE at the mod's 10:1 ratio;
- the block, its blockstate, a port-authored model (bricks casing + the jar's front overlay), the
  **coke** and **coke dust** items, the lang keys, the self-drop loot table and the same
  `mineable/pickaxe` + `needs_stone_tool` tags, all in this package's `assets/` and `data/`.

The multiblock companions an add-on builds with are registered too: `bronze_plated_bricks`, the
`cupronickel_coil` and the `kanthal_coil` (each placed, read back and self-dropping), and the coke
oven's recipes are a **data recipe kind** (`modern_industrialization:coke_oven`) under
`data/modern_industrialization/recipe/coke_oven/`, which another mod reads with the SDK's
`recipes_of_type` rather than from a compiled table.

`/modern_industrialization info <pos>` reports a machine's buffer, progress, rate, efficiency, and
for the multiblock its shell (`formed true|false, missing N, hatches N`); `recipes` lists the coke
oven kind's recipes as another mod reads them; `shape <pos>` matches the matcher's 3×3×4 harness
fixture (the add-on's shell: 17 bronze-plated bricks and 16 coil positions, two holding hatches);
`energy` and `efficiency` are the harness controls.

Everything else in the mod (the other machine families, the blast furnace and the rest of the
multiblocks, the energy hatches and the other tiers, the generators, the pipes, the material part
system, the tools and the GuideME book) is not ported; `notes.md` has the complete gap list.

## The package

One flat `.hmod`, built by the pinned SDK's tool:

```text
modern_industrialization.hmod
  horizon-mod.toml   the only manifest: id, api = "0.2", [native] pinned to 0a875da/e59da90
  Cargo.toml         horizon-mod-api from .horizon/sdk/ (fetched, never packed in the sources)
  src/lib.rs         the one #[mod_api::horizon_mod] impl Hooks: the two block entity types and
                     their Parts, the shape match (block_changed + level_tick_post), the
                     container and energy declarations, and the commands
  src/runtime.rs     the port's pure logic, the shell template, the hatch flags and its 50 tests
  assets/ data/      the carried content (blocks, items, models, textures, lang, loot, tags)
  LICENSE, LICENSE-NOTICE.md, data/modern_industrialization/license/
```

## Build and check

```sh
cargo horizon-mod check                 # compiles native and wasm against the pinned SDK
cargo horizon-mod build                 # target/horizon-mod/modern_industrialization.hmod
cargo horizon-mod check <file>.hmod     # the package's shape, size and sha256
cargo horizon-mod fits <file>.hmod      # whether another build can still load it
cargo test --offline                    # the pure core's tests
```

Compile steps in this campaign run through `ports/tools/build-slot.sh`
(`run --job W1-modern_industrialization --port ports/modern_industrialization --step check -- …`),
and the in-game leg through the launcher-built game (`ports/tools/launcher-game.sh build
modern_industrialization`, then `tools/e2e.sh`).

**Local-only** until the maintainer approves publishing (ARCHITECTURE.md §8). The original mod is
MIT (rank 0); this port is MIT, © 2026 the Horizon port authors. `LICENSE-NOTICE.md` carries the
provenance, and it travels inside the package.
