//! Modern Industrialization for Horizon — the electric macerator.
//!
//! The port's slice 1 is one block, `modern_industrialization:electric_macerator`: a 32,000 FE
//! buffer, one item input slot, four item output slots, and upstream's `CrafterComponent` tick
//! with the LV tier's overclock. Every number (the buffer, the recipe table, the rate, the
//! overclock) lives in [`runtime`], which is pure; this file is the glue that connects it to the
//! game.
//!
//! Upstream's machine is a `MachineBlockEntity` plus three NeoForge capabilities (its item
//! handler, its energy storage and its GUI). The SDK's shape is typed: the block entity type is
//! declared once in [`Hooks::register`], and the moments the machine answers — the saved data it
//! starts with, that it ticks, one tick — are its Part's methods, so they are asked only for this
//! block entity type and never for another mod's machine. The item slots and the FE buffer are the
//! game's own default capabilities (`Registrar::block_container`, `Registrar::block_energy`),
//! which is what a hopper, a pipe or another mod's cable reach.
//!
//! `/modern_industrialization info|energy|efficiency` is the readout and the harness controls; the
//! two mutation usages do not exist upstream and are recorded as additions in `notes.md`.
pub mod runtime;

use mod_api::{
    self, BlockEntityEvent, BlockEntityPart, BlockEntityRef, BlockEntityTickEvent,
    BlockEntityTickReply, BlockEntityTicksEvent, BlockPart, Command, CommandArgumentValue,
    CommandInvocation, Compound, Flow, Hooks, Host, Identifier, ItemKey, ItemStack, LevelSide,
    ModContainer, ModEnergyBuffer, RegisterCommandsEvent, Registrar, RegistrationContext,
    RunModCommandEvent, ServerHost, Tag, Text,
};

use runtime::{
    CAPACITY_FE, FE_PER_EU, MACERATOR_ID, Macerator, OUTPUT_SLOTS, PROBABILITY_SCALE, STACK_SIZE,
    Saved, Slots, Status, is_macerator, recipe,
};

/// The item input slot (`MachineRecipeType.MACERATOR.withItemInputs()`).
const INPUT_SLOT: usize = 0;

/// The first of the four item output slots (`itemOutputCount = 4`).
const FIRST_OUTPUT: usize = 1;

/// The machine's slots in all: one input and four outputs.
const SLOTS: usize = FIRST_OUTPUT + OUTPUT_SLOTS;

/// The block entity type's name, `modern_industrialization:electric_macerator`: the same path as
/// the block's, so one string names both halves in this mod's namespace.
const ENTITY_TYPE: &str = "electric_macerator";

/// The marker this port keeps in its first data slot — no menu shows it — meaning "the engine has
/// sized this entity's item slots". `data` is the one payload field nothing saves, so it is empty
/// on a fresh entity and after every reload, which is exactly when the slots have to be grown.
const SLOTS_READY: i32 = 1;

