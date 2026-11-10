// The pure core of Modern Industrialization slice 1: the electric macerator's
// energy buffer, recipe matching, tick and overclock.
//
// This exact file is the standalone `horizon-modern_industrialization` library
// and, injected by horizon-mixins, the `crate::game::modern_industrialization`
// module of the staged game. It has no game types: the block entity glue passes
// in the container's slots through [`Slots`] and the RNG through a closure, and
// reads back the stored FE, the progress and the efficiency ticks.
//
// The numbers are Modern Industrialization 2.5.12's own, read from the jar and
// from tag 2.5.12 of `AztechMC/Modern-Industrialization`:
//
// - `MachineTier.LV` is `maxEu 32`, `baseEu 8` and `getMaxStoredEu = maxEu * 100
//   = 3200 EU` (`machines/init/MachineTier.java`), and
//   `SingleBlockCraftingMachines.registerMachineTiers` builds the electric
//   macerator with `MachineTier.LV, 3200`, one item input and four item outputs.
// - `CrafterComponent` consumes `recipeMaxEu` EU a tick until
//   `usedEnergy == eu * duration`, takes the inputs when the recipe starts, and
//   puts the outputs when it ends; `getRecipeMaxEu` is
//   `min(totalEu, min(max(baseEu, recipeEu) + efficiency * totalEu / 600, maxEu))`
//   and one finished recipe adds an efficiency tick, a tick that used less than
//   the maximum removes one (`machines/components/CrafterComponent.java`).
// - The mod's own default conversion is **1 EU = 10 FE** (2.3.2 changelog: "Make
//   the ratio between EU and FE configurable. Default: 1 EU = 10 FE"), so this
//   port stores FE and converts at that ratio: 3200 EU = 32,000 FE, 8 EU/t = 80
//   FE/t, 32 EU/t = 320 FE/t.

/// The mod's own EU to FE ratio: 1 EU = 10 FE (MI 2.3.2's configurable default).
pub const FE_PER_EU: i32 = 10;

/// The LV tier's buffer: `MachineTier.LV.getMaxStoredEu()` (32 x 100 EU).
pub const CAPACITY_FE: i32 = 3_200 * FE_PER_EU;

/// The LV tier's base rate, EU a tick (`MachineTier.LV.getBaseEu()`).
pub const BASE_EU: i32 = 8;

/// The LV tier's ceiling, EU a tick (`MachineTier.LV.getMaxEu()`).
pub const MAX_EU: i32 = 32;

/// The divisor an overclock tick adds `totalEu / OVERCLOCK_DIVISOR` of
/// (`CrafterComponent.getRecipeMaxEu`: `20 * 30`).
pub const OVERCLOCK_DIVISOR: i64 = 20 * 30;

/// The electric macerator's item output slots (`registerMachineTiers(.., 1, 4, ..)`).
pub const OUTPUT_SLOTS: usize = 4;

/// A stack's capacity in a slot (`ConfigurableItemStack`'s default 64).
pub const STACK_SIZE: u32 = 64;

/// Probabilities are kept in ten-thousandths; this value is certainty.
pub const PROBABILITY_SCALE: u32 = 10_000;

/// One item requirement of a recipe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Input {
    /// The item id the machine matches on (`minecraft:stone`).
    pub item: &'static str,
    /// How many of it one recipe run takes.
    pub amount: u32,
}

/// One item result of a recipe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Output {
    /// The item id given.
    pub item: &'static str,
    /// How many are given when the output lands.
    pub amount: u32,
    /// The chance it lands, in ten-thousandths; [`PROBABILITY_SCALE`] is certain.
    pub permille: u32,
}

/// One macerator recipe, as the mod's datapack holds it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Recipe {
    /// A stable id, saved in the block entity's compound.
    pub id: &'static str,
    /// EU a tick, before the tier's base rate and the overclock (`eu`).
    pub eu: i32,
    /// Ticks at `eu` before the overclock (`duration`).
    pub duration: i32,
    /// What one run takes, at the start.
    pub inputs: &'static [Input],
    /// What one run gives, at the end.
    pub outputs: &'static [Output],
}

impl Recipe {
    /// The energy one run costs, `eu * duration`, overclock or not.
    pub fn total_eu(&self) -> i64 {
        i64::from(self.eu) * i64::from(self.duration)
    }
}

/// The slots a machine works on: the glue's container, or a test's bench.
///
/// The block entity keeps one input slot (`count`/`take`) and four output slots
/// (`room`/`put`), as upstream's `MachineInventoryComponent` does.
pub trait Slots {
    /// How many of `item` the input slot holds.
    fn count(&self, item: &str) -> u32;
    /// Takes up to `amount` of `item` from the input slot, answering how many.
    fn take(&mut self, item: &str, amount: u32) -> u32;
    /// How many of `amount` fit in the output slots, without changing them.
    fn room(&self, item: &str, amount: u32) -> u32;
    /// Puts `amount` into the output slots, answering how many did not fit.
    fn put(&mut self, item: &str, amount: u32) -> u32;
}

/// What the block entity saves, so a reload resumes the run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Saved {
    /// Stored FE.
    pub energy: i32,
    /// EU the active run has used.
    pub used_eu: i64,
    /// EU the active run costs in all.
    pub recipe_energy: i64,
    /// EU a tick the active run takes now.
    pub recipe_max_eu: i64,
    /// Overclock ticks earned.
    pub efficiency: i32,
    /// The most overclock ticks this machine can hold.
    pub max_efficiency: i32,
    /// The active recipe's id.
    pub active: Option<&'static str>,
}

/// What `/modern_industrialization info` prints.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Status {
    /// Stored FE.
    pub energy: i32,
    /// EU the active run has used.
    pub used_eu: i64,
    /// EU the active run costs in all.
    pub recipe_energy: i64,
    /// EU a tick the active run takes now.
    pub recipe_max_eu: i64,
    /// Overclock ticks earned.
    pub efficiency: i32,
    /// The most overclock ticks this machine can hold.
    pub max_efficiency: i32,
    /// The active recipe's id.
    pub active: Option<&'static str>,
    /// Progress through the active run, in thousandths.
    pub progress_permille: u16,
}

