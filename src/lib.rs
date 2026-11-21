//! Modern Industrialization for Horizon — the electric macerator and the coke oven.
//!
//! Slice 1 is one block, `modern_industrialization:electric_macerator`: a 32,000 FE
//! buffer, one item input slot, four item output slots, and upstream's `CrafterComponent` tick
//! with the LV tier's overclock. Slice 2 adds the mod's first **multiblock**,
//! `modern_industrialization:coke_oven`: a 3×3×3 shell of bricks around a controller, a
//! `formed` state the shape match writes, and the jar's coke recipe, which only runs while
//! the shell is whole. Every number (the buffer, the recipe tables, the rate, the overclock,
//! the shell, the shape match) lives in [`runtime`], which is pure; this file is the glue
//! that connects it to the game.
//!
//! Upstream's machine is a `MachineBlockEntity` plus three NeoForge capabilities (its item
//! handler, its energy storage and its GUI). The SDK's shape is typed: the block entity type is
//! declared once in [`Hooks::register`], and the moments a machine answers — the saved data it
//! starts with, that it ticks, one tick — are its Part's methods, so they are asked only for this
//! block entity type and never for another mod's machine. The item slots and the FE buffer are the
//! game's own default capabilities (`Registrar::block_container`, `Registrar::block_energy`),
//! which is what a hopper, a pipe or another mod's cable reach.
//!
//! A multiblock needs a moment the single block did not: the shape match reads blocks, and a
//! block-entity tick cannot (its event carries `pos` and the entity, no level). So the match
//! runs in [`Hooks::block_changed`], which has the level as a read-only [`LevelHost`], and the
//! result is written into the controller's saved data in [`Hooks::level_tick_post`], which has
//! the writer. That is also how upstream keeps its `ShapeMatcher` current: on block updates, not
//! every tick.
//!
//! `/modern_industrialization info|energy|efficiency` is the readout and the harness controls; the
//! mutation usages do not exist upstream and are recorded as additions in `notes.md`.
pub mod runtime;

use std::sync::{Mutex, MutexGuard};

use mod_api::{
    self, BlockChangedEvent, BlockEntityEvent, BlockEntityPart, BlockEntityRef,
    BlockEntityTickEvent, BlockEntityTickReply, BlockEntityTicksEvent, BlockPart, Command,
    CommandArgumentValue, CommandInvocation, Compound, Flow, Hooks, Host, HostSide, Identifier,
    ItemKey, ItemStack, LevelHost, LevelSide, LevelTickPostEvent, ModContainer, ModEnergyBuffer,
    RegisterCommandsEvent, Registrar, RegistrationContext, RunModCommandEvent, ServerHost, Tag,
    Text,
};

