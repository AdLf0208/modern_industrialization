// The electric macerator's item slots: one input, four outputs.
fn observe_block_entity_container(
    block: &str,
    container: &mut Option<horizon_client::containers::ModContainer>,
) {
    crate::game::modern_industrialization_state::on_block_entity_container(block, container);
}
