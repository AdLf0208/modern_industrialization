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

/// The slice's one block id (`SingleBlockCraftingMachines`, TIER_ELECTRIC). The
/// hook helpers compare a block entity's own `id` with it, so a command or a
/// tick never reads or writes this machine's keys in another mod's entity.
pub const MACERATOR_ID: &str = "modern_industrialization:electric_macerator";

/// Whether a block entity is this slice's machine. `/modern_industrialization
/// energy` and `… efficiency` accept a position, not a block, so they must
/// check the entity at it before writing macerator state; the tick and the
/// template hooks already key on the block's id.
#[must_use]
pub fn is_macerator(entity_id: &str) -> bool {
    entity_id == MACERATOR_ID
}

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
pub struct MachineRecipe {
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

impl MachineRecipe {
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
pub fn recipe(id: &str) -> Option<&'static MachineRecipe> {
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
pub fn max_efficiency_ticks(recipe: &MachineRecipe) -> i32 {
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
pub fn can_start(recipe: &MachineRecipe, slots: &dyn Slots) -> bool {
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
fn take_inputs(recipe: &MachineRecipe, slots: &mut dyn Slots) {
    for input in recipe.inputs {
        slots.take(input.item, input.amount);
    }
}

/// Gives the outputs of a finished run, rolling each chance with `roll`
/// (`CrafterComponent.putItemOutputs`). `roll` answers 0..[`PROBABILITY_SCALE`].
pub fn apply_outputs(recipe: &MachineRecipe, slots: &mut dyn Slots, roll: &mut dyn FnMut() -> u32) {
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
pub const RECIPES: &[MachineRecipe] = &[
    MachineRecipe {
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
    MachineRecipe {
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
    MachineRecipe {
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
    MachineRecipe {
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
    MachineRecipe {
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
    MachineRecipe {
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
    MachineRecipe {
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
    MachineRecipe {
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
    MachineRecipe {
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
    MachineRecipe {
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
    MachineRecipe {
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
    MachineRecipe {
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
    MachineRecipe {
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
    MachineRecipe {
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
    MachineRecipe {
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
    MachineRecipe {
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
    MachineRecipe {
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
    MachineRecipe {
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
    MachineRecipe {
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
    MachineRecipe {
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
    MachineRecipe {
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
    MachineRecipe {
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
    MachineRecipe {
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
    MachineRecipe {
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
    MachineRecipe {
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
    MachineRecipe {
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
    MachineRecipe {
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
    MachineRecipe {
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
    MachineRecipe {
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
    MachineRecipe {
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
    MachineRecipe {
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
    MachineRecipe {
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
    MachineRecipe {
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

// ---------------------------------------------------------------------------
// Slice 2: the coke oven multiblock
// ---------------------------------------------------------------------------

/// The controller's block id: upstream's `MultiblockMachines.cokeOven`.
pub const COKE_OVEN_ID: &str = "modern_industrialization:coke_oven";

/// The block the shell must be: upstream's `SimpleMember.forBlock(() -> Blocks.BRICKS)`.
/// The coke oven is the one multiblock whose casing is vanilla's brick, which is
/// why its shell is not one of the mod's own machine blocks.
pub const COKE_OVEN_BRICK: &str = "minecraft:bricks";

/// The hollow interior of the middle layer. Upstream's `add3by3(hollow = true)`
/// skips `(0, 0, 1)` and `ShapeTemplate.build()` removes the controller's own
/// `(0, 0, 0)`, so neither position is part of the shape and neither is checked:
/// air, stone or a machine there is the player's business.
pub const COKE_OVEN_INTERIOR: [i32; 3] = [0, 0, 1];

/// The bronze item input hatch: one input slot
/// (`MultiblockHatches.BRONZE_ITEM`, `registerItemHatches(…, rows 1, columns 1,
/// …)`).
pub const BRONZE_ITEM_INPUT_HATCH: &str = "modern_industrialization:bronze_item_input_hatch";

/// The bronze item output hatch: one output slot.
pub const BRONZE_ITEM_OUTPUT_HATCH: &str = "modern_industrialization:bronze_item_output_hatch";

/// The bronze fluid input hatch: one 4,000 mB tank (`registerFluidHatches(…, 4)`).
pub const BRONZE_FLUID_INPUT_HATCH: &str = "modern_industrialization:bronze_fluid_input_hatch";

/// The bronze fluid output hatch: one 4,000 mB tank.
pub const BRONZE_FLUID_OUTPUT_HATCH: &str = "modern_industrialization:bronze_fluid_output_hatch";

/// The bronze fluid hatch's tank, in millibuckets: upstream's
/// `registerFluidHatches("Bronze", "bronze", …, 4)` is four buckets.
pub const BRONZE_FLUID_HATCH_CAPACITY: i32 = 4 * 1_000;

/// The bricked bronze casing block of the add-on's pyrolyse oven shell
/// (`MIMaterials`' `MACHINE_CASING_SPECIAL.of("Bronze Plated Bricks", …)`).
pub const BRONZE_PLATED_BRICKS: &str = "modern_industrialization:bronze_plated_bricks";

/// The cupronickel coil block (`MIParts.COIL` for cupronickel; the add-on's
/// `pyrolyse_oven_tier` data map keys batch 8 on it).
pub const CUPRONICKEL_COIL: &str = "modern_industrialization:cupronickel_coil";

/// The kanthal coil block (batch 32 in the add-on's data map).
pub const KANTHAL_COIL: &str = "modern_industrialization:kanthal_coil";

/// The hatch kinds the coke oven's end-layer positions accept: upstream's
/// `HatchFlags.Builder().with(ITEM_INPUT).with(ITEM_OUTPUT).with(FLUID_INPUT)
/// .with(FLUID_OUTPUT).build()` plus the **energy family** this slice adds.
///
/// Upstream's coke oven is a steam machine and its own `HatchFlags` name only the
/// four item/fluid kinds; a position that accepts `ENERGY_INPUT` is an *electric*
/// multiblock's (`AbstractElectricCraftingMultiblockBlockEntity`'s
/// `appendEnergyInputs`). This port has no electric multiblock yet, so the coke
/// oven carries the energy hatch's aggregation — the one controller this slice
/// owns — and `notes.md` records that as a deviation: a player can build a coke
/// oven shell with an energy hatch in it, where upstream would refuse the shape.
pub const COKE_OVEN_HATCHES: &[HatchKind] = &[
    HatchKind::ItemInput,
    HatchKind::ItemOutput,
    HatchKind::FluidInput,
    HatchKind::FluidOutput,
    HatchKind::EnergyHatch {
        tier: HatchTier::Lv,
        input: true,
    },
    HatchKind::EnergyHatch {
        tier: HatchTier::Lv,
        input: false,
    },
    HatchKind::EnergyHatch {
        tier: HatchTier::Mv,
        input: true,
    },
    HatchKind::EnergyHatch {
        tier: HatchTier::Mv,
        input: false,
    },
    HatchKind::EnergyHatch {
        tier: HatchTier::Hv,
        input: true,
    },
    HatchKind::EnergyHatch {
        tier: HatchTier::Hv,
        input: false,
    },
    HatchKind::EnergyHatch {
        tier: HatchTier::Ev,
        input: true,
    },
    HatchKind::EnergyHatch {
        tier: HatchTier::Ev,
        input: false,
    },
    HatchKind::EnergyHatch {
        tier: HatchTier::Superconductor,
        input: true,
    },
    HatchKind::EnergyHatch {
        tier: HatchTier::Superconductor,
        input: false,
    },
];

/// A cable tier: upstream's `CableTier`, which decides an energy hatch's buffer.
///
/// `CableTier`'s `eu` is the EU a tick the tier carries — 32, 128, 1,024, 8,192
/// and 128,000,000 — and `EnergyHatch`'s buffer is `30 * 20 * tier.getEu()` EU
/// (one second of that tier's transfer, in EU). The five tiers are the ones
/// `MultiblockHatches.registerEnergyHatches` registers in base MI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum HatchTier {
    /// Low voltage: 32 EU/t.
    Lv,
    /// Medium voltage: 128 EU/t.
    Mv,
    /// High voltage: 1,024 EU/t.
    Hv,
    /// Extreme voltage: 8,192 EU/t.
    Ev,
    /// Superconductor: 128,000,000 EU/t.
    Superconductor,
}

impl HatchTier {
    /// Every tier base MI registers, in `CableTier`'s own order.
    pub const ALL: [Self; 5] = [Self::Lv, Self::Mv, Self::Hv, Self::Ev, Self::Superconductor];

    /// The block-id prefix and `CableTier.name` (`lv`, …, `superconductor`).
    #[must_use]
    pub fn prefix(self) -> &'static str {
        match self {
            Self::Lv => "lv",
            Self::Mv => "mv",
            Self::Hv => "hv",
            Self::Ev => "ev",
            Self::Superconductor => "superconductor",
        }
    }

    /// The tier a block-id prefix names.
    #[must_use]
    pub fn from_prefix(prefix: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|tier| tier.prefix() == prefix)
    }

    /// `CableTier.eu`: the EU a tick the tier carries (`getEu`).
    #[must_use]
    pub fn eu(self) -> i64 {
        match self {
            Self::Lv => 32,
            Self::Mv => 128,
            Self::Hv => 1_024,
            Self::Ev => 8_192,
            Self::Superconductor => 128_000_000,
        }
    }

    /// The hatch's buffer in FE: upstream's `30 * 20 * tier.getEu()` EU at this
    /// mod's own 1 EU = 10 FE.
    ///
    /// The superconductor tier's 76,800,000,000 EU do not fit this pin's 32-bit FE
    /// buffer (`ModEnergyBuffer::capacity` is an `i32`), so it keeps the largest it
    /// can — `i32::MAX` FE, 214,748,364 EU, 0.28 % of upstream's — and `notes.md`
    /// records the difference rather than hiding it.
    #[must_use]
    pub fn capacity_fe(self) -> i32 {
        let fe = 600 * self.eu() * i64::from(FE_PER_EU);
        i32::try_from(fe).unwrap_or(i32::MAX)
    }
}

/// The steam tier's EU a tick: `SteamCraftingMultiblockBlockEntity.getBaseRecipeEu`
/// is `overclockComponent.getRecipeEu(steelHatches ? 4 : 2)` and `getMaxRecipeEu`
/// returns the same, so a coke oven runs at exactly 2 EU/t (20 FE/t) with the
/// bronze hatches the jar's shape allows, and the steel hatches' 4 EU/t are not
/// ported. There is no overclock: the recipe always costs `eu * duration`.
pub const COKE_EU: i32 = 2;

/// The controller's buffer in FE. Upstream's steam machines store steam, not EU
/// (`SteamHelper.consumeSteamEu`: 1 mB of steam is 1 EU); the pin cannot keep a
/// mod's fluid in a block-entity tank, so this slice stores the same amount of
/// energy as FE at the mod's own ratio — 3,200 EU = 32,000 FE, the LV tier's
/// buffer, as slice 1's electric macerator does. `notes.md` records the deviation.
pub const COKE_CAPACITY_FE: i32 = CAPACITY_FE;

/// One position of a multiblock shape and the block that must be there
/// (upstream's `ShapeTemplate.simpleMembers`: an offset from the controller and
/// a `SimpleMember`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShellMember {
    /// The position relative to the controller's own block.
    pub offset: [i32; 3],
    /// The block id the position must hold when it holds no hatch.
    pub block: &'static str,
    /// The hatch kinds this position may hold *instead of* `block` — upstream's
    /// `HatchFlags` for the position. Empty means brick only.
    pub hatches: &'static [HatchKind],
}

impl ShellMember {
    /// A position that must hold the shell's own block: upstream's
    /// `HatchFlags.NO_HATCH`.
    const fn brick(offset: [i32; 3], block: &'static str) -> Self {
        Self {
            offset,
            block,
            hatches: &[],
        }
    }

    /// A position that may hold any of the coke oven's hatch kinds.
    const fn hatchable(offset: [i32; 3]) -> Self {
        Self {
            offset,
            block: COKE_OVEN_BRICK,
            hatches: COKE_OVEN_HATCHES,
        }
    }

    /// Whether this position accepts `kind` instead of its own block.
    #[must_use]
    pub fn allows(&self, kind: HatchKind) -> bool {
        self.hatches.contains(&kind)
    }
}

/// A kind of hatch a multiblock position may hold: the four kinds the coke
/// oven's `HatchFlags` allows and the energy family `MultiBlockHatches` registers
/// (ten blocks: five cable tiers, in and out). Upstream's `HatchType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HatchKind {
    /// An item hatch the multiblock takes its inputs from.
    ItemInput,
    /// An item hatch the multiblock puts its outputs into.
    ItemOutput,
    /// A fluid hatch the multiblock takes its inputs from.
    FluidInput,
    /// A fluid hatch the multiblock puts its outputs into.
    FluidOutput,
    /// An energy hatch of one cable tier: upstream's `EnergyHatch`, whose
    /// `appendEnergyInputs`/`appendEnergyOutputs` put its buffer into the
    /// multiblock's (`AbstractElectricCraftingMultiblockBlockEntity`).
    EnergyHatch {
        /// The cable tier, which decides the buffer (`HatchTier::capacity_fe`).
        tier: HatchTier,
        /// Whether it feeds the multiblock (an input hatch) or drains it.
        input: bool,
    },
}

impl HatchKind {
    /// Every kind slice 3 ports, in upstream's registration order.
    pub const ALL: [Self; 4] = [
        Self::ItemInput,
        Self::ItemOutput,
        Self::FluidInput,
        Self::FluidOutput,
    ];

    /// The energy hatches, in `registerEnergyHatches`'s order: one input and one
    /// output per tier, tier by tier.
    pub const ENERGY: [Self; 10] = [
        Self::EnergyHatch {
            tier: HatchTier::Lv,
            input: true,
        },
        Self::EnergyHatch {
            tier: HatchTier::Lv,
            input: false,
        },
        Self::EnergyHatch {
            tier: HatchTier::Mv,
            input: true,
        },
        Self::EnergyHatch {
            tier: HatchTier::Mv,
            input: false,
        },
        Self::EnergyHatch {
            tier: HatchTier::Hv,
            input: true,
        },
        Self::EnergyHatch {
            tier: HatchTier::Hv,
            input: false,
        },
        Self::EnergyHatch {
            tier: HatchTier::Ev,
            input: true,
        },
        Self::EnergyHatch {
            tier: HatchTier::Ev,
            input: false,
        },
        Self::EnergyHatch {
            tier: HatchTier::Superconductor,
            input: true,
        },
        Self::EnergyHatch {
            tier: HatchTier::Superconductor,
            input: false,
        },
    ];

    /// Every hatch kind this slice ships: the four item/fluid kinds, then the ten
    /// energy ones.
    pub fn every() -> impl Iterator<Item = Self> {
        Self::ALL.into_iter().chain(Self::ENERGY)
    }

    /// The block this kind is made of: the bronze hatch or the tier's energy hatch.
    #[must_use]
    pub fn block(self) -> &'static str {
        match self {
            Self::ItemInput => BRONZE_ITEM_INPUT_HATCH,
            Self::ItemOutput => BRONZE_ITEM_OUTPUT_HATCH,
            Self::FluidInput => BRONZE_FLUID_INPUT_HATCH,
            Self::FluidOutput => BRONZE_FLUID_OUTPUT_HATCH,
            Self::EnergyHatch { tier, input } => match (tier, input) {
                (HatchTier::Lv, true) => "modern_industrialization:lv_energy_input_hatch",
                (HatchTier::Lv, false) => "modern_industrialization:lv_energy_output_hatch",
                (HatchTier::Mv, true) => "modern_industrialization:mv_energy_input_hatch",
                (HatchTier::Mv, false) => "modern_industrialization:mv_energy_output_hatch",
                (HatchTier::Hv, true) => "modern_industrialization:hv_energy_input_hatch",
                (HatchTier::Hv, false) => "modern_industrialization:hv_energy_output_hatch",
                (HatchTier::Ev, true) => "modern_industrialization:ev_energy_input_hatch",
                (HatchTier::Ev, false) => "modern_industrialization:ev_energy_output_hatch",
                (HatchTier::Superconductor, true) => {
                    "modern_industrialization:superconductor_energy_input_hatch"
                }
                (HatchTier::Superconductor, false) => {
                    "modern_industrialization:superconductor_energy_output_hatch"
                }
            },
        }
    }

    /// The kind a block id is, if it is one of this slice's hatches.
    #[must_use]
    pub fn from_block(block: &str) -> Option<Self> {
        Self::every().find(|kind| kind.block() == block)
    }

    /// The stable number this port saves in a controller's `hatchKinds`: 0–3 for
    /// the item/fluid kinds, then 4–13 for the energy ones, tier by tier.
    #[must_use]
    pub fn code(self) -> i32 {
        match self {
            Self::ItemInput => 0,
            Self::ItemOutput => 1,
            Self::FluidInput => 2,
            Self::FluidOutput => 3,
            Self::EnergyHatch { .. } => {
                let index = Self::ENERGY
                    .iter()
                    .position(|kind| *kind == self)
                    .unwrap_or(0);
                4 + i32::try_from(index).unwrap_or(0)
            }
        }
    }

    /// The kind a saved code is, if it is one.
    #[must_use]
    pub fn from_code(code: i32) -> Option<Self> {
        Self::every().find(|kind| kind.code() == code)
    }

    /// Whether the hatch takes items or fluid out of the multiblock.
    #[must_use]
    pub fn is_item(self) -> bool {
        matches!(self, Self::ItemInput | Self::ItemOutput)
    }

    /// Whether the hatch carries energy rather than an item or a fluid.
    #[must_use]
    pub fn is_energy(self) -> bool {
        matches!(self, Self::EnergyHatch { .. })
    }

    /// Whether the hatch feeds the multiblock rather than draining it.
    #[must_use]
    pub fn is_input(self) -> bool {
        match self {
            Self::ItemInput | Self::FluidInput => true,
            Self::ItemOutput | Self::FluidOutput => false,
            Self::EnergyHatch { input, .. } => input,
        }
    }

    /// The hatch's own FE capacity: the tier's buffer for an energy hatch, 0 for
    /// the item and fluid kinds (which hold no energy).
    #[must_use]
    pub fn capacity_fe(self) -> i32 {
        match self {
            Self::EnergyHatch { tier, .. } => tier.capacity_fe(),
            Self::ItemInput | Self::ItemOutput | Self::FluidInput | Self::FluidOutput => 0,
        }
    }
}

/// Whether a block id is any of this slice's hatches (a change of one can
/// change a shell match, exactly as a brick's can).
#[must_use]
pub fn is_hatch(block: &str) -> bool {
    HatchKind::from_block(block).is_some()
}

/// The coke oven's shell, exactly as tag 2.5.12 builds it:
///
/// ```java
/// ShapeTemplate cokeOvenShape = new ShapeTemplate.Builder(MachineCasings.BRICKS)
///         .add3by3Levels(-1, 1, bricks, cokeOvenHatches).build();
/// ```
///
/// `add3by3Levels(-1, 1, …)` writes the full 3×3 at y=-1, the 3×3 at y=0 with the
/// interior `(0,0,1)` hollowed out, and the full 3×3 at y=1; `build()` then
/// removes `(0,0,0)`, the controller's own position. That is 9 + 7 + 9 = 25
/// members, all `minecraft:bricks`, in a 3×3×3 box whose front-centre of the
/// middle layer is the controller and which extends two blocks to +z.
pub const COKE_OVEN_SHELL: &[ShellMember] = &[
    // y = -1: the full 3x3 floor (upstream: hatchable, `add3by3(-1, …, false, …)`).
    ShellMember::hatchable([-1, -1, 0]),
    ShellMember::hatchable([0, -1, 0]),
    ShellMember::hatchable([1, -1, 0]),
    ShellMember::hatchable([-1, -1, 1]),
    ShellMember::hatchable([0, -1, 1]),
    ShellMember::hatchable([1, -1, 1]),
    ShellMember::hatchable([-1, -1, 2]),
    ShellMember::hatchable([0, -1, 2]),
    ShellMember::hatchable([1, -1, 2]),
    // y = 0: the ring around the hollow interior and the controller (7). The
    // jar gives these positions `HatchFlags.NO_HATCH`: bricks only.
    ShellMember::brick([-1, 0, 0], COKE_OVEN_BRICK),
    ShellMember::brick([1, 0, 0], COKE_OVEN_BRICK),
    ShellMember::brick([-1, 0, 1], COKE_OVEN_BRICK),
    ShellMember::brick([1, 0, 1], COKE_OVEN_BRICK),
    ShellMember::brick([-1, 0, 2], COKE_OVEN_BRICK),
    ShellMember::brick([0, 0, 2], COKE_OVEN_BRICK),
    ShellMember::brick([1, 0, 2], COKE_OVEN_BRICK),
    // y = 1: the full 3x3 roof (upstream: hatchable).
    ShellMember::hatchable([-1, 1, 0]),
    ShellMember::hatchable([0, 1, 0]),
    ShellMember::hatchable([1, 1, 0]),
    ShellMember::hatchable([-1, 1, 1]),
    ShellMember::hatchable([0, 1, 1]),
    ShellMember::hatchable([1, 1, 1]),
    ShellMember::hatchable([-1, 1, 2]),
    ShellMember::hatchable([0, 1, 2]),
    ShellMember::hatchable([1, 1, 2]),
];
/// Whether a block change can change a coke oven's match: only a shell member's
/// block or the controller's own block can, so a world full of leaf decay and
/// water flow does not make the glue read 25 blocks per change.
#[must_use]
pub fn shell_relevant(old: &str, new: &str) -> bool {
    [old, new]
        .iter()
        .any(|id| *id == COKE_OVEN_BRICK || *id == COKE_OVEN_ID || is_hatch(id))
}

/// The world as a shape match sees it: the block id at an absolute position.
///
/// The id is owned because not every source can borrow one: a command reads a
/// [`crate::BlockState`] and formats its id, while `block_changed` already has
/// the string. A match is 25 reads on a block update, never per tick, so the
/// allocation is not on a hot path.
pub trait Shell {
    /// The block at `pos`, or `None` where the level has none.
    fn block_at(&self, pos: [i32; 3]) -> Option<String>;
}

/// What a shape match found.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ShellMatch {
    /// Whether every member holds its declared block or an allowed hatch.
    pub formed: bool,
    /// How many members are wrong or missing (upstream's "missing blocks").
    pub missing: u8,
    /// Every hatchable position that holds a hatch, and the kind it holds: the
    /// per-position `Option<HatchKind>` the shell template's `HatchFlags`
    /// resolves to. Saved in the controller's `hatchKinds`.
    pub hatches: Vec<([i32; 3], HatchKind)>,
}

/// Whether the block at a member's position satisfies it: the member's own
/// block, or a hatch whose kind the position's `HatchFlags` allows.
fn member_holds(member: &ShellMember, block: Option<&str>) -> bool {
    match block {
        Some(id) if id == member.block => true,
        Some(id) => HatchKind::from_block(id).is_some_and(|kind| member.allows(kind)),
        None => false,
    }
}

/// Matches `members` around the controller at `origin`.
#[must_use]
pub fn match_shell(shell: &dyn Shell, origin: [i32; 3], members: &[ShellMember]) -> ShellMatch {
    let mut missing = 0;
    let mut hatches = Vec::new();
    for member in members {
        let pos = [
            origin[0] + member.offset[0],
            origin[1] + member.offset[1],
            origin[2] + member.offset[2],
        ];
        let block = shell.block_at(pos);
        if !member_holds(member, block.as_deref()) {
            missing += 1;
            continue;
        }
        if members_hatch_position(member)
            && let Some(kind) = block.as_deref().and_then(HatchKind::from_block)
        {
            hatches.push((member.offset, kind));
        }
    }
    ShellMatch {
        formed: missing == 0,
        missing,
        hatches,
    }
}

/// Whether a position accepts hatches at all (upstream's `hatchFlags` map).
fn members_hatch_position(member: &ShellMember) -> bool {
    !member.hatches.is_empty()
}

/// The coke oven's hatchable positions, in the order its controller saves their
/// kinds (`hatchKinds`).
pub fn hatch_members() -> impl Iterator<Item = &'static ShellMember> {
    COKE_OVEN_SHELL
        .iter()
        .filter(|member| members_hatch_position(member))
}

/// What a coke oven saves, so a reload resumes its run and its shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OvenSaved {
    /// Stored FE (the block entity's saved `energy`).
    pub energy: i32,
    /// EU the active run has used.
    pub used_eu: i64,
    /// EU the active run costs in all.
    pub recipe_energy: i64,
    /// EU a tick the active run takes now.
    pub recipe_max_eu: i64,
    /// The active recipe's id.
    pub active: Option<&'static str>,
    /// Whether the shell matched at the last check.
    pub formed: bool,
    /// The members that were wrong or missing at the last check.
    pub missing: u8,
}

/// What `/modern_industrialization info` prints for a coke oven.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OvenStatus {
    /// Stored FE.
    pub energy: i32,
    /// EU a tick the active run takes now.
    pub recipe_max_eu: i64,
    /// The active recipe's id.
    pub active: Option<&'static str>,
    /// Whether the shell matched at the last check.
    pub formed: bool,
    /// The members that were wrong or missing at the last check.
    pub missing: u8,
    /// Progress through the active run, in thousandths.
    pub progress_permille: u16,
}

/// The coke oven's state: its buffer, its run, and whether its shell matches.
///
/// Upstream's `AbstractCraftingMultiblockBlockEntity` ticks its `CrafterComponent`
/// only while the shape matches (`OperatingState.NORMAL_OPERATION`); while it does
/// not (`NOT_MATCHED`) the crafter is not ticked at all, so a run is paused and
/// resumes (`TRYING_TO_RESUME`) once the shell is whole again. This type is that
/// state machine without the level: the glue writes [`formed`](Self::formed).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CokeOven {
    /// Stored FE.
    pub energy: i32,
    /// EU the active run has used.
    pub used_eu: i64,
    /// EU the active run costs in all.
    pub recipe_energy: i64,
    /// EU a tick the active run takes now.
    pub recipe_max_eu: i64,
    /// The active recipe's id.
    pub active: Option<&'static str>,
    /// Whether the shell matched at the last check.
    pub formed: bool,
    /// The members that were wrong or missing at the last check.
    pub missing: u8,
}

impl CokeOven {
    /// The oven a saved compound describes; a recipe the table no longer holds
    /// resets the run, as upstream resets `usedEnergy` for a lost recipe.
    pub fn from_saved(saved: &OvenSaved) -> Self {
        let active = saved.active.filter(|id| coke_recipe(id).is_some());
        let (used_eu, recipe_energy, recipe_max_eu) = match active {
            Some(_) => (
                saved.used_eu.max(0),
                saved.recipe_energy.max(0),
                saved.recipe_max_eu.max(0),
            ),
            None => (0, 0, 0),
        };
        Self {
            energy: saved.energy.clamp(0, COKE_CAPACITY_FE),
            used_eu,
            recipe_energy,
            recipe_max_eu,
            active,
            formed: saved.formed,
            missing: saved.missing.min(COKE_OVEN_SHELL.len() as u8),
        }
    }

    /// What to save.
    pub fn saved(&self) -> OvenSaved {
        OvenSaved {
            energy: self.energy,
            used_eu: self.used_eu,
            recipe_energy: self.recipe_energy,
            recipe_max_eu: self.recipe_max_eu,
            active: self.active,
            formed: self.formed,
            missing: self.missing,
        }
    }

    /// Takes up to `amount` FE into the buffer, answering how many it took.
    pub fn insert_fe(&mut self, amount: i32) -> i32 {
        let taken = amount.clamp(0, COKE_CAPACITY_FE - self.energy);
        self.energy += taken;
        taken
    }

    /// What the info command prints.
    pub fn status(&self) -> OvenStatus {
        let progress = if self.recipe_energy > 0 {
            (self.used_eu * 1_000 / self.recipe_energy).clamp(0, 1_000)
        } else {
            0
        };
        OvenStatus {
            energy: self.energy,
            recipe_max_eu: self.recipe_max_eu,
            active: self.active,
            formed: self.formed,
            missing: self.missing,
            progress_permille: u16::try_from(progress).unwrap_or(0),
        }
    }

    /// One server tick of `CrafterComponent` at the steam tier's 2 EU/t.
    ///
    /// An unformed oven ticks nothing: the run stays exactly where it was, no
    /// energy is taken and no input is consumed, which is upstream's
    /// `NOT_MATCHED` branch.
    pub fn tick(&mut self, slots: &mut dyn Slots, roll: &mut dyn FnMut() -> u32) -> Tick {
        if !self.formed {
            return Tick::default();
        }
        let mut started = false;

        // START, or resume the paused run upstream puts in TRYING_TO_RESUME.
        if self.used_eu == 0 && self.energy >= FE_PER_EU {
            let resumable = self
                .active
                .and_then(coke_recipe)
                .filter(|found| can_start(found, slots));
            let found =
                resumable.or_else(|| COKE_RECIPES.iter().find(|found| can_start(found, slots)));
            if let Some(found) = found {
                take_inputs(found, slots);
                self.active = Some(found.id);
                self.used_eu = 0;
                self.recipe_energy = found.total_eu();
                self.recipe_max_eu = i64::from(COKE_EU);
                started = true;
            }
        }

        // PROCESS.
        let mut energy_used_eu = 0;
        let mut crafted = None;
        if let Some(found) = self.active.and_then(coke_recipe) {
            self.recipe_max_eu = i64::from(COKE_EU);
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
                    crafted = Some(found.id);
                }
            }
        }
        // The steam oven has no efficiency ticks, so a finished run clears its
        // recipe at once (`clearActiveRecipeIfPossible` with 0 ticks).
        if self.used_eu == 0 && crafted.is_some() {
            self.active = None;
        }

        Tick {
            energy_used_eu,
            crafted,
            active: energy_used_eu > 0,
        }
    }
}

/// [`COKE_OVEN_ID`]'s entity check, as [`is_macerator`] is for the macerator.
#[must_use]
pub fn is_coke_oven(entity_id: &str) -> bool {
    entity_id == COKE_OVEN_ID
}

/// The coke oven recipe with this id, if the table holds it.
pub fn coke_recipe(id: &str) -> Option<&'static MachineRecipe> {
    COKE_RECIPES.iter().find(|recipe| recipe.id == id)
}

/// The coke oven recipes this slice carries, from the jar's
/// `data/modern_industrialization/recipe/materials/coke_oven/`.
///
/// Both rows are the jar's own: `coke.json` (1 coal → 1 coke at 2 EU/t for 600
/// ticks) and `coke_dust.json` (1 `#c:dusts/coal` → 1 coke dust, the tag resolved
/// to this package's own coal dust). Each jar recipe also names a **fluid**
/// output — 500 mB of `modern_industrialization:creosote` at 50% — which this
/// slice does not model: a fluid tank for the mod's own fluid is the gap recorded
/// in `notes.md`, and dropping a chance output is upstream's own behaviour when a
/// machine has nowhere to put it.
pub const COKE_RECIPES: &[MachineRecipe] = &[
    MachineRecipe {
        // materials/coke_oven/coke.json
        id: "modern_industrialization:coke_oven_coal_to_coke",
        inputs: &[Input {
            item: "minecraft:coal",
            amount: 1,
        }],
        outputs: &[Output {
            item: "modern_industrialization:coke",
            amount: 1,
            permille: 10000,
        }],
        eu: COKE_EU,
        duration: 600,
    },
    MachineRecipe {
        // materials/coke_oven/coke_dust.json (tag c:dusts/coal)
        id: "modern_industrialization:coke_oven_coal_dust_to_coke_dust",
        inputs: &[Input {
            item: "modern_industrialization:coal_dust",
            amount: 1,
        }],
        outputs: &[Output {
            item: "modern_industrialization:coke_dust",
            amount: 1,
            permille: 10000,
        }],
        eu: COKE_EU,
        duration: 600,
    },
];

// ---------------------------------------------------------------------------
// Slice 3: hatches, casings and the mod-readable coke recipe kind
// ---------------------------------------------------------------------------

/// The coke oven's recipe kind, registered with the game and read by another
/// mod through `recipes_of_type`: upstream's `MIMachineRecipeTypes.COKE_OVEN`
/// serializes under `modern_industrialization:coke_oven`. The machine's table
/// stays [`COKE_RECIPES`]; the shipped `data/modern_industrialization/recipe/
/// coke_oven/` files are the same two runs in the kind another mod can read,
/// and a test pins the two together.
pub const COKE_RECIPE_KIND: &str = "modern_industrialization:coke_oven";

/// The shipped recipe files of [`COKE_RECIPE_KIND`], as the package's
/// `data/modern_industrialization/recipe/coke_oven/` carries them.
pub const COKE_RECIPE_FILES: &[(&str, &str)] = &[
    (
        "modern_industrialization:coke_oven/coke",
        include_str!("../data/modern_industrialization/recipe/coke_oven/coke.json"),
    ),
    (
        "modern_industrialization:coke_oven/coke_dust",
        include_str!("../data/modern_industrialization/recipe/coke_oven/coke_dust.json"),
    ),
];

/// How many of `source` a move into `target` lifts: a whole stack into an empty
/// slot, the free room into a stack of the same item, nothing into another
/// item or a full slot. This is the item hatch's pump half — the controller
/// has the other half (`CrafterComponent`'s slots) — and it is pure so the
/// arithmetic is tested without a world.
#[must_use]
pub fn merge_count(source: (&str, u32), target: Option<(&str, u32)>, stack_size: u32) -> u32 {
    let (item, count) = source;
    match target {
        Some((held, amount)) if held == item => stack_size.saturating_sub(amount).min(count),
        Some(_) => 0,
        None => stack_size.min(count),
    }
}

/// How much energy a move lifts from one buffer into another: what the source
/// holds, what the target's room allows, and the move's own limit, whichever is
/// smallest, never negative.
///
/// This is the energy half of the hatch pump (`merge_count` is the item half):
/// upstream's `AbstractElectricCraftingMultiblockBlockEntity.consumeEu` draws the
/// EU straight out of the hatches' `EnergyComponent`s, and this port's glue moves
/// the same energy into the controller's own buffer one tick at a time, so the
/// arithmetic is pure and tested without a world.
#[must_use]
pub fn move_energy_count(from: i32, to: i32, to_capacity: i32, max: i32) -> i32 {
    let room = to_capacity.saturating_sub(to).max(0);
    from.max(0).min(max.max(0)).min(room)
}

/// The add-on `industrialization_overdrive`'s pyrolyse oven shell, as its
/// parent-surface matrix describes it: a 3×3×4 box of 33 members — 17
/// `bronze_plated_bricks`, then 16 coil positions of which the two roof corners
/// may hold a hatch — around a controller with two interior blocks.
///
/// It is a **harness fixture**: the add-on owns the controller and declares its
/// own template; this port owns the matcher, the blocks and the hatch kinds, and
/// `/modern_industrialization shape` matches this template in game so the 3×3×4
/// case is proven before the add-on's slice exists.
#[must_use]
pub fn pyrolyse_shape_fixture() -> Vec<ShellMember> {
    let mut offsets = Vec::new();
    for y in 0..4 {
        for z in 0..3 {
            for x in -1..=1 {
                if [x, y, z] == [0, 0, 0] || [x, y, z] == [0, 0, 1] || [x, y, z] == [0, 0, 2] {
                    continue; // the controller and its two interior blocks
                }
                offsets.push([x, y, z]);
            }
        }
    }
    debug_assert_eq!(
        offsets.len(),
        33,
        "3 x 3 x 4 minus a controller and 2 interior"
    );
    let hatchable = [[-1, 3, 0], [1, 3, 2]];
    offsets
        .into_iter()
        .enumerate()
        .map(|(index, offset)| ShellMember {
            offset,
            block: if index < 17 {
                BRONZE_PLATED_BRICKS
            } else {
                CUPRONICKEL_COIL
            },
            hatches: if hatchable.contains(&offset) {
                COKE_OVEN_HATCHES
            } else {
                &[]
            },
        })
        .collect()
}

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
        let seed = MachineRecipe {
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

    #[test]
    fn the_mutation_commands_only_touch_this_slices_machine() {
        assert!(is_macerator(MACERATOR_ID));
        // `/modern_industrialization energy` and `… efficiency` take a
        // position; an entity of another mod standing there must not be read or
        // written (its own `energy` key and this port's run keys would clash).
        assert!(!is_macerator("minecraft:furnace"));
        assert!(!is_macerator("modern_industrialization:steam_boiler"));
        assert!(!is_macerator(""));
    }

    // -----------------------------------------------------------------------
    // Slice 2: the coke oven multiblock
    // -----------------------------------------------------------------------

    /// A shell as the game would answer it: absolute position to block id.
    #[derive(Clone)]
    struct ShellBench {
        blocks: Vec<([i32; 3], &'static str)>,
    }

    impl ShellBench {
        /// The controller at `origin` standing in a complete shell of bricks.
        fn complete(origin: [i32; 3]) -> Self {
            Self::complete_with(origin, COKE_OVEN_SHELL)
        }

        /// The controller at `origin` standing in `members`, each holding its
        /// declared block.
        fn complete_with(origin: [i32; 3], members: &[ShellMember]) -> Self {
            Self {
                blocks: members
                    .iter()
                    .map(|member| {
                        (
                            [
                                origin[0] + member.offset[0],
                                origin[1] + member.offset[1],
                                origin[2] + member.offset[2],
                            ],
                            member.block,
                        )
                    })
                    .collect(),
            }
        }

        fn with(mut self, pos: [i32; 3], block: &'static str) -> Self {
            self.blocks.retain(|(at, _)| *at != pos);
            self.blocks.push((pos, block));
            self
        }

        fn without(mut self, pos: [i32; 3]) -> Self {
            self.blocks.retain(|(at, _)| *at != pos);
            self
        }
    }

    impl Shell for ShellBench {
        fn block_at(&self, pos: [i32; 3]) -> Option<String> {
            self.blocks
                .iter()
                .find(|(at, _)| *at == pos)
                .map(|(_, block)| (*block).to_owned())
        }
    }

    /// The 25 shell positions of a controller at the origin, as the jar's
    /// `ShapeTemplate` lays them out.
    fn shelf() -> ShellBench {
        ShellBench::complete([0, 0, 0])
    }

    #[test]
    fn the_coke_oven_shell_is_upstreams_three_by_three_by_three() {
        assert_eq!(COKE_OVEN_SHELL.len(), 25, "9 + 7 + 9 positions");
        assert!(
            COKE_OVEN_SHELL
                .iter()
                .all(|m| m.block == "minecraft:bricks")
        );
        for (y, want) in [(-1, 9), (0, 7), (1, 9)] {
            let layer = COKE_OVEN_SHELL.iter().filter(|m| m.offset[1] == y).count();
            assert_eq!(layer, want, "layer y={y}");
        }
        // The controller's own position and the hollow interior are not members.
        assert!(!COKE_OVEN_SHELL.iter().any(|m| m.offset == [0, 0, 0]));
        assert!(
            !COKE_OVEN_SHELL
                .iter()
                .any(|m| m.offset == COKE_OVEN_INTERIOR)
        );
        // Every position is unique, and the shape is 3 wide and 3 deep.
        for (index, member) in COKE_OVEN_SHELL.iter().enumerate() {
            assert!(
                !COKE_OVEN_SHELL[..index]
                    .iter()
                    .any(|other| other.offset == member.offset),
                "{:?} twice",
                member.offset
            );
            assert!((-1..=1).contains(&member.offset[0]));
            assert!((0..=2).contains(&member.offset[2]));
        }
    }

    #[test]
    fn a_shell_of_bricks_forms_the_coke_oven() {
        let matched = match_shell(&shelf(), [0, 0, 0], COKE_OVEN_SHELL);
        assert!(matched.formed, "{matched:?}");
        assert_eq!(matched.missing, 0);
    }

    #[test]
    fn one_missing_brick_leaves_the_coke_oven_unformed() {
        let matched = match_shell(&shelf().without([1, 1, 2]), [0, 0, 0], COKE_OVEN_SHELL);
        assert!(!matched.formed);
        assert_eq!(matched.missing, 1);
    }

    #[test]
    fn a_shell_member_of_another_block_does_not_form() {
        // The macerator is this port's own block and still not the oven's brick.
        let matched = match_shell(
            &shelf().with([-1, -1, 0], "modern_industrialization:electric_macerator"),
            [0, 0, 0],
            COKE_OVEN_SHELL,
        );
        assert!(!matched.formed);
        assert_eq!(matched.missing, 1);
    }

    #[test]
    fn the_coke_ovens_interior_and_its_controller_are_not_checked() {
        // Upstream's hollow layer skips (0,0,1) and `build()` removes (0,0,0):
        // what stands there is the player's business, not the shape's.
        let mut bench = shelf();
        bench.blocks.push((COKE_OVEN_INTERIOR, "minecraft:stone"));
        bench.blocks.push(([0, 0, 0], COKE_OVEN_ID));
        let matched = match_shell(&bench, [0, 0, 0], COKE_OVEN_SHELL);
        assert!(matched.formed, "{matched:?}");
    }

    #[test]
    fn the_match_is_relative_to_the_controller() {
        let bench = ShellBench::complete([10, 100, -4]);
        assert!(match_shell(&bench, [10, 100, -4], COKE_OVEN_SHELL).formed);
        assert!(!match_shell(&bench, [0, 0, 0], COKE_OVEN_SHELL).formed);
    }

    #[test]
    fn only_a_shell_member_or_the_controller_can_change_a_match() {
        // Reading 25 blocks for every block change in the world is wasteful; a
        // change of anything else cannot change a coke oven's match.
        assert!(
            shell_relevant(COKE_OVEN_BRICK, "minecraft:air"),
            "a brick goes"
        );
        assert!(
            shell_relevant("minecraft:air", COKE_OVEN_BRICK),
            "a brick comes"
        );
        assert!(
            shell_relevant(COKE_OVEN_ID, "minecraft:air"),
            "the controller goes"
        );
        assert!(
            shell_relevant("minecraft:air", COKE_OVEN_ID),
            "the controller comes"
        );
        assert!(
            shell_relevant(BRONZE_ITEM_INPUT_HATCH, "minecraft:air"),
            "a hatch goes"
        );
        assert!(
            shell_relevant("minecraft:air", BRONZE_FLUID_OUTPUT_HATCH),
            "a hatch comes"
        );
        assert!(!shell_relevant("minecraft:stone", "minecraft:gravel"));
        assert!(!shell_relevant("minecraft:air", "minecraft:water"));
    }

    #[test]
    fn the_carried_coke_recipe_is_the_jars_two_eu_six_hundred_ticks() {
        let coke = coke_recipe("modern_industrialization:coke_oven_coal_to_coke")
            .expect("the jar's coke recipe is carried");
        assert_eq!(coke.eu, 2);
        assert_eq!(coke.duration, 600);
        assert_eq!(coke.total_eu(), 1_200, "2 EU/t x 600 ticks");
        assert_eq!(coke.inputs.len(), 1);
        assert_eq!(
            coke.inputs[0],
            Input {
                item: "minecraft:coal",
                amount: 1
            }
        );
        assert_eq!(coke.outputs.len(), 1, "the item half of the recipe");
        assert_eq!(coke.outputs[0].item, "modern_industrialization:coke");
        assert_eq!(coke.outputs[0].amount, 1);
        assert_eq!(coke.outputs[0].permille, PROBABILITY_SCALE);
        assert_eq!(COKE_RECIPES.len(), 2, "coal and coal dust");
    }

    #[test]
    fn an_unformed_coke_oven_does_nothing_and_consumes_nothing() {
        let mut oven = CokeOven::default();
        assert!(!oven.formed);
        oven.insert_fe(20_000);
        let mut bench = Bench::new(&[("minecraft:coal", 1)]);
        for _ in 0..600 {
            oven.tick(&mut bench, &mut always);
        }
        assert_eq!(oven.energy, 20_000, "an unformed oven burns nothing");
        assert_eq!(oven.active, None);
        assert_eq!(bench.held("minecraft:coal"), 1, "the input is kept");
        assert_eq!(bench.out("modern_industrialization:coke"), 0);
        assert_eq!(oven.status().progress_permille, 0);
    }

    #[test]
    fn a_formed_coke_oven_makes_one_coke_for_exactly_twelve_thousand_fe() {
        let mut oven = CokeOven {
            formed: true,
            ..Default::default()
        };
        oven.insert_fe(20_000);
        let mut bench = Bench::new(&[("minecraft:coal", 1)]);
        for _ in 0..600 {
            oven.tick(&mut bench, &mut always);
        }
        assert_eq!(bench.held("minecraft:coal"), 0, "the coal was consumed");
        assert_eq!(bench.out("modern_industrialization:coke"), 1);
        assert_eq!(oven.energy, 8_000, "1,200 EU = 12,000 FE exactly");
        assert_eq!(oven.active, None, "the run is over");
    }

    #[test]
    fn the_coke_run_takes_exactly_six_hundred_ticks() {
        let mut oven = CokeOven {
            formed: true,
            ..Default::default()
        };
        oven.insert_fe(20_000);
        let mut bench = Bench::new(&[("minecraft:coal", 1)]);
        for _ in 0..599 {
            oven.tick(&mut bench, &mut always);
        }
        assert_eq!(
            bench.out("modern_industrialization:coke"),
            0,
            "599 is too few"
        );
        assert_eq!(oven.status().progress_permille, 998, "1198 of 1200 EU");
        assert_eq!(oven.energy, 20_000 - 11_980);
        oven.tick(&mut bench, &mut always);
        assert_eq!(bench.out("modern_industrialization:coke"), 1);
        assert_eq!(oven.energy, 8_000);
    }

    #[test]
    fn breaking_a_brick_stops_the_oven_and_reforming_resumes_it() {
        let mut oven = CokeOven {
            formed: true,
            ..Default::default()
        };
        oven.insert_fe(20_000);
        let mut bench = Bench::new(&[("minecraft:coal", 1)]);
        for _ in 0..100 {
            oven.tick(&mut bench, &mut always);
        }
        let paused_energy = oven.energy;
        let paused_progress = oven.status().progress_permille;
        assert!(paused_progress > 0 && paused_progress < 1_000);

        // The shell is broken: the controller is told so by the glue.
        oven.formed = false;
        for _ in 0..200 {
            oven.tick(&mut bench, &mut always);
        }
        assert_eq!(oven.energy, paused_energy, "an unformed oven burns nothing");
        assert_eq!(oven.status().progress_permille, paused_progress);
        assert_eq!(bench.out("modern_industrialization:coke"), 0);

        // Rebuilt: upstream resumes the run (`TRYING_TO_RESUME`) rather than
        // starting it again, and the whole run still costs the recipe's energy.
        oven.formed = true;
        for _ in 0..500 {
            oven.tick(&mut bench, &mut always);
        }
        assert_eq!(bench.out("modern_industrialization:coke"), 1);
        assert_eq!(oven.energy, 8_000, "the paused ticks cost nothing");
    }

    #[test]
    fn an_oven_without_its_input_starts_nothing() {
        let mut oven = CokeOven {
            formed: true,
            ..Default::default()
        };
        oven.insert_fe(20_000);
        let mut bench = Bench::new(&[]);
        for _ in 0..40 {
            oven.tick(&mut bench, &mut always);
        }
        assert_eq!(oven.energy, 20_000);
        assert_eq!(oven.active, None);
    }

    #[test]
    fn the_coke_ovens_hatch_flags_are_upstreams_two_end_layers() {
        // `add3by3Levels(-1, 1, bricks, cokeOvenHatches)` flags the floor and the
        // roof; the middle ring keeps `HatchFlags.NO_HATCH`.
        let hatchable: Vec<_> = COKE_OVEN_SHELL
            .iter()
            .filter(|member| !member.hatches.is_empty())
            .collect();
        assert_eq!(hatchable.len(), 18, "9 floor + 9 roof positions");
        assert!(
            hatchable
                .iter()
                .all(|member| member.offset[1] != 0 && member.hatches == COKE_OVEN_HATCHES)
        );
        assert_eq!(
            COKE_OVEN_SHELL
                .iter()
                .filter(|member| member.offset[1] == 0 && member.hatches.is_empty())
                .count(),
            7,
            "the middle ring is brick-only"
        );
        assert_eq!(HatchKind::ALL.len(), 4);
        // The jar's flags name four kinds; the energy family (10 blocks, 5 tiers x
        // in/out) is the fifth hatch kind this slice adds to the same positions.
        assert_eq!(COKE_OVEN_HATCHES.len(), 14);
    }

    #[test]
    fn an_allowed_hatch_forms_the_coke_oven_and_is_recorded() {
        let bench = shelf()
            .with([-1, -1, 0], BRONZE_ITEM_INPUT_HATCH)
            .with([1, 1, 2], BRONZE_ITEM_OUTPUT_HATCH);
        let matched = match_shell(&bench, [0, 0, 0], COKE_OVEN_SHELL);
        assert!(matched.formed, "{matched:?}");
        assert_eq!(matched.missing, 0);
        assert_eq!(
            matched.hatches,
            vec![
                ([-1, -1, 0], HatchKind::ItemInput),
                ([1, 1, 2], HatchKind::ItemOutput),
            ],
            "the per-position hatch kinds are the layout the controller saves"
        );
    }

    #[test]
    fn a_hatch_in_a_brick_only_position_does_not_form() {
        // (1, 0, 0) is in the middle ring, which the jar gives no hatch flags.
        let bench = shelf().with([1, 0, 0], BRONZE_FLUID_INPUT_HATCH);
        let matched = match_shell(&bench, [0, 0, 0], COKE_OVEN_SHELL);
        assert!(!matched.formed);
        assert_eq!(matched.missing, 1);
    }

    #[test]
    fn a_hatch_whose_kind_the_position_does_not_allow_does_not_form() {
        // A template that allows only an item input at its one hatch position,
        // as the jar's other shapes do (the steam boiler takes no item output).
        const ALLOWED: &[HatchKind] = &[HatchKind::ItemInput];
        let members = [
            ShellMember {
                offset: [0, 1, 0],
                block: COKE_OVEN_BRICK,
                hatches: ALLOWED,
            },
            ShellMember {
                offset: [0, 2, 0],
                block: COKE_OVEN_BRICK,
                hatches: &[],
            },
        ];
        let allowed =
            ShellBench::complete_with([0, 0, 0], &members).with([0, 1, 0], BRONZE_ITEM_INPUT_HATCH);
        assert!(match_shell(&allowed, [0, 0, 0], &members).formed);
        let refused = ShellBench::complete_with([0, 0, 0], &members)
            .with([0, 1, 0], BRONZE_ITEM_OUTPUT_HATCH);
        let matched = match_shell(&refused, [0, 0, 0], &members);
        assert!(!matched.formed);
        assert_eq!(matched.missing, 1, "an item output is not allowed here");
    }

    #[test]
    fn a_coil_where_a_brick_belongs_does_not_form() {
        let bench = shelf().with([-1, -1, 0], CUPRONICKEL_COIL);
        let matched = match_shell(&bench, [0, 0, 0], COKE_OVEN_SHELL);
        assert!(!matched.formed);
        assert_eq!(matched.missing, 1);
        let bench = shelf().with([-1, -1, 0], KANTHAL_COIL);
        assert!(!match_shell(&bench, [0, 0, 0], COKE_OVEN_SHELL).formed);
    }

    #[test]
    fn the_three_by_three_by_four_add_on_shell_matches() {
        let members = pyrolyse_shape_fixture();
        let bricks = members
            .iter()
            .filter(|member| member.block == BRONZE_PLATED_BRICKS)
            .count();
        let coils = members
            .iter()
            .filter(|member| member.block == CUPRONICKEL_COIL)
            .count();
        assert_eq!((bricks, coils), (17, 16), "the add-on's 33-member shell");
        let hatchable: Vec<_> = members
            .iter()
            .filter(|member| !member.hatches.is_empty())
            .map(|member| member.offset)
            .collect();
        assert_eq!(hatchable.len(), 2);

        // 17 bricks, 16 coils: the matcher accepts a mixed shell.
        let complete = ShellBench::complete_with([0, 0, 0], &members);
        let matched = match_shell(&complete, [0, 0, 0], &members);
        assert!(matched.formed, "{matched:?}");
        assert_eq!(matched.missing, 0);

        // Two of the coil positions hold an item hatch and a fluid hatch.
        let with_hatches = complete
            .clone()
            .with(hatchable[0], BRONZE_ITEM_INPUT_HATCH)
            .with(hatchable[1], BRONZE_FLUID_INPUT_HATCH);
        let matched = match_shell(&with_hatches, [0, 0, 0], &members);
        assert!(matched.formed, "{matched:?}");
        assert_eq!(matched.hatches.len(), 2);

        // One coil broken: one member missing, and it unforms.
        let broken = complete.clone().without([1, 2, 0]);
        let matched = match_shell(&broken, [0, 0, 0], &members);
        assert!(!matched.formed);
        assert_eq!(matched.missing, 1);

        // A brick where a coil belongs is still unformed.
        let wrong = complete.clone().with([1, 2, 0], BRONZE_PLATED_BRICKS);
        let matched = match_shell(&wrong, [0, 0, 0], &members);
        assert!(!matched.formed);
        assert_eq!(matched.missing, 1);

        // A hatch at a coil position that allows none is unformed too.
        let refused = complete.with([1, 2, 0], BRONZE_ITEM_OUTPUT_HATCH);
        let matched = match_shell(&refused, [0, 0, 0], &members);
        assert!(!matched.formed);
        assert_eq!(matched.missing, 1);
    }

    #[test]
    fn the_hatch_kinds_are_upstreams_four_blocks() {
        assert_eq!(HatchKind::ItemInput.block(), BRONZE_ITEM_INPUT_HATCH);
        assert_eq!(HatchKind::ItemOutput.block(), BRONZE_ITEM_OUTPUT_HATCH);
        assert_eq!(HatchKind::FluidInput.block(), BRONZE_FLUID_INPUT_HATCH);
        assert_eq!(HatchKind::FluidOutput.block(), BRONZE_FLUID_OUTPUT_HATCH);
        for kind in HatchKind::ALL {
            assert_eq!(HatchKind::from_block(kind.block()), Some(kind));
            assert_eq!(HatchKind::from_code(kind.code()), Some(kind));
            assert!(is_hatch(kind.block()));
        }
        assert_eq!(HatchKind::from_block("minecraft:bricks"), None);
        assert!(HatchKind::ItemInput.is_item() && HatchKind::ItemInput.is_input());
        assert!(HatchKind::ItemOutput.is_item() && !HatchKind::ItemOutput.is_input());
        assert!(!HatchKind::FluidOutput.is_item());
    }

    #[test]
    fn an_item_hatch_moves_a_whole_stack_and_respects_its_room() {
        assert_eq!(merge_count(("minecraft:coal", 3), None, 64), 3);
        assert_eq!(
            merge_count(("minecraft:coal", 100), None, 64),
            64,
            "one stack"
        );
        assert_eq!(
            merge_count(("minecraft:coal", 5), Some(("minecraft:coal", 62)), 64),
            2,
            "the target's last two"
        );
        assert_eq!(
            merge_count(("minecraft:coal", 5), Some(("minecraft:coke", 1)), 64),
            0,
            "another item does not mix"
        );
        assert_eq!(
            merge_count(("minecraft:coal", 5), Some(("minecraft:coal", 64)), 64),
            0,
            "a full slot takes nothing"
        );
        assert_eq!(merge_count(("minecraft:coal", 0), None, 64), 0);
    }

    #[test]
    fn the_shipped_coke_recipe_data_is_the_kind_another_mod_reads() {
        // The jar's two files, shipped under `data/modern_industrialization/
        // recipe/coke_oven/` and registered as a mod-readable recipe type; the
        // machine's table stays `COKE_RECIPES`, and this pins the two together
        // so the data and the run cannot drift apart.
        assert_eq!(COKE_RECIPE_FILES.len(), 2);
        let compact = |json: &str| -> String {
            json.chars()
                .filter(|c| !c.is_whitespace())
                .collect::<String>()
        };
        for (id, json) in COKE_RECIPE_FILES {
            let text = compact(json);
            assert!(
                id.starts_with("modern_industrialization:coke_oven/"),
                "{id} is a coke oven recipe file"
            );
            assert!(
                text.contains(r#""type":"modern_industrialization:coke_oven""#),
                "{id} names the registered kind: {text}"
            );
            assert!(text.contains(r#""eu":2"#), "{id} is 2 EU/t");
            assert!(text.contains(r#""duration":600"#), "{id} is 600 ticks");
        }
        let coke = compact(COKE_RECIPE_FILES[0].1);
        assert!(coke.contains(r#""item":"minecraft:coal""#));
        assert!(coke.contains(r#""item":"modern_industrialization:coke""#));
        let dust = compact(COKE_RECIPE_FILES[1].1);
        assert!(dust.contains(r#""tag":"c:dusts/coal""#));
        assert!(dust.contains(r#""item":"modern_industrialization:coke_dust""#));
        // The table's first row is the coal run, the second the coal-dust run.
        assert_eq!(COKE_RECIPES[0].inputs[0].item, "minecraft:coal");
        assert_eq!(
            COKE_RECIPES[1].inputs[0].item,
            "modern_industrialization:coal_dust"
        );
        assert_eq!(COKE_RECIPES[0].eu, 2);
        assert_eq!(COKE_RECIPES[0].duration, 600);
        assert_eq!(COKE_RECIPE_KIND, "modern_industrialization:coke_oven");
    }

    // -----------------------------------------------------------------------
    // Slice 4: the energy hatch family
    // -----------------------------------------------------------------------

    #[test]
    fn the_energy_hatch_family_is_the_five_cable_tiers() {
        // `MultiblockHatches.registerEnergyHatches(CableTier)` registers one pair per
        // tier, and `EnergyHatch`'s buffer is `30 * 20 * tier.getEu()` EU, i.e. 600 EU
        // per tier EU. At this mod's own 1 EU = 10 FE that is 6,000 FE per tier EU.
        let want = [
            (HatchTier::Lv, "lv", 32, 192_000),
            (HatchTier::Mv, "mv", 128, 768_000),
            (HatchTier::Hv, "hv", 1_024, 6_144_000),
            (HatchTier::Ev, "ev", 8_192, 49_152_000),
            // 600 x 128,000,000 EU = 76,800,000,000 EU = 768,000,000,000 FE, past the
            // pin's 32-bit FE buffer: the port keeps the largest it can (i32::MAX FE,
            // 214,748,364 EU) and `notes.md` records the difference.
            (
                HatchTier::Superconductor,
                "superconductor",
                128_000_000,
                i32::MAX,
            ),
        ];
        assert_eq!(
            HatchTier::ALL.len(),
            want.len(),
            "the jar's five cable tiers"
        );
        for (tier, prefix, eu, capacity) in want {
            assert_eq!(tier.prefix(), prefix);
            assert_eq!(tier.eu(), eu, "{prefix} EU");
            assert_eq!(tier.capacity_fe(), capacity, "{prefix} FE");
        }
    }

    #[test]
    fn every_hatch_kind_has_one_block_and_one_saved_code() {
        let kinds: Vec<HatchKind> = HatchKind::every().collect();
        assert_eq!(kinds.len(), 14, "4 item/fluid + 5 tiers x 2 directions");
        for (index, kind) in kinds.iter().enumerate() {
            assert!(
                kinds[..index]
                    .iter()
                    .all(|other| other.block() != kind.block()),
                "{} twice",
                kind.block()
            );
            assert!(kind.block().starts_with("modern_industrialization:"));
            assert_eq!(HatchKind::from_block(kind.block()), Some(*kind));
            assert_eq!(kind.code(), i32::try_from(index).unwrap());
            assert_eq!(HatchKind::from_code(kind.code()), Some(*kind));
            assert!(is_hatch(kind.block()));
        }
        assert_eq!(HatchKind::from_block("modern_industrialization:nope"), None);
        assert_eq!(HatchKind::from_code(14), None);
        // The four kinds slice 3 shipped keep their ids and their saved codes.
        assert_eq!(HatchKind::ItemInput.code(), 0);
        assert_eq!(HatchKind::ItemOutput.code(), 1);
        assert_eq!(HatchKind::FluidInput.code(), 2);
        assert_eq!(HatchKind::FluidOutput.code(), 3);
    }

    #[test]
    fn the_energy_hatch_blocks_are_upstreams_ten_ids() {
        let input = HatchKind::EnergyHatch {
            tier: HatchTier::Lv,
            input: true,
        };
        let output = HatchKind::EnergyHatch {
            tier: HatchTier::Lv,
            input: false,
        };
        assert_eq!(
            input.block(),
            "modern_industrialization:lv_energy_input_hatch"
        );
        assert_eq!(
            output.block(),
            "modern_industrialization:lv_energy_output_hatch"
        );
        assert_eq!(
            HatchKind::EnergyHatch {
                tier: HatchTier::Superconductor,
                input: false,
            }
            .block(),
            "modern_industrialization:superconductor_energy_output_hatch"
        );
        // `registerEnergyHatches` names the block `<tier>_energy_<input|output>_hatch`.
        for kind in HatchKind::ENERGY {
            let Some(HatchKind::EnergyHatch { tier, input }) = Some(kind) else {
                panic!("{kind:?} is not an energy hatch");
            };
            let direction = if input {
                "energy_input"
            } else {
                "energy_output"
            };
            assert_eq!(
                kind.block(),
                format!(
                    "modern_industrialization:{}_{direction}_hatch",
                    tier.prefix()
                )
            );
        }
    }

    #[test]
    fn an_energy_hatch_is_a_direction_and_a_tier_but_never_an_item_hatch() {
        let input = HatchKind::EnergyHatch {
            tier: HatchTier::Mv,
            input: true,
        };
        let output = HatchKind::EnergyHatch {
            tier: HatchTier::Mv,
            input: false,
        };
        assert!(input.is_energy() && output.is_energy());
        assert!(input.is_input() && !output.is_input());
        assert!(!input.is_item() && !output.is_item());
        assert_eq!(input.capacity_fe(), 768_000);
        assert_eq!(output.capacity_fe(), 768_000);
        assert_eq!(HatchKind::ItemInput.capacity_fe(), 0);
        assert!(!HatchKind::ItemInput.is_energy());
    }

    #[test]
    fn a_hatchable_coke_oven_position_takes_an_energy_hatch_of_any_tier() {
        for member in COKE_OVEN_SHELL
            .iter()
            .filter(|member| !member.hatches.is_empty())
        {
            for kind in HatchKind::ENERGY {
                assert!(member.allows(kind), "{kind:?} at {:?}", member.offset);
            }
        }
        // The middle ring keeps `HatchFlags.NO_HATCH`: bricks only, energy or not.
        let ring = COKE_OVEN_SHELL
            .iter()
            .find(|member| member.offset == [1, 0, 0])
            .expect("the middle ring's +x position");
        assert!(!ring.allows(HatchKind::EnergyHatch {
            tier: HatchTier::Lv,
            input: true,
        }));
    }

    #[test]
    fn an_energy_hatch_forms_the_coke_oven_and_its_tier_is_recorded() {
        let bench = shelf()
            .with(
                [-1, -1, 0],
                "modern_industrialization:lv_energy_input_hatch",
            )
            .with([1, 1, 2], "modern_industrialization:ev_energy_output_hatch");
        let matched = match_shell(&bench, [0, 0, 0], COKE_OVEN_SHELL);
        assert!(matched.formed, "{matched:?}");
        assert_eq!(matched.missing, 0);
        assert_eq!(
            matched.hatches,
            vec![
                (
                    [-1, -1, 0],
                    HatchKind::EnergyHatch {
                        tier: HatchTier::Lv,
                        input: true
                    }
                ),
                (
                    [1, 1, 2],
                    HatchKind::EnergyHatch {
                        tier: HatchTier::Ev,
                        input: false
                    }
                ),
            ],
            "the per-position kinds are the layout the controller saves"
        );
        // A player swapping a brick for an energy hatch changes a match.
        assert!(shell_relevant(
            "minecraft:bricks",
            "modern_industrialization:lv_energy_input_hatch"
        ));
        assert!(shell_relevant(
            "modern_industrialization:hv_energy_output_hatch",
            "minecraft:air"
        ));
    }

    #[test]
    fn the_add_on_shell_takes_an_energy_hatch_at_its_hatch_positions() {
        // `industrialization_overdrive`'s pyrolyse shell flags ENERGY_INPUT at its
        // hatch positions (FINDINGS.md:52), so the fixture must accept one.
        let members = pyrolyse_shape_fixture();
        let hatchable: Vec<[i32; 3]> = members
            .iter()
            .filter(|member| !member.hatches.is_empty())
            .map(|member| member.offset)
            .collect();
        assert_eq!(hatchable.len(), 2);
        let bench = ShellBench::complete_with([0, 0, 0], &members)
            .with(
                hatchable[0],
                "modern_industrialization:mv_energy_input_hatch",
            )
            .with(
                hatchable[1],
                "modern_industrialization:lv_energy_input_hatch",
            );
        let matched = match_shell(&bench, [0, 0, 0], &members);
        assert!(matched.formed, "{matched:?}");
        assert_eq!(matched.hatches.len(), 2);
    }

    #[test]
    fn the_pump_takes_only_what_the_target_holds_room_for() {
        assert_eq!(
            move_energy_count(5_000, 0, COKE_CAPACITY_FE, i32::MAX),
            5_000
        );
        assert_eq!(
            move_energy_count(192_000, 30_000, COKE_CAPACITY_FE, i32::MAX),
            2_000,
            "the controller's own buffer is the room"
        );
        assert_eq!(
            move_energy_count(1_000, COKE_CAPACITY_FE, COKE_CAPACITY_FE, i32::MAX),
            0,
            "a full target takes nothing"
        );
        assert_eq!(move_energy_count(0, 5, COKE_CAPACITY_FE, i32::MAX), 0);
        assert_eq!(
            move_energy_count(-5, 0, COKE_CAPACITY_FE, i32::MAX),
            0,
            "a corrupt buffer moves nothing"
        );
        assert_eq!(
            move_energy_count(5_000, 0, COKE_CAPACITY_FE, 1_000),
            1_000,
            "a limit caps the move"
        );
    }

    #[test]
    fn an_energy_hatch_pays_for_a_whole_coke_run_and_twenty_fe_short_of_one_more() {
        // The controller's buffer is fed from the hatch (`move_energy_count`) and the
        // run draws from it: exactly 12,000 FE makes one coke and leaves nothing.
        let mut oven = CokeOven {
            formed: true,
            ..Default::default()
        };
        let moved = move_energy_count(12_000, 0, COKE_CAPACITY_FE, i32::MAX);
        assert_eq!(oven.insert_fe(moved), 12_000);
        let mut bench = Bench::new(&[("minecraft:coal", 1)]);
        for _ in 0..600 {
            oven.tick(&mut bench, &mut always);
        }
        assert_eq!(oven.energy, 0, "the run took exactly what the hatch held");
        assert_eq!(bench.out("modern_industrialization:coke"), 1);

        // One tick's worth short (11,980 FE) leaves the run 2 EU from its end: no coke.
        let mut oven = CokeOven {
            formed: true,
            ..Default::default()
        };
        assert_eq!(
            oven.insert_fe(move_energy_count(11_980, 0, COKE_CAPACITY_FE, i32::MAX)),
            11_980
        );
        let mut bench = Bench::new(&[("minecraft:coal", 1)]);
        for _ in 0..620 {
            oven.tick(&mut bench, &mut always);
        }
        assert_eq!(bench.out("modern_industrialization:coke"), 0);
        assert_eq!(oven.energy, 0);
        assert_eq!(oven.status().progress_permille, 998, "1198 of 1200 EU");
        assert!(oven.active.is_some(), "the run is stalled, not over");
    }

    #[test]
    fn the_oven_state_survives_a_save_and_a_load() {
        let mut oven = CokeOven {
            formed: true,
            ..Default::default()
        };
        oven.insert_fe(20_000);
        let mut bench = Bench::new(&[("minecraft:coal", 1)]);
        for _ in 0..300 {
            oven.tick(&mut bench, &mut always);
        }
        let reloaded = CokeOven::from_saved(&oven.saved());
        assert_eq!(reloaded, oven, "the run, the buffer and formed are saved");
        assert!(reloaded.formed);
    }
}
