// The battle music: runs the state machine once a tick and sets the track
// and its volume, or leaves the game's own music when no battle is on.
fn pvp_music_music(
    game: &crate::game::Game<'_>,
    music: &mut Option<horizon_client::music::Music>,
    volume: &mut f32,
) {
    crate::game::pvp_music::music_tick(game, music, volume)
}
