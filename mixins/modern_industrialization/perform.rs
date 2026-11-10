// The `/modern_industrialization` command bodies.
fn observe_perform_command(
    game: &mut crate::game::Game<'_>,
    player: &NativeServerPlayer,
    command: &str,
    ran: &mut bool,
) {
    crate::game::modern_industrialization_state::on_perform_command(game, player, command, ran);
}
