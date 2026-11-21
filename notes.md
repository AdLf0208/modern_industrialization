# Modern Industrialization → Horizon, slice 1 (the electric macerator)

Ported from **Modern Industrialization 2.5.12** (`AztechMC/Modern-Industrialization`,
**MIT**), the exact ATM10 jar `Modern-Industrialization-2.5.12.jar` (SHA-1
`20be5747c0c7f175f691f77de6df055be86d1a7a` = the pack's `modrinthHash`; SHA-256
`56f59a5a…5262`; kept at `ports/_shared/licence-cache/`, provenance in
`ports/_shared/carry-work/modern_industrialization/SOURCE.md`). Upstream
behaviour was read from tag `2.5.12` (commit `a02b76f4…8b57`), analysis only: the
port is a clean-room Rust rewrite. What was read and the numbers extracted:
[`FINDINGS.md`](FINDINGS.md).

**LOCAL-ONLY** until the maintainer approves publishing (ARCHITECTURE.md §8). MIT
is licence rank 0 (permissive); the approval is still required.

Status: **slices 1–3 implemented and verified in-game** — the electric macerator
(a 32,000 FE buffer, one input, four outputs, the LV overclock, 33 jar recipes),
the coke oven (a 3×3×3 brick shell with `formed`/`missing`, the jar's two coke
recipes) and, at slice 3, the four bronze hatches (item and fluid, in and out) a
shell position may hold, the controller's item pump through them, the
`bronze_plated_bricks`/coil companions and the coke oven's mod-readable recipe
kind. Everything else in the mod (the other 23 machine families, the blast
furnace and the rest of the multiblocks, the energy hatches, the generators, the
pipes, the 290-odd material recipes) is a planned gap (below).

