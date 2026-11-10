// Game-side glue for Modern Industrialization slice 1 (the electric macerator).
//
// Injected into `crate::game` as `modern_industrialization_state`; it needs the
// game, so it is not part of the standalone library (only `runtime.rs` is).
//
// The block entity holds what upstream's `MachineInventoryComponent` holds for
// this machine: one item input slot (0) and four item output slots (1..=4), and
// its FE buffer is the block entity's own (`native_block_entity_energy`, API
// 142), whose amount is the int `energy` in the saved compound, so it survives
// a save and the block's own tick reads it there. The run in progress
// (`used_eu`, `recipe_energy`, `recipe_max_eu`, `efficiency`,
// `max_efficiency`, `active`) is saved under upstream's own keys, so a reload
// resumes the run.
//
// `ModContainer` has no per-slot `may_place` for a player's menu at this pin and
// `menu_slot_rules` reaches only content menu types, so this slice declares no
// menu: hoppers and pipes insert into slot 0 (`inputs`) and pull from slots
// 1..=4 (`outputs`), which is the automation path upstream's machine exposes
// through `registerItemApi`.

use horizon_client::containers::{ModBlockEntity, ModContainer, ModEnergyBuffer};
use horizon_client::items::{ItemStack, Items};
use horizon_world::nbt::{Compound, JavaString, Tag};

use crate::game::modern_industrialization::{
    CAPACITY_FE, FE_PER_EU, Macerator, OUTPUT_SLOTS, PROBABILITY_SCALE, Saved, Slots, Status,
    recipe,
};
use crate::game::native_server::{NativeCommand, NativeServerPlayer};

/// The slice's one block (`SingleBlockCraftingMachines`, TIER_ELECTRIC).
const MACERATOR: &str = "modern_industrialization:electric_macerator";

/// The item input slot (`MachineRecipeType.MACERATOR.withItemInputs()`).
const INPUT_SLOT: usize = 0;

/// The first of the four item output slots (`itemOutputCount = 4`).
const FIRST_OUTPUT: usize = 1;

/// A saved int of the block entity's compound, 0 when absent.
fn be_int(entity: &ModBlockEntity, key: &str) -> i32 {
    match entity.nbt.get(&JavaString::from(key)) {
        Some(Tag::Int(value)) => *value,
        _ => 0,
    }
}

fn set_be_int(entity: &mut ModBlockEntity, key: &str, value: i32) {
    entity.nbt.insert(JavaString::from(key), Tag::Int(value));
}

/// A saved long of the block entity's compound, 0 when absent.
fn be_long(entity: &ModBlockEntity, key: &str) -> i64 {
    match entity.nbt.get(&JavaString::from(key)) {
        Some(Tag::Long(value)) => *value,
        _ => 0,
    }
}

fn set_be_long(entity: &mut ModBlockEntity, key: &str, value: i64) {
    entity.nbt.insert(JavaString::from(key), Tag::Long(value));
}

/// The saved active recipe id, if any (`CrafterComponent.readNbt` writes it as a
/// string and looks it up again on load).
fn be_recipe(entity: &ModBlockEntity) -> Option<String> {
    match entity.nbt.get(&JavaString::from("activeRecipe")) {
        Some(Tag::String(value)) => Some(value.to_string_lossy()),
        _ => None,
    }
}

fn set_be_recipe(entity: &mut ModBlockEntity, id: Option<&str>) {
    match id {
        Some(id) => {
            entity
                .nbt
                .insert(JavaString::from("activeRecipe"), Tag::String(JavaString::from(id)));
        }
        None => {
            entity.nbt.remove(&JavaString::from("activeRecipe"));
        }
    }
}

/// The saved machine; a fresh entity starts empty, as `EnergyComponent` and
/// `CrafterComponent` do.
fn macerator_state(entity: &ModBlockEntity) -> Macerator {
    Macerator::from_saved(&Saved {
        energy: be_int(entity, "energy"),
        used_eu: be_long(entity, "usedEnergy"),
        recipe_energy: be_long(entity, "recipeEnergy"),
        recipe_max_eu: be_long(entity, "recipeMaxEu"),
        efficiency: be_int(entity, "efficiencyTicks"),
        max_efficiency: be_int(entity, "maxEfficiencyTicks"),
        active: be_recipe(entity).and_then(|id| recipe(&id).map(|recipe| recipe.id)),
    })
}