/// The mod: no settings of its own, so its state is the game's.
#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    /// Declare the machine: its block entity type with the Part that answers for it, and the two
    /// capabilities the block exposes (its slots and its buffer).
    fn register(&self, r: &mut dyn Registrar, _cx: &RegistrationContext) {
        let block = r.block_key(MACERATOR_ID);
        let kind = r
            .block_entity_type(ENTITY_TYPE)
            .blocks(&[ENTITY_TYPE])
            .build();
        // The type is this mod's own and the Part is attached once, so neither refusal (a vanilla
        // type, or the same Part twice) can happen here.
        let _ = r.block_entity_part(&kind, Machine);
        r.block_container(
            &block,
            ModContainer {
                slots: SLOTS,
                faces: std::array::from_fn(|_| (0..SLOTS).collect()),
                inputs: vec![INPUT_SLOT],
                outputs: (FIRST_OUTPUT..SLOTS).collect(),
                ..Default::default()
            },
        );
        // Upstream's electric machine is insert-only (`EnergyComponent.buildInsertable`): any face
        // fills the buffer and nothing drains it through the capability.
        r.block_energy(
            &block,
            ModEnergyBuffer {
                capacity: CAPACITY_FE,
                max_insert: CAPACITY_FE,
                max_extract: 0,
                insert_faces: [true; 6],
                extract_faces: [false; 6],
            },
        );
    }

    /// `/modern_industrialization info|energy|efficiency`: the readout and the harness controls.
    fn register_commands(&self, _host: &mut dyn ServerHost, event: &mut RegisterCommandsEvent<'_>) {
        event.commands.push(Command::new(
            "modern_industrialization info <pos:block_pos>",
        ));
        event.commands.push(Command::op(
            "modern_industrialization energy <amount:integer> <pos:block_pos>",
            2,
        ));
        event.commands.push(Command::op(
            "modern_industrialization efficiency <amount:integer> <pos:block_pos>",
            2,
        ));
    }

    /// The `/modern_industrialization` command bodies (upstream's executor; NeoForge's
    /// `CommandEvent` only watches, the tree node runs).
    fn run_mod_command(
        &self,
        host: &mut dyn ServerHost,
        event: &mut RunModCommandEvent<'_>,
    ) -> Flow {
        let literals = &event.command.literals;
        if literals.first().map(String::as_str) != Some("modern_industrialization") {
            return Flow::Continue;
        }
        match literals.get(1).map(String::as_str) {
            Some("info") => {
                report(host, event.command);
                Flow::Cancel
            }
            Some("energy") => {
                set(host, event.command, Alter::Energy);
                Flow::Cancel
            }
            Some("efficiency") => {
                set(host, event.command, Alter::Efficiency);
                Flow::Cancel
            }
            _ => Flow::Continue,
        }
    }
}

/// The macerator's block entity Part: the data a fresh machine starts with, that it ticks, and one
/// tick of `CrafterComponent`.
struct Machine;

#[mod_api::horizon_part]
impl BlockPart for Machine {}

#[mod_api::horizon_part]
impl BlockEntityPart for Machine {
    /// A fresh machine starts empty, as `EnergyComponent` and `CrafterComponent` do. The machine
    /// keeps its run under upstream's own keys, so a reload resumes it (see [`Saved`]).
    fn block_entity(
        &self,
        _level: LevelSide,
        _event: &mut BlockEntityEvent<'_>,
    ) -> Option<Compound> {
        Some(Compound::new())
    }

    /// The macerator ticks on the server (`EntityBlock.getTicker`). Asked for each of the block's
    /// states when a world opens.
    fn block_entity_ticks(&self, _host: &dyn Host, event: &mut BlockEntityTicksEvent<'_>) {
        event.ticks = true;
    }

    /// One tick of the machine: start a run if the buffer can pay for it, feed it, finish it into
    /// the output slots, and move the overclock ticks.
    fn block_entity_tick(&self, host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
        let pos = event.pos;
        let fields = &mut *event.entity;
        if fields.items.len() < SLOTS {
            fields.items.resize(SLOTS, None);
        }
        // The engine sizes a mod block entity's own `items` to the declared container only on a
        // tick that answers `Changed`: `BlockEntityTickReply::Unchanged` discards the payload, and
        // a fresh or reloaded entity starts with an empty `items` (the save keeps only the
        // occupied slots). So the first tick of each has to write back, or the machine exposes no
        // slots at all and nothing can ever be inserted into it. `data` is the one payload field
        // nothing saves, so an empty `data` is exactly "this entity has not been materialised in
        // this session yet"; the tick sets it and lets the write-back happen once.
        let slots_ready = !fields.data.is_empty();
        if !slots_ready {
            fields.data = vec![SLOTS_READY];
        }
        let before = macerator_state(&fields.nbt);
        let mut state = before;
        // The tick event carries no level, so the roll is seeded from the machine's position and
        // the buffer it holds: every carried recipe is certain, and this only decides a future
        // chance output (`ThreadLocalRandom` upstream).
        let mut seed = roll_seed(pos, before.energy);
        let mut roll = || next_roll(&mut seed);
        let tick = {
            let mut slots = SlotsView {
                host,
                items: &mut fields.items,
            };
            state.tick(&mut slots, &mut roll)
        };
        if tick.active || tick.crafted.is_some() || state != before {
            store(&mut fields.nbt, &state);
        } else if slots_ready {
            // Nothing ran and nothing moved, and the slots are already there: no chunk dirty, no
            // comparator update, and the original payload is kept
            // (`BlockEntityTickReply::Unchanged`).
            *event.reply = BlockEntityTickReply::Unchanged;
        }
    }
}

