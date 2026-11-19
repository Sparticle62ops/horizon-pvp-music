# pvp_music (Horizon native mod)

A 1:1 Rust port of [PvP Music](https://modrinth.com/mod/pvp-music) (CC0-1.0),
which plays battle music when you engage a player or (if enabled) a mob.
Upstream publishes no source repository, so the port follows the documented
behavior. The package is the `.hmod` source: a native mod built against the
pinned Horizon SDK (`api 0.2`, game 26.3).

## Behavior (the original's, kept)

- Mutual-combat detection: attacks on and by players and mobs are remembered
  for 5 s; a landed hit on a player opens the 500 ms target confirm, and a
  mob fight starts when both sides have recently attacked each other.
- The wither and explosion triggers, with their own radii.
- The fading battle track: per-tick volume, death fades when you die or the
  target dies, and the short and long end sounds queued when a battle of
  30 s or more ends.
- The phased two-half battle music, switched at low health on a beat.
- Settings through `/pvpmusic` (short form `/pvm`): volume, fades, the mob
  rules, the phase lengths, the beat, the end thresholds and the disabled
  mob list, exactly the original's config fields.

## Wiring

- `music` (client): the state machine runs once a tick and answers the
  track and its volume, replacing vanilla's situational music while a battle
  plays (`Music::new(track, 0, 0, true)`), and leaves vanilla's when idle.
- `client_attack_target` (client): records our attack clicks (target type
  and entity id) as combat candidates.
- `client_tick_pre` (client): refreshes the max-health read, detects a
  health drop as a hurt, and plays the queued end sound once the battle
  track has stopped (`SoundPlay::ui`).
- `command` + `register_client_commands`: `/pvpmusic`, answered locally,
  shown as a chat line, and consumed so the game does not run it.
- The battle tracks and end sounds ship as `assets/pvp_music/sounds/`
  (the original's sound events, CC0-1.0).

## Horizon seams

Where the client cannot see what the original read, the port degrades the
same way the original's own fallbacks do:

- A landed player attack: the API's client attack hook fires before the
  server confirms the hit, so a click on a player opens the confirm.
- The hurt source: the client has no hurt event carrying the damage source
  type, so a health drop records a hurt with an unknown source and the
  machine's own mob fallbacks apply (a mob we recently attacked within
  20 blocks, else any non-disabled mob within 10).

Both are the client-side combat facts the port needs; the ask is filed on
horizon-rust/horizon as issue #10064 and tracked until patched, at which
point the wiring switches to it directly.

## License

Upstream PvP Music: CC0-1.0. This port carries the same license; see
LICENSE.
