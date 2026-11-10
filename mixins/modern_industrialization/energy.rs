// The electric macerator's FE buffer, kept in its saved compound as `energy`.
fn observe_block_entity_energy(
    block: &str,
    energy: &mut Option<horizon_client::containers::ModEnergyBuffer>,
) {
    crate::game::modern_industrialization_state::on_block_entity_energy(block, energy);
}