/// The block entity's slots seen through [`Slots`]: slot 0 in, slots 1..=4 out, each one stack of
/// the item's own `max_stack_size`.
struct SlotsView<'a> {
    host: &'a dyn ServerHost,
    items: &'a mut Vec<Option<ItemStack>>,
}

impl SlotsView<'_> {
    fn host(&self) -> &dyn Host {
        self.host
    }

    fn slot(&self, index: usize) -> Option<&ItemStack> {
        self.items.get(index).and_then(Option::as_ref)
    }

    /// The item id a stack holds.
    fn id_of(&self, stack: &ItemStack) -> String {
        stack.item().id(self.host())
    }

    /// The key of an item id, or `None` for an id this world does not have.
    fn key(&self, item: &str) -> Option<ItemKey> {
        self.host().item_key(&Identifier::parse(item).ok()?)
    }

    fn stack_size(&self, item: &str) -> u32 {
        self.key(item)
            .and_then(|key| self.host().max_stack_size(key))
            .unwrap_or(STACK_SIZE)
    }
}

impl Slots for SlotsView<'_> {
    fn count(&self, item: &str) -> u32 {
        match self.slot(INPUT_SLOT) {
            Some(stack) if self.id_of(stack) == item => stack.count(),
            _ => 0,
        }
    }

    fn take(&mut self, item: &str, amount: u32) -> u32 {
        let Self { host, items } = self;
        let host: &dyn Host = &**host;
        let Some(stack) = items.get_mut(INPUT_SLOT).and_then(Option::as_mut) else {
            return 0;
        };
        let key = stack.item();
        let held = stack.count();
        if key.id(host) != item {
            return 0;
        }
        let taken = held.min(amount);
        items[INPUT_SLOT] = ItemStack::new(key, held - taken);
        taken
    }

    fn room(&self, item: &str, amount: u32) -> u32 {
        let mut room = 0;
        for index in FIRST_OUTPUT..SLOTS {
            match self.slot(index) {
                Some(stack) if self.id_of(stack) == item => {
                    room += self.stack_size(item).saturating_sub(stack.count());
                }
                Some(_) => {}
                None => room += self.stack_size(item),
            }
        }
        room.min(amount)
    }

    fn put(&mut self, item: &str, amount: u32) -> u32 {
        let Self { host, items } = self;
        let host: &dyn Host = &**host;
        let Some(key) = Identifier::parse(item)
            .ok()
            .and_then(|id| host.item_key(&id))
        else {
            return amount;
        };
        let stack_size = host.max_stack_size(key).unwrap_or(STACK_SIZE);
        let mut left = amount;
        for index in FIRST_OUTPUT..SLOTS {
            if left == 0 {
                break;
            }
            match items.get_mut(index).and_then(Option::as_mut) {
                Some(stack) if stack.item() == key => {
                    let held = stack.count();
                    let fits = stack_size.saturating_sub(held).min(left);
                    items[index] = ItemStack::new(key, held + fits);
                    left -= fits;
                }
                Some(_) => {}
                None => {
                    let fits = stack_size.min(left);
                    items[index] = ItemStack::new(key, fits);
                    left -= fits;
                }
            }
        }
        left
    }
}

/// A saved int of the block entity's compound, 0 when absent.
fn be_int(nbt: &Compound, key: &str) -> i32 {
    match nbt.get(&key.into()) {
        Some(Tag::Int(value)) => *value,
        _ => 0,
    }
}