use runtime::{
    CAPACITY_FE, COKE_CAPACITY_FE, COKE_OVEN_ID, COKE_OVEN_SHELL, CokeOven, FE_PER_EU,
    MACERATOR_ID, Macerator, MachineRecipe, OUTPUT_SLOTS, OvenSaved, PROBABILITY_SCALE, STACK_SIZE,
    Saved, Shell, ShellMatch, Slots, Status, coke_recipe, is_coke_oven, is_macerator, match_shell,
    recipe, shell_relevant,
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

/// The coke oven's block entity type: the controller block's own name.
const COKE_ENTITY_TYPE: &str = "coke_oven";

/// The coke oven's item input slot. Upstream feeds the multiblock through an item
/// input hatch; the hatch model is not ported, so the controller carries the input
/// and its output (recorded in `notes.md`).
const COKE_INPUT: usize = 0;

/// The coke oven's item output slot.
const COKE_OUTPUT: usize = 1;

/// The coke oven's slots in all.
const COKE_SLOTS: usize = COKE_OUTPUT + 1;

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

        // The coke oven's controller: its own block entity type, the Part that answers for it,
        // the item slots its hatches would carry, and the buffer its steam is modelled as.
        let oven = r.block_key(COKE_OVEN_ID);
        let oven_kind = r
            .block_entity_type(COKE_ENTITY_TYPE)
            .blocks(&[COKE_ENTITY_TYPE])
            .build();
        let _ = r.block_entity_part(&oven_kind, CokeOvenPart);
        r.block_container(
            &oven,
            ModContainer {
                slots: COKE_SLOTS,
                faces: std::array::from_fn(|_| (0..COKE_SLOTS).collect()),
                inputs: vec![COKE_INPUT],
                outputs: (COKE_OUTPUT..COKE_SLOTS).collect(),
                ..Default::default()
            },
        );
        // A steam machine stores steam, not FE: upstream's `SteamHelper.consumeSteamEu` takes one
        // millibucket of steam per EU, and the pin keeps no mod fluid in a block-entity tank, so
        // the same energy is stored as FE at the mod's own 1 EU = 10 FE.
        r.block_energy(
            &oven,
            ModEnergyBuffer {
                capacity: COKE_CAPACITY_FE,
                max_insert: COKE_CAPACITY_FE,
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

    /// `/modern_industrialization` command bodies and the shape match (upstream's executor;
    /// NeoForge's `CommandEvent` only watches, the tree node runs).
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

    /// A block the authoritative server wrote: if it is a shell member or a coke oven
    /// controller, re-match the controller that position belongs to and queue the answer.
    ///
    /// Upstream rematches its `ShapeMatcher` on block updates for the same reason: the shell
    /// is cheap to check and the world is not to poll. This hook is read-only (a
    /// [`LevelHost`]), so the answer is written in [`Hooks::level_tick_post`], which has the
    /// writer; a block-entity tick cannot do it at all, because its event carries no level.
    fn block_changed(&self, level: &dyn LevelHost, event: &mut BlockChangedEvent) {
        let Some(old) = level.state_block(event.old) else {
            return;
        };
        let Some(new) = level.state_block(event.new) else {
            return;
        };
        if !shell_relevant(old, new) {
            return;
        }
        let dimension = event.dimension.key().id();
        let pos = [event.pos.x, event.pos.y, event.pos.z];
        // A change can only affect a controller whose shape contains (or is) that position.
        // `(0, 0, 0)` covers "the controller itself was just placed on a waiting shell".
        for offset in COKE_OVEN_SHELL
            .iter()
            .map(|member| member.offset)
            .chain(std::iter::once([0, 0, 0]))
        {
            let controller = [pos[0] - offset[0], pos[1] - offset[1], pos[2] - offset[2]];
            if level_block(level, controller) != Some(COKE_OVEN_ID) {
                continue;
            }
            let matched = match_shell(&LevelShell { level }, controller);
            queue_match(dimension, controller, matched);
        }
    }

    /// The end of a level's tick: write the queued shape matches into the controllers' saved
    /// data, which is where the tick and the readout read `formed` from.
    fn level_tick_post(&self, host: HostSide<'_>, event: &mut LevelTickPostEvent) {
        let HostSide::Server(host) = host else {
            return;
        };
        let Some(dimension) = event.level.dimension().id(&*host) else {
            return;
        };
        let pending = take_matches(&dimension);
        for entry in pending {
            let Some(entity) = host.block_entity(event.level, entry.pos) else {
                // The block entity may not exist yet on the tick the controller was placed;
                // try once more on the next tick, then give up rather than keep the queue.
                if entry.retries > 0 {
                    queue_retry(entry);
                }
                continue;
            };
            let (kind, nbt) = {
                let base: &dyn Host = &*host;
                (
                    entity.kind(base).map(|kind| kind.id(base)),
                    entity.data(base),
                )
            };
            if kind.as_deref() != Some(COKE_OVEN_ID) {
                continue;
            }
            let Some(nbt) = nbt else {
                if entry.retries > 0 {
                    queue_retry(entry);
                }
                continue;
            };
            if be_int(&nbt, "formed") != i32::from(entry.formed)
                || be_int(&nbt, "shellMissing") != i32::from(entry.missing)
            {
                let mut patch = Compound::new();
                patch.insert("formed".into(), Tag::Int(i32::from(entry.formed)));
                patch.insert("shellMissing".into(), Tag::Int(i32::from(entry.missing)));
                let _ = entity.merge_data(host, patch);
            }
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
                input: INPUT_SLOT,
                first_output: FIRST_OUTPUT,
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

/// The coke oven's block entity Part: the controller's saved data, its tick, and the item slots
/// its hatches would carry.
///
/// The `formed` flag is not written here: it is the shape match's answer, written by
/// [`Hooks::level_tick_post`] because a block-entity tick has no level to read the shell from.
/// The tick reads it and pauses the run while it is false, which is upstream's `NOT_MATCHED`.
struct CokeOvenPart;

#[mod_api::horizon_part]
impl BlockPart for CokeOvenPart {}

#[mod_api::horizon_part]
impl BlockEntityPart for CokeOvenPart {
    /// A fresh controller: empty, unformed, and `missing` unknown until the first match.
    fn block_entity(
        &self,
        _level: LevelSide,
        _event: &mut BlockEntityEvent<'_>,
    ) -> Option<Compound> {
        Some(Compound::new())
    }

    /// The controller ticks on the server, so an unformed oven can still be seen to be idle.
    fn block_entity_ticks(&self, _host: &dyn Host, event: &mut BlockEntityTicksEvent<'_>) {
        event.ticks = true;
    }

    /// One tick of the oven: the same `CrafterComponent` step as the macerator's, at the steam
    /// tier's fixed 2 EU/t, and only while the shell matches.
    fn block_entity_tick(&self, host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
        let pos = event.pos;
        let fields = &mut *event.entity;
        // The same first-tick write-back the macerator needs (see `Machine::block_entity_tick`):
        // the engine only sizes a mod block entity's own `items` on a tick that answers
        // `Changed`, and `Unchanged` discards the payload. `data` is the one payload field
        // nothing saves, so it is empty exactly on a fresh or reloaded entity; the length check
        // beside it also catches a reload that shrank the vector again (the shape match writes
        // `formed` with `merge_data`, which reloads the entity).
        let fresh = !fields.data.is_empty() && fields.items.len() >= COKE_SLOTS;
        if !fresh {
            fields.items.resize(COKE_SLOTS, None);
            fields.data = vec![SLOTS_READY];
        }
        let before = oven_state(&fields.nbt);
        let mut state = before;
        let mut seed = roll_seed(pos, before.energy);
        let mut roll = || next_roll(&mut seed);
        let tick = {
            let mut slots = SlotsView {
                host,
                items: &mut fields.items,
                input: COKE_INPUT,
                first_output: COKE_OUTPUT,
            };
            state.tick(&mut slots, &mut roll)
        };
        if tick.active || tick.crafted.is_some() || state != before {
            store_oven(&mut fields.nbt, &state);
        } else if fresh {
            *event.reply = BlockEntityTickReply::Unchanged;
        }
    }
}

/// The block entity's slots seen through [`Slots`]: one input, the outputs from `first_output` on,
/// each one stack of the item's own `max_stack_size`. The macerator's container is slot 0 in and
/// 1..=4 out; the coke oven's is 0 in and 1 out.
struct SlotsView<'a> {
    host: &'a dyn ServerHost,
    items: &'a mut Vec<Option<ItemStack>>,
    /// The slot a run takes its input from.
    input: usize,
    /// The first slot a run puts its output into (every slot from here on is an output).
    first_output: usize,
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
        match self.slot(self.input) {
            Some(stack) if self.id_of(stack) == item => stack.count(),
            _ => 0,
        }
    }

    fn take(&mut self, item: &str, amount: u32) -> u32 {
        let Self {
            host, items, input, ..
        } = self;
        let host: &dyn Host = &**host;
        let input = *input;
        let Some(stack) = items.get_mut(input).and_then(Option::as_mut) else {
            return 0;
        };
        let key = stack.item();
        let held = stack.count();
        if key.id(host) != item {
            return 0;
        }
        let taken = held.min(amount);
        items[input] = ItemStack::new(key, held - taken);
        taken
    }

    fn room(&self, item: &str, amount: u32) -> u32 {
        let mut room = 0;
        for index in self.first_output..self.items.len() {
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
        let Self {
            host,
            items,
            first_output,
            ..
        } = self;
        let host: &dyn Host = &**host;
        let first = *first_output;
        let Some(key) = Identifier::parse(item)
            .ok()
            .and_then(|id| host.item_key(&id))
        else {
            return amount;
        };
        let stack_size = host.max_stack_size(key).unwrap_or(STACK_SIZE);
        let mut left = amount;
        for index in first..items.len() {
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
    be_recipe_with(nbt, recipe)
}

/// The same, through another machine's table.
fn be_recipe_with(
    nbt: &Compound,
    lookup: fn(&str) -> Option<&'static MachineRecipe>,
) -> Option<&'static str> {
    match nbt.get(&"activeRecipe".into()) {
        Some(Tag::String(value)) => {
            let id = value.to_string_lossy();
            lookup(&id).map(|found| found.id)
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

/// The saved coke oven; `formed` and `shellMissing` are the shape match's answer, written by
/// [`Hooks::level_tick_post`] rather than by the tick.
fn oven_state(nbt: &Compound) -> CokeOven {
    CokeOven::from_saved(&OvenSaved {
        energy: be_int(nbt, "energy"),
        used_eu: be_long(nbt, "usedEnergy"),
        recipe_energy: be_long(nbt, "recipeEnergy"),
        recipe_max_eu: be_long(nbt, "recipeMaxEu"),
        active: be_recipe_with(nbt, coke_recipe),
        formed: be_int(nbt, "formed") != 0,
        missing: u8::try_from(be_int(nbt, "shellMissing").clamp(0, 25)).unwrap_or(0),
    })
}

/// Writes the oven back under upstream's keys, leaving `formed`/`shellMissing` to the match.
fn store_oven(nbt: &mut Compound, state: &CokeOven) {
    let saved = state.saved();
    set_be_int(nbt, "energy", saved.energy);
    set_be_long(nbt, "usedEnergy", saved.used_eu);
    set_be_long(nbt, "recipeEnergy", saved.recipe_energy);
    set_be_long(nbt, "recipeMaxEu", saved.recipe_max_eu);
    set_be_recipe(nbt, saved.active);
}

/// A shape match waiting for the level's tick to end, so it can be written into the
/// controller's saved data (a `block_changed` host is read-only).
#[derive(Debug, Clone)]
struct PendingMatch {
    /// The level the controller stands in.
    dimension: String,
    /// The controller's block position.
    pos: [i32; 3],
    /// Whether the shell matched.
    formed: bool,
    /// How many members were wrong or missing.
    missing: u8,
    /// How many more ticks to try when the block entity is not there yet (0 = give up).
    retries: u8,
}

/// How many times a match may wait for its controller's block entity.
const PENDING_RETRIES: u8 = 4;

/// How many matches may wait at once; a whole 3×3×3 built in one tick is 25 positions, and a
/// server whose level tick never ends is a bug this queue must not grow without bound for.
const PENDING_CAP: usize = 512;

/// The matches waiting for their level's tick to end: written by [`Hooks::block_changed`] and
/// drained by [`Hooks::level_tick_post`]. The SDK keeps one `Default` [`Mod`] per session, so
/// this is session state; it is keyed by dimension and position and holds at most one entry per
/// controller, so a stale entry can only ever touch a block entity that is a coke oven again.
static PENDING: Mutex<Vec<PendingMatch>> = Mutex::new(Vec::new());

fn pending() -> MutexGuard<'static, Vec<PendingMatch>> {
    PENDING
        .lock()
        .expect("the match queue's lock is not poisoned")
}

/// Queues a match, replacing an older answer for the same controller.
fn queue_match(dimension: &str, pos: [i32; 3], matched: ShellMatch) {
    let mut queue = pending();
    queue.retain(|entry| !(entry.dimension == dimension && entry.pos == pos));
    if queue.len() >= PENDING_CAP {
        queue.remove(0);
    }
    queue.push(PendingMatch {
        dimension: dimension.to_owned(),
        pos,
        formed: matched.formed,
        missing: matched.missing,
        retries: PENDING_RETRIES,
    });
}

/// Queues an entry again for the next tick.
fn queue_retry(mut entry: PendingMatch) {
    entry.retries -= 1;
    let mut queue = pending();
    if queue.len() >= PENDING_CAP {
        return;
    }
    queue.push(entry);
}

/// Takes this level's queued matches out of the queue.
fn take_matches(dimension: &str) -> Vec<PendingMatch> {
    let mut queue = pending();
    let (mine, others) = std::mem::take(&mut *queue)
        .into_iter()
        .partition(|entry: &PendingMatch| entry.dimension == dimension);
    *queue = others;
    mine
}

/// The block id at `pos` in a read-only level (the `block_changed` view).
fn level_block(level: &dyn LevelHost, pos: [i32; 3]) -> Option<&str> {
    level.state_block(level.block_state(pos)?)
}

/// The shape match's view of a level, which answers absolute positions.
struct LevelShell<'a> {
    level: &'a dyn LevelHost,
}

impl Shell for LevelShell<'_> {
    fn block_at(&self, pos: [i32; 3]) -> Option<&str> {
        level_block(self.level, pos)
    }
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

/// The block position argument, as the plain coordinates the runtime works in.
///
/// The variant carries the three coordinates itself, so this is a copy of an array, not the
/// construction of an SDK type. The generated `uses.json` nevertheless records
/// `type:BlockPos built: full` here, because it is a source-text scan and resolves the variant's
/// name against the SDK's `BlockPos`: the same class of false entry as the `Recipe` one that
/// `notes.md` §"Registry fit-gate repair" records, and harmless for the same kind of reason —
/// `BlockPos` is constructible and has the same three fields at the previous build, so
/// `fits --build 0b14f8a` is green (see `sdk-fits-next.log`).
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

/// Whether the block entity is the coke oven's controller, answering its id when it is.
fn oven_id(host: &dyn Host, entity: BlockEntityRef) -> Option<String> {
    let id = entity.kind(host)?.id(host);
    is_coke_oven(&id).then_some(id)
}

/// `/modern_industrialization info <pos>`: what the machine holds, its run, its overclock, and
/// for a multiblock its shell.
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
        let Some(nbt) = entity.data(base) else {
            say(host, "No block entity there.");
            return;
        };
        let now = entity
            .level(base)
            .and_then(|level| level.game_time(base))
            .unwrap_or(0);
        if let Some(id) = machine_id(base, entity) {
            let status: Status = macerator_state(&nbt).status();
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
        } else if let Some(id) = oven_id(base, entity) {
            let status = oven_state(&nbt).status();
            let active = status.active.unwrap_or("none");
            let eu = status.energy / FE_PER_EU;
            let fe_per_tick = status.recipe_max_eu * i64::from(FE_PER_EU);
            let formed = if status.formed { "true" } else { "false" };
            format!(
                "{id} at {},{},{}: {}/{} FE ({} EU), progress {}/1000, {fe_per_tick} FE/t, formed {formed}, missing {}, active {active}, t={now}",
                pos[0],
                pos[1],
                pos[2],
                status.energy,
                COKE_CAPACITY_FE,
                eu,
                status.progress_permille,
                status.missing,
            )
        } else {
            say(
                host,
                "That block entity is not one of this port's machines.",
            );
            return;
        }
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
    let is_oven = oven_id(base, entity).is_some();
    // `energy` fills either machine's buffer; `efficiency` is the macerator's overclock and the
    // coke oven has none (upstream's steam tier caps its rate at its own 2 EU/t).
    if machine_id(base, entity).is_none() && !is_oven {
        say(
            host,
            "That block entity is not one of this port's machines; nothing was changed.",
        );
        return;
    }
    if is_oven && matches!(alter, Alter::Efficiency) {
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
    // Only the keys the control changes are merged, so the run in progress and any key another
    // mod keeps on this block entity stay as they are.
    let mut patch = Compound::new();
    let after = if is_oven {
        let mut state = oven_state(&nbt);
        match alter {
            Alter::Energy => {
                state.energy = 0;
                state.insert_fe(amount.max(0))
            }
            Alter::Efficiency => 0,
        }
    } else {
        let mut state = macerator_state(&nbt);
        match alter {
            Alter::Energy => {
                // Set the buffer: empty it, then take as much as its capacity lets in.
                state.energy = 0;
                state.insert_fe(amount.max(0))
            }
            Alter::Efficiency => {
                state.efficiency = amount.max(0);
                state.max_efficiency = state.max_efficiency.max(state.efficiency);
                patch.insert("efficiencyTicks".into(), Tag::Int(state.efficiency));
                patch.insert("maxEfficiencyTicks".into(), Tag::Int(state.max_efficiency));
                state.efficiency
            }
        }
    };
    if matches!(alter, Alter::Energy) {
        patch.insert("energy".into(), Tag::Int(after));
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
