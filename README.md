# Modern Industrialization for Horizon — the electric macerator

The port's slice 1: one machine from the electric age of
[Modern Industrialization](https://github.com/AztechMC/Modern-Industrialization) 2.5.12 (MIT) —
`modern_industrialization:electric_macerator`.

- a **32,000 FE buffer** (upstream's 3,200 EU at the mod's own 1 EU = 10 FE), filled from any face,
  drained by nothing (`EnergyComponent.buildInsertable` is insert-only);
- **one item input** slot and **four item output** slots, reachable by hoppers and pipes;
- upstream's `CrafterComponent` tick: a run costs `eu * duration` EU, runs at the LV tier's
  `max(8, eu)` EU/t, and an earned overclock tick adds `totalEu / 600` EU/t up to 32 EU/t, so an
  overclocked run is faster, never cheaper;
- **33 of the jar's own macerator recipes** (the `vanilla_recipes/macerator/` set and the material
  recipes over vanilla items) — `src/runtime.rs`'s `RECIPES` table, generated from the pack jar by
  `tools/carry_recipes.py`;
- the block, its blockstate, a port-authored model, its textures, the nine dusts, the lang keys,
  the self-drop loot table and the `mineable/pickaxe` + `needs_stone_tool` tags, all in this
  package's `assets/` and `data/`;
- `/modern_industrialization info <pos>` — buffer, progress, rate, efficiency and the active
  recipe — plus `energy` and `efficiency`, the harness controls.

Everything else in the mod (the other machine families, the multiblocks, the generators, the
pipes, the material part system, the tools and the GuideME book) is not ported; `notes.md` has the
complete gap list.

## The package

One flat `.hmod`, built by the pinned SDK's tool:

```text
modern_industrialization.hmod
  horizon-mod.toml   the only manifest: id, api = "0.2", [native] pinned to 0b14f8a/e59da90
  Cargo.toml         horizon-mod-api from .horizon/sdk/ (fetched, never packed in the sources)
  src/lib.rs         the one #[mod_api::horizon_mod] impl Hooks: the block entity type, its Part,
                     the block's container and energy declarations, and the command
  src/runtime.rs     the port's pure logic and its 27 tests (also what `cargo test` runs)
  assets/ data/      the carried content (block, items, models, textures, lang, loot, tags)
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
(`run --job sdk-mi --port ports/modern_industrialization --step check -- …`).

**Local-only** until the maintainer approves publishing (ARCHITECTURE.md §8). The original mod is
MIT (rank 0); this port is MIT, © 2026 the Horizon port authors. `LICENSE-NOTICE.md` carries the
provenance, and it travels inside the package.
