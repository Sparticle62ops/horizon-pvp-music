# pvp-music (Horizon port)

A clean-room Rust port of [PvP Music](https://modrinth.com/mod/pvp-music)
(CC0-1.0), which plays battle music when you engage a player or (if enabled) a
mob. Upstream publishes no source repository, so this port follows the
documented behavior, not upstream code.

## Behavior (from the upstream description)

- A random song starts when you engage in battle; re-engaging snaps it back
  to full volume.
- The song fades out slowly while you fight without re-engaging.
- When you die or the enemy dies, it fades out faster (configurable speed).
- Without a compatible resource pack, a fallback battle track plays.
- Custom songs come from resource packs: assets/pvpmusic/sounds.json with a
  battle_music sound event listing pvpmusic:custom/*.ogg tracks, exactly the
  upstream contract, so the MCSM music pack works as is.

## How it works in Horizon

The state machine (idle -> fighting -> fading -> stop) lives in
mixins/pvp_music/runtime.rs and drives Horizon's music manager through
native_music(music, volume) (API 119), which takes a new track and volume
every tick, so the fades are per-tick volume answers. Song selection picks a
random track and never repeats until the pool is exhausted.

## Horizon seams

- Combat engagement detection client-side: server hooks (native_attack_entity,
  native_living_incoming_damage) fire only where Horizon is the server.
  On a remote server the client half needs a client-side combat surface; until
  then single-player and LAN-host fights drive the mod, and the wiring reads
  engagement from the server hooks on those paths.
- Filed on horizon-rust/horizon as an API ask (client-side combat events).
  Tracked until patched; when it lands the wiring switches to it directly.

## License

Upstream PvP Music: CC0-1.0. This port carries the same license; see LICENSE.
