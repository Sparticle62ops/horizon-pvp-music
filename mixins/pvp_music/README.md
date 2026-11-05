# PVP Music for Horizon

A Rust port of PvP Music (net.punchthecake.pvpmusic, Modrinth `Tkf0xIjA`,
CC0-1.0): battle music during combat, with the original's behavior kept.

- Mutual-combat detection: attacks on and by players and mobs are
  remembered for 5 s; a mob we hit that then hits us starts a mob battle.
- A landed hit on a player starts a player battle (the original waited for
  the target's hurtTime; the client surface here has no read of another
  player's hurt state).
- A mob-type hurt looks for a mob we recently attacked within 20 blocks
  (the original's own fallback), then any mob within 10 blocks.
- Explosion and bad-respawn-point hurts pull in combat within 12 blocks.
- A wither within 30 blocks starts a fight (the original also required the
  Wither effect; the client surface here carries no per-effect read).
- The battle track fades in and out, death fades run at the configured
  multiplier, and the phased two-half music switches at low health on a
  beat. Battles of 30 s or more end with the short end sound, of 60 s or
  more with the long end sound.
- `/pvpmusic` drives every setting the original's config screen kept.

Settings (`/pvpmusic status`): volume 50, fadeout 60 s, fadein 0 s,
deathfade x2.5, mob battles on (mobfadeout 20 s, mobfadein 0 s,
mobdeathfade x4), phased on, beat 4, threshold 0.4, shortend 600 ticks,
longend 1200 ticks, disabled: silverfish endermite bee panda goat
ender_dragon.

License: CC0-1.0 (https://creativecommons.org/publicdomain/zero/1.0/).
The original's assets are CC0 and are bundled as the mod's own sounds.
