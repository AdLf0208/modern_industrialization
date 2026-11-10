// The electric macerator ticks on the server.
fn observe_block_entity_ticks(block: &str, ticks: &mut bool) {
    crate::game::modern_industrialization_state::on_block_entity_ticks(block, ticks);
}