/// What one tick did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Tick {
    /// EU the tick consumed from the buffer.
    pub energy_used_eu: i64,
    /// The recipe that finished this tick, if one did.
    pub crafted: Option<&'static str>,
    /// Whether the machine ran this tick.
    pub active: bool,
}

/// The electric macerator's state: the buffer, the run in progress and the
/// overclock ticks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Macerator {
    /// Stored FE (the block entity's saved `energy`).
    pub energy: i32,
    /// EU the active run has used.
    pub used_eu: i64,
    /// EU the active run costs in all.
    pub recipe_energy: i64,
    /// EU a tick the active run takes now.
    pub recipe_max_eu: i64,
    /// Overclock ticks earned.
    pub efficiency: i32,
    /// The most overclock ticks this machine can hold.
    pub max_efficiency: i32,
    /// The active recipe's id; a warm machine stays locked to it.
    pub active: Option<&'static str>,
}

/// The recipe with this id, if the table holds it.
pub fn recipe(id: &str) -> Option<&'static Recipe> {
    RECIPES.iter().find(|recipe| recipe.id == id)
}

/// `CrafterComponent.getRecipeMaxEu`: the EU a tick, with the tier's base rate
/// and the overclock ticks, never above the recipe's total or the tier's cap.
pub fn recipe_max_eu(recipe_eu: i32, total_eu: i64, efficiency: i32) -> i64 {
    let base = i64::from(BASE_EU).max(i64::from(recipe_eu));
    let overclocked = base + i64::from(efficiency) * total_eu / OVERCLOCK_DIVISOR;
    total_eu.min(overclocked.min(i64::from(MAX_EU)))
}

/// `CrafterComponent.getRecipeMaxEfficiencyTicks`: the overclock ticks at which
/// a recipe reaches `min(maxEu, totalEu)`.
pub fn max_efficiency_ticks(recipe: &Recipe) -> i32 {
    let total = recipe.total_eu();
    let target = total.min(i64::from(MAX_EU));
    if recipe_max_eu(recipe.eu, total, 0) >= target {
        return 0;
    }
    let base = i64::from(BASE_EU).max(i64::from(recipe.eu));
    let product = (target - base) * OVERCLOCK_DIVISOR;
    let ticks = (product + total - 1) / total;
    i32::try_from(ticks).unwrap_or(i32::MAX)
}

/// Whether a run can start now: every input is in the slot and every certain
/// output has room (`CrafterComponent.canStartRecipe`, without the locks).
pub fn can_start(recipe: &Recipe, slots: &dyn Slots) -> bool {
    let inputs = combined(recipe.inputs.iter().map(|input| (input.item, input.amount)));
    if inputs
        .iter()
        .any(|(item, amount)| slots.count(item) < *amount)
    {
        return false;
    }
    let outputs = combined(
        recipe
            .outputs
            .iter()
            .filter(|output| output.permille >= PROBABILITY_SCALE)
            .map(|output| (output.item, output.amount)),
    );
    outputs
        .iter()
        .all(|(item, amount)| slots.room(item, *amount) >= *amount)
}

/// Adds requirements that name the same item (`amount` is a total, as upstream's
/// `remainingAmount` is).
fn combined<'a>(items: impl Iterator<Item = (&'a str, u32)>) -> Vec<(&'a str, u32)> {
    let mut needs: Vec<(&str, u32)> = Vec::new();
    for (item, amount) in items {
        match needs.iter_mut().find(|(name, _)| *name == item) {
            Some((_, total)) => *total += amount,
            None => needs.push((item, amount)),
        }
    }
    needs
}

/// Takes the inputs of a run (`CrafterComponent.takeItemInputs`).
fn take_inputs(recipe: &Recipe, slots: &mut dyn Slots) {
    for input in recipe.inputs {
        slots.take(input.item, input.amount);
    }
}

/// Gives the outputs of a finished run, rolling each chance with `roll`
/// (`CrafterComponent.putItemOutputs`). `roll` answers 0..[`PROBABILITY_SCALE`].
pub fn apply_outputs(recipe: &Recipe, slots: &mut dyn Slots, roll: &mut dyn FnMut() -> u32) {
    for output in recipe.outputs {
        if output.permille < PROBABILITY_SCALE && roll() >= output.permille {
            continue;
        }
        // Upstream inserts what fits and loses the rest (`putItemOutputs`
        // answers false and nothing retries); the start check keeps that
        // unreachable unless a player fills an output slot mid-run.
        let leftover = slots.put(output.item, output.amount);
        debug_assert_eq!(leftover, 0, "an output without room is lost");
    }
}

impl Macerator {
    /// The machine a saved compound describes; a recipe the table no longer
    /// holds resets the run, as upstream resets `usedEnergy` for a lost recipe.
    pub fn from_saved(saved: &Saved) -> Self {
        let active = saved.active.filter(|id| recipe(id).is_some());
        let (used_eu, recipe_energy, recipe_max_eu) = match active {
            Some(_) => (
                saved.used_eu.max(0),
                saved.recipe_energy.max(0),
                saved.recipe_max_eu.max(0),
            ),
            None => (0, 0, 0),
        };
        Self {
            energy: saved.energy.clamp(0, CAPACITY_FE),
            used_eu,
            recipe_energy,
            recipe_max_eu,
            efficiency: saved.efficiency.max(0),
            max_efficiency: saved.max_efficiency.max(0),
            active,
        }
    }

    /// What to save.
    pub fn saved(&self) -> Saved {
        Saved {
            energy: self.energy,
            used_eu: self.used_eu,
            recipe_energy: self.recipe_energy,
            recipe_max_eu: self.recipe_max_eu,
            efficiency: self.efficiency,
            max_efficiency: self.max_efficiency,
            active: self.active,
        }
    }

    /// Takes up to `amount` FE into the buffer, answering how many it took.
    pub fn insert_fe(&mut self, amount: i32) -> i32 {
        let taken = amount.clamp(0, CAPACITY_FE - self.energy);
        self.energy += taken;
        taken
    }

    /// What the info command prints.
    pub fn status(&self) -> Status {
        let progress = if self.recipe_energy > 0 {
            (self.used_eu * 1_000 / self.recipe_energy).clamp(0, 1_000)
        } else {
            0
        };
        Status {
            energy: self.energy,
            used_eu: self.used_eu,
            recipe_energy: self.recipe_energy,
            recipe_max_eu: self.recipe_max_eu,
            efficiency: self.efficiency,
            max_efficiency: self.max_efficiency,
            active: self.active,
            progress_permille: u16::try_from(progress).unwrap_or(0),
        }
    }

