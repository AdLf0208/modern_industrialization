// One tick of the electric macerator: feed the run, finish it, cool or warm.
fn observe_block_entity_tick(
    game: &mut crate::game::Game<'_>,
    pos: [i32; 3],
    entity: &mut horizon_client::containers::ModBlockEntity,
) {
    crate::game::modern_industrialization_state::on_block_entity_tick(game, pos, entity);
}
