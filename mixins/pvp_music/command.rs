// `/pvpmusic`: the settings the original keeps in its config screen. The
// parsing lives in the runtime module (`command_entry`).
fn pvp_music_command(
    game: &mut crate::game::Game<'_>,
    command: &str,
    ran: &mut bool,
) {
    crate::game::pvp_music::command_entry(game, command, ran)
}