fn set_be_int(nbt: &mut Compound, key: &str, value: i32) {
    nbt.insert(key.into(), Tag::Int(value));
}

/// A saved long of the block entity's compound, 0 when absent.
fn be_long(nbt: &Compound, key: &str) -> i64 {
    match nbt.get(&key.into()) {
        Some(Tag::Long(value)) => *value,
        _ => 0,
    }
}

fn set_be_long(nbt: &mut Compound, key: &str, value: i64) {
    nbt.insert(key.into(), Tag::Long(value));
}

/// The saved active recipe id (`CrafterComponent.readNbt` writes it as a string and looks it up
/// again on load); a recipe the table no longer holds is no run.
fn be_recipe(nbt: &Compound) -> Option<&'static str> {
    match nbt.get(&"activeRecipe".into()) {
        Some(Tag::String(value)) => {
            let id = value.to_string_lossy();
            recipe(&id).map(|found| found.id)
        }
        _ => None,
    }
}

fn set_be_recipe(nbt: &mut Compound, id: Option<&str>) {
    match id {
        Some(id) => {
            nbt.insert("activeRecipe".into(), Tag::String(id.into()));
        }
        None => {
            nbt.remove(&"activeRecipe".into());
        }
    }
}

/// The saved machine; a fresh compound is a fresh machine.
fn macerator_state(nbt: &Compound) -> Macerator {
    Macerator::from_saved(&Saved {
        energy: be_int(nbt, "energy"),
        used_eu: be_long(nbt, "usedEnergy"),
        recipe_energy: be_long(nbt, "recipeEnergy"),
        recipe_max_eu: be_long(nbt, "recipeMaxEu"),
        efficiency: be_int(nbt, "efficiencyTicks"),
        max_efficiency: be_int(nbt, "maxEfficiencyTicks"),
        active: be_recipe(nbt),
    })
}

/// Writes the machine back under upstream's own keys, so a reload resumes the run.
fn store(nbt: &mut Compound, state: &Macerator) {
    let saved = state.saved();
    set_be_int(nbt, "energy", saved.energy);
    set_be_long(nbt, "usedEnergy", saved.used_eu);
    set_be_long(nbt, "recipeEnergy", saved.recipe_energy);
    set_be_long(nbt, "recipeMaxEu", saved.recipe_max_eu);
    set_be_int(nbt, "efficiencyTicks", saved.efficiency);
    set_be_int(nbt, "maxEfficiencyTicks", saved.max_efficiency);
    set_be_recipe(nbt, saved.active);
}

/// A roll seed from the machine's position and buffer, which change as a run goes on.
fn roll_seed(pos: [i32; 3], energy: i32) -> u64 {
    (u64::from(pos[0] as u32) << 40)
        ^ (u64::from(pos[1] as u32) << 20)
        ^ u64::from(pos[2] as u32)
        ^ (u64::from(energy as u32) << 8)
}

/// A roll in `0..PROBABILITY_SCALE` (SplitMix64's finaliser over the seed).
fn next_roll(seed: &mut u64) -> u32 {
    *seed = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut value = *seed;
    value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^= value >> 31;
    u32::try_from(value % u64::from(PROBABILITY_SCALE)).unwrap_or(0)
}

/// Answers the command's source, as vanilla's success feedback does.
fn say(host: &mut dyn ServerHost, line: impl Into<String>) {
    let _ = host.commands().feedback(Text::literal(line.into()));
}

/// The block position argument.
fn block_pos(command: &CommandInvocation, name: &str) -> Option<[i32; 3]> {
    match command.argument(name)? {
        CommandArgumentValue::BlockPos(pos) => Some(*pos),
        _ => None,
    }
}

/// The integer argument of a usage whose parser already validated it.
fn integer(command: &CommandInvocation, name: &str) -> Option<i32> {
    match command.argument(name)? {
        CommandArgumentValue::Integer(value) => Some(*value),
        _ => None,
    }
}

