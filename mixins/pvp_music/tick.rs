// Play a queued end sound once the battle track has stopped.
fn pvp_music_tick(game: &mut crate::game::Game<'_>) {
    crate::game::pvp_music::game_tick(game)
}