> **Format (2026-11-18):** the port is now **one flat SDK `.hmod`** (mod API `0.2`, pin
> `0b14f8a`). The record of that migration — file map, helper→hook table, gaps, traps and the
> verification actually run — is [§Migration to the SDK format](#migration-to-the-sdk-format-2026-11-18)
> at the end of this file. Every `mixins/`, `.hnmod`, `content_root` and `horizon.py` paragraph
> above and below it is the history of the retired container; it is kept because it is the
> behaviour contract this slice must still honour, not because those files exist.


## Class + tier, approach, licence

- **Class `content`, tier `deep`** (`ports/ledger.csv`, from
  `ports/_shared/source-map/atm10-sources.csv`); it is **core** (not an add-on)
  and P0. Its declared dependencies are libraries: `grandpower` and `guideme`
  (required), `ae2`/`kuebjs`/`jei` (optional) — the first two map to Horizon
  APIs, not to ports (ARCHITECTURE.md §8, libraries are mapped).
- The **slice shape is machines**, not the mod's whole deep tier: a block entity,
  its state in NBT, a container, its own FE capability, a recipe tick and harness
  commands (`horizon/docs/modpacks.md`: a machines feature needs exactly that).
  The deep parts (custom renderers, the multiblock controllers, the generator
  networks) are deferred.
- Queue position: licence **rank 0**, mechanics, **core**, and the first machine a
  player meets in the pack's ore chain — its recipes are what turn ore into raw
  metal and ingots into dusts.
- **Licence: MIT → rank 0** (read from the jar's `neoforge.mods.toml` and its
  `LICENSE`). Local-only until the maintainer approves publishing.

## What the slice is

`modern_industrialization:electric_macerator`, one block:

- a **32,000 FE buffer** (`native_block_entity_energy`, API 142; the int `energy`
  in the saved compound) that any face may fill and nothing may drain
  (upstream's `EnergyComponent.buildInsertable` is insert-only). The mod's own
  default is **1 EU = 10 FE**, so 3,200 EU = 32,000 FE;
- **one item input slot** and **four item output slots** (`ModContainer`):
  hoppers insert into slot 0 and pull from slots 1..=4;
- **`CrafterComponent`'s tick** in the pure runtime: a run starts when the buffer
  holds at least 1 EU, takes its inputs at the start, consumes
  `min(recipeMaxEu, recipeEnergy - usedEnergy)` EU a tick, and gives its outputs
  when `usedEnergy == eu * duration`;
- the **LV tier's numbers**: base rate `max(8, eu)` EU/t, ceiling 32 EU/t, and the
  efficiency-tick overclock `base + efficiency * totalEu / 600` — 72 ticks for a
  200-EU recipe, 9 for a 1,600-EU one; one finished run adds a tick, a tick that
  used less than the maximum removes one, and a warm machine stays locked to its
  recipe until the ticks reach zero;
- **33 recipes** from the jar (`tools/carry_recipes.py` prints them from the
  package, so every number is the jar's): the `vanilla_recipes/macerator/` set and
  the material recipes over vanilla items;
- `/modern_industrialization info <x> <y> <z>` — the readout the check asserts —
  plus `energy` and `efficiency`, the harness controls (precedent: `/powah set`,
  `/mekanismgenerators energy`).

## Behaviour (upstream → this port)

| | Upstream | This port |
|---|---|---|
| buffer | `EnergyComponent` over `casing::getEuCapacity`, 3,200 EU for LV | 32,000 FE at 1 EU = 10 FE, in the block's `Capabilities.Energy.BLOCK` store |
| insert | `insertEu(max)` = `min(max, capacity - stored)`, every face | `max_insert = capacity`, every `insert_face`; `max_extract = 0` |
| start | `usedEnergy == 0`, `consumeEu(1, SIMULATE) == 1`, `canStartRecipe` | same, plus a start-time output-room check |
| inputs | taken at the start (`takeItemInputs`) | same |
| rate | `min(totalEu, min(max(baseEu, eu) + eff*total/600, maxEu))` | same, unit-tested |
| progress | `usedEnergy` grows by the EU consumed; `getProgress = used/receiptEnergy` | same; the info line prints it in thousandths |
| finish | outputs put at the end, `probability` rolled per output | same (a seeded roll per tick and position) |
| overclock | `getRecipeMaxEfficiencyTicks` per recipe; +1 on a finished run, -1 on a starved tick; clamped on a recipe change | same, unit-tested (72 and 9) |
| warm lock | while `efficiency > 0`, only the active recipe is considered | same |
| block | `destroyTime 4.0`, `requiresCorrectToolForDrops`, `mapColor METAL`, `mineable/pickaxe` + `needs_stone_tool`, no spawns, not a redstone conductor | iron-block template, hardness 4.0, resistance 6.0, stone sounds, requires the right tool, both tags; spawns/redstone conduction are the template's |
| model | a custom `modern_industrialization:machine` loader: casing + front/top overlays composited, `active` variants | a plain model: the `basic_machine_hull` casing with the macerator's front and top overlays as thin outer quads, no `active` state |
| GUI | progress bar, energy bar, efficiency bar, slot panel, auto-extract, side configuration | none: the container is the automation path and the command is the readout |
| loot | self-drop with its contents in components | self-drop; the buffer and inventory are lost |

## Hooks

| Hook (API) | Helper | What it does |
|---|---|---|
| `block_entity` (15) | `block_entity.rs` | The block gets a block entity. |
| `block_entity_ticks` (15) | `ticks.rs` | It ticks on the server. |
| `block_entity_container` (16) | `container.rs` | One input slot, four output slots. |
| `block_entity_energy` (142) | `energy.rs` | The 32,000 FE buffer (`energy` in the saved compound). |
| `block_entity_tick` (17) | `tick.rs` | One tick of the machine. |
| `register_commands` (8) | `register.rs` | `/modern_industrialization info\|energy\|efficiency`. |
| `perform_command` (8) | `perform.rs` | The command bodies. |
| `content_root` (AE2 pattern) | `embedded_content.rs` | Writes the embedded `.hmod` to `.horizon-cache/` and appends it to the dev list. |

**8 injections: 7 stamped hooks + the `content_root` target; 0 unstamped hooks,
no guessed API.** The buffer is a real `Capabilities.Energy.BLOCK` store: another
mod's cable or cell can fill it with `native_energy_insert`/`native_move_energy`
(API 142), which is the interop the pack's `powah` cable and
`mekanismgenerators` generator speak.

## Commands (harness additions)

```
/modern_industrialization info <x> <y> <z>
modern_industrialization:electric_macerator at 0,100,0: 8000/32000 FE (800 EU), progress 0/1000, 0 FE/t, efficiency 0/72, active none, t=340
/modern_industrialization energy <amount> <x> <y> <z>        (operator level 2)
Energy set to 10000 FE.
/modern_industrialization efficiency <ticks> <x> <y> <z>     (operator level 2)
Efficiency set to 72 ticks.
```

`t=` is the world game time, so the in-game check can tell a fresh reply from a
stale one. `energy` and `efficiency` are harness controls, not upstream gameplay
(upstream has no such command), recorded as additions.

## Content (`.hmod`)

`content/` is checked in assembled; `content/README.md` lists every file and
where it came from. The package declares the block, the nine dust items, a
creative tab, the blockstate, a port-authored block model, 10 item models and
their textures, the lang keys, the 26.3 self-drop loot table (the **singular
typed** condition), and the `mineable/pickaxe` + `needs_stone_tool` tags.
**No crafting recipe is carried**: the machine's own recipe needs six MI
components (invar rotary blade, motors, pistons, a basic machine hull, a tin
cable, analog circuits) that this slice does not register, so carrying it would
drop it at load. The block is obtainable from its creative tab.

## Deviations and gaps (slice 1)

1. **No crafting recipe** for the machine or the dusts (see above); the creative
   tab is the entry point.
2. **33 of the jar's 324 macerator recipes.** The other 290-odd need MI's own
   ingots, plates, gears, rods and wires, which this slice does not register;
   tag inputs (`c:ores/iron`…) were resolved to the vanilla items the tags hold,
   which is why three jar rows became four rows for the ore variants.
3. **No data-driven recipe type.** The pin offers no machine-recipe registry for
   a native mod, so the table is a `const` in `runtime.rs`; adding a recipe is a
   code change. Recorded as the issue shape "a data-driven machine recipe type".
4. **No menu/GUI.** `ModContainer` has no per-slot `may_place` for a player's menu
   at this pin (a hopper menu would let a player fill the output slots), so the
   container has `menu: None`; automation and the command are the interfaces.
5. **No side configuration, no auto-extract, no upgrades/casing/overdrive** — the
   LV tier's own overclock is the only speed mechanism.
6. **No `active` block state**, so the idle model always shows, and there is no
   light, no particles, no hum. The front/top overlays stay as carried.
7. **No orientation**: upstream's facing is block-entity model data; the port's
   model is fixed, front to the north.
8. **Output slots are not machine-locked** during a run: the start check keeps the
   run from beginning without room, and a player who fills an output slot
   mid-run makes the finished output overflow and be lost (upstream loses it too,
   after locking the slot at the start).
9. **Breaking the machine loses its FE and its inventory** (upstream carries them
   in block-item components).
10. **The rest of the mod**: the other machine families (mixer, assembler, packer,
    cutting machine, chemical reactor, electrolyzer, centrifuge, blast furnace,
    distillery, wiremill, polarizer, water pump, quarry…), the multiblocks (coke
    oven, blast furnace, distillation tower, heat exchanger, vacuum freezer,
    implosion compressor, fusion reactor), the steam machines and boilers, the
    generators and the nuclear chain, pipes/cables/hatches/storage units, the
    material part system, the tools/armor/quantum gear, the GuideME book, the
    recycling/replicator chain and the config are **not ported**.

Skills: superpowers/using-superpowers,brainstorming,writing-plans,test-driven-development,systematic-debugging,verification-before-completion; BeautifulRust/rust-best-practices,rust-port

## Verification

Pin **`1d63e0a5a37a7a708eb0b32b49bf0dfeb28ed8a9`**, mixins
**`5a64af938ec86b00d53d16a408aabc0038fb3e57`** (schema 6); checkouts
`ports/_shared/pins/horizon-1d63e0a` and `.../horizon-mixins-5a64af9`. Every
build, test and e2e ran through `ports/tools/build-slot.sh` (never a bare
`cargo`, `CARGO_INCREMENTAL=0`, no `--target-dir`); compile steps ran on the
second box through the same runner, the game legs locally (slot `e2e`; the
runner forwards them when the script is portable). The e2e used the pin-matched
shared `horizon-ctl` (`ports/_shared/bin/horizon-1d63e0a5/horizon-ctl`) and the
port's own fast game.

### Ladder

| Step | Command | Result |
|---|---|---|
| runtime tests (RED) | `cargo test --offline` with the tests and no implementation | **cargo_exit 101**, `65 previous errors` (`cannot find function 'recipe_max_eu'`, `'apply_outputs'`, the type's methods) — the API the tests demand did not exist |
| runtime tests | `cargo test --offline` in the port | **26 passed**, 0 failed |
| format | `cargo fmt --check` | clean |
| runtime lints | `cargo clippy --all-targets --offline -- -D warnings` | exit 0, **0 warnings** |
| stage | `tools/horizon.py check` (stages first) | 8 injections, 2 modules |
| check | `tools/horizon.py check` | `cargo_exit: 0`, **0 warnings** (`horizon-check.log`) |
| embedded test | `tools/horizon.py test` | `test result: ok. 26 passed; 0 failed; 1240 filtered out`, `cargo_exit: 0` (`horizon-test.log`) |
| fast build | `tools/horizon.py build` | `cargo_exit: 0`, **0 warnings** (`horizon-build.log`) |
| e2e | `build-slot.sh e2e -- … -- tools/e2e.sh` | **PASS, `EXIT:0`**, 0 `FAIL:` lines (`e2e-default.log`) |
| fail-capable run | `SELFTEST=1 tools/e2e.sh` | **FAIL as required, `EXIT:1`** (`e2e-selftest.log`) |
| content `.hmod` | `cargo-horizon-mod build` + `check` | **18,106 B**, SHA-256 `d6c75d1f…7558`, **50 entries**, API 0.2, `no code: content only` |
| native `.hnmod` | `python3 tools/package.py` | **28,136 B**, SHA-256 `20767db7…1833`, **13 entries** (the embedded `.hmod` is byte-identical to `content/`) |
| load-check | `tools/hmod-load.sh` (shared game) | **PASS with `--dev-mod`**, unknown without it (`hmod-load.log`) |
| e2e log checker | `python3 ports/tools/check-e2e-logs.py modern_industrialization --strict` | `logs=2 ok=2 missing-exit=0 bad-exit=0`, exit 0 |

Evidence: `ports/_shared/evidence/modern_industrialization/`
(`horizon-check.log`, `horizon-test.log`, `horizon-build.log`, `e2e-default.log`,
`e2e-selftest.log`, `hmod-load.log`).

Two behaviour tests were added **after** the first in-game run found the gap they
pin: a machine whose efficiency was set but which had no run (the harness
control's state) did nothing, because the start phase only considered the active
recipe. The e2e's `F` leg failed with `the overclocked machine does not report
320 FE/t: … 10000/32000 FE, progress 0/1000, active none`, the fallback and the
upstream clamp (`efficiency = min(efficiency, maxEfficiencyTicks)`) went in, and
the two tests now hold both.

### The in-game check (quoted from `e2e-default.log`)

Overworld arena at y=100; the test machine at `0,100,0`, the "no energy" control
at `0,100,4`, the "no input" control at `4,100,0`. Quoted verbatim (the `{}`
lines are `ctl` replies):

```
== control: the same binary without its embedded content ==
  without the embedded content, block(0,100,0) = 'minecraft:air'
== A: a fresh machine is empty and idle ==
  modern_industrialization:electric_macerator at 0,100,0: 0/32000 FE (0 EU), progress 0/1000, 0 FE/t, efficiency 0/0, active none, t=148
== B: control — input, no energy, nothing happens ==
  modern_industrialization:electric_macerator at 0,100,4: 0/32000 FE (0 EU), progress 0/1000, 0 FE/t, efficiency 0/0, active none, t=215
  has(0,100,4) minecraft:stone x1: ok
== C: control — energy, no input, nothing happens ==
  info(4,100,0): modern_industrialization:electric_macerator at 4,100,0: 10000/32000 FE (1000 EU), progress 0/1000, 0 FE/t, efficiency 0/0, active none, t=285
== D: one stone becomes one cobblestone for exactly 2000 FE ==
  modern_industrialization:electric_macerator at 0,100,0: 8000/32000 FE (800 EU), progress 0/1000, 0 FE/t, efficiency 0/72, active none, t=340
  has(0,100,0) minecraft:cobblestone x1: ok
== E: one iron ore becomes three raw iron for exactly 4000 FE ==
  modern_industrialization:electric_macerator at 0,100,0: 4000/32000 FE (400 EU), progress 0/1000, 0 FE/t, efficiency 0/36, active none, t=421
  has(0,100,0) minecraft:raw_iron x3: ok
== F: at efficiency 72 the machine runs at 320 FE/t ==
  modern_industrialization:electric_macerator at 0,100,0: 18080/32000 FE (1808 EU), progress 120/1000, 320 FE/t, efficiency 9/9, active modern_industrialization:ancient_debris_to_netherite_scrap, t=454
  modern_industrialization:electric_macerator at 0,100,0: 4000/32000 FE (400 EU), progress 0/1000, 0 FE/t, efficiency 0/9, active none, t=534
  has(0,100,0) minecraft:netherite_scrap x2: ok
== G: a run in progress survives save, quit and relaunch ==
  before save: modern_industrialization:electric_macerator at 0,100,0: 18080/32000 FE (1808 EU), progress 120/1000, 80 FE/t, efficiency 0/9, active modern_industrialization:diamond_to_diamond_dust, t=576
  after relaunch: modern_industrialization:electric_macerator at 0,100,0: 8800/32000 FE (880 EU), progress 700/1000, 80 FE/t, efficiency 0/9, active modern_industrialization:diamond_to_diamond_dust, t=693
  resumed to: modern_industrialization:electric_macerator at 0,100,0: 4000/32000 FE (400 EU), progress 0/1000, 0 FE/t, efficiency 0/9, active none, t=1552
  has(0,100,0) modern_industrialization:diamond_dust x1: ok
  embedded content: 18106 B, byte-identical to content/modern_industrialization.hmod
PASS: … one .hnmod installs both the block and the behaviour
EXIT:0
```

What the numbers prove:

- a machine with **no energy** leaves its stone in the input for 40 ticks and
  stores nothing; a machine with **energy and no input** keeps its 10,000 FE for
  40 ticks — the two controls the scenario is built around;
- a stone costs **exactly 2,000 FE** (200 EU at 8 EU/t: 25 ticks) and gives one
  cobblestone; the buffer reads 8,000 of 10,000 afterwards and the machine is
  idle;
- an iron ore costs **exactly 4,000 FE** (400 EU, a 200-tick recipe at 2 EU/t) and
  gives **three** raw iron — the duration is honoured, not a fixed 25 ticks;
- at efficiency 72 the machine runs at **320 FE/t** (32 EU/t) and an ancient
  debris still costs **exactly 16,000 FE** (1,600 EU) — the overclock is faster,
  not cheaper. The info shows `efficiency 9/9` because upstream clamps the 72
  earned ticks to the *recipe's* own maximum (9 for a 1,600-EU recipe) when the
  recipe changes; the rate is the same 32 EU/t either way;
- a half-finished diamond run (progress 120‰ before the save) is **still the same
  run** after `save`, `quit` and relaunch (progress 700‰, same recipe, same
  rate), then finishes at exactly 20,000 − 16,000 = **4,000 FE** with one
  diamond dust;
- the embedded content file the bundle wrote is **byte-identical** to
  `content/modern_industrialization.hmod`, and the same binary with
  `.horizon-cache` made unusable cannot register the block at all — so the "one
  `.hnmod` installs both halves" claim is not vacuous.

### Fail-capable check (evidence `e2e-selftest.log`)

`SELFTEST=1` flips B's "no energy, no output" assertion; the run then fails as
required:

```
  SELFTEST: expecting a machine with no energy to have processed its input (this must fail)
FAIL: SELFTEST flipped: the machine with no energy made nothing, as it should
EXIT:1
```

### Content load check (evidence `hmod-load.log`)

The shared unmodded `1d63e0a5` game with
`--dev-mod content/modern_industrialization.hmod` registers the block
(`block 0,64,0 = modern_industrialization:electric_macerator`) and gives its item;
the **same world without `--dev-mod`** answers
`Unknown block type 'modern_industrialization:electric_macerator'` and the block
stays air, so the check cannot pass vacuously. `PASS`, `EXIT:0`.

## Pin findings for the next slice

- `block_entity_energy` (API 142) is a real `Capabilities.Energy.BLOCK` store: the
  amount is the int `energy` in the saved compound, and the capability is
  insert/extract/move (`native_energy_insert`, `native_energy_extract`,
  `native_move_energy`). This slice writes `energy` from its own tick and the
  engine's capability path reads the same key — a cable from another port can
  fill the buffer without any change here.
- `ModContainer` has `slots`, `faces`, `inputs`, `outputs` and `menu`; hoppers
  insert only into `inputs` and pull only from `outputs`, and `faces` decides
  which slots a side reaches. There is **no per-slot player rule** for a mod menu
  (`native_menu_slot_rules` reaches only content menu types), so a machine's GUI
  cannot keep a player out of its output slots today.
- A block entity's writes inside its own tick persist here (the run's progress,
  efficiency and buffer all survived save + relaunch), and `mark_modified` is
  still called for the chunk.
- `horizon.py check` requires the embedded `.hmod` to exist at
  `mixins/<id>/data/<id>-content.hmod` **before** it runs: `native_data!` is an
  `include_bytes!` of `native-data/<id>/<file>` in the staged tree, so a fresh
  clone must build the content package first (`tools/package.sh` does both).
- The `.hnmod` bytes are reproducible **on one box** (two local builds gave
  `20767db7…`): the two build machines differ only in Python's zlib
  (3.14.9 + zlib-ng vs 3.14.4 + zlib), whose deflate output differs, so the same
  contents hash differently across boxes (verified entry-by-entry equal).

## Slice 2 — the coke oven multiblock (2026-11-21)

**Pin** `0a875da91dbb0fdcb7ba8c42c11341aa229af51a` + mixins `e59da9094f0c2fdda6c627fe9eff22dec6a906f0`
(the campaign pin; `.horizon/sdk` was deleted so the SDK refetched, and `cargo horizon-mod check`
reports `Made for 0a875da`). Package version **0.2.1**.

**The gap it closes.** "Next slices" #6 below (the multiblocks: controllers, hatches and shape
matching), which is also the add-on `industrialization_overdrive`'s most blocking parent gap: its
`tools/parent_surface.py` run names `modern_industrialization:coke_oven` and "multiblock shell
(ShapeTemplate / SimpleMember / HatchFlags)" ABSENT
(`ports/_shared/evidence/industrialization_overdrive/parent-surface-matrix.log`). Class `content`,
tier `deep`; shape `machines`, as `horizon/docs/modpacks.md` asks of a multiblock (a controller
block entity, its saved state, its container, its energy and a recipe tick).

### The shape and the numbers, read from tag 2.5.12

`MultiblockMachines.cokeOven`:

```java
SimpleMember bricks = SimpleMember.forBlock(() -> Blocks.BRICKS);
ShapeTemplate cokeOvenShape = new ShapeTemplate.Builder(MachineCasings.BRICKS)
        .add3by3Levels(-1, 1, bricks, cokeOvenHatches).build();
```

- `add3by3Levels(-1, 1, …)` writes the full 3×3 at y=-1, the 3×3 at y=0 with `(0,0,1)` hollow, and
  the full 3×3 at y=1; `build()` removes `(0,0,0)`. That is **25 members, all `minecraft:bricks`**
  (9 + 7 + 9): a 3×3×3 box whose front-centre of the middle layer is the controller and which
  extends two blocks to +z. **The shell is vanilla bricks** because that is the block upstream's own
  `SimpleMember` names; the mod's own block in this multiblock is the controller. (A 3×3 shell of the
  mod's *own* machine blocks is the steam blast furnace's shape — `fire_clay_bricks` over 3×3×4 —
  not the coke oven's. The artefact wins over the brief's wording here, and the e2e still uses one
  of the mod's own blocks in the shell: the macerator in a gap must *not* form it.)
- `SteamCraftingMultiblockBlockEntity.getBaseRecipeEu` is `getRecipeEu(steelHatches ? 4 : 2)` and
  `getMaxRecipeEu` returns the same, so **2 EU/t with no overclock**; `SteamHelper.consumeSteamEu`
  takes 1 mB of steam per EU.
- the jar's two recipes under `materials/coke_oven/`: `coke.json` (1 coal → 1 coke, 2 EU/t, 600
  ticks) and `coke_dust.json` (1 `#c:dusts/coal` → 1 coke dust, the same). Each also names a fluid
  output — 500 mB of `modern_industrialization:creosote` at 50%, which this slice does not model.
- `AbstractCraftingMultiblockBlockEntity`: while the shape does not match (`NOT_MATCHED`) the crafter
  is **not ticked at all**, and a re-match goes to `TRYING_TO_RESUME` and continues the run — so a
  broken shell pauses a run and rebuilding resumes it, exactly as this port does.

### What was built

| piece | where |
|---|---|
| the shell template (`ShellMember`, `COKE_OVEN_SHELL`'s 25 rows) and `match_shell` | `src/runtime.rs` |
| `shell_relevant`: only a member's or the controller's block can change a match | `src/runtime.rs` |
| `CokeOven` (buffer, run, `formed`, `missing`) at the steam tier's fixed 2 EU/t | `src/runtime.rs` |
| `COKE_RECIPES` (2 rows) and `coke_recipe` | `src/runtime.rs` |
| `MachineRecipe` — `MaceratorRecipe` renamed to upstream's own class name, now that two machines have tables | `src/runtime.rs`, `tools/carry_recipes.py` |
| the controller's Part (saved data, ticks, one tick, its two slots) | `src/lib.rs` |
| the match: `block_changed` reads the shell, `level_tick_post` writes `formed`/`shellMissing` | `src/lib.rs` |
| `info`/`energy` learn the coke oven; `info` prints `formed true|false, missing N` | `src/lib.rs` |
| the block, the `coke` and `coke_dust` items, models, textures, loot, tags, lang, creative tab | `assets/`, `data/` |

**Two new hooks, both stamped and both present at the previous pin: `block_changed` (v1) and
`level_tick_post` (v1)** — 10 hooks in all. The match cannot run in a block-entity tick:
`BlockEntityTickEvent` carries `pos` and the entity and **no level** (slice 1's gap 2). So
`block_changed` does the reads through a read-only `LevelHost` and `level_tick_post`, which has the
`ServerHost`, writes the answer with `merge_data`. The cadence is upstream's too — on block updates,
not every tick.

### The red proof (verbatim, `evidence/modern_industrialization/sdk-test-red-slice2.log`)

`cargo test --offline` with the 14 new tests written and nothing implemented:

```
error[E0425]: cannot find value `COKE_OVEN_SHELL` in this scope
error[E0405]: cannot find trait `Shell` in this scope
error[E0422]: cannot find struct, variant or union type `CokeOven` in this scope
error[E0425]: cannot find function `shell_relevant` in this scope
error: could not compile `modern_industrialization` (lib test) due to 37 previous errors
```

37 errors, exit 101.

### Green

| step | evidence | result |
|---|---|---|
| tests | `sdk-test.log` | **41 passed**, 0 failed (27 + 14 new) |
| lints | `sdk-clippy.log` | exit 0, **0 warnings** |
| format | `sdk-fmt.log` | `rustfmt --edition 2024 --check` exit 0 |
| build 1 + 2 | `sdk-build.log`, `sdk-build-2.log` | **byte-identical**, `7308bcf2…` |
| package | — | **62,560 B**, SHA-256 `7308bcf29c29cabca7def755bb99837a8617ffdb3813c10d4e658fd22dcd5289`, **75 entries**, no packed `Cargo.lock` |
| check | `sdk-check.log` | exit 0, `modern_industrialization 0.2.1 (native-mod, format native-1/0a875da…/e59da909…, for 26.3)` |
| fits (the pin) | `sdk-fits.log` | `0a875da  made for`, exit 0 |
| fits (another build) | `sdk-fits-next.log` | `0b14f8a  fits`, exit 0 |
| gate | `verify-sdk-package.py` | **1/1** |
| in game | `evidence/modern_industrialization-e2e/e2e-launcher.log` | **PASS** + `EXIT:0` |
| control | `e2e-launcher-control.log` | **FAIL** + `EXIT:1` (`expected …electric_macerator at 0,100,0, got 'minecraft:air'`) |
| fail-capability | `e2e-launcher-selftest.log` | **FAIL** + `EXIT:1` (`SELFTEST flipped: the unformed oven ran nothing and burned nothing, as it should`) |
| log gate | `check-e2e-logs.py modern_industrialization-e2e --strict` | logs=4 ok=4 |

One `fits` run failed transiently (`fits`: GitHub API `Connection reset by peer`, exit 2) and was
re-run; the green log is the re-run. The launcher rebuilt the game for this version
(`Compiling modern_industrialization (1 of 1)`, `native-0a875da-60718813c863`), and that export is
what the three legs above ran against.

### The in-game leg (quoted from `e2e-launcher.log`, Overworld, controller at 8,101,0)

```
== A: the coke oven controller alone is unformed and names the 25 missing members ==
  block(8,101,0) = modern_industrialization:coke_oven
  slots(8,101,0) = 2
  oven(8,101,0): modern_industrialization:coke_oven at 8,101,0: 0/32000 FE (0 EU), progress 0/1000, 0 FE/t, formed false, missing 25, active none, t=1550
== B: control — an unformed coke oven runs nothing and burns nothing over 620 ticks ==
  set_energy(8,101,0) = 20000 FE
  modern_industrialization:coke_oven at 8,101,0: 20000/32000 FE (2000 EU), progress 0/1000, 0 FE/t, formed false, missing 25, active none, t=2187
  has(8,101,0) minecraft:coal x1: ok
  has(8,101,0) modern_industrialization:coke x0: ok
== C: a 3x3x3 shell of 25 bricks forms it; one coal becomes one coke for exactly 12000 FE ==
  shell(8,101,0) = 25 bricks
  oven(8,101,0): modern_industrialization:coke_oven at 8,101,0: 19880/32000 FE (1988 EU), progress 10/1000, 20 FE/t, formed true, missing 0, active modern_industrialization:coke_oven_coal_to_coke, t=2249
  running: modern_industrialization:coke_oven at 8,101,0: 19100/32000 FE (1910 EU), progress 75/1000, 20 FE/t, formed true, missing 0, active modern_industrialization:coke_oven_coal_to_coke, t=2288
  has(8,101,0) minecraft:coal x0: ok
  modern_industrialization:coke_oven at 8,101,0: 8000/32000 FE (800 EU), progress 0/1000, 0 FE/t, formed true, missing 0, active none, t=2920
  has(8,101,0) modern_industrialization:coke x1: ok
== C2: the jar's second coke oven recipe (coal dust), and the items the package ships ==
  block(8,101,0) = modern_industrialization:coke_oven
  oven(8,101,0): modern_industrialization:coke_oven at 8,101,0: 0/32000 FE (0 EU), progress 0/1000, 0 FE/t, formed true, missing 0, active none, t=2973
  slots(8,101,0) = 2
  set_energy(8,101,0) = 20000 FE
  modern_industrialization:coke_oven at 8,101,0: 8000/32000 FE (800 EU), progress 0/1000, 0 FE/t, formed true, missing 0, active none, t=3609
  has(8,101,0) modern_industrialization:coke_dust x1: ok
  give(coke_dust) = {"given":1,"ok":true}
  give(no_such_item) = {"error":"unknown item modern_industrialization:no_such_item"}
== D: breaking one brick unforms the oven and freezes the run ==
  before the break: modern_industrialization:coke_oven at 8,101,0: 18740/32000 FE (1874 EU), progress 105/1000, 20 FE/t, formed true, missing 0, active modern_industrialization:coke_oven_coal_to_coke, t=3747
  oven(8,101,0): modern_industrialization:coke_oven at 8,101,0: 18420/32000 FE (1842 EU), progress 131/1000, 20 FE/t, formed false, missing 1, active modern_industrialization:coke_oven_coal_to_coke, t=3776
  after 120 unformed ticks: modern_industrialization:coke_oven at 8,101,0: 18420/32000 FE (1842 EU), progress 131/1000, 20 FE/t, formed false, missing 1, active modern_industrialization:coke_oven_coal_to_coke, t=3912
  has(8,101,0) modern_industrialization:coke x0: ok
  a wrong block in the gap does not re-form it (the shell is block-specific)
  block(9,102,2) = modern_industrialization:electric_macerator
  oven(8,101,0): modern_industrialization:coke_oven at 8,101,0: 18420/32000 FE (1842 EU), progress 131/1000, 20 FE/t, formed false, missing 1, active modern_industrialization:coke_oven_coal_to_coke, t=3936
  a brick put back re-forms it and the paused run finishes
  block(9,102,2) = minecraft:bricks
  oven(8,101,0): modern_industrialization:coke_oven at 8,101,0: 18140/32000 FE (1814 EU), progress 155/1000, 20 FE/t, formed true, missing 0, active modern_industrialization:coke_oven_coal_to_coke, t=3958
  resumed: modern_industrialization:coke_oven at 8,101,0: 8000/32000 FE (800 EU), progress 0/1000, 0 FE/t, formed true, missing 0, active none, t=4588
  has(8,101,0) modern_industrialization:coke x1: ok
PASS: …
EXIT:0
```

What the numbers prove: the shell is matched member by member (25 → 0, and one brick short is
`missing 1`); the unformed controller burns nothing over the recipe's whole 620 ticks; the formed
one runs at exactly 20 FE/t (2 EU/t) and one coal costs exactly 12,000 FE (1,200 EU over 600 ticks)
into one coke; a **player break** of one member flips `formed` to false within the tick the level
commits it, and the run sits at 131‰ for 120 ticks without spending a FE; a *different* block in the
gap (this port's own macerator) does not form it, so the template is block-specific; the brick back
re-forms it and the same run finishes for the recipe's total, 8,000 FE left of 20,000.

### Coverage (every family this slice ships)

| family | state |
|---|---|
| the multiblock shell, `formed` and the missing-block readout | **tested** — A (controller alone: `formed false, missing 25`), C (25 bricks: `formed true, missing 0`), C2 (a controller re-placed on a standing shell re-forms it), D (a player break: `formed false, missing 1`; a macerator in the gap: still `missing 1`; the brick back: `formed true, missing 0`) |
| the jar's coke recipe (1 coal → 1 coke, 2 EU/t, 600 ticks) | **tested** — C: exactly 12,000 FE, 1 coke, the coal consumed; the unit test pins 998‰ at tick 599 |
| the jar's coke-dust recipe (`#c:dusts/coal` → 1 coke dust) | **tested** — C2: exactly 12,000 FE, 1 coke dust |
| the unformed controller (the counterfactual) | **tested** — B: 20,000 FE and the coal unchanged over 620 ticks, no product, no progress; `SELFTEST=1` inverts it |
| pause and resume on a broken/rebuilt shell | **tested** — D: frozen at 131‰ for 120 ticks with no energy, then the same run finishes for the recipe's total |
| the controller block | **tested** — placed, read back by id, and broken by a player (the self-drop loot table is carried; the drop itself is not asserted) |
| the `coke` and `coke_dust` items | **tested** — made by the recipes and read back; `ctl give` gives them; an unknown id is refused with `{"error":"unknown item …"}` |
| the creosote fluid output (500 mB at 50%) | **not ported** — a mod fluid cannot live in a block-entity tank at this pin (`enderstorage`'s measured gap); the item half is carried and the fluid half is dropped, as upstream drops a chance output that has nowhere to go |
| hatches (item/fluid input/output and the 18 hatchable positions) | **not ported** — the shell is 25 bricks and the controller carries one input and one output slot |
| the `active` model state, the GUI, the orientation | **not ported** — as slice 1 (no custom loader, no menu, no facing) |

### Deviations this slice introduces

1. **Steam is modelled as FE.** Upstream's steam multiblock stores steam (1 mB = 1 EU) in fluid
   hatches; this slice stores the same energy as FE at the mod's own 1 EU = 10 FE (32,000 FE for the
   3,200 EU an LV-equivalent buffer holds). A steam tank is the gap above, not a preference.
2. **The controller carries the item slots** (one in, one out) instead of hatches, so a run whose
   product differs from what the output slot holds waits until the slot is empty — the e2e empties
   it by breaking and re-placing the controller, which a full port would do with a hopper or a
   second output hatch.
3. **No orientation:** upstream rotates the shape with the controller's facing; this shape is fixed
   (the box extends two blocks to +z) because the port has no facing state (slice 1, gap 7).
4. **No overclock:** the coke oven's only overclock source is steel hatches (4 EU/t), not ported; the
   rate is the bronze tier's 2 EU/t, as `getBaseRecipeEu` has it.
5. **`/modern_industrialization info`'s refusal** now reads "That block entity is not one of this
   port's machines." (it covers two machines); `energy` fills either buffer and `efficiency` stays
   the macerator's.

### Pin findings for the next slice

- **`BlockEntityTickEvent` still has no level** (slice 1's gap 2). Anything in a block-entity tick
  that reads the world needs the two-hook split this slice uses: `block_changed` gives
  `&dyn LevelHost` + `BlockChangedEvent { dimension, pos, old, new }` (reads), `level_tick_post`
  gives `HostSide::Server(&mut dyn ServerHost)` + `LevelRef` (writes). `LevelHost` has no write at
  all. `LevelDimension::key().id()` is the dimension key the queue is keyed by.
- **`block_changed` hears both `/setblock` and a player's `destroyBlock`** (legs A/C/D), so a shape
  match needs no `break_block` listener.
- **A block-entity tick's write-back needs a *payload* difference, not just a resized `items`
  vector.** Slice 1's `data` marker is what makes the reply real: keying materialisation on
  `items.len()` alone left `ctl container` answering `{"slots":[]}` while the tick left the reply
  `Changed`. The oven now sets the marker and also checks the length, so a `merge_data` reload that
  shrinks the vector re-materialises it.
- **A machine inside its own shell is unreachable by a click** (the same fact `rftoolsbuilder`
  learned): the e2e breaks the controller from in front of its face (z=-1.5) because from behind the
  shell's own back wall is what the ray hits, and `ctl break` exits 1 when the ray finds nothing.
- **`uses.json` records `type:BlockPos built: full`** at the `block_pos` command helper (and `cargo
  horizon-mod check` prints the same advisory), because the scan
  resolves the `CommandArgumentValue::BlockPos` *variant* to the SDK type of the same name, while the
  variant carries `[i32; 3]`. It is the `Recipe` false entry's class (§"Registry fit-gate repair")
  and harmless here — `BlockPos` is constructible with the same three fields at `0b14f8a`, and
  `fits --build 0b14f8a` is green.
- **Upstream observation, not a defect fix:** the guidebook says the coke oven needs a *steam* fluid
  input hatch to start, while both jar recipes declare no fluid input at all. The port runs the
  recipes as the data declares them; the guidebook's sentence is upstream's prose, not a rule its
  data implements.

Skills: superpowers/using-superpowers,brainstorming,writing-plans,test-driven-development,systematic-debugging,verification-before-completion; BeautifulRust/rust-best-practices,rust-port

## Slice 3 — the hatches, the multiblock companions and the recipe kind (2026-11-21)

**Pin** `0a875da91dbb0fdcb7ba8c42c11341aa229af51a` + mixins
`e59da9094f0c2fdda6c627fe9eff22dec6a906f0`. Package version **0.2.2**.

**The gap it closes.** `notes.md` 6a ("the hatches: a block per hatch type that a shell position
may hold, its own one-slot container/tank, and the controller aggregating them"), plus the two
parent-surface rows `industrialization_overdrive`'s own `tools/parent_surface.py` names as blocking
after the shape: `bronze_plated_bricks` (its pyrolyse oven's shell and casing) and
`cupronickel_coil`/`kanthal_coil` (its `pyrolyse_oven_tier` data map), and "a mod-registered
*machine* recipe kind" / `MachineRecipeType` (its pyrolyse oven proxies the parent's coke recipes).
Class `content`, tier `deep`; shape `machines`, as `horizon/docs/modpacks.md` asks of a multiblock's
inventory and its shell.

### The numbers, read from tag 2.5.12

| Where | What it says |
|---|---|
| `MultiblockMachines.cokeOven` | `cokeOvenHatches = new HatchFlags.Builder().with(ITEM_INPUT).with(ITEM_OUTPUT).with(FLUID_INPUT).with(FLUID_OUTPUT).build()` on both end layers (`add3by3Levels(-1, 1, bricks, cokeOvenHatches)`) |
| `ShapeTemplate.hatchFlags` | a `Map<BlockPos, HatchFlags>`: the middle ring keeps `HatchFlags.NO_HATCH`, so a hatch there cannot form the oven |
| `MultiblockHatches.registerItemHatches("Bronze", "bronze", …, 1, 1, …)` | `bronze_item_input_hatch` / `bronze_item_output_hatch`, **one** item slot each |
| `MultiblockHatches.registerFluidHatches("Bronze", "bronze", …, 4)` | `bronze_fluid_input_hatch` / `bronze_fluid_output_hatch`, **4,000 mB** each (four buckets) |
| `HatchType`/`HatchTypes` | the `HatchType` values `item_input`, `item_output`, `fluid_input`, `fluid_output`, plus the energy/nuclear/large-tank kinds this slice does not port |
| `MultiblockHatches` energy hatches | `lv_energy_input_hatch` etc. are a **different, tiered** family; the add-on's fifth hatch kind is one of them, a later slice |
| `HatchBlockEntity`/`ItemHatch`/`FluidHatch` | `appendItemInputs`/`appendItemOutputs`/`appendFluidInputs`/`appendFluidOutputs` put the hatch's own storage into the multiblock's aggregated inventory |
| `MIParts.COIL` (`asColumnBlock`) and `MACHINE_CASING_SPECIAL` | the coils are column blocks, destroy time 5.0; `bronze_plated_bricks` is a casing block, the same |

### What was built

| piece | where |
|---|---|
| `HatchKind` (4 kinds), `HatchKind::from_block`/`code`/`from_code`, `is_hatch` | `src/runtime.rs` |
| `ShellMember.hatches: &'static [HatchKind]` — upstream's `HatchFlags` per position — and `ShellMember::brick`/`hatchable` | `src/runtime.rs` |
| `match_shell(shell, origin, members)`: a member holds its block **or** an allowed hatch, and the match records each hatch's `HatchKind` (the per-position `Option<HatchKind>`) | `src/runtime.rs` |
| `pyrolyse_shape_fixture`: the add-on's 3×3×4 shell (17 bronze-plated bricks, 16 coil positions, two roof corners hatchable), a matcher fixture only | `src/runtime.rs` |
| `merge_count`: the pump's pure arithmetic (whole stack into empty, free room into same item, nothing into another item) | `src/runtime.rs` |
| `COKE_RECIPE_KIND` + `COKE_RECIPE_FILES` (`include_str!` of the shipped data) | `src/runtime.rs` |
| four hatch block entity types, one-slot/tank declarations, `HatchPart` (materialises storage once) | `src/lib.rs` |
| the shell match saves `hatchKinds` (IntArray, `-1` = no hatch) beside `formed`/`shellMissing` | `src/lib.rs` |
| `pump_hatches`/`pump_item`: every formed controller's item input/output hatches move one stack a tick, in a `transaction`; a loaded controller rejoins the list through `block_entity_on_load` | `src/lib.rs` |
| `register_recipe_type("modern_industrialization:coke_oven")` + `register_raw_recipe_serializer(…)`, the two recipe files under `data/…/recipe/coke_oven/` | `src/lib.rs`, `data/` |
| `/modern_industrialization recipes` (reads `Host::recipes_of_type`) and `shape <pos>` (matches the 3×3×4 fixture), both read-only harness additions | `src/lib.rs` |
| the 7 blocks, their blockstates/models/items/textures/loot/lang/tags/creative tab | `assets/`, `data/` |

**Hooks:** the slice adds **11 API items** (352 vs 342 at `check`): `block_entity_on_load` (v1) joined
the ten already used. The rest is data/declarations, not new hooks.

### The red proof (verbatim)

`sdk-test-prelog.log` — the 9 new runtime tests existed and nothing implemented the new shell API;
the crate did not compile, which is the TDD red for the family:

```
error[E0053]: method `block_at` has an incompatible type for trait
    --> src/lib.rs:719:42
error[E0061]: this function takes 3 arguments but 2 arguments were supplied
    --> src/lib.rs:227:27
error: could not compile `modern_industrialization` (lib) due to 2 previous errors
```

The first run of the new tests then failed on `the_three_by_three_by_four_add_on_shell_matches`
(`assertion failed: !matched.formed` at an offset the fixture made a brick, not a coil); choosing the
actual first coil offset fixed the test, and the test now pins the add-on's 17+16 counts.

### Green

| step | evidence | result |
|---|---|---|
| tests | `sdk-test.log` | **50 passed**, 0 failed (41 + 9 new) |
| lints | `sdk-clippy-slice3.log` | exit 0, **0 warnings** |
| format | `cargo fmt --check` | clean |
| build 1 + 2 | `sdk-build.log`, `sdk-build-2.log` | **byte-identical**, `d2d388ce…` |
| package | — | **103,597 B**, SHA-256 `d2d388ce1932fbd94afeb75c2d804951692f992d0786c0596e41fddf22276ae7`, **129 entries**, no packed `Cargo.lock` |
| check | `sdk-check.log` | exit 0, `modern_industrialization 0.2.2 (native-mod, format native-1/0a875da…/e59da909…, for 26.3)` |
| fits (the pin) | `sdk-fits.log` | `0a875da  made for`, exit 0 |
| fits (another build) | `sdk-fits-next.log` | `0b14f8a  fits`, exit 0 |
| gate | `verify-sdk-package.py` | **1/1** |
| in game | `evidence/modern_industrialization-e2e/e2e-launcher.log` | **PASS** + `EXIT:0` |
| control | `e2e-launcher-control.log` | **FAIL** + `EXIT:1` (`expected …electric_macerator at 0,100,0, got 'minecraft:air'`) |
| fail-capability | `e2e-launcher-selftest.log` | **FAIL** + `EXIT:1` (`SELFTEST flipped: the unformed oven ran nothing and burned nothing, as it should`) |
| log gate | `check-e2e-logs.py modern_industrialization-e2e --strict` | logs=5 ok=5 |

The launcher rebuilt the game for this version (`native-0a875da-7f4050165798`); all three legs ran
against that export.

### The in-game leg (quoted from `e2e-launcher.log`)

Controller at `8,101,0`; item input hatch at `7,100,0`, item output hatch at `9,102,2`, fluid input
hatch at `8,100,0`; the matcher's 3×3×4 fixture at `20,101,0`.

```
== C: a 3x3x3 shell of 22 bricks and three bronze hatches forms it ==
  shell(8,101,0) = 22 bricks + 3 hatches
  oven(8,101,0): modern_industrialization:coke_oven at 8,101,0: 0/32000 FE (0 EU), progress 0/1000, 0 FE/t, formed true, missing 0, hatches 3, active none, t=2272
  hatch slots(7,100,0) = 1
  hatch slots(9,102,2) = 1
  hatch slots(8,100,0) = 0
== D: one coal in the ITEM INPUT HATCH becomes one coke in the ITEM OUTPUT HATCH ==
  running: modern_industrialization:coke_oven at 8,101,0: 19320/32000 FE (1932 EU), progress 56/1000, 20 FE/t, formed true, missing 0, hatches 3, active modern_industrialization:coke_oven_coal_to_coke, t=2326
  has(7,100,0) minecraft:coal x0: ok
  modern_industrialization:coke_oven at 8,101,0: 8000/32000 FE (800 EU), progress 0/1000, 0 FE/t, formed true, missing 0, hatches 3, active none, t=2962
  has(9,102,2) modern_industrialization:coke x1: ok
  has(8,101,0) modern_industrialization:coke x0: ok
== D2: the jar's second coke oven recipe (coal dust), through the hatch ==
  has(9,102,2) modern_industrialization:coke_dust x1: ok
== E: breaking one brick unforms the oven and freezes the run ==
  oven(8,101,0): … formed false, missing 1, hatches 3, active modern_industrialization:coke_oven_coal_to_coke, t=3796
  after 120 unformed ticks: … 18280/32000 FE, progress 143/1000, formed false, missing 1, hatches 3 …
  a coil in the gap does not re-form it (the shell is block-specific)
  a brick put back re-forms it and the paused run finishes
  resumed: … 8000/32000 FE (800 EU), progress 0/1000, formed true, missing 0, hatches 3 …
  has(9,102,2) modern_industrialization:coke x1: ok
== F: a hatch in a middle-ring position (HatchFlags.NO_HATCH) does not form it ==
  oven(8,101,0): … formed false, missing 1 …
== G: /modern_industrialization recipes reads the coke oven kind back ==
  modern_industrialization:coke_oven/coke [modern_industrialization:coke_oven] {"duration":600,"eu":2,…,"item_inputs":{"amount":1,"item":"minecraft:coal"},"item_outputs":{"amount":1,"item":"modern_industrialization:coke"},"type":"modern_industrialization:coke_oven"}
== H: the add-on companions and the 3x3x4 matcher check ==
  block(20,100,6) = modern_industrialization:bronze_plated_bricks
  block(20,100,7) = modern_industrialization:cupronickel_coil
  block(20,100,8) = modern_industrialization:kanthal_coil
  shape fixture(20,101,0) = 33 members
  shape(20,101,0): pyrolyse shape at 20,101,0: formed true, missing 0, hatches 2 (33 members), t=4747
  shape(20,101,0): pyrolyse shape at 20,101,0: formed false, missing 1, hatches 2 (33 members), t=4763
  shape(20,101,0): pyrolyse shape at 20,101,0: formed false, missing 1, hatches 2 (33 members), t=4779
  shape(20,101,0): pyrolyse shape at 20,101,0: formed true, missing 0, hatches 2 (33 members), t=4795
PASS: …
EXIT:0
```

What the numbers prove: the shell accepts a hatch exactly where the jar's `HatchFlags` do (22 bricks +
3 hatches forms; the same hatch in the middle ring is `missing 1`); an item hatch exposes one slot
and a fluid hatch none; **the controller's run is fed through the input hatch and its product leaves
by the output hatch** (the coal is gone from the hatch and the controller's own output is empty when
the coke sits in the output hatch — a missing pump cannot produce that); both jar recipes cost exactly
12,000 FE through hatches; a player break still freezes the run at 143‰ for 120 ticks; the 3×3×4
fixture matches with 17 bronze bricks and 16 coil positions and two hatches, and unforms on one broken
coil or a brick where a coil belongs; `/modern_industrialization recipes` shows the 2 recipes the
add-on reads by type, with `duration: 600` and the item ids in the files.

### Coverage (every family this slice ships)

| family | state |
|---|---|
| the four bronze hatches as shell members | **tested** — C (22 bricks + 3 hatches: `formed true, missing 0, hatches 3`), F (a hatch in the middle ring: `formed false, missing 1`), the unit tests for the allowed kinds and the brick-only ring |
| hatch storage | **tested** — C: item hatches expose 1 slot, the fluid hatch 0 (`ctl container`), the controller keeps its 2 internal slots |
| the controller ↔ item hatch pump (in and out) | **tested** — D/D2: the coal/coal dust enters through the input hatch (`has … x0` after the pump), the coke/coke dust leaves into the output hatch while the controller's own output is empty |
| the fluid hatch's 4,000 mB tank | **not ported (contents)** — the block forms the shell and is read back, but the harness has no fluid command and a mod fluid cannot live in a BE tank via `fluid_insert` (`enderstorage`'s measured gap); the tank is declared, the coke recipes take no fluid input, and the creosote output stays dropped (`notes.md` 6c) |
| the energy hatch family | **not ported** — a different, tiered family (`lv/mv/hv/ev/superconductor`); the add-on's fifth hatch kind; a later slice |
| the multiblock companions `bronze_plated_bricks`, `cupronickel_coil`, `kanthal_coil` | **tested** — H: placed and read back; the coils are also the 16-member half of the 3×3×4 fixture; all self-drop and are in the tags/creative tab |
| the coke oven recipe kind another mod reads | **tested** — G: `recipes_of_type("modern_industrialization:coke_oven")` answers 2 recipes with their ids and file JSON; the unit test pins the shipped files to `COKE_RECIPES` |
| the 3×3×4 shell matcher (the add-on's shape) | **tested** — H: formed true/missing 0/hatches 2, one coil broken missing 1, a brick where a coil belongs still missing 1; the unit test also pins 17 bricks + 16 coil positions |
| the coke oven itself and the macerator (regression) | **tested** — C–F and the unchanged macerator legs |

### Deviations this slice introduces

1. **The controller keeps its two item slots** as the run buffer, and the pump feeds/drains them. Upstream's
   controller has no inventory: the hatches are the inventory. Keeping the slots means a player or a
   hopper can still feed the controller directly (the slice-2 deviation); the hatch path is the one the
   e2e proves, and `notes.md` records the extra route.
2. **Steam is still modelled as FE**, as slice 2; the fluid hatches hold a declared tank but the machine
   takes no fluid, because neither jar recipe has a fluid input.
3. **The creosote fluid output is still dropped** (500 mB at 50%) — the fluid-tank gap (6c), not a choice.
4. **The hatches do not auto-insert/auto-extract** (`tickTransfer`): a hopper or pipe uses the container
   capability, which is the visible behaviour; the GUI's auto-extract toggle is part of the unported GUI.
5. **The hatches' models** are port-authored plain models (bronze casing + the jar's side/output
   overlays) because the jar's `modern_industrialization:machine` model loader is a client mod feature;
   the textures are the jar's.
6. **`shape` is a harness command**, not a game object: it matches the add-on's 3×3×4 template without a
   pyrolyse-oven controller (which the add-on owns and this port must not build). It is the parent's
   matcher proof at 3×3×4, and it is recorded as an addition like `energy`/`efficiency`.
7. **The machine's table is still `COKE_RECIPES`** (a constant): the *kind* is data and another mod reads
   it, but the oven itself does not re-read the files at runtime. The unit test
   `the_shipped_coke_recipe_data_is_the_kind_another_mod_reads` pins the two together, so they cannot
   drift; a future slice can make the tick read the data through the same `recipes_of_type` call.
8. **An oven saved by 0.2.1 has no `hatchKinds`**, so its hatches start pumping on the first shell
   change (or after re-placing the controller), not immediately on load. Nothing is lost: the saved
   run, buffer and `formed` are read as before; only the new layout is missing until a rematch.

### Pin findings for the next slice

- **Another mod can read a machine recipe kind at this pin.** `Registrar::register_recipe_type(id)` +
  `register_raw_recipe_serializer(serializer, id)` (or `register_read_only_recipe_serializer` for an
  independently declared type), then `Host::recipes_of_type(id) -> Arc<[Recipe]>` gives each recipe's
  `id`, `kind` and the file's `serde_json::Value` (its `Display` is compact JSON). The recipe files
  load from the package's own `data/<ns>/recipe/` even though the package is native. This closes the
  "no mod-readable machine recipe kind" half of slice 1's gap 3; the machine-side table is still code.
- **`item_slot` / `set_item_slot` work across block entities** inside
  `ServerHost::transaction_open()` … `transaction_close(&seal, true)`: a controller can read and write a
  hatch's slot in the same tick. `set_item_slot` is silent, so `set_changed` is what dirties the chunk;
  `item_slot` returns `Ok(None)` for an empty slot and `Err(Stale)` for a removed/reloaded entity.
- **`block_entity_on_load` (v1)** gives `HostSide` + `BlockEntityOnLoadEvent { id, entity }`, so a
  controller can rejoin a session list after a world reload; without it the pump would only start on
  the next shell change. `entity.level(host)` + `level.dimension().id(host)` give the key.
- **A length-less `level_tick_post` stays the right place for per-tick world work**: the block-entity
  tick still has no level (slice 1's gap 2), so the pump and the shape write both live in
  `level_tick_post`; the Part's tick only reads the saved layout.
- **The harness cannot fill a fluid tank** (`ctl` has no fluid command), and `enderstorage`'s gap stands:
  a mod fluid's amount in a BE tank is not written by `fluid_insert`. The fluid hatch's contents are
  therefore unasserted, and the creosote output waits for the tank slice.

Skills: superpowers/using-superpowers,brainstorming,writing-plans,test-driven-development,systematic-debugging,verification-before-completion; BeautifulRust/rust-best-practices,rust-port

## Next slices (in dependency order)

1. **The steam pair** (`bronze_macerator`, `bronze_furnace`) on the same tick: a
   steam input slot and the `SteamCraftingMachineBlockEntity` numbers, so the
   bronze age plays before electricity.
2. **The material recipes**: register the dust→ingot and ore→crushed chains
   (`c:ores/*`, `c:dusts/*`) with the MI items they need, which grows the
   macerator's table back toward the jar's 324 and makes the dusts useful.
3. **A data-driven recipe type** — **half done (slice 3):** the coke oven's kind is registered, shipped
   as data and readable by another mod (`Host::recipes_of_type`); the oven's own table is still a
   constant. What is left: make the tick read the data (then both jar recipes stop being code).
4. **The item/fluid pipes and the LV cable**: MI energy is its own cable tier;
   with API 142's move helper a cable slice can wire machines together.
5. **The electric machines that are the same shape** (furnace, compressor,
   cutting machine, mixer, packer, unpacker, wiremill) — each adds its recipe
   type and slots to the same runtime.
6. ~~**The multiblocks** (coke oven, blast furnace, distillation tower): controllers,
   hatches and shape matching, a slice of their own.~~ **Partly done (slice 2):** the shell
   template, the controller, `formed` and the coke oven. What is left, in order:
   6a. ~~**The hatches** (`bronze_item_input_hatch`, `bronze_item_output_hatch`,
       `bronze_fluid_input_hatch`, `bronze_fluid_output_hatch`): a block per hatch type, its own
       one-slot container/tank, the controller aggregating them.~~ **Done (slice 3):** the four
       bronze hatches, the per-position `HatchFlags`, the item pump through the shell, the
       `bronze_plated_bricks`/coil companions and the readable coke recipe kind are in. What is
       left for `industrialization_overdrive`: the **energy hatch family**
       (`lv_energy_input_hatch` etc.) — its fifth hatch kind — and its own controller, which is the
       add-on's slice.
   6b. **The steam blast furnace** (`steam_blast_furnace`, the mod's own `fire_clay_bricks`
       over 3×3×4) on the same machinery, plus `fire_clay_bricks` and its recipe.
   6c. **A fluid tank for a mod's own fluid** (`modern_industrialization:steam`, `creosote`) —
       the recorded gap (`Registrar::block_fluids` exists; the harness cannot fill it, a mod fluid's
       `fluid_insert` does not land in a BE tank, and the coke oven's creosote output is the first
       thing it would carry).
   6d. **The distillation tower, heat exchanger, vacuum freezer, implosion compressor, fusion
       reactor** — each a `ShapeTemplate` with pipes and tier data maps (data maps are the tier
       surface the ledger notes as not yet used by any port).

## Reviewer repairs (2026-11-11) — PR review items and what changed


Each item below is quoted from `ports/_shared/horizon-mods/reviews/pr-*-modern_industrialization.md` (the review is the specification), with what this repair did about it.

- **Reviewer:** `support.rs` erased other mods' energy declarations
  **Repair:** Fixed: `on_block_entity_energy` returns for any block it does not own instead of assigning `None`. Tests: `the_energy_hook_declares_the_macerators_buffer` and `the_energy_hook_leaves_another_mods_declaration_alone`.
- **Reviewer:** mutation commands accept any block entity at the position
  **Repair:** Fixed: both commands check `runtime::is_macerator(entity.id)` before reading or storing macerator state and answer "That block entity is not an electric macerator; nothing was changed." Test: `the_mutation_commands_only_touch_this_slices_machine`.
- **Reviewer:** licence notice
  **Repair:** `LICENSE` + `LICENSE-NOTICE.md` in the `.hnmod`; `data/modern_industrialization/license/` in the carried `.hmod`; copyright `2020 Azercoco & Technici4n`.
- **Reviewer:** truncated description
  **Repair:** Rewritten: the run description, the 34/324 recipe count and the recipe-table limitation are complete sentences.
- **Reviewer:** registry artifact delivery
  **Repair:** **Staging (orchestrator, not ours)**: the artifact is not in registry release/staging as `<sha256>.<ext>`; the orchestrator answers it with `horizon-mods#686` (no write access to the registry's releases). Every repackaged artifact below has a fresh sha256 for that re-push, and the author release must be re-uploaded from the port repository.

Skills: superpowers/using-superpowers,systematic-debugging,verification-before-completion; BeautifulRust/rust-best-practices

### Repair verification (2026-11-11/12)

- Staged `check`: check exit 0 (`ports/_shared/evidence/repair-2026-11-11/modern_industrialization-check.log`)
- Embedded tests: embedded **27 passed; 0 failed** (`ports/_shared/evidence/repair-2026-11-11/modern_industrialization-test.log`, the three new hook/command tests included)
- In-game: e2e **PASS** + `EXIT:0` (`ports/_shared/evidence/repair-2026-11-11/modern_industrialization-e2e.log`): buffer, recipes, overclock, save/relaunch
- `cargo test` (standalone lib), `cargo clippy --all-targets -- -D warnings` and
  `cargo fmt --check` are clean in the port; the artifacts were built twice and
  hash identically (`registry.toml` carries the sha256 above).
- Skills: superpowers/using-superpowers,systematic-debugging,test-driven-development,verification-before-completion; BeautifulRust/rust-best-practices,rust-port


## Migration to the SDK format (2026-11-18)

Horizon's mod-API merge (#16941, launcher ≥ 0.2.70) retired `.hnmod`; the registry closed PR
**#838** with that reason. This port is now **one flat `.hmod`** — code and content in the same
package — built by `cargo horizon-mod build` against the pinned SDK:

- SDK nightly **`0b14f8acfd97fa2c750e42795249dcc981178ca4`** (2026-11-18), mixins
  **`e59da9094f0c2fdda6c627fe9eff22dec6a906f0`**, launcher 0.2.78, rust 1.99.1,
  `content_api 0.2`, package format 6.
- `api = "0.2"`, `side = "both"`, `capabilities = ["registries"]` (the block and the nine dusts are
  declarations under `data/`), `[native] game = "26.3"`, `horizon`/`mixins` at the commits above.
- **Class `content`, tier `deep`; slice shape `machines`** (unchanged): a block entity type, its
  Part, an item container, an FE capability, a recipe tick and a command.

### File map (old → new)

| Old (retired `.hnmod`) | New (flat SDK package) |
|---|---|
| `Cargo.toml` (`[lib] path = "mixins/…/runtime.rs"`) | `Cargo.toml` — the crate is `src/lib.rs`; `horizon-mod-api` comes from `.horizon/sdk/` (fetched, never packed in the sources) |
| `mixins/<id>/horizon-mixin.toml` (the plan) | `horizon-mod.toml` — the only manifest, and the only identity |
| `mixins/<id>/runtime.rs` | `src/runtime.rs` — **byte-identical**, SHA-256 `0f3b451a2371ec6ddbcb28fe3b07af3a2d358d843d0960a3ad0bef5f05a28c7b`, its **27 tests green** |
| `mixins/<id>/support.rs` | folded into `src/lib.rs` (register + Part + commands); no support module and no per-hook helper files exist any more |
| `mixins/<id>/{block_entity,ticks,container,energy,tick,register,perform}.rs` | methods of `impl mod_api::Hooks` / `impl BlockEntityPart` in `src/lib.rs` |
| `mixins/<id>/embedded_content.rs` + the `content_root` target injection | **gone**: the package carries `assets/` and `data/` itself, so nothing is written to `.horizon-cache/` and no dev list is appended |
| `mixins/<id>/data/<id>-content.hmod`, `content/…hmod`, `<id>.hnmod` | one `modern_industrialization.hmod` (48,563 B, SHA-256 `e78721b80ec26c55bae5c3cd90d157cae1663932d438bc4559bb6385753ba5ca`, 60 entries) |
| `compatibility.json`, `rust-toolchain.toml`, `native-hooks.json` | removed: `[native]` in the manifest is the pin, and the SDK toolchain is the SDK's |
| `tools/{horizon.py,package.py,package.sh,stamp_license.py,hmod-load.sh}` | removed; `cargo horizon-mod build|check|fits` replace them. `tools/carry_recipes.py` stays (it generates the recipe table), `tools/e2e.sh` is kept **retired** (see gaps) |

### Helper → hook table (7 mapped, 0 hooks lost, 1 retired target, 2 API gaps)

| Old helper (`.hnmod`) | Old hook / target | New SDK construct | State |
|---|---|---|---|
| `block_entity.rs` → `on_block_entity` | `block_entity` v1 (15) | `Registrar::block_entity_type("electric_macerator").blocks(&["electric_macerator"]).build()` declares the type, and `BlockEntityPart::block_entity` answers the initial saved data (`Some(Compound::new())`) — the machine starts empty, as `EnergyComponent`/`CrafterComponent` do | **kept** |
| `ticks.rs` → `on_block_entity_ticks` | `block_entity_ticks` v1 (15) | `BlockEntityPart::block_entity_ticks` sets `event.ticks = true`; the part is attached to the type and the game asks it **by block** (`a_block_entity_part_answers_by_block_and_by_entity_type`, SDK test) | **kept** |
| `container.rs` → `on_block_entity_container` | `block_entity_container` v1 (16) | `Registrar::block_container(&block, ModContainer{ slots: 5, faces: all six faces reach every slot, inputs: [0], outputs: 1..=4, menu: none })` — the game's own `horizon:items` provider, so hoppers/pipes reach it without a hook | **kept** (declaration, not a hook) |
| `energy.rs` → `on_block_entity_energy` | `block_entity_energy` v1 (142) | `Registrar::block_energy(&block, ModEnergyBuffer{ capacity: 32_000, max_insert: 32_000, max_extract: 0, insert both })` — the amount is the int `energy` in the saved compound, which the tick writes and the capability reads | **kept** (declaration, not a hook) |
| `tick.rs` → `on_block_entity_tick` | `block_entity_tick` v1 (17) | `BlockEntityPart::block_entity_tick(host, event)`: `event.pos`, `event.entity` (Deref → `nbt`/`items`/`data`), `*event.reply = Unchanged` when nothing ran (no chunk dirty) | **kept** |
| `register.rs` → `on_register_commands` | `register_commands` v1 (8) | `Hooks::register_commands` pushing `Command::new` / `Command::op(…, 2)` into `event.commands` | **kept** |
| `perform.rs` → `on_perform_command` | `perform_command` v1 (8) | `Hooks::run_mod_command` returning `Flow::Cancel`; the raw line became a parsed `CommandInvocation` (`literals`, `BlockPos`/`Integer` arguments, execution dimension), and feedback is `host.commands().feedback(Text::literal(…))` | **kept** (typed in, so no `split_whitespace` and no word-index parsing) |
| `embedded_content.rs` | `content_root` target (AE2 pattern) | the flat package's own `assets/` + `data/` with `capabilities = ["registries"]` | **retired** — the container split it worked around is gone |

Host calls the new glue uses, per old call it replaces:

| Old game call | New |
|---|---|
| `game.native_with_mod_block_entity(pos, …)` | `host.block_entity(level, pos)` → `BlockEntityRef`, then `entity.data(host)` / `entity.kind(host)` |
| writing a block entity's keys from a command | `entity.merge_data(host, Compound)` (as `/data merge block`: reload, `setChanged`, client update) |
| `entity.nbt` read/write inside the tick | `event.entity.nbt` (`ModBlockEntityFields`), kept by the default `BlockEntityTickReply::Changed` |
| `game.world.modified.insert(…)` (`mark_modified`) | the reply: `Changed` dirties the chunk and notifies comparators, `Unchanged` does neither |
| `game.res.items().get(id).max_stack` | `host.item_key(&Identifier)` + `host.max_stack_size(key)` |
| `game.native_feedback(Text::literal(…))` | `host.commands().feedback(Text::literal(…))` |
| `game.world.game_time` (`t=` in the info line) | `entity.level(host)` → `level.game_time(host)` |

### Gaps recorded by this migration (not faked)

1. **No machine-recipe registry.** The SDK still has no data-driven machine recipe type: the
   package can declare *crafting* recipe serializers (`CraftingRecipeHandler`), not a machine's own
   processing recipe. The 33-row table therefore stays a `const` in `src/runtime.rs`, and adding a
   recipe is a code change. Issue shape: "a mod-registered, data-driven machine recipe type" (same
   gap as slice 1; the migration did not close it).
2. **No level in the block entity tick event.** `BlockEntityTickEvent` carries `pos` and the
   entity, not its level, so the chance-output roll cannot be seeded from the world's game time as
   the `.hnmod` glue did. It is seeded from the position and the buffer the machine holds instead
   (`roll_seed`). Every carried recipe is certain (`permille = 10_000`), so nothing observable
   depends on it today; a future chance recipe would roll the same at the same position and buffer.
   Issue shape: "the owning level in `block_entity_tick`, or a game-time read that needs no level".
3. **No in-game run at this pin — the honest limit of this slice.** The SDK pin `0b14f8a` has no
   game binary on this machine: `cargo horizon-mod run` needs a `horizon-client` built from the
   same SDK, and the campaign's binaries are `1d63e0a5`/`41f822b6` (a `.hnmod` game cannot load an
   SDK package). `tools/e2e.sh` is kept retired as the scenario specification. What is verified
   today: the SDK compiles the mod for native **and** wasm (`cargo horizon-mod check`, exit 0, 261
   API items), the package passes `check` and `fits` at `0b14f8a`, `cargo test` runs the pure core's
   **27 tests**, and `clippy -D warnings`/`fmt --check` are clean. The in-game claims of slice 1
   (buffer, recipes, overclock, save/relaunch) were verified at pin `1d63e0a5` and are **not**
   re-verified here; the next slice with a game binary must re-run `tools/e2e.sh`.
4. **`block_entity_ticks` by block, not by type.** The ticker question is asked per *block* (SDK
   test `a_block_entity_part_answers_by_block_and_by_entity_type`) and the Part is attached to the
   *type*; the registrar files a type's part under each of the type's blocks, which is what makes
   `event.ticks = true` reach `modern_industrialization:electric_macerator`. If a later port sees a
   machine that never ticks, check the type's `blocks(…)` list first — that is the one wiring the
   compile cannot prove.
5. **`ModContainer` has no per-slot player rule at this pin either** — but `menu_slot_rules` now
   documents "registered vanilla and mod menus", so the GUI that slice 1 deferred (and the
   output-slot protection it needs) is a *slice* gap with a plausible route, not a hard API hole.
   Still no menu, no `active` block state, no orientation, no side configuration.

### Deviations this migration introduced

- `/modern_industrialization info` now answers `That block entity is not an electric macerator.`
  for another mod's block entity instead of printing a zeroed machine readout; the two mutation
  usages refuse it as they already did. A user notices only a clearer message.
- The mutation usages merge **only** the keys they change (`energy`, `efficiencyTicks`,
  `maxEfficiencyTicks`) instead of rewriting the whole saved compound, so another mod's keys on the
  same block entity survive a harness control. Upstream has no such command; this is harness-only.
- `ModEnergyBuffer` is built field by field although `cargo horizon-mod check` advises ending it
  with `..Default::default()`; `clippy::needless_update` refuses that today (all five fields are
  named). A field added to the struct in a later build makes the line fail to compile, which the
  check reports — the safe failure.
- `Cargo.toml`'s `[lib]` section is gone: the crate is `src/lib.rs` with `pub mod runtime`, which is
  the SDK layout, so `cargo test` builds the SDK once and then runs the same 27 tests.

### Build, check and package (commands, and what they printed)

All compile steps ran through the campaign slot registry; `BUILD_LOCAL=1` because the second box
has neither the SDK, the `wasm32-unknown-unknown` target nor the SDK's toolchain, so forwarding
cannot work for this package.

```sh
BUILD_LOCAL=1 ports/tools/build-slot.sh run --job sdk-mi --port ports/modern_industrialization \
    --step check -- cargo horizon-mod check
BUILD_LOCAL=1 ports/tools/build-slot.sh run --job sdk-mi --port ports/modern_industrialization \
    --step build -- cargo horizon-mod build
BUILD_LOCAL=1 ports/tools/build-slot.sh run --job sdk-mi --port ports/modern_industrialization \
    --step check -- cargo horizon-mod check modern_industrialization.hmod
BUILD_LOCAL=1 ports/tools/build-slot.sh run --job sdk-mi --port ports/modern_industrialization \
    --step check -- cargo horizon-mod fits modern_industrialization.hmod
BUILD_LOCAL=1 ports/tools/build-slot.sh run --job sdk-mi --port ports/modern_industrialization \
    --step test -- cargo test --offline
```

| Step | Result |
|---|---|
| `cargo horizon-mod check` | exit 0; `Modern Industrialization uses 261 items of the API (4 Part kinds, 1 capability, 59 fields or variants, 8 hooks, 12 hook arguments, 2 macros, 82 methods, 19 traits, 74 types). Made for 0b14f8a.` |
| `cargo horizon-mod build` | exit 0; `Built ./target/horizon-mod/modern_industrialization.hmod`; two builds byte-identical (`e78721b8…`) |
| `cargo horizon-mod check <hmod>` | exit 0; `modern_industrialization 0.2.0 (native-mod, format native-1/0b14f8ac…/e59da909…, for 26.3)`, **48,563 bytes**, SHA-256 `e78721b80ec26c55bae5c3cd90d157cae1663932d438bc4559bb6385753ba5ca`, unsigned |
| `cargo horizon-mod fits <hmod>` | exit 0; `Modern Industrialization: 0b14f8a made for` |
| `cargo test --offline` | **27 passed; 0 failed** (the byte-identical `runtime.rs`) |
| `cargo clippy --all-targets -- -D warnings` | exit 0, 0 warnings |
| `cargo fmt --check` | clean |

Evidence: `ports/_shared/evidence/modern_industrialization/sdk-{check,check-package,build,test,clippy,fits}.log`.

### Traps for the next SDK port

- `cargo horizon-mod check` refuses **any** folder path it does not know (`error: unexpected
  package path: mixins/…`), so the retired layout has to be deleted before the first check, not
  after.
- The check compiles for **wasm32 as well as native**: a machine without
  `rustup target add wasm32-unknown-unknown` fails with `can't find crate for core`. It is a
  one-time, per-machine install.
- `cargo horizon-mod check` prints advisories (field-by-field struct literals, unknown-path notes)
  with exit 0; `clippy -D warnings` can then refuse the `..Default::default()` the same tool
  suggested (`needless_update`). Prefer explicit fields when every field is named.
- A *content* `cargo horizon-mod build` refuses a folder with a `Cargo.toml`; the **native** build
  accepts it and packs `horizon-mod.toml`, `Cargo.lock`, `Cargo.toml`, `LICENSE`,
  `LICENSE-NOTICE.md`, `README.md`, `src/`, `uses.json` and `assets/`+`data/`. Whatever lands in
  the package root is shipped, so a root README is a **user-facing** document here.
- The `.hmod` is written to `target/horizon-mod/`; it must be copied to the port root, because
  `target/` is disposable.
- The `data/**/horizon/*.json` declarations are the same shape a pure content `.hmod` uses; the
  native package needs no `native_blocks`/`native_items` and no embedded `.hmod`, because the
  package *is* content and code at once.

Skills: superpowers/using-superpowers,brainstorming,writing-plans,test-driven-development,systematic-debugging,verification-before-completion; BeautifulRust/rust-best-practices,rust-port

### Migration verification (2026-11-18)

Everything in the table above was re-read from the logs in
`ports/_shared/evidence/modern_industrialization/` before this line was written: `sdk-check.log`
(exit 0, 261 API items), `sdk-build.log` (two builds, one sha256), `sdk-check-package.log`
(48,563 B, `e78721b8…`), `sdk-fits.log` (`0b14f8a made for`), `sdk-test.log`
(`27 passed; 0 failed`), `sdk-clippy.log` (exit 0).

## Packaging gate and in-game leg (2026-11-20)

The package carried a packed `Cargo.lock` (60 entries): `cargo test` writes one into the port
directory and `cargo horizon-mod build` packs the directory's file list. Rebuilt with
`rm -f Cargo.lock` before **each** build — twice, byte-identical — and nothing else changed:

| step | command | result |
|---|---|---|
| lock | `rm -f Cargo.lock` before every build | absent before both builds and after them |
| tests | `cargo test --offline` | **27 passed**, 0 failed, exit 0 (`evidence/modern_industrialization/sdk-test.log`) |
| lints | `cargo clippy --all-targets --offline -- -D warnings` | exit 0, **0 warnings** (`sdk-clippy.log`) |
| format | `rustfmt --edition 2024 --check src/*.rs` | exit 0 (`sdk-fmt.log`) |
| build 1 + 2 | `cargo horizon-mod build`, `cmp` | **byte-identical** (`sdk-build.log`, `sdk-build-2.log`) |
| package | `unzip -l modern_industrialization.hmod` | **46,217 B**, SHA-256 `759795e7a99304cceb3f7d28ed00382cf8f5f4dbe00144912d7c06a3f620eca0`, **59 entries**, `no packed Cargo.lock` |
| package check | `cargo horizon-mod check modern_industrialization.hmod` | exit 0 (`sdk-check.log`) |
| fits | `cargo horizon-mod fits modern_industrialization.hmod --build 0b14f8a` | exit 0, *"0b14f8a made for"* (`sdk-fits.log`) |
| gate | `python3 ports/tools/verify-sdk-package.py modern_industrialization` | **1/1** |
| in-game (modded) | `launcher-game.sh build … --rebuild` then the leg | **FAIL** + `EXIT:1` (`evidence/modern_industrialization-e2e/e2e-launcher.log`) |

`tools/e2e.sh` was re-pointed from the retired `.hnmod` ladder (`.horizon-cache`, `--dev-mod`,
`content/…hmod`) at the launcher-built game; the control is the same Nightly without the package
(`GAME=$CONTROL_GAME`). The leg passes A (the block, its block entity and the `/… info` readout are
there) and fails at B's first item insertion:

```
== B: control — input, no energy, nothing happens ==
  block(0,100,4) = modern_industrialization:electric_macerator
ctl container 0 100 4 minecraft:stone 1 -> {"error":"container is full"}
```

`ctl container 0 100 4` alone answers `{"slots":[]}`: the placed macerator's block entity exposes
**no item slots** to the engine, although `r.block_container` is called with a 5-slot
`ModContainer` and the same shape works for `charginggadgets` at this pin (its leg's
`ctl container` inserts and burns a coal). Reproduced with both placement paths (`setblock` and
`ctl place --item`). That pass changed nothing in `src/`; the fix is the next section.

## The declared container the game could not see (2026-11-20)

**Defect.** A freshly placed macerator answered `ctl container 0 100 0` with `{"slots":[]}` and
refused an insert with `{"error":"container is full"}`, although `register` declares a five-slot
`ModContainer` for the block. The block, the block entity and `/modern_industrialization info` were
all there — only the item slots were missing.

**Root cause (the difference from `charginggadgets`).** A mod block entity is created with
**empty** `items` (`block_entity_for_block` → `ModBlockEntityFields { nbt: initial, ..Default }`,
`horizon-client/src/game/native_hooks/shared.rs:1949`), and the engine sizes that vector to the
declared container only in the `Changed` branch of the tick write-back
(`api_block_entity_tick`, `horizon-client/src/game/modding/block_entity_tick_reply.rs`:
`if changed { if entity.items.len() < slots { entity.items.resize(slots, None); } … }`). An
`Unchanged` reply "discards this Part's payload edits: no reply snapshot is encoded, decoded or
applied", so an idle tick throws the normalisation away. A save keeps only the occupied slots
(`save/block_entity_encode.rs` writes `Items` only when a slot is occupied; the decoder pops the
trailing empty ones), so a **reloaded** machine starts empty again and needs the same first
`Changed` tick.

`charginggadgets` never meets the trap because its first tick answers `Changed` for another
reason: it has a menu, and `event.entity.data` (`data_slots()`, four values) differs from the fresh
entity's empty `data`, so `data_changed` forces the write-back that materialises its two slots.
This port has no menu and no `data`, so its idle tick answered `Unchanged` on the very first tick
and the machine never got any slots — nothing could be inserted, so nothing ever changed, so it
never would.

**Fix.** The tick now writes its payload back once per entity lifetime, which is the tick the
engine sizes the slots on:

```rust
let slots_ready = !fields.data.is_empty();
if !slots_ready {
    fields.data = vec![SLOTS_READY];
}
…
if tick.active || tick.crafted.is_some() || state != before {
    store(&mut fields.nbt, &state);
} else if slots_ready {
    *event.reply = BlockEntityTickReply::Unchanged;
}
```

`data` is the one payload field nothing saves (its own documentation says "which the mod sets and
nothing saves"), so it is empty exactly on a fresh entity *and* after every reload — the two
moments the slots have to be materialised. After that tick the idle path keeps its
`Unchanged` reply, so a machine that is doing nothing still does not dirty its chunk, advance its
revision or notify comparators. The alternative — always answering `Changed` — would also work and
is one line shorter, but it dirties the chunk every tick for the machine's whole life.

**Is this the port's defect or the engine's?** The API's contract for `block_entity_tick` says the
payload's `items` must keep "their count: as many as its container has", which the snapshot already
does — the entity's *own* vector is what only a `Changed` tick grows, and no API call can grow it
(`BlockEntityRef::set_item_slot` returns early when the slot already reads empty). So the port
cannot materialise the slots except through a write-back, and a write-back is what it now makes.
An engine that sized `items` to the declared container when it created the entity (as
`open_mod_menu` already does when a menu opens) would make the flag unnecessary; that is worth an
upstream note, not a blocker. No upstream report filed.

**Test that pins it.** `tools/e2e.sh` gained `expect_slots X Y Z COUNT` and asserts **5** in A (a
fresh machine) and again in G, right after the save → quit → relaunch. Watched red on the unfixed
package (`evidence/modern_industrialization-e2e/e2e-red-slots.log`:
`slots(0,100,0) = 0` / `FAIL: … exposes 0 item slots, expected 5: {"slots":[]}`) and green after the
fix (`slots(0,100,0) = 5` both times, with the whole run reaching `PASS` + `EXIT:0`).

**Version.** Stays `0.2.0`: the fixed package has never been published, so no consumer holds an
older `0.2.0`.

### Verification after the fix (2026-11-20)

All steps through `ports/tools/build-slot.sh` with `BUILD_LOCAL=1` (the SDK path needs the fetched
`.horizon/sdk`), `rm -f Cargo.lock` before every build, `CARGO_INCREMENTAL=0`, no `--target-dir`.

| step | command | evidence | result |
|---|---|---|---|
| tests | `cargo test --offline` | `evidence/modern_industrialization/sdk-test.log` | **27 passed**, 0 failed, exit 0 |
| lints | `cargo clippy --all-targets --offline -- -D warnings` | `sdk-clippy.log` | exit 0, 0 warnings |
| format | `rustfmt --edition 2024 --check src/*.rs` | `sdk-fmt.log` | exit 0 |
| build 1 + 2 | `cargo horizon-mod build`, `cmp` | `sdk-build.log`, `sdk-build-2.log` | **byte-identical**, `db21a20b…4873` |
| package | `unzip -l modern_industrialization.hmod` | — | **46,651 B**, SHA-256 `db21a20b6c16790ac097831a9d936b803d2c69849b357f7aa2d2105031dc4873`, **59 entries**, no packed `Cargo.lock` |
| package check | `cargo horizon-mod check modern_industrialization.hmod` | `sdk-check.log` | exit 0 |
| fits | `cargo horizon-mod fits modern_industrialization.hmod --build 0b14f8a` | `sdk-fits.log` | exit 0, *"0b14f8a made for"* |
| gate | `python3 ports/tools/verify-sdk-package.py modern_industrialization` | — | **1/1** |
| in-game (modded) | `launcher-game.sh build modern_industrialization`, then the leg | `evidence/modern_industrialization-e2e/e2e-launcher.log` | **PASS** + `EXIT:0`; A `slots(0,100,0) = 5`, G after relaunch `slots(0,100,0) = 5` |
| in-game (control) | `GAME=$CONTROL_GAME`, same command | `e2e-launcher-control.log` | **FAIL** + `EXIT:1` (`expected modern_industrialization:electric_macerator at 0,100,0, got 'minecraft:air'`) |
| fail-capability | `SELFTEST=1` | `e2e-launcher-selftest.log` | **FAIL** + `EXIT:1` (`SELFTEST flipped: the machine with no energy made nothing, as it should`) |
| logs gate | `python3 ports/tools/check-e2e-logs.py modern_industrialization-e2e --strict` | — | **4 ok, 0 missing-exit, 0 bad-exit** |

### Registry fit-gate repair (2026-11-20) — `uses.json` named a local type as an SDK type

The reviewer's finding (`levzzz5154`): the generated `uses.json` mapped the port's **local**
`Recipe` (`src/runtime.rs:89`) to the SDK's `type:Recipe`, so `fits` refused the package against
any build other than the one it was made for:

```
f00125f  does not fit: 1 item changed
    the type `Recipe` can no longer be built with a literal, and `src/runtime.rs:437` builds it
```

Root cause: `uses.json` is a **source-text scan** (`cargo horizon-mod check --uses`). It resolves
a name against the SDK surface by name, so the local `Recipe` literal at `RECIPES[0]` was recorded
as the SDK type with `"built": "full"`, the SDK type's field list and `built_at:
src/runtime.rs:437`. `fits` refuses a `built: full` entry when the target type is not
constructible (`constructible: false`; the SDK type is `#[non_exhaustive]`). The scan is
regenerated by every build, so rebuilding at the same pin produces the same false entry; the
honest fix is to rename the local type so the table is true. `uses.json` was not hand-edited.

Fix: `Recipe` → `MaceratorRecipe` in `src/runtime.rs` and in the generator
`tools/carry_recipes.py` (which emits the table); no row, number or behaviour changed. The scan
now records no `type:Recipe` at all (this package never names the SDK type), so the false
`built: full` entry is gone.

| step | command | evidence | result |
|---|---|---|---|
| tests | `cargo test --offline` | `sdk-test.log` | **27 passed**, 0 failed, exit 0 |
| lints | `cargo clippy --all-targets --offline -- -D warnings` | `sdk-clippy.log` | exit 0, 0 warnings |
| format | `cargo fmt --check` | `sdk-fmt.log` | exit 0 |
| build 1 + 2 | `cargo horizon-mod build` twice, `cmp` | — | **byte-identical** |
| package | `sha256sum; stat -c%s` | — | **46,145 B**, SHA-256 `e495ddae3a5d63d81555b209a85dd5e9617267566fa3015f35397da6922f8a14`, 59 entries, no packed `Cargo.lock` |
| package check | `cargo horizon-mod check modern_industrialization.hmod` | `sdk-check.log` | exit 0 |
| fits (pin) | `cargo horizon-mod fits modern_industrialization.hmod --build 0b14f8a` | `sdk-fits.log` | `0b14f8a  made for`, exit 0 |
| fits (the refusal build) | `cargo horizon-mod fits modern_industrialization.hmod --build f00125f` | `sdk-fits-next.log` | **`f00125f  fits`, exit 0** |
| gate | `python3 ports/tools/verify-sdk-package.py modern_industrialization` | — | **1/1** |
| in game (modded) | `launcher-game.sh run modern_industrialization` | `e2e-default-0.2.0.log` | **PASS** + `EXIT:0` |
| in game (control) | `GAME=$CONTROL_GAME`, same command | `e2e-control-nopackage.log` | **FAIL** + `EXIT:1` (unknown block) |
| fail-capability | `SELFTEST=1` | `e2e-selftest-0.2.0.log` | **FAIL** + `EXIT:1` |
| log gate | `python3 ports/tools/check-e2e-logs.py modern_industrialization --strict` | — | 6 ok, 0 missing-exit, 0 bad-exit |

`src/runtime.rs` is no longer byte-identical to the retired bundle (SHA-256 `c39edc7a…ed7a`):
only the type's name changed, not its fields, values or tests.

Skills: superpowers/using-superpowers,systematic-debugging,test-driven-development,verification-before-completion; BeautifulRust/rust-best-practices