/// The block entity the command's `pos` names, in the command's own execution level.
fn entity_at(
    host: &dyn Host,
    command: &CommandInvocation,
    pos: [i32; 3],
) -> Option<BlockEntityRef> {
    let dimension = command.dimension.id(host)?;
    let level = host.level(&dimension)?;
    host.block_entity(level, pos)
}

/// Whether the block entity is this slice's machine, answering its id when it is.
fn machine_id(host: &dyn Host, entity: BlockEntityRef) -> Option<String> {
    let id = entity.kind(host)?.id(host);
    is_macerator(&id).then_some(id)
}

/// `/modern_industrialization info <pos>`: what the machine holds, its run and its overclock.
fn report(host: &mut dyn ServerHost, command: &CommandInvocation) {
    let Some(pos) = block_pos(command, "pos") else {
        say(host, "Usage: /modern_industrialization info <x> <y> <z>");
        return;
    };
    let Some(entity) = entity_at(host, command, pos) else {
        say(host, "No block entity there.");
        return;
    };
    let line = {
        let base: &dyn Host = host;
        let Some(id) = machine_id(base, entity) else {
            say(host, "That block entity is not an electric macerator.");
            return;
        };
        let Some(nbt) = entity.data(base) else {
            say(host, "No block entity there.");
            return;
        };
        let status: Status = macerator_state(&nbt).status();
        let now = entity
            .level(base)
            .and_then(|level| level.game_time(base))
            .unwrap_or(0);
        let active = status.active.unwrap_or("none");
        let eu = status.energy / FE_PER_EU;
        let fe_per_tick = status.recipe_max_eu * i64::from(FE_PER_EU);
        format!(
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
        )
    };
    say(host, line);
}

/// What a harness control changes.
#[derive(Clone, Copy)]
enum Alter {
    Energy,
    Efficiency,
}

/// `/modern_industrialization energy|efficiency`: the harness controls. Both refuse a block
/// entity that is not this machine, so another mod's data is never touched.
fn set(host: &mut dyn ServerHost, command: &CommandInvocation, alter: Alter) {
    let usage = match alter {
        Alter::Energy => "Usage: /modern_industrialization energy <amount> <x> <y> <z>",
        Alter::Efficiency => "Usage: /modern_industrialization efficiency <amount> <x> <y> <z>",
    };
    let (Some(pos), Some(amount)) = (block_pos(command, "pos"), integer(command, "amount")) else {
        say(host, usage);
        return;
    };
    let Some(entity) = entity_at(host, command, pos) else {
        say(host, "No block entity there.");
        return;
    };
    let base: &dyn Host = host;
    if machine_id(base, entity).is_none() {
        say(
            host,
            "That block entity is not an electric macerator; nothing was changed.",
        );
        return;
    }
    let Some(nbt) = entity.data(base) else {
        say(host, "No block entity there.");
        return;
    };
    let mut state = macerator_state(&nbt);
    let after = match alter {
        Alter::Energy => {
            // Set the buffer: empty it, then take as much as its capacity lets in.
            state.energy = 0;
            state.insert_fe(amount.max(0))
        }
        Alter::Efficiency => {
            state.efficiency = amount.max(0);
            state.max_efficiency = state.max_efficiency.max(state.efficiency);
            state.efficiency
        }
    };
    // Only the keys the control changes are merged, so the run in progress and any key another
    // mod keeps on this block entity stay as they are.
    let mut patch = Compound::new();
    match alter {
        Alter::Energy => {
            patch.insert("energy".into(), Tag::Int(after));
        }
        Alter::Efficiency => {
            patch.insert("efficiencyTicks".into(), Tag::Int(after));
            patch.insert("maxEfficiencyTicks".into(), Tag::Int(state.max_efficiency));
        }
    }
    if entity.merge_data(host, patch).is_err() {
        say(host, "No block entity there.");
        return;
    }
    match alter {
        Alter::Energy => say(host, format!("Energy set to {after} FE.")),
        Alter::Efficiency => say(host, format!("Efficiency set to {after} ticks.")),
    }
}