fn store(entity: &mut ModBlockEntity, state: &Macerator) {
    let saved = state.saved();
    set_be_int(entity, "energy", saved.energy);
    set_be_long(entity, "usedEnergy", saved.used_eu);
    set_be_long(entity, "recipeEnergy", saved.recipe_energy);
    set_be_long(entity, "recipeMaxEu", saved.recipe_max_eu);
    set_be_int(entity, "efficiencyTicks", saved.efficiency);
    set_be_int(entity, "maxEfficiencyTicks", saved.max_efficiency);
    set_be_recipe(entity, saved.active);
}

/// `BlockEntity.setChanged`: the chunk is written at the next save.
fn mark_modified(game: &mut crate::game::Game<'_>, pos: [i32; 3]) {
    game.world.modified.insert((pos[0] >> 4, pos[2] >> 4));
}

/// The block entity's slots seen through [`Slots`]: slot 0 in, slots 1..=4 out,
/// each one stack of the item's own `max_stack_size`.
struct Container<'a> {
    entity: &'a mut ModBlockEntity,
    registry: &'a Items,
}

impl Container<'_> {
    fn slot(&self, index: usize) -> Option<&ItemStack> {
        self.entity.items.get(index).and_then(Option::as_ref)
    }

    fn stack_size(&self, item: &str) -> u32 {
        self.registry
            .get(item)
            .map_or(crate::game::modern_industrialization::STACK_SIZE, |info| {
                info.max_stack
            })
    }
}

impl Slots for Container<'_> {
    fn count(&self, item: &str) -> u32 {
        self.slot(INPUT_SLOT)
            .filter(|stack| &*stack.item == item)
            .map_or(0, |stack| stack.count)
    }

    fn take(&mut self, item: &str, amount: u32) -> u32 {
        let Some(stack) = self.entity.items.get_mut(INPUT_SLOT).and_then(Option::as_mut) else {
            return 0;
        };
        if &*stack.item != item {
            return 0;
        }
        let taken = stack.count.min(amount);
        stack.count -= taken;
        if stack.count == 0 {
            self.entity.items[INPUT_SLOT] = None;
        }
        taken
    }

    fn room(&self, item: &str, amount: u32) -> u32 {
        let mut room = 0;
        for index in FIRST_OUTPUT..FIRST_OUTPUT + OUTPUT_SLOTS {
            match self.slot(index) {
                Some(stack) if &*stack.item == item => {
                    room += self.stack_size(item).saturating_sub(stack.count);
                }
                Some(_) => {}
                None => room += self.stack_size(item),
            }
        }
        room.min(amount)
    }

    fn put(&mut self, item: &str, amount: u32) -> u32 {
        let mut left = amount;
        for index in FIRST_OUTPUT..FIRST_OUTPUT + OUTPUT_SLOTS {
            if left == 0 {
                break;
            }
            let stack_size = self.stack_size(item);
            match self.entity.items.get_mut(index).and_then(Option::as_mut) {
                Some(stack) if &*stack.item == item => {
                    let fits = stack_size.saturating_sub(stack.count).min(left);
                    stack.count += fits;
                    left -= fits;
                }
                Some(_) => {}
                None => {
                    let fits = stack_size.min(left);
                    self.entity.items[index] = Some(ItemStack::new(item, fits));
                    left -= fits;
                }
            }
        }
        left
    }
}

/// The macerator gets a block entity (`native_block_entity`, API 15).
#[allow(private_interfaces)]
pub fn on_block_entity(block: &str, entity: &mut Option<(String, Compound)>) {
    if block == MACERATOR {
        *entity = Some((block.to_owned(), Compound::new()));
    }
}

/// The macerator ticks on the server (`native_block_entity_ticks`, API 15).
#[allow(private_interfaces)]
pub fn on_block_entity_ticks(block: &str, ticks: &mut bool) {
    if block == MACERATOR {
        *ticks = true;
    }
}

