#!/usr/bin/env python3
"""Generate the energy hatch family's content files (slice 4).

Ten blocks: `<tier>_energy_<input|output>_hatch` for the five cable tiers
`MultiblockHatches.registerEnergyHatches(CableTier)` registers (LV, MV, HV, EV,
SUPERCONDUCTOR). Each block needs the same seven files the bronze hatches have —
blockstate, block model, item model, item definition, `horizon/block` declaration,
loot table and a lang key — plus its entry in the two block tags and the creative
tab. Writing them from one table keeps the ten identical except for the tier's
casing texture and the name, and `--check` proves the tree matches the table.

The block model is port-authored (upstream's `modern_industrialization:machine`
loader is a client mod feature): a casing cube with the jar's energy hatch side
overlay, and — for an output hatch — the jar's output overlay on the north face,
exactly as the bronze hatches are drawn. The textures are the jar's own (MIT).

    python3 tools/gen_energy_hatches.py            # write
    python3 tools/gen_energy_hatches.py --check    # exit 1 if anything differs
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

PORT = Path(__file__).resolve().parent.parent
ASSETS = PORT / "assets/modern_industrialization"
DATA = PORT / "data/modern_industrialization"
NS = "modern_industrialization"

# `CableTier.name`, the short English name in the block's own label, and the casing
# texture its model uses. Upstream generates the tier casings at runtime from the
# machine hull; the LV..EV casings are the basic machine hull's look and the
# superconductor tier's the quantum hull's, so those two files stand in for them.
TIERS = [
    ("lv", "LV", "basic_machine_hull"),
    ("mv", "MV", "basic_machine_hull"),
    ("hv", "HV", "basic_machine_hull"),
    ("ev", "EV", "basic_machine_hull"),
    ("superconductor", "SV", "quantum_machine_hull"),
]

CUBE = {
    "from": [0, 0, 0],
    "to": [16, 16, 16],
    "faces": {
        side: {"uv": [0, 0, 16, 16], "texture": "#casing", "cullface": side}
        for side in ("north", "east", "south", "west", "up", "down")
    },
}

SIDE_OVERLAY = {
    "name": "side_overlay",
    "from": [-0.06, -0.02, -0.06],
    "to": [16.06, 16.02, 16.06],
    "faces": {
        side: {"uv": [0, 0, 16, 16], "texture": "#side"}
        for side in ("north", "south", "east", "west")
    },
}

OUTPUT_OVERLAY = {
    "name": "output_overlay",
    "from": [0, 0, -0.12],
    "to": [16, 16, -0.06],
    "faces": {"north": {"uv": [0, 0, 16, 16], "texture": "#output"}},
}


def dump(value: object) -> str:
    return json.dumps(value, indent=2) + "\n"


def block_name(tier: str, short: str, input_: bool) -> str:
    return f"{tier}_energy_{'input' if input_ else 'output'}_hatch"


def label(short: str, input_: bool) -> str:
    return f"{short} Energy {'Input' if input_ else 'Output'} Hatch"


def model(name: str, casing: str, input_: bool) -> dict:
    textures = {
        "particle": f"{NS}:block/{casing}",
        "casing": f"{NS}:block/{casing}",
        "side": f"{NS}:block/machines/hatch_energy/overlay_side",
    }
    elements = [dict(CUBE), SIDE_OVERLAY]
    if not input_:
        textures["output"] = f"{NS}:block/overlays/output_energy"
        elements.append(OUTPUT_OVERLAY)
    return {"textures": textures, "render_type": "minecraft:cutout", "elements": elements}


def per_block_files() -> dict[Path, str]:
    files: dict[Path, str] = {}
    for tier, short, casing in TIERS:
        for input_ in (True, False):
            name = block_name(tier, short, input_)
            files[ASSETS / f"blockstates/{name}.json"] = dump(
                {"variants": {"": {"model": f"{NS}:block/{name}"}}}
            )
            files[ASSETS / f"models/block/{name}.json"] = dump(model(name, casing, input_))
            files[ASSETS / f"models/item/{name}.json"] = dump({"parent": f"{NS}:block/{name}"})
            files[ASSETS / f"items/{name}.json"] = dump(
                {"model": {"type": "minecraft:model", "model": f"{NS}:block/{name}"}}
            )
            # The bronze hatches' iron-block template: hardness 4.0, the right tool,
            # stone sounds, and an item of its own.
            files[DATA / f"horizon/block/{name}.json"] = dump(
                {
                    "template": "minecraft:iron_block",
                    "properties": {
                        "hardness": 4.0,
                        "explosion_resistance": 6.0,
                        "requires_tool": True,
                        "sounds": "minecraft:stone",
                    },
                    "item": True,
                }
            )
            files[DATA / f"loot_table/blocks/{name}.json"] = dump(
                {
                    "type": "minecraft:block",
                    "pools": [
                        {
                            "rolls": 1.0,
                            "bonus_rolls": 0.0,
                            "entries": [{"type": "minecraft:item", "name": f"{NS}:{name}"}],
                            "condition": {"type": "minecraft:survives_explosion"},
                        }
                    ],
                    "random_sequence": f"{NS}:blocks/{name}",
                }
            )
    return files


def merge_lists() -> dict[Path, str]:
    """The shared files that gain the ten ids: the two block tags, the lang file
    and the creative tab. Existing entries keep their order; the energy hatches
    follow the bronze ones in tier order."""
    ids = [
        f"{NS}:{block_name(tier, short, input_)}"
        for tier, short, _ in TIERS
        for input_ in (True, False)
    ]
    files: dict[Path, str] = {}
    for tag in ("mineable/pickaxe", "needs_stone_tool"):
        path = PORT / f"data/minecraft/tags/block/{tag}.json"
        data = json.loads(path.read_text())
        data["values"] = list(data["values"]) + [i for i in ids if i not in data["values"]]
        files[path] = dump(data)
    tab = DATA / "horizon/creative_tab/machines.json"
    data = json.loads(tab.read_text())
    data["items"] = list(data["items"]) + [i for i in ids if i not in data["items"]]
    files[tab] = dump(data)
    lang = ASSETS / "lang/en_us.json"
    data = json.loads(lang.read_text())
    for tier, short, _ in TIERS:
        for input_ in (True, False):
            name = block_name(tier, short, input_)
            data[f"block.{NS}.{name}"] = label(short, input_)
    files[lang] = dump(dict(sorted(data.items())))
    return files


def main() -> int:
    check = "--check" in sys.argv[1:]
    files = {**per_block_files(), **merge_lists()}
    dirty = []
    for path, text in sorted(files.items()):
        current = path.read_text() if path.exists() else None
        if current == text:
            continue
        dirty.append(path)
        if not check:
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(text)
    if check:
        for path in dirty:
            print(f"differs: {path.relative_to(PORT)}", file=sys.stderr)
        print(f"{len(files) - len(dirty)}/{len(files)} files match", file=sys.stderr)
        return 1 if dirty else 0
    print(f"wrote {len(dirty)} of {len(files)} files")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
