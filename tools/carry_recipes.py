#!/usr/bin/env python3
"""Print the Rust recipe table from the pack jar, so every number is the jar's.

The electric macerator's recipe type is `modern_industrialization:macerator`:
`eu` (EU/t), `duration` (ticks), `item_inputs` and `item_outputs` (with an
optional `probability`). This slice carries the recipes whose inputs and outputs
both exist in the pack without the mod's own materials (the `vanilla_recipes/`
set and the material recipes over vanilla items), so nothing is dropped when the
content loads. Tag inputs (`#c:ingots/iron`) are resolved to the vanilla items
the tag holds in this pack; the tag is recorded in the row's comment.

Usage: python3 tools/carry_recipes.py [--jar PATH]
"""
import argparse
import json
from pathlib import Path
import zipfile

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_JAR = ROOT.parent / "_shared/licence-cache/Modern-Industrialization-2.5.12.jar"
RECIPES = "data/modern_industrialization/recipe/"

# (jar path under recipe/, optional input item override for a tag, id)
SELECTED = [
    ("vanilla_recipes/macerator/stone_to_cobblestone.json", None, "stone_to_cobblestone"),
    ("vanilla_recipes/macerator/gravel.json", None, "cobblestone_to_gravel"),
    ("vanilla_recipes/macerator/sand.json", None, "gravel_to_sand"),
    ("vanilla_recipes/macerator/sandstone_to_sand.json", None, "sandstone_to_sand"),
    ("vanilla_recipes/macerator/red_sandstone_to_sand.json", None, "red_sandstone_to_red_sand"),
    ("vanilla_recipes/macerator/cobbled_deepslate.json", None, "deepslate_to_cobbled_deepslate"),
    ("vanilla_recipes/macerator/glowstone_dust.json", None, "glowstone_to_glowstone_dust"),
    ("vanilla_recipes/macerator/bone_meal_from_block.json", None, "bone_block_to_bone_meal"),
    ("vanilla_recipes/macerator/bone_meal_from_bone.json", None, "bone_to_bone_meal"),
    ("vanilla_recipes/macerator/blaze_dust.json", None, "blaze_rod_to_blaze_powder"),
    ("vanilla_recipes/macerator/clay_ball.json", None, "clay_to_clay_ball"),
    ("vanilla_recipes/macerator/sugar.json", None, "sugar_cane_to_sugar"),
    ("vanilla_recipes/macerator/gilded_blackstone.json", None, "gilded_blackstone_to_gold_nugget"),
    ("vanilla_recipes/macerator/netherite_scrap.json", None, "ancient_debris_to_netherite_scrap"),
    ("materials/macerator/brick_dust_from_block.json", None, "bricks_to_brick_dust"),
    ("materials/macerator/copper_ore_to_raw.json", "c:ores/copper",
     "copper_ore_to_raw_copper"),
    ("materials/coal/macerator/gem.json", None, "coal_to_coal_dust"),
    ("materials/brick/macerator/ingot.json", None, "brick_to_brick_dust"),
    ("materials/diamond/macerator/gem.json", None, "diamond_to_diamond_dust"),
    ("materials/emerald/macerator/gem.json", None, "emerald_to_emerald_dust"),
    ("materials/lapis/macerator/gem.json", None, "lapis_lazuli_to_lapis_dust"),
    ("materials/quartz/macerator/gem.json", None, "quartz_to_quartz_dust"),
    ("materials/iron/macerator/ingot.json", "c:ingots/iron", "iron_ingot_to_iron_dust"),
    ("materials/copper/macerator/ingot.json", "c:ingots/copper", "copper_ingot_to_copper_dust"),
    ("materials/gold/macerator/ingot.json", "c:ingots/gold", "gold_ingot_to_gold_dust"),
    ("materials/iron/macerator/raw_metal.json", "c:raw_materials/iron", "raw_iron_to_iron_dust"),
    ("materials/copper/macerator/raw_metal.json", "c:raw_materials/copper",
     "raw_copper_to_copper_dust"),
    ("materials/gold/macerator/raw_metal.json", "c:raw_materials/gold", "raw_gold_to_gold_dust"),
    ("materials/iron/macerator/ore_to_raw.json", "c:ores/iron", "iron_ore_to_raw_iron"),
    ("materials/gold/macerator/ore_to_raw.json", "c:ores/gold", "gold_ore_to_raw_gold"),
]

# The vanilla items a `#c:` tag holds in this pack. The tag is what the jar
# names; these are the concrete ids the port matches on (a second recipe per
# variant, as the tag would give the machine).
TAG_ITEMS = {
    "c:ores/copper": ["minecraft:copper_ore", "minecraft:deepslate_copper_ore"],
    "c:ores/iron": ["minecraft:iron_ore", "minecraft:deepslate_iron_ore"],
    "c:ores/gold": ["minecraft:gold_ore", "minecraft:deepslate_gold_ore"],
    "c:ingots/iron": ["minecraft:iron_ingot"],
    "c:ingots/copper": ["minecraft:copper_ingot"],
    "c:ingots/gold": ["minecraft:gold_ingot"],
    "c:raw_materials/iron": ["minecraft:raw_iron"],
    "c:raw_materials/copper": ["minecraft:raw_copper"],
    "c:raw_materials/gold": ["minecraft:raw_gold"],
}


def rows(jar):
    with zipfile.ZipFile(jar) as archive:
        for path, tag, recipe_id in SELECTED:
            data = json.loads(archive.read(RECIPES + path))
            assert data["type"] == "modern_industrialization:macerator", path
            inputs = data["item_inputs"]
            inputs = inputs if isinstance(inputs, list) else [inputs]
            outputs = data["item_outputs"]
            outputs = outputs if isinstance(outputs, list) else [outputs]
            items = [item for entry in outputs for item in _output(entry)]
            yield path, tag, recipe_id, data, inputs, outputs, items


def _output(entry):
    return [(entry["item"], entry["amount"], entry.get("probability", 1))]


def rust(jar):
    lines = []
    for path, tag, recipe_id, data, inputs, outputs, _ in rows(jar):
        assert len(inputs) == 1, path
        entry = inputs[0]
        if "item" in entry:
            variants = [(entry["item"], entry["amount"])]
        else:
            variants = [(item, entry["amount"]) for item in TAG_ITEMS[entry["tag"]]]
        outs = ", ".join(
            "Output { item: \"%s\", amount: %d, permille: %d }"
            % (entry["item"], entry["amount"], round(entry.get("probability", 1) * 10_000))
            for entry in outputs
        )
        for position, (item, amount) in enumerate(variants):
            lines.append(
                "    Recipe {\n"
                "        // %s\n"
                "        id: \"%s\",\n"
                "        inputs: &[Input { item: \"%s\", amount: %d }],\n"
                "        outputs: &[%s],\n"
                "        eu: %d,\n"
                "        duration: %d,\n"
                "    },"
                % (
                    path + (" (tag %s)" % tag if tag else ""),
                    "modern_industrialization:"
                    + (recipe_id if position == 0 else recipe_id + "_from_" + item.split(":")[1]),
                    item,
                    amount,
                    outs,
                    data["eu"],
                    data["duration"],
                )
            )
    return "\n".join(lines)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--jar", type=Path, default=DEFAULT_JAR)
    args = parser.parse_args()
    print(rust(args.jar))
    count = sum(1 for _ in rows(args.jar))
    print("# %d jar recipes selected" % count, file=__import__("sys").stderr)


if __name__ == "__main__":
    main()