/// The item slots (`native_block_entity_container`, API 16): one input a hopper
/// may fill, four outputs a hopper may empty. No menu is declared (see the
/// header): the pin has no per-slot rule for a player's menu.
#[allow(private_interfaces)]
pub fn on_block_entity_container(block: &str, container: &mut Option<ModContainer>) {
    if block != MACERATOR {
        return;
    }
    *container = Some(ModContainer {
        slots: FIRST_OUTPUT + OUTPUT_SLOTS,
        faces: std::array::from_fn(|_| (0..FIRST_OUTPUT + OUTPUT_SLOTS).collect()),
        inputs: vec![INPUT_SLOT],
        outputs: (FIRST_OUTPUT..FIRST_OUTPUT + OUTPUT_SLOTS).collect(),
        menu: None,
    });
}

/// The FE buffer (`native_block_entity_energy`, API 142): 32,000 FE any side may
/// fill, and nothing may drain — upstream's electric machine is insert-only
/// (`EnergyComponent.buildInsertable`).
#[allow(private_interfaces)]
pub fn on_block_entity_energy(block: &str, energy: &mut Option<ModEnergyBuffer>) {
    *energy = (block == MACERATOR).then_some(ModEnergyBuffer {
        capacity: CAPACITY_FE,
        max_insert: CAPACITY_FE,
        max_extract: 0,
        insert_faces: [true; 6],
        extract_faces: [false; 6],
    });
}

/// One server tick (`native_block_entity_tick`, API 17): feed the run, finish
/// it into the output slots, and move the overclock ticks.
#[allow(private_interfaces)]
pub fn on_block_entity_tick(
    game: &mut crate::game::Game<'_>,
    pos: [i32; 3],
    entity: &mut ModBlockEntity,
) {
    if entity.id != MACERATOR {
        return;
    }
    if entity.items.len() < FIRST_OUTPUT + OUTPUT_SLOTS {
        entity.items.resize(FIRST_OUTPUT + OUTPUT_SLOTS, None);
    }
    let before = macerator_state(entity);
    let mut state = before;
    let mut roll = roll_for(game.world.game_time as u64, pos);
    let tick = {
        let mut container = Container {
            entity,
            registry: game.res.items(),
        };
        state.tick(&mut container, &mut roll)
    };
    store(entity, &state);
    if tick.crafted.is_some() || state != before {
        mark_modified(game, pos);
    }
}

/// A roll in `0..PROBABILITY_SCALE`, from the tick and the block's position.
/// Every carried recipe is certain, so this only decides a future chance
/// output; upstream draws from `ThreadLocalRandom`.
fn roll_for(game_time: u64, pos: [i32; 3]) -> impl FnMut() -> u32 + use<> {
    let mut seed = game_time
        ^ (u64::from(pos[0] as u32) << 32)
        ^ (u64::from(pos[1] as u32) << 16)
        ^ u64::from(pos[2] as u32);
    move || {
        seed = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut value = seed;
        value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        value ^= value >> 31;
        u32::try_from(value % u64::from(PROBABILITY_SCALE)).unwrap_or(0)
    }
}

/// What `/modern_industrialization info` prints, read from the compound.
struct Report {
    id: String,
    status: Status,
}

/// `/modern_industrialization info|energy|efficiency` (`register_commands`,
/// API 8). `energy` and `efficiency` are harness controls, as `/powah set` and
/// `/mekanismgenerators energy` are — upstream has no such command.
#[allow(private_interfaces)]
pub fn on_register_commands(commands: &mut Vec<NativeCommand>) {
    commands.push(NativeCommand::new(
        "modern_industrialization info <pos:block_pos>",
    ));
    commands.push(NativeCommand::op(
        "modern_industrialization energy <amount:integer> <pos:block_pos>",
        2,
    ));
    commands.push(NativeCommand::op(
        "modern_industrialization efficiency <amount:integer> <pos:block_pos>",
        2,
    ));
}