    /// One server tick (`CrafterComponent.tickRecipe`): start a run if the
    /// buffer holds at least 1 EU, feed it `recipe_max_eu` EU, finish it, and
    /// move the overclock ticks.
    pub fn tick(&mut self, slots: &mut dyn Slots, roll: &mut dyn FnMut() -> u32) -> Tick {
        let mut started = false;

        // START. A warm machine only restarts its own recipe, as upstream's
        // `getRecipes` answers the active recipe while efficiency ticks remain.
        // `active` is only None when the machine is cold, so a set efficiency
        // without a run (the harness control, and a state upstream cannot
        // reach) still searches the table.
        if self.used_eu == 0 && self.energy >= FE_PER_EU {
            let locked = if self.efficiency > 0 {
                self.active.and_then(recipe)
            } else {
                None
            };
            let candidate = locked.or_else(|| {
                RECIPES
                    .iter()
                    .find(|recipe| recipe.eu <= MAX_EU && can_start(recipe, slots))
            });
            if let Some(found) = candidate
                && can_start(found, slots)
            {
                if self.active != Some(found.id) || self.efficiency == 0 {
                    self.max_efficiency = max_efficiency_ticks(found);
                    // `CrafterComponent` clamps the earned ticks to the new
                    // recipe's own maximum when the tier or recipe changes.
                    self.efficiency = self.efficiency.min(self.max_efficiency);
                }
                take_inputs(found, slots);
                self.active = Some(found.id);
                self.used_eu = 0;
                self.recipe_energy = found.total_eu();
                self.recipe_max_eu = recipe_max_eu(found.eu, self.recipe_energy, self.efficiency);
                started = true;
            }
        }

        // PROCESS.
        let mut energy_used_eu = 0;
        let mut finished = false;
        let mut crafted = None;
        if let Some(found) = self.active.and_then(recipe) {
            self.recipe_max_eu = recipe_max_eu(found.eu, self.recipe_energy, self.efficiency);
            if self.used_eu > 0 || started {
                let needed_eu = (self.recipe_energy - self.used_eu)
                    .max(0)
                    .min(self.recipe_max_eu);
                let moved_eu = (needed_eu * i64::from(FE_PER_EU)).min(i64::from(self.energy))
                    / i64::from(FE_PER_EU);
                self.energy -= i32::try_from(moved_eu * i64::from(FE_PER_EU)).unwrap_or(0);
                self.used_eu += moved_eu;
                energy_used_eu = moved_eu;
                if self.recipe_energy > 0 && self.used_eu == self.recipe_energy {
                    apply_outputs(found, slots, roll);
                    self.used_eu = 0;
                    finished = true;
                    crafted = Some(found.id);
                }
            }
        }
        let active = energy_used_eu > 0;

        // EFFICIENCY.
        if finished {
            if self.efficiency < self.max_efficiency {
                self.efficiency += 1;
            }
        } else if energy_used_eu < self.recipe_max_eu && self.efficiency > 0 {
            self.efficiency -= 1;
        }
        if self.efficiency == 0 && self.used_eu == 0 {
            self.active = None;
        }

        Tick {
            energy_used_eu,
            crafted,
            active,
        }
    }
}

