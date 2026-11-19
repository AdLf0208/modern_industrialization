# FINDINGS — Modern Industrialization 2.5.12, the electric macerator family

Slice 1 of `modern_industrialization` (class `content`, tier `deep`, core, P0;
MIT, rank 0).

- **Pack jar:** `Modern-Industrialization-2.5.12.jar`, SHA-1
  `20be5747c0c7f175f691f77de6df055be86d1a7a` = the ATM10 `modrinthHash`
  (`ports/_shared/licence-cache/atm10-modlist.json`), SHA-256
  `56f59a5a4481c98d44fa1c582e577b7324e43788227cab5ba66b5b17eeab5262`, kept at
  `ports/_shared/licence-cache/` (evidence; provenance in
  `ports/_shared/carry-work/modern_industrialization/SOURCE.md`).
- **Licence:** read from the jar's `META-INF/neoforge.mods.toml`
  (`license="MIT"`) and its bundled `LICENSE` (MIT, "Copyright (c) 2020 Azercoco
  & Technici4n"). **MIT → rank 0 (permissive)**; local-only until the maintainer
  approves publishing (ARCHITECTURE.md §8).
- **Upstream analysis:** tag `2.5.12`, commit
  `a02b76f48db04f090ccee9f1c99bd87f5ef58b57` of
  `https://github.com/AztechMC/Modern-Industrialization` (shallow clone, deleted
  after these findings). Analysis only: the Rust is a clean-room rewrite.

## Jar inventory (what is actually in it)

9,256 entries; 3,951 recipe JSONs under `data/modern_industrialization/recipe/`.

| Kind | Count | Notes |
|---|---|---|
| recipe files | 3,951 | `materials` 2,053, `dyes` 433, `vanilla_recipes` 330, `compat` 322, `electric_age` 182, `assembler_generated` 173, `hatches` 142, `petrochem` 50, `cutting_machine` 45, `alloy` 41, `steam_age` 38, `oil` 33, `upgrade` 32, `packer` 23, `quarry` 15, `heat_exchanger` 13, `tools` 8, `armor` 5, `fusion_reactor` 5, `other` 3 |
| recipe types (`"type"`) | 24 machine types | macerator **324**, mixer 407, assembler 331, packer 295, unpacker 192, forge_hammer 181, chemical_reactor 146, cutting_machine 113, compressor 92, blast_furnace 40, distillery 24, centrifuge 21, heat_exchanger 20, wiremill 13, vacuum_freezer 13, electrolyzer 12, implosion_compressor 10, pressurizer 9, quarry 8, distillation_tower 5, fusion_reactor 4, polarizer 3, oil_drilling_rig 3, coke_oven 2 — plus 611 shaped / 201 shapeless / 56 smelting / 52 blasting vanilla recipes |
| item models | 848 | 554 item textures, 139 block textures, 9,256 model/blockstate files in all |
| blockstates | 307 | the electric machines are a single `""` variant (orientation is block-entity data, not a block state) |
| loot tables | 222 | one per block, `copy_components` on the machines |
| `data/c/tags` | 483 | the common tags the recipes use (`c:ingots/*`, `c:ores/*`, `c:plates/*`…) |

**Machine families present** (block ids from `models/item/`): the bronze/steel
steam machines (`bronze_macerator`, `steel_macerator`, `bronze_furnace`,
`bronze_compressor`, `bronze_cutting_machine`, `bronze_mixer`, `bronze_water_pump`,
`bronze_boiler`), the electric single blocks (`electric_macerator`,
`electric_furnace`, `electric_compressor`, `electric_cutting_machine`,
`electric_mixer`, `electric_packer`, `electric_unpacker`, `electric_wiremill`,
`electric_polarizer`, `electric_centrifuge`, `electric_electrolyzer`,
`electric_distillery`, `electric_chemical_reactor`, `electric_water_pump`,
`electric_blast_furnace`, `electric_quarry`), the multiblocks (coke oven, blast
furnace, distillation tower, heat exchanger, vacuum freezer, implosion
compressor, fusion reactor…), generators (steam turbine, diesel generator, solar
boiler, nuclear reactor family), pipes/cables/hatches/storage units, and the
material part families (ingots, plates, gears, rods, wires, dusts…).

## The slice: the electric macerator

`modern_industrialization:electric_macerator` is the smallest machine the pin can
express **end to end**: one block entity, one item input, four item outputs, an
FE buffer, a recipe-driven tick, and a real inert/powered/overclocked behaviour
with numbers a game can assert. It is also the pack's first ore-processing step
(ore → 3 raw metal, ingot → dust), so a later slice has an obvious recipe target.

### Behaviour read from the source (tag 2.5.12)

| Where | What it says |
|---|---|
| `machines/init/MachineTier.java` | `LV("lv", false, maxEu=32, baseEu=8)`; `getMaxStoredEu() = maxEu * 100` = **3200 EU** |
| `machines/init/SingleBlockCraftingMachines.java` | the macerator is registered `(itemInputCount 1, itemOutputCount 4, fluidInput/Output 0, TIER_BRONZE|STEEL|ELECTRIC, ioBucketCapacity 16)`; the electric one is built with `MachineTier.LV, 3200`, four output slots |
| `machines/init/MIMachineRecipeTypes.java` | `MACERATOR = create("macerator").withItemInputs().withItemOutputs()` |
| `machines/recipe/MachineRecipe.java` | a recipe is `eu` (EU/t), `duration` (ticks), `item_inputs`, `fluid_inputs`, `item_outputs`, `fluid_outputs`, `process_conditions`; `getTotalEu() = eu * duration`; an output may carry a `probability` |
| `machines/components/CrafterComponent.java` | `tickRecipe`: start when `usedEnergy == 0` and at least 1 EU is stored (`consumeEu(1, SIMULATE) == 1`); a run takes its **inputs at the start** and puts its **outputs at the end**; it consumes `min(recipeMaxEu, recipeEnergy - usedEnergy)` EU a tick and is active only when it consumed something; `recipeEnergy = eu * duration`; a finished run adds one efficiency tick, a tick that consumed less than `recipeMaxEu` removes one; the active recipe is cleared only when `efficiency == 0 && usedEnergy == 0` |
| `CrafterComponent.getRecipeMaxEu` | `min(totalEu, min(max(baseEu, recipeEu) + efficiency * totalEu / 600, maxEu))` — 600 = `20 * 30` |
| `CrafterComponent.getRecipeMaxEfficiencyTicks` | the first efficiency at which the above reaches `min(maxEu, totalEu)` |
| `CrafterComponent.getRecipes` | while `efficiency > 0` only the **active** recipe is considered, so a warm machine stays locked to it |
| `CrafterComponent.putItemOutputs` | inserts into a matching or empty output slot, rolls `probability` per output, and loses what does not fit; output space is **not** checked for a probabilistic output when starting |
| `CrafterComponent.takeItemInputs` | takes `amount` from the input slots, across stacks |
| `machines/blockentities/ElectricCraftingMachineBlockEntity.java` | `consumeEu`/`insertEu` on an `EnergyComponent`; `buildInsertable` is **insert-only** (`canExtract() = false`) |
| `machines/components/EnergyComponent.java` | `insertEu(max)` takes `min(max, capacity - stored)` — no per-call cap; `consumeEu` takes what is stored |
| `machines/init/MachineRegistrationHelper.java` | machine block `Properties.of().mapColor(METAL).destroyTime(4.0f).requiresCorrectToolForDrops()`, tags `mineable/pickaxe` + `needs_stone_tool`, no mob spawns, not a redstone conductor |
| CHANGELOG (2.3.2) | "Make the ratio between EU and FE configurable. Default: **1 EU = 10 FE**" |

### Derived numbers this port pins

| Quantity | Value | How |
|---|---|---|
| buffer | 32,000 FE | 3,200 EU × 10 |
| base rate | 80 FE/t | `max(baseEu 8, eu 2)` = 8 EU/t × 10 |
| ceiling | 320 FE/t | `maxEu 32` × 10 |
| a stone (2 EU/t, 100 ticks) | 200 EU = **2,000 FE**, 25 ticks | 200 EU at 8 EU/t |
| an iron ore (2 EU/t, 200 ticks) | 400 EU = **4,000 FE**, 50 ticks | 400 EU at 8 EU/t |
| a diamond (2 EU/t, 800 ticks) | 1,600 EU = **16,000 FE**, 200 ticks | 1,600 EU at 8 EU/t |
| overclock for a 200-EU recipe | 72 efficiency ticks → 32 EU/t | `8 + 72·200/600 = 32`; at 71 it is 31 |
| overclock for a 1,600-EU recipe | 9 efficiency ticks | `8 + 9·1600/600 = 32`; at 8 it is 29 |

## The recipe table carried in code

`tools/carry_recipes.py` prints `RECIPES` from the jar, so every `eu`, `duration`,
amount and probability is the jar's. The table carries the **30 recipes whose
inputs and outputs the pack already has** — the `vanilla_recipes/macerator/` set
(stone→cobblestone, cobblestone→gravel, gravel→sand, sandstones→sand,
deepslate→cobbled_deepslate, glowstone→4 glowstone dust, bone block→9 bone meal,
bone→6 bone meal, blaze rod→4 blaze powder, clay→4 clay balls, sugar cane→3
sugar, gilded blackstone→8 gold nuggets, ancient debris→2 netherite scrap) and
the material recipes over vanilla items (coal/diamond/emerald/lapis/quartz/brick
→ dust, ingots → dust, raw metals → 2 dust, ores → raw metal / 8 raw copper,
bricks → 4 brick dust). That is 33 rows because three tag inputs (`c:ores/iron`,
`c:ores/gold`, `c:ores/copper`) became one row per vanilla ore variant.

Every carried row is `eu = 2`, so the machine's base rate (8 EU/t) governs them
all; the durations are 50, 100, 200 or 800 ticks. No carried recipe has a
probability output — the field and its roll are implemented and unit-tested
against a synthetic recipe, and the jar's `materials/macerator/lead_crushed_to_dust`
is the real-world case that would use it.

## Where the rewrite differs from upstream

| Upstream | This port | Why |
|---|---|---|
| EU, with a 1 EU = 10 FE bridge and a `MIEnergyStorage` capability | FE directly in the block's `Capabilities.Energy.BLOCK` store (API 142), at the same 10:1 ratio | the pin's energy capability is FE; the numbers are preserved |
| a custom recipe registry with 324 macerator recipes over tags and its own materials | 33 recipes in the pure runtime table, tagged inputs resolved to the vanilla items the tag holds | the pin has no data-driven machine-recipe type for a content/native port; the register is recorded as a gap |
| an overclock driven by machine upgrades, overdrive modules and steam catalysts | the LV tier's efficiency-tick overclock only | upgrades are a later slice |
| `putItemOutputs` machine-locks an output slot for the active run | the run re-checks the output space from the recipe at the end | no per-slot lock API for a mod container at this pin; the same start-time check keeps it from starting without room |
| a custom GUI with a progress bar, an energy bar, an efficiency bar, a slot panel, side configuration and auto-extract | no menu; the container is the automation path and `/modern_industrialization info` is the readout | `ModContainer` has no per-slot `may_place` for a player's menu at this pin, so a hopper menu would let a player fill the output slots |
| a machine model drawn by the `modern_industrialization:machine` model loader (casing + overlays composited, `active` state) | a plain model: the `basic_machine_hull` casing and the macerator's front/top overlays as thin outer quads, no `active` state | a content pack cannot run a custom model loader, and the machine has no orientation/active block state to switch |
| breaking the machine drops it with its contents in components | a self-drop loot table; the buffer and inventory are lost | the pin has no block-item component carry for a mod block entity |