fn parse_pos(words: &[&str]) -> Option<[i32; 3]> {
    Some([
        words.first()?.parse().ok()?,
        words.get(1)?.parse().ok()?,
        words.get(2)?.parse().ok()?,
    ])
}

/// The `/modern_industrialization` commands (`perform_command`, API 8).
#[allow(private_interfaces)]
pub fn on_perform_command(
    game: &mut crate::game::Game<'_>,
    _player: &NativeServerPlayer,
    command: &str,
    ran: &mut bool,
) {
    let words: Vec<&str> = command.split_whitespace().collect();
    match words.as_slice() {
        ["modern_industrialization", "info", x, y, z] => {
            *ran = true;
            let Some(pos) = parse_pos(&[x, y, z]) else {
                game.native_feedback(horizon_client::text::Text::literal(
                    "Usage: /modern_industrialization info <x> <y> <z>",
                ));
                return;
            };
            let Some(report) = game.native_with_mod_block_entity(pos, |entity| Report {
                id: entity.id.clone(),
                status: macerator_state(entity).status(),
            }) else {
                game.native_feedback(horizon_client::text::Text::literal(
                    "No block entity there.",
                ));
                return;
            };
            let now = game.world.game_time;
            let Report { id, status } = report;
            let active = status.active.unwrap_or("none");
            let eu = status.energy / FE_PER_EU;
            let fe_per_tick = status.recipe_max_eu * i64::from(FE_PER_EU);
            game.native_feedback(horizon_client::text::Text::literal(format!(
                "{id} at {},{},{}: {}/{} FE ({} EU), progress {}/1000, {fe_per_tick} FE/t, efficiency {}/{}, active {active}, t={now}",
                pos[0],
                pos[1],
                pos[2],
                status.energy,
                CAPACITY_FE,
                eu,
                status.progress_permille,
                status.efficiency,
                status.max_efficiency,
            )));
        }
        ["modern_industrialization", "energy", amount, x, y, z] => {
            *ran = true;
            let Some(pos) = parse_pos(&[x, y, z]) else {
                game.native_feedback(horizon_client::text::Text::literal(
                    "Usage: /modern_industrialization energy <amount> <x> <y> <z>",
                ));
                return;
            };
            let Ok(amount) = amount.parse::<i32>() else {
                game.native_feedback(horizon_client::text::Text::literal(
                    "The amount must be a whole number of FE.",
                ));
                return;
            };
            let Some(after) = game.native_with_mod_block_entity(pos, |entity| {
                let mut state = macerator_state(entity);
                // Set the buffer: empty it, then take as much as its capacity
                // lets in (`Macerator::insert_fe`).
                state.energy = 0;
                let after = state.insert_fe(amount.max(0));
                store(entity, &state);
                after
            }) else {
                game.native_feedback(horizon_client::text::Text::literal(
                    "No block entity there.",
                ));
                return;
            };
            game.native_feedback(horizon_client::text::Text::literal(format!(
                "Energy set to {after} FE."
            )));
        }
        ["modern_industrialization", "efficiency", amount, x, y, z] => {
            *ran = true;
            let Some(pos) = parse_pos(&[x, y, z]) else {
                game.native_feedback(horizon_client::text::Text::literal(
                    "Usage: /modern_industrialization efficiency <amount> <x> <y> <z>",
                ));
                return;
            };
            let Ok(amount) = amount.parse::<i32>() else {
                game.native_feedback(horizon_client::text::Text::literal(
                    "The amount must be a whole number of efficiency ticks.",
                ));
                return;
            };
            let Some(after) = game.native_with_mod_block_entity(pos, |entity| {
                let mut state = macerator_state(entity);
                state.efficiency = amount.max(0);
                state.max_efficiency = state.max_efficiency.max(state.efficiency);
                store(entity, &state);
                state.efficiency
            }) else {
                game.native_feedback(horizon_client::text::Text::literal(
                    "No block entity there.",
                ));
                return;
            };
            game.native_feedback(horizon_client::text::Text::literal(format!(
                "Efficiency set to {after} ticks."
            )));
        }
        _ => {}
    }
}