/// The macerator recipes this slice carries.
///
/// Generated from the pack jar by `tools/carry_recipes.py` (every `eu`,
/// `duration`, amount and probability is the jar's; each row's comment names its
/// file), restricted to the recipes whose inputs and outputs the pack already
/// has: the `vanilla_recipes/macerator/` set and the material recipes over
/// vanilla items. A `#c:` tag input became one row per item the tag holds in
/// this pack. The mod's other 290-odd macerator recipes need its own ingots,
/// plates, gears, rods and wires, which this slice does not register.
pub const RECIPES: &[Recipe] = &[
    Recipe {
        // vanilla_recipes/macerator/stone_to_cobblestone.json
        id: "modern_industrialization:stone_to_cobblestone",
        inputs: &[Input {
            item: "minecraft:stone",
            amount: 1,
        }],
        outputs: &[Output {
            item: "minecraft:cobblestone",
            amount: 1,
            permille: 10000,
        }],
        eu: 2,
        duration: 100,
    },
    Recipe {
        // vanilla_recipes/macerator/gravel.json
        id: "modern_industrialization:cobblestone_to_gravel",
        inputs: &[Input {
            item: "minecraft:cobblestone",
            amount: 1,
        }],
        outputs: &[Output {
            item: "minecraft:gravel",
            amount: 1,
            permille: 10000,
        }],
        eu: 2,
        duration: 100,
    },
    Recipe {
        // vanilla_recipes/macerator/sand.json
        id: "modern_industrialization:gravel_to_sand",
        inputs: &[Input {
            item: "minecraft:gravel",
            amount: 1,
        }],
        outputs: &[Output {
            item: "minecraft:sand",
            amount: 1,
            permille: 10000,
        }],
        eu: 2,
        duration: 100,
    },
    Recipe {
        // vanilla_recipes/macerator/sandstone_to_sand.json
        id: "modern_industrialization:sandstone_to_sand",
        inputs: &[Input {
            item: "minecraft:sandstone",
            amount: 1,
        }],
        outputs: &[Output {
            item: "minecraft:sand",
            amount: 4,
            permille: 10000,
        }],
        eu: 2,
        duration: 100,
    },
    Recipe {
        // vanilla_recipes/macerator/red_sandstone_to_sand.json
        id: "modern_industrialization:red_sandstone_to_red_sand",
        inputs: &[Input {
            item: "minecraft:red_sandstone",
            amount: 1,
        }],
        outputs: &[Output {
            item: "minecraft:red_sand",
            amount: 4,
            permille: 10000,
        }],
        eu: 2,
        duration: 100,
    },
    Recipe {
        // vanilla_recipes/macerator/cobbled_deepslate.json
        id: "modern_industrialization:deepslate_to_cobbled_deepslate",
        inputs: &[Input {
            item: "minecraft:deepslate",
            amount: 1,
        }],
        outputs: &[Output {
            item: "minecraft:cobbled_deepslate",
            amount: 1,
            permille: 10000,
        }],
        eu: 2,
        duration: 100,
    },
    Recipe {
        // vanilla_recipes/macerator/glowstone_dust.json
        id: "modern_industrialization:glowstone_to_glowstone_dust",
        inputs: &[Input {
            item: "minecraft:glowstone",
            amount: 1,
        }],
        outputs: &[Output {
            item: "minecraft:glowstone_dust",
            amount: 4,
            permille: 10000,
        }],
        eu: 2,
        duration: 100,
    },
    Recipe {
        // vanilla_recipes/macerator/bone_meal_from_block.json
        id: "modern_industrialization:bone_block_to_bone_meal",
        inputs: &[Input {
            item: "minecraft:bone_block",
            amount: 1,
        }],
        outputs: &[Output {
            item: "minecraft:bone_meal",
            amount: 9,
            permille: 10000,
        }],
        eu: 2,
        duration: 100,
    },
    Recipe {
        // vanilla_recipes/macerator/bone_meal_from_bone.json
        id: "modern_industrialization:bone_to_bone_meal",
        inputs: &[Input {
            item: "minecraft:bone",
            amount: 1,
        }],
        outputs: &[Output {
            item: "minecraft:bone_meal",
            amount: 6,
            permille: 10000,
        }],
        eu: 2,
        duration: 100,
    },
    Recipe {
        // vanilla_recipes/macerator/blaze_dust.json
        id: "modern_industrialization:blaze_rod_to_blaze_powder",
        inputs: &[Input {
            item: "minecraft:blaze_rod",
            amount: 1,
        }],
        outputs: &[Output {
            item: "minecraft:blaze_powder",
            amount: 4,
            permille: 10000,
        }],
        eu: 2,
        duration: 100,
    },
    Recipe {
        // vanilla_recipes/macerator/clay_ball.json
        id: "modern_industrialization:clay_to_clay_ball",
        inputs: &[Input {
            item: "minecraft:clay",
            amount: 1,
        }],
        outputs: &[Output {
            item: "minecraft:clay_ball",
            amount: 4,
            permille: 10000,
        }],
        eu: 2,
        duration: 100,
    },
    Recipe {
        // vanilla_recipes/macerator/sugar.json
        id: "modern_industrialization:sugar_cane_to_sugar",
        inputs: &[Input {
            item: "minecraft:sugar_cane",
            amount: 1,
        }],
        outputs: &[Output {
            item: "minecraft:sugar",
            amount: 3,
            permille: 10000,
        }],
        eu: 2,
        duration: 100,
    },
    Recipe {
        // vanilla_recipes/macerator/gilded_blackstone.json
        id: "modern_industrialization:gilded_blackstone_to_gold_nugget",
        inputs: &[Input {
            item: "minecraft:gilded_blackstone",
            amount: 1,
        }],
        outputs: &[Output {
            item: "minecraft:gold_nugget",
            amount: 8,
            permille: 10000,
        }],
        eu: 2,
        duration: 100,
    },
    Recipe {
        // vanilla_recipes/macerator/netherite_scrap.json
        id: "modern_industrialization:ancient_debris_to_netherite_scrap",
        inputs: &[Input {
            item: "minecraft:ancient_debris",
            amount: 1,
        }],
        outputs: &[Output {
            item: "minecraft:netherite_scrap",
            amount: 2,
            permille: 10000,
        }],
        eu: 2,
        duration: 800,
    },
    Recipe {
        // materials/macerator/brick_dust_from_block.json
        id: "modern_industrialization:bricks_to_brick_dust",
        inputs: &[Input {
            item: "minecraft:bricks",
            amount: 1,
        }],
        outputs: &[Output {
            item: "modern_industrialization:brick_dust",
            amount: 4,
            permille: 10000,
        }],
        eu: 2,
        duration: 200,
    },
    Recipe {
        // materials/macerator/copper_ore_to_raw.json (tag c:ores/copper)
        id: "modern_industrialization:copper_ore_to_raw_copper",
        inputs: &[Input {
            item: "minecraft:copper_ore",
            amount: 1,
        }],
        outputs: &[Output {
            item: "minecraft:raw_copper",
            amount: 8,
            permille: 10000,
        }],
        eu: 2,
        duration: 100,
    },
    Recipe {
        // materials/macerator/copper_ore_to_raw.json (tag c:ores/copper)
        id: "modern_industrialization:copper_ore_to_raw_copper_from_deepslate_copper_ore",
        inputs: &[Input {
            item: "minecraft:deepslate_copper_ore",
            amount: 1,
        }],
        outputs: &[Output {
            item: "minecraft:raw_copper",
            amount: 8,
            permille: 10000,
        }],
        eu: 2,
        duration: 100,
    },
    Recipe {
        // materials/coal/macerator/gem.json
        id: "modern_industrialization:coal_to_coal_dust",
        inputs: &[Input {
            item: "minecraft:coal",
            amount: 1,
        }],
        outputs: &[Output {
            item: "modern_industrialization:coal_dust",
            amount: 1,
            permille: 10000,
        }],
        eu: 2,
        duration: 100,
    },
    Recipe {
        // materials/brick/macerator/ingot.json
        id: "modern_industrialization:brick_to_brick_dust",
        inputs: &[Input {
            item: "minecraft:brick",
            amount: 1,
        }],
        outputs: &[Output {
            item: "modern_industrialization:brick_dust",
            amount: 1,
            permille: 10000,
        }],
        eu: 2,
        duration: 100,
    },
    Recipe {
        // materials/diamond/macerator/gem.json
        id: "modern_industrialization:diamond_to_diamond_dust",
        inputs: &[Input {
            item: "minecraft:diamond",
            amount: 1,
        }],
        outputs: &[Output {
            item: "modern_industrialization:diamond_dust",
            amount: 1,
            permille: 10000,
        }],
        eu: 2,
        duration: 800,
    },
    Recipe {
        // materials/emerald/macerator/gem.json
        id: "modern_industrialization:emerald_to_emerald_dust",
        inputs: &[Input {
            item: "minecraft:emerald",
            amount: 1,
        }],
        outputs: &[Output {
            item: "modern_industrialization:emerald_dust",
            amount: 1,
            permille: 10000,
        }],
        eu: 2,
        duration: 800,
    },
    Recipe {
        // materials/lapis/macerator/gem.json
        id: "modern_industrialization:lapis_lazuli_to_lapis_dust",
        inputs: &[Input {
            item: "minecraft:lapis_lazuli",
            amount: 1,
        }],
        outputs: &[Output {
            item: "modern_industrialization:lapis_dust",
            amount: 1,
            permille: 10000,
        }],
        eu: 2,
        duration: 100,
    },
    Recipe {
        // materials/quartz/macerator/gem.json
        id: "modern_industrialization:quartz_to_quartz_dust",
        inputs: &[Input {
            item: "minecraft:quartz",
            amount: 1,
        }],
        outputs: &[Output {
            item: "modern_industrialization:quartz_dust",
            amount: 1,
            permille: 10000,
        }],
        eu: 2,
        duration: 100,
    },
    Recipe {
        // materials/iron/macerator/ingot.json (tag c:ingots/iron)
        id: "modern_industrialization:iron_ingot_to_iron_dust",
        inputs: &[Input {
            item: "minecraft:iron_ingot",
            amount: 1,
        }],
        outputs: &[Output {
            item: "modern_industrialization:iron_dust",
            amount: 1,
            permille: 10000,
        }],
        eu: 2,
        duration: 200,
    },
    Recipe {
        // materials/copper/macerator/ingot.json (tag c:ingots/copper)
        id: "modern_industrialization:copper_ingot_to_copper_dust",
        inputs: &[Input {
            item: "minecraft:copper_ingot",
            amount: 1,
        }],
        outputs: &[Output {
            item: "modern_industrialization:copper_dust",
            amount: 1,
            permille: 10000,
        }],
        eu: 2,
        duration: 100,
    },
    Recipe {
        // materials/gold/macerator/ingot.json (tag c:ingots/gold)
        id: "modern_industrialization:gold_ingot_to_gold_dust",
        inputs: &[Input {
            item: "minecraft:gold_ingot",
            amount: 1,
        }],
        outputs: &[Output {
            item: "modern_industrialization:gold_dust",
            amount: 1,
            permille: 10000,
        }],
        eu: 2,
        duration: 100,
    },
    Recipe {
        // materials/iron/macerator/raw_metal.json (tag c:raw_materials/iron)
        id: "modern_industrialization:raw_iron_to_iron_dust",
        inputs: &[Input {
            item: "minecraft:raw_iron",
            amount: 1,
        }],
        outputs: &[
            Output {
                item: "modern_industrialization:iron_dust",
                amount: 1,
                permille: 10000,
            },
            Output {
                item: "modern_industrialization:iron_dust",
                amount: 1,
                permille: 5000,
            },
        ],
        eu: 2,
        duration: 100,
    },
    Recipe {
        // materials/copper/macerator/raw_metal.json (tag c:raw_materials/copper)
        id: "modern_industrialization:raw_copper_to_copper_dust",
        inputs: &[Input {
            item: "minecraft:raw_copper",
            amount: 1,
        }],
        outputs: &[
            Output {
                item: "modern_industrialization:copper_dust",
                amount: 1,
                permille: 10000,
            },
            Output {
                item: "modern_industrialization:copper_dust",
                amount: 1,
                permille: 5000,
            },
        ],
        eu: 2,
        duration: 50,
    },
    Recipe {
        // materials/gold/macerator/raw_metal.json (tag c:raw_materials/gold)
        id: "modern_industrialization:raw_gold_to_gold_dust",
        inputs: &[Input {
            item: "minecraft:raw_gold",
            amount: 1,
        }],
        outputs: &[
            Output {
                item: "modern_industrialization:gold_dust",
                amount: 1,
                permille: 10000,
            },
            Output {
                item: "modern_industrialization:gold_dust",
                amount: 1,
                permille: 5000,
            },
        ],
        eu: 2,
        duration: 50,
    },
    Recipe {
        // materials/iron/macerator/ore_to_raw.json (tag c:ores/iron)
        id: "modern_industrialization:iron_ore_to_raw_iron",
        inputs: &[Input {
            item: "minecraft:iron_ore",
            amount: 1,
        }],
        outputs: &[Output {
            item: "minecraft:raw_iron",
            amount: 3,
            permille: 10000,
        }],
        eu: 2,
        duration: 200,
    },
    Recipe {
        // materials/iron/macerator/ore_to_raw.json (tag c:ores/iron)
        id: "modern_industrialization:iron_ore_to_raw_iron_from_deepslate_iron_ore",
        inputs: &[Input {
            item: "minecraft:deepslate_iron_ore",
            amount: 1,
        }],
        outputs: &[Output {
            item: "minecraft:raw_iron",
            amount: 3,
            permille: 10000,
        }],
        eu: 2,
        duration: 200,
    },
    Recipe {
        // materials/gold/macerator/ore_to_raw.json (tag c:ores/gold)
        id: "modern_industrialization:gold_ore_to_raw_gold",
        inputs: &[Input {
            item: "minecraft:gold_ore",
            amount: 1,
        }],
        outputs: &[Output {
            item: "minecraft:raw_gold",
            amount: 3,
            permille: 10000,
        }],
        eu: 2,
        duration: 100,
    },
    Recipe {
        // materials/gold/macerator/ore_to_raw.json (tag c:ores/gold)
        id: "modern_industrialization:gold_ore_to_raw_gold_from_deepslate_gold_ore",
        inputs: &[Input {
            item: "minecraft:deepslate_gold_ore",
            amount: 1,
        }],
        outputs: &[Output {
            item: "minecraft:raw_gold",
            amount: 3,
            permille: 10000,
        }],
        eu: 2,
        duration: 100,
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    /// The container under test: one input slot holding stacks, four output
    /// slots of 64, as the block entity's `items` are laid out.
    struct Bench {
        input: Vec<(String, u32)>,
        outputs: [Option<(String, u32)>; OUTPUT_SLOTS],
    }

    impl Bench {
        fn new(input: &[(&'static str, u32)]) -> Self {
            Self {
                input: input
                    .iter()
                    .map(|(item, count)| ((*item).to_owned(), *count))
                    .collect(),
                outputs: std::array::from_fn(|_| None),
            }
        }

        fn held(&self, item: &str) -> u32 {
            self.input
                .iter()
                .filter(|(id, _)| id == item)
                .map(|(_, count)| *count)
                .sum()
        }

        fn out(&self, item: &str) -> u32 {
            self.outputs
                .iter()
                .flatten()
                .filter(|(id, _)| id == item)
                .map(|(_, count)| *count)
                .sum()
        }
    }

    impl Slots for Bench {
        fn count(&self, item: &str) -> u32 {
            self.held(item)
        }

        fn take(&mut self, item: &str, amount: u32) -> u32 {
            let mut left = amount;
            for (id, count) in &mut self.input {
                if id == item && *count > 0 {
                    let taken = (*count).min(left);
                    *count -= taken;
                    left -= taken;
                }
            }
            self.input.retain(|(_, count)| *count > 0);
            amount - left
        }

        fn room(&self, item: &str, amount: u32) -> u32 {
            let mut room = 0;
            for slot in &self.outputs {
                match slot {
                    Some((id, count)) if id == item => room += STACK_SIZE - count,
                    Some(_) => {}
                    None => room += STACK_SIZE,
                }
            }
            room.min(amount)
        }

        fn put(&mut self, item: &str, amount: u32) -> u32 {
            let mut left = amount;
            for slot in &mut self.outputs {
                match slot {
                    Some((id, count)) if id == item => {
                        let fits = (STACK_SIZE - *count).min(left);
                        *count += fits;
                        left -= fits;
                    }
                    Some(_) => {}
                    None => {
                        let fits = STACK_SIZE.min(left);
                        *slot = Some((item.to_owned(), fits));
                        left -= fits;
                    }
                }
                if left == 0 {
                    break;
                }
            }
            left
        }
    }

    /// A roll that always lands under any probability.
    fn always() -> u32 {
        0
    }

    /// A roll that never lands under a probability below certainty.
    fn never() -> u32 {
        PROBABILITY_SCALE - 1
    }

    #[test]
    fn a_fresh_macerator_is_empty_and_idle() {
        let machine = Macerator::default();
        assert_eq!(machine.energy, 0);
        assert_eq!(machine.active, None);
        assert_eq!(machine.efficiency, 0);
        assert_eq!(machine.status().progress_permille, 0);
    }

    #[test]
    fn the_buffer_is_the_lv_tier_capacity() {
        assert_eq!(CAPACITY_FE, 32_000, "3200 EU x 10 FE");
        assert_eq!(CAPACITY_FE / FE_PER_EU, 3_200);
        assert_eq!(BASE_EU, 8);
        assert_eq!(MAX_EU, 32);
        assert_eq!(
            OUTPUT_SLOTS, 4,
            "the electric macerator has four output slots"
        );
    }

    #[test]
    fn a_machine_never_exceeds_its_buffer() {
        let mut machine = Macerator::default();
        assert_eq!(machine.insert_fe(100_000), CAPACITY_FE);
        assert_eq!(machine.energy, CAPACITY_FE);
        assert_eq!(machine.insert_fe(1), 0, "a full buffer takes nothing");
    }

    #[test]
    fn a_recipe_needs_energy_before_it_starts() {
        let mut machine = Macerator::default();
        let mut bench = Bench::new(&[("minecraft:stone", 1)]);
        for _ in 0..40 {
            machine.tick(&mut bench, &mut always);
        }
        assert_eq!(machine.active, None, "no energy means no recipe");
        assert_eq!(bench.held("minecraft:stone"), 1, "the input is kept");
        assert_eq!(bench.out("minecraft:cobblestone"), 0);
        assert_eq!(machine.energy, 0);
    }

    #[test]
    fn the_first_tick_burns_the_tier_base_rate() {
        let mut machine = Macerator::default();
        machine.insert_fe(10_000);
        let mut bench = Bench::new(&[("minecraft:stone", 1)]);
        let tick = machine.tick(&mut bench, &mut always);
        assert_eq!(tick.energy_used_eu, i64::from(BASE_EU), "8 EU = 80 FE");
        assert_eq!(machine.energy, 10_000 - BASE_EU * FE_PER_EU);
        assert!(tick.active);
        assert_eq!(
            machine.active,
            Some("modern_industrialization:stone_to_cobblestone")
        );
    }

    #[test]
    fn the_recipe_takes_its_inputs_at_the_start() {
        let mut machine = Macerator::default();
        machine.insert_fe(10_000);
        let mut bench = Bench::new(&[("minecraft:stone", 2)]);
        machine.tick(&mut bench, &mut always);
        assert_eq!(bench.held("minecraft:stone"), 1, "one stone was taken");
    }

    #[test]
    fn one_stone_becomes_one_cobblestone_in_twenty_five_ticks() {
        let mut machine = Macerator::default();
        machine.insert_fe(10_000);
        let mut bench = Bench::new(&[("minecraft:stone", 1)]);
        let mut crafted = None;
        for tick in 1..=25 {
            let step = machine.tick(&mut bench, &mut always);
            if step.crafted.is_some() {
                crafted = Some(tick);
            }
        }
        assert_eq!(crafted, Some(25), "200 EU at 8 EU/t");
        assert_eq!(bench.out("minecraft:cobblestone"), 1);
        assert_eq!(bench.held("minecraft:stone"), 0);
        assert_eq!(
            machine.energy,
            10_000 - 200 * FE_PER_EU,
            "the recipe cost 200 EU = 2000 FE"
        );
    }

    #[test]
    fn a_recipe_stops_when_the_energy_runs_out() {
        let mut machine = Macerator::default();
        machine.insert_fe(1_000); // 100 EU: half of the recipe
        let mut bench = Bench::new(&[("minecraft:stone", 1)]);
        for _ in 0..60 {
            machine.tick(&mut bench, &mut always);
        }
        assert_eq!(machine.energy, 0);
        assert_eq!(machine.used_eu, 100);
        assert_eq!(bench.out("minecraft:cobblestone"), 0);
        assert_eq!(
            machine.active,
            Some("modern_industrialization:stone_to_cobblestone")
        );
        assert_eq!(machine.status().progress_permille, 500);
    }

    #[test]
    fn a_stalled_recipe_resumes_when_energy_returns() {
        let mut machine = Macerator::default();
        machine.insert_fe(1_000);
        let mut bench = Bench::new(&[("minecraft:stone", 1)]);
        for _ in 0..60 {
            machine.tick(&mut bench, &mut always);
        }
        machine.insert_fe(1_000);
        for _ in 0..60 {
            machine.tick(&mut bench, &mut always);
        }
        assert_eq!(bench.out("minecraft:cobblestone"), 1);
        assert_eq!(
            machine.energy, 0,
            "1000 + 1000 FE is exactly the 2000 FE cost"
        );
        assert_eq!(machine.used_eu, 0, "a finished recipe resets its counter");
    }

    #[test]
    fn a_longer_recipe_costs_more_energy() {
        let mut machine = Macerator::default();
        machine.insert_fe(20_000);
        let mut bench = Bench::new(&[("minecraft:iron_ore", 1)]);
        let mut crafted = None;
        for tick in 1..=60 {
            if machine.tick(&mut bench, &mut always).crafted.is_some() {
                crafted = Some(tick);
            }
        }
        assert_eq!(crafted, Some(50), "400 EU at 8 EU/t");
        assert_eq!(
            bench.out("minecraft:raw_iron"),
            3,
            "the jar gives three raw iron"
        );
        assert_eq!(machine.energy, 20_000 - 400 * FE_PER_EU);
    }

    #[test]
    fn a_recipe_keeps_its_output_count() {
        let mut machine = Macerator::default();
        machine.insert_fe(10_000);
        let mut bench = Bench::new(&[("minecraft:bricks", 1)]);
        for _ in 0..60 {
            machine.tick(&mut bench, &mut always);
        }
        assert_eq!(bench.out("modern_industrialization:brick_dust"), 4);
    }

    #[test]
    fn a_full_output_keeps_the_recipe_from_starting() {
        let mut machine = Macerator::default();
        machine.insert_fe(10_000);
        let mut bench = Bench::new(&[("minecraft:stone", 1)]);
        for slot in &mut bench.outputs {
            *slot = Some(("minecraft:basalt".to_owned(), STACK_SIZE));
        }
        for _ in 0..40 {
            machine.tick(&mut bench, &mut always);
        }
        assert_eq!(machine.active, None, "no room for the output");
        assert_eq!(bench.held("minecraft:stone"), 1, "the input is kept");
        assert_eq!(
            machine.energy, 10_000,
            "an unstartable recipe burns nothing"
        );
    }

    #[test]
    fn a_different_input_is_taken_once_the_machine_cools() {
        let mut machine = Macerator::default();
        machine.insert_fe(20_000);
        let mut bench = Bench::new(&[("minecraft:stone", 1)]);
        for _ in 0..25 {
            machine.tick(&mut bench, &mut always);
        }
        assert_eq!(
            machine.efficiency, 1,
            "one finished recipe warmed the machine"
        );
        bench.input.push(("minecraft:gravel".to_owned(), 1));
        // The warm machine is locked to stone until its efficiency reaches zero.
        let step = machine.tick(&mut bench, &mut always);
        assert_eq!(step.crafted, None);
        assert_eq!(machine.efficiency, 0, "a starved tick cools it");
        for _ in 0..30 {
            machine.tick(&mut bench, &mut always);
        }
        assert_eq!(bench.out("minecraft:sand"), 1);
    }

    #[test]
    fn completing_a_recipe_adds_an_efficiency_tick() {
        let mut machine = Macerator::default();
        machine.insert_fe(100_000);
        let mut bench = Bench::new(&[("minecraft:stone", 8)]);
        for _ in 0..25 {
            machine.tick(&mut bench, &mut always);
        }
        assert_eq!(machine.efficiency, 1);
        for _ in 0..25 {
            machine.tick(&mut bench, &mut always);
        }
        assert_eq!(machine.efficiency, 2, "the second recipe warmed it again");
    }

    #[test]
    fn a_starved_tick_cools_the_machine() {
        let mut machine = Macerator::default();
        machine.insert_fe(100_000);
        let mut bench = Bench::new(&[("minecraft:stone", 1)]);
        for _ in 0..25 {
            machine.tick(&mut bench, &mut always);
        }
        assert_eq!(machine.efficiency, 1);
        for _ in 0..3 {
            machine.tick(&mut bench, &mut always); // no input left
        }
        assert_eq!(machine.efficiency, 0);
        assert_eq!(machine.active, None);
    }

    #[test]
    fn the_maximum_efficiency_is_the_overclock_point() {
        assert_eq!(
            max_efficiency_ticks(recipe("modern_industrialization:stone_to_cobblestone").unwrap()),
            72
        );
        assert_eq!(
            max_efficiency_ticks(
                recipe("modern_industrialization:ancient_debris_to_netherite_scrap").unwrap()
            ),
            9,
            "1600 EU reaches 32 EU/t after nine ticks of efficiency"
        );
    }

    #[test]
    fn an_overclocked_recipe_takes_fewer_ticks() {
        let mut machine = Macerator::default();
        machine.insert_fe(10_000);
        machine.active = Some("modern_industrialization:stone_to_cobblestone");
        machine.efficiency = 72;
        machine.max_efficiency = 72;
        let mut bench = Bench::new(&[("minecraft:stone", 1)]);
        let mut ticks = 0;
        for tick in 1..=20 {
            if machine.tick(&mut bench, &mut always).crafted.is_some() {
                ticks = tick;
                break;
            }
        }
        assert_eq!(ticks, 7, "200 EU at the 32 EU/t cap");
        assert_eq!(
            machine.energy,
            10_000 - 200 * FE_PER_EU,
            "the total stays 200 EU"
        );
    }

    #[test]
    fn the_overclock_formula_matches_upstream() {
        // getRecipeMaxEu(eu=2, total=200, efficiency=3) = 8 + 3 * 200 / 600.
        assert_eq!(recipe_max_eu(2, 200, 0), 8);
        assert_eq!(recipe_max_eu(2, 200, 3), 9);
        assert_eq!(recipe_max_eu(2, 200, 72), 32);
        assert_eq!(
            recipe_max_eu(2, 200, 1_000),
            32,
            "capped at the tier maximum"
        );
        // A recipe shorter than the base rate never exceeds its own total.
        assert_eq!(recipe_max_eu(2, 100, 0), 8);
        assert_eq!(recipe_max_eu(2, 10, 0), 8);
        assert_eq!(
            recipe_max_eu(1, 3, 0),
            3,
            "a recipe shorter than the base rate"
        );
    }

    #[test]
    fn an_unknown_item_starts_nothing() {
        let mut machine = Macerator::default();
        machine.insert_fe(10_000);
        let mut bench = Bench::new(&[("minecraft:dirt", 1)]);
        for _ in 0..20 {
            machine.tick(&mut bench, &mut always);
        }
        assert_eq!(machine.active, None);
        assert_eq!(bench.held("minecraft:dirt"), 1);
    }

    #[test]
    fn a_probability_output_is_rolled() {
        let seed = Recipe {
            id: "test:chanced",
            eu: 2,
            duration: 100,
            inputs: &[Input {
                item: "minecraft:stone",
                amount: 1,
            }],
            outputs: &[
                Output {
                    item: "minecraft:cobblestone",
                    amount: 1,
                    permille: PROBABILITY_SCALE,
                },
                Output {
                    item: "minecraft:flint",
                    amount: 1,
                    permille: 300,
                },
            ],
        };
        let mut bench = Bench::new(&[]);
        apply_outputs(&seed, &mut bench, &mut always);
        assert_eq!(bench.out("minecraft:cobblestone"), 1);
        assert_eq!(
            bench.out("minecraft:flint"),
            1,
            "a roll of 0 lands under 0.3"
        );

        let mut bench = Bench::new(&[]);
        apply_outputs(&seed, &mut bench, &mut never);
        assert_eq!(bench.out("minecraft:cobblestone"), 1);
        assert_eq!(bench.out("minecraft:flint"), 0, "a roll of 9999 does not");
    }

    #[test]
    fn the_status_reports_progress() {
        let mut machine = Macerator::default();
        machine.insert_fe(10_000);
        let mut bench = Bench::new(&[("minecraft:stone", 1)]);
        for _ in 0..5 {
            machine.tick(&mut bench, &mut always);
        }
        let status = machine.status();
        assert_eq!(status.energy, 9_600);
        assert_eq!(status.progress_permille, 200, "40 of 200 EU");
        assert_eq!(
            status.active,
            Some("modern_industrialization:stone_to_cobblestone")
        );
        assert_eq!(status.recipe_max_eu, 8);
        assert_eq!(status.max_efficiency, 72);
    }

    #[test]
    fn a_set_efficiency_without_a_run_still_starts_a_recipe() {
        let mut machine = Macerator::default();
        machine.insert_fe(20_000);
        machine.efficiency = 72;
        let mut bench = Bench::new(&[("minecraft:ancient_debris", 1)]);
        let step = machine.tick(&mut bench, &mut always);
        assert!(step.active, "a warm machine with no run still starts one");
        assert_eq!(step.energy_used_eu, i64::from(MAX_EU), "the 32 EU/t cap");
        assert_eq!(
            machine.active,
            Some("modern_industrialization:ancient_debris_to_netherite_scrap")
        );
    }

    #[test]
    fn a_new_recipe_clamps_the_earned_efficiency() {
        let mut machine = Macerator::default();
        machine.insert_fe(20_000);
        machine.efficiency = 72;
        machine.max_efficiency = 72;
        let mut bench = Bench::new(&[("minecraft:ancient_debris", 1)]);
        machine.tick(&mut bench, &mut always);
        assert_eq!(
            machine.efficiency, 9,
            "1600 EU reaches 32 EU/t after nine efficiency ticks, so 72 clamps to 9"
        );
        assert_eq!(machine.max_efficiency, 9);
        assert_eq!(machine.recipe_max_eu, i64::from(MAX_EU), "still the cap");
    }

    #[test]
    fn a_saved_machine_reads_back() {
        let mut machine = Macerator::default();
        machine.insert_fe(10_000);
        let mut bench = Bench::new(&[("minecraft:stone", 1)]);
        for _ in 0..5 {
            machine.tick(&mut bench, &mut always);
        }
        let saved = machine.saved();
        let reloaded = Macerator::from_saved(&saved);
        assert_eq!(reloaded.energy, machine.energy);
        assert_eq!(reloaded.used_eu, machine.used_eu);
        assert_eq!(reloaded.active, machine.active);
        assert_eq!(reloaded.efficiency, machine.efficiency);
    }

    #[test]
    fn a_saved_recipe_that_is_gone_resets_the_progress() {
        let machine = Macerator::from_saved(&Saved {
            energy: 5_000,
            used_eu: 40,
            recipe_energy: 200,
            recipe_max_eu: 8,
            efficiency: 2,
            max_efficiency: 72,
            active: Some("modern_industrialization:removed_recipe"),
        });
        assert_eq!(machine.active, None);
        assert_eq!(machine.used_eu, 0, "an unknown recipe cannot be resumed");
        assert_eq!(machine.energy, 5_000, "the stored FE is kept");
    }

    #[test]
    fn every_recipe_is_well_formed() {
        let mut seen = Vec::new();
        for recipe in RECIPES {
            assert!(recipe.id.contains(':'), "{} is namespaced", recipe.id);
            for input in recipe.inputs {
                assert!(input.item.contains(':'), "{}: input namespaced", recipe.id);
                assert!(input.amount > 0, "{}: a positive input", recipe.id);
                assert!(
                    !seen.contains(&input.item),
                    "{} repeats the input {}",
                    recipe.id,
                    input.item
                );
                seen.push(input.item);
            }
            assert!(!recipe.outputs.is_empty(), "{} has an output", recipe.id);
            for output in recipe.outputs {
                assert!(
                    output.item.contains(':'),
                    "{}: output namespaced",
                    recipe.id
                );
                assert!(output.amount > 0, "{}: a positive output", recipe.id);
                assert!(
                    output.permille <= PROBABILITY_SCALE,
                    "{}: a probability at most one",
                    recipe.id
                );
            }
            assert_eq!(recipe.eu, 2, "{}: every carried recipe is LV", recipe.id);
            assert!(recipe.duration > 0, "{}: a positive duration", recipe.id);
            assert!(recipe.eu <= MAX_EU, "{}: an LV machine runs it", recipe.id);
        }
        assert!(RECIPES.len() >= 30, "the carried recipe set");
    }
}
