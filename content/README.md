# Content: `modern_industrialization.hmod`

Assembled from `Modern-Industrialization-2.5.12.jar` (MIT, SHA-1
`20be5747…d1a7`, the pack's own file; provenance in
`../../_shared/carry-work/modern_industrialization/SOURCE.md`). Rebuild with
`tools/package.sh` in the port root (it runs `cargo horizon-mod build` here and
copies the result into the `.hnmod`).

## Carried

| Files | Source |
|---|---|
| `assets/modern_industrialization/blockstates/electric_macerator.json` | the jar's blockstate, which is a single `""` variant (upstream's machine orientation is block-entity model data, not a block state) |
| `assets/modern_industrialization/models/block/electric_macerator.json` | **authored here**: upstream's is a custom model loader (`"loader": "modern_industrialization:machine"` + `"casing": "lv"` + `default_overlays`), which a content pack cannot run, so this is a plain model — a `basic_machine_hull` casing cube plus the macerator's front and top overlays as thin outer quads (`render_type: cutout`) |
| `assets/modern_industrialization/textures/block/basic_machine_hull.png`, `textures/block/machines/macerator/overlay_front.png`, `overlay_top.png` | the jar's own block textures |
| `assets/modern_industrialization/textures/item/{coal,iron,copper,gold,diamond,emerald,quartz,brick,lapis}_dust.png` | the jar's own item textures |
| `assets/modern_industrialization/models/item/*.json`, `items/*.json` | authored in the 26.3 shape (`{"model":{"type":"minecraft:model",...}}`); the dust models parent `minecraft:item/generated`, the block's item model parents the block model |
| `assets/modern_industrialization/lang/en_us.json` | the jar's key values for the block and the nine dusts |
| `data/modern_industrialization/horizon/block/electric_macerator.json` | the iron-block template with upstream's `destroyTime 4.0` and `requiresCorrectToolForDrops`, stone sounds, resistance 6.0 |
| `data/modern_industrialization/horizon/item/*_dust.json` | nine item declarations on the `minecraft:paper` template (stack 64, no components) |
| `data/modern_industrialization/horizon/creative_tab/machines.json` | the block and the nine dusts (upstream builds its tabs in code) |
| `data/modern_industrialization/loot_table/blocks/electric_macerator.json` | the jar's self-drop, rewritten in the 26.3 **singular typed** condition shape |
| `data/minecraft/tags/block/mineable/pickaxe.json`, `data/minecraft/tags/block/needs_stone_tool.json` | upstream's own tags for the machine block |

## Not carried, and why

- **The machine's crafting recipe** (`recipe/electric_age/machine/macerator_asbl.json`,
  and the assembler version): it needs `invar_rotary_blade`, `motor`, `piston`,
  `basic_machine_hull`, `tin_cable` and `analog_circuit`, none of which this slice
  registers — the recipe would be dropped at load. The block is in its creative
  tab instead.
- **The machine's GUI** (`assets/.../textures/gui/*`) and the `active` overlay
  variants: this slice has no menu and no `active` state.
- **The `active`/casing model variants** and the custom loader: see above.
- **The dusts' smelting/blasting/crafting uses**: out of the slice (they are
  recipes of later machine families).
