# modern_industrialization (Horizon port)

A Horizon port of [modern_industrialization](AztechMC/Modern-Industrialization) — MIT.

**Status: first slices.** What is below is what the port actually does today.

## Implemented so far

- `modern_industrialization:electric_macerator`, one block:
- a **32,000 FE buffer** (`native_block_entity_energy`, API 142; the int `energy`
- in the saved compound) that any face may fill and nothing may drain
- (upstream's `EnergyComponent.buildInsertable` is insert-only). The mod's own
- default is **1 EU = 10 FE**, so 3,200 EU = 32,000 FE;
- one item input slot** and **four item output slots** (`ModContainer`):
- hoppers insert into slot 0 and pull from slots 1..=4;
- `CrafterComponent`'s tick** in the pure runtime: a run starts when the buffer

## Not yet ported

- No crafting recipe** for the machine or the dusts (see above); the creative
- tab is the entry point.
- of the jar's 324 macerator recipes.** The other 290-odd need MI's own
- ingots, plates, gears, rods and wires, which this slice does not register;
- tag inputs (`c:ores/iron`…) were resolved to the vanilla items the tags hold,
- which is why three jar rows became four rows for the ore variants.
- No data-driven recipe type.** The pin offers no machine-recipe registry for
- a native mod, so the table is a `const` in `runtime.rs`; adding a recipe is a

## Install

Build the bundle with a native-mod capable launcher, or add the file from the
registry once it is listed. Pinned to Horizon `1d63e0a` and horizon-mixins
`5a64af93` for game 26.3.

## Build from source

```sh
cargo test
python3 tools/horizon.py check --horizon <horizon checkout>
python3 tools/horizon.py build --horizon <horizon checkout>
python3 tools/package.py
```

## Credits and licence

Original mod: AztechMC/Modern-Industrialization. Port: AdLf0208. Licence: MIT — see [LICENSE](LICENSE);
the original notices are preserved.
