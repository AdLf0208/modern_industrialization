// `/modern_industrialization info|energy|efficiency`: the readout and harness controls.
fn observe_register_commands(commands: &mut Vec<NativeCommand>) {
    crate::game::modern_industrialization_state::on_register_commands(commands);
}
