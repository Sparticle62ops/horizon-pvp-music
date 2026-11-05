// PVP Music for Horizon: battle music during combat.
//
// A Rust port of PvP Music (net.punchthecake.pvpmusic, Modrinth Tkf0xIjA,
// CC0-1.0, https://modrinth.com/mod/pvp-music). The original's behavior is
// kept: mutual-combat detection (attacks on and by players and mobs are
// remembered for 5 s), the wither and explosion triggers, the fading battle
// track with death fades, the phased two-half battle music switched at low
// health on a beat, and the short and long end sounds queued when a battle
// of 30 s or more ends. Horizon drives the settings with the /pvpmusic
// command instead of the original's config screen.
//
// Where the client cannot see what the original read, this port degrades
// the same way the original's own fallbacks do:
// - The original waits for the attacked player's hurtTime after a landed
//   hit; a landed hit on a player starts the battle here.
// - The original reads the attacker off the damage source; the client hurt
//   log carries only the damage type, so a mob-type hurt looks for a mob
//   we recently attacked (the original's own 20-block fallback), then any
//   mob within 10 blocks (the original's own 10-block fallback).
// - The original starts a wither fight only with the Wither effect; the
//   client surface here carries no per-effect read, so proximity alone
//   triggers it.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;

use crate::music::Music;

/// The mod's id (its sound namespace and settings folder in the original).
pub const ID: &str = "pvp_music";
/// The command words the mod answers to.
pub const HEADS: [&str; 2] = ["pvpmusic", "pvm"];
/// One line of how the command reads, shown when it is not understood.
pub const USAGE: &str = "Usage: /pvpmusic [status | volume <0-100> | fadeout <s> | fadein <s> | deathfade <x> | mob on|off | mobfadeout <s> | mobfadein <s> | mobdeathfade <x> | threshold <0-1> | beat <n> | phase1 <s> | phase2 <s> | shortend <t> | longend <t> | disabled add|remove <mob>]";

/// The battle tracks and end sounds (the original's sound events).
pub const FIRST_HALF: &str = "pvp_music:first_half";
pub const SECOND_HALF: &str = "pvp_music:second_half";
pub const SHORT_END: &str = "pvp_music:short_end";
pub const LONG_END: &str = "pvp_music:long_end";

/// How long an attack stays remembered for mutual-combat detection (the
/// original's 5 s attacker memory).
const MUTUAL_MS: u128 = 5000;
/// How long a landed player attack stays open as a battle candidate (the
/// original's 500 ms target confirm).
const CONFIRM_MS: u128 = 500;
/// The wither fight starts with a wither this close.
const WITHER_RADIUS: f64 = 30.0;
/// An explosion hurt pulls in combat this close.
const EXPLOSION_RADIUS: f64 = 12.0;
/// A mob hurt looks for a mob we recently attacked this close.
const MOB_RADIUS: f64 = 20.0;
/// The fallback when no remembered mob is in reach.
const MOB_FALLBACK_RADIUS: f64 = 10.0;

/// `PvpmusicConfig`: the original's config screen fields and their defaults.
#[derive(Clone, Debug)]
pub struct Config {
    pub fade_out: f64,
    pub fade_in: f64,
    pub volume: f64,
    pub death_fadeout_multiplier: f64,
    pub mob_battle: bool,
    pub mob_fade_out: f64,
    pub mob_fade_in: f64,
    pub mob_death_fadeout_multiplier: f64,
    pub phased: bool,
    pub phase1_duration: f64,
    pub phase2_duration: f64,
    pub beat_divisor: i32,
    pub phase2_health_threshold: f64,
    pub short_end_threshold: i64,
    pub long_end_threshold: i64,
    pub disabled_mobs: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            fade_out: 60.0,
            fade_in: 0.0,
            volume: 50.0,
            death_fadeout_multiplier: 250.0,
            mob_battle: true,
            mob_fade_out: 20.0,
            mob_fade_in: 0.0,
            mob_death_fadeout_multiplier: 400.0,
            phased: true,
            phase1_duration: 74.667,
            phase2_duration: 85.333,
            beat_divisor: 4,
            phase2_health_threshold: 0.4,
            short_end_threshold: 600,
            long_end_threshold: 1200,
            disabled_mobs: [
                "silverfish", "endermite", "bee", "panda", "goat", "ender_dragon",
            ]
            .iter()
            .map(|m| m.to_string())
            .collect(),
        }
    }
}

/// Which battle track is playing.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Track {
    FirstHalf,
    SecondHalf,
}

impl Track {
    fn sound(self) -> &'static str {
        match self {
            Track::FirstHalf => FIRST_HALF,
            Track::SecondHalf => SECOND_HALF,
        }
    }
    /// The track's own length, in seconds (the original's constants).
    fn duration(self) -> f64 {
        match self {
            Track::FirstHalf => 74.667,
            Track::SecondHalf => 85.333,
        }
    }
}

/// One running battle: the track, its clock and its death fade.
struct Battle {
    track: Track,
    target: Option<u32>,
    is_mob: bool,
    started: Instant,
    ticks_played: i64,
    /// Phase 2 waits for its first beat after the phase 1 length.
    synced: bool,
    death_fade: bool,
    death_started: Option<Instant>,
}

/// One queued end sound (the original's `pendingEndSound`).
struct PendingEnd {
    short: bool,
}

/// The manager: the original's `BattleMusicManager` and
/// `PhasedBattleMusicController`, driven once a client tick from the music
/// hook.
pub struct Manager {
    cfg: Config,
    current: Option<Battle>,
    potential_target: Option<(u32, Instant)>,
    recent_player_attacks: HashMap<u32, Instant>,
    recent_mob_attacks: HashMap<u32, Instant>,
    pending_end: Option<PendingEnd>,
    last_attack_seen: i64,
    last_hurt_seen: i64,
    /// The latest max-health read (the music hook holds the game only
    /// immutably, so the tick hook keeps this fresh).
    max_health: f64,
}

impl Default for Manager {
    fn default() -> Self {
        Self::new()
    }
}

impl Manager {
    pub fn new() -> Self {
        Self {
            cfg: Config::default(),
            current: None,
            potential_target: None,
            recent_player_attacks: HashMap::new(),
            recent_mob_attacks: HashMap::new(),
            pending_end: None,
            last_attack_seen: i64::MIN,
            last_hurt_seen: i64::MIN,
            max_health: 20.0,
        }
    }

    fn scale(&self, is_mob: bool) -> f32 {
        (if is_mob {
            self.cfg.volume
        } else {
            self.cfg.volume
        } / 100.0) as f32
    }
}

/// The mod's state, shared by the hook helpers.
pub static STATE: Mutex<Option<Manager>> = Mutex::new(None);

fn state() -> std::sync::MutexGuard<'static, Option<Manager>> {
    STATE.lock().unwrap_or_else(|e| e.into_inner())
}

fn dist3(
    ax: f64,
    ay: f64,
    az: f64,
    bx: f64,
    by: f64,
    bz: f64,
) -> f64 {
    ((ax - bx).powi(2) + (ay - by).powi(2) + (az - bz).powi(2)).sqrt()
}

/// Once a client tick, from the music hook: run the battle state machine,
/// then set the track and its volume, or leave the game's own music when
/// the battle is over.
pub fn music_tick(game: &crate::game::Game<'_>, music: &mut Option<Music>, volume: &mut f32) {
    let mut manager = state();
    let Some(manager) = manager.as_mut() else {
        *manager = Some(Manager::new());
        let manager = manager.as_mut().unwrap();
        return drive(manager, game, music, volume);
    };
    drive(manager, game, music, volume);
}

fn drive(
    manager: &mut Manager,
    game: &crate::game::Game<'_>,
    music: &mut Option<Music>,
    volume: &mut f32,
) {
    let Some(p) = game.player.as_ref() else {
        return;
    };
    let pos = p.position;
    let health = p.health.health();
    let max = manager.max_health;
    let now = Instant::now();
    let new_attacks: Vec<(i64, String, u32, bool)> = p
        .attack_log
        .iter()
        .filter(|r| r.game_time > manager.last_attack_seen)
        .map(|r| (r.game_time, r.target.clone(), r.id, r.landed))
        .collect();
    let new_hurts: Vec<(i64, String)> = p
        .hurt_log
        .iter()
        .filter(|r| r.game_time > manager.last_hurt_seen)
        .map(|r| (r.game_time, r.source.clone()))
        .collect();
    let mut playing_id = None;
    if let Some(b) = &manager.current {
        if let Some(id) = battle_target(b) {
            playing_id = Some(id);
        }
    }
    // The target and the mobs around us, read before the mutable half.
    let target_alive = playing_id
        .map(|id| game.mobs.iter().any(|m| m.id == id && m.health > 0.0))
        .unwrap_or(true);
    let wither_near = game
        .mobs
        .iter()
        .any(|m| m.kind.id.contains("wither") && dist3(m.position.x as f64, m.position.y as f64, m.position.z as f64, pos.x as f64, pos.y as f64, pos.z as f64) <= WITHER_RADIUS);
    let nearby_mobs: Vec<(u32, String, f64)> = game
        .mobs
        .iter()
        .filter_map(|m| {
            let kind = m.kind.id.trim_start_matches("minecraft:").to_string();
            Some((m.id, kind, dist3(m.position.x as f64, m.position.y as f64, m.position.z as f64, pos.x as f64, pos.y as f64, pos.z as f64)))
        })
        .collect();

    if let Some((last_attack, _, _, _)) = new_attacks.last() {
        manager.last_attack_seen = *last_attack;
    }
    if let Some(last_hurt) = new_hurts.last().map(|(t, _)| *t) {
        manager.last_hurt_seen = last_hurt;
    }

    // The battle's own tick: the envelope, the phase switch, the death
    // fade and the end sound, before anything new can start.
    let scale = manager.cfg.volume as f32 / 100.0;
    if let Some(b) = manager.current.as_mut() {
        if !b.death_fade {
            b.ticks_played += 1;
        }
        // Phase 2: at low health, on a beat, after the phase 1 length.
        if b.track == Track::FirstHalf
            && b.ticks_played as f64 >= manager.cfg.phase1_duration * 20.0
            && max > 0.0
        {
            let total = (manager.cfg.phase1_duration * 20.0) as i64;
            let interval = (total / manager.cfg.beat_divisor.max(1) as i64).max(1);
            let on_beat = b.ticks_played % interval < 2;
            if on_beat {
                b.synced = true;
            }
            if b.synced && on_beat && health > 0.0 && health as f64 / max <= manager.cfg.phase2_health_threshold {
                b.track = Track::SecondHalf;
                b.ticks_played = 0;
            }
        }
        // The end paths: the player dead, the target dead, the fade out.
        let fade_out = if b.death_fade {
            let base = if b.is_mob {
                manager.cfg.mob_fade_out
            } else {
                manager.cfg.fade_out
            };
            let mult = if b.is_mob {
                manager.cfg.mob_death_fadeout_multiplier
            } else {
                manager.cfg.death_fadeout_multiplier
            };
            let span = base * mult / 100.0;
            let started = b.death_started.unwrap_or(now);
            (now.duration_since(started).as_secs_f64() / span.max(0.1)) >= 1.0
        } else {
            false
        };
        let player_gone = health <= 0.0;
        let target_gone = !target_alive && b.target.is_some();
        if fade_out || player_gone || target_gone {
            if !b.death_fade {
                b.death_fade = true;
                b.death_started = Some(now);
            }
            if player_gone || target_gone {
                // The original queues the end sound for when the current
                // sound stops; here the fade has already stopped it.
                let ticks = b.ticks_played;
                if ticks >= manager.cfg.long_end_threshold {
                    manager.pending_end = Some(PendingEnd { short: false });
                } else if ticks >= manager.cfg.short_end_threshold {
                    manager.pending_end = Some(PendingEnd { short: true });
                }
            }
        }
        // The envelope while the battle plays.
        let fade_in = (if b.is_mob {
            manager.cfg.mob_fade_in
        } else {
            manager.cfg.fade_in
        })
        .max(0.1);
        let fade_in_progress =
            (now.duration_since(b.started).as_secs_f64() / fade_in).min(1.0);
        let fade_out_progress = if b.death_fade {
            let base = if b.is_mob {
                manager.cfg.mob_fade_out
            } else {
                manager.cfg.fade_out
            };
            let mult = if b.is_mob {
                manager.cfg.mob_death_fadeout_multiplier
            } else {
                manager.cfg.death_fadeout_multiplier
            };
            let span = base * mult / 100.0;
            let started = b.death_started.unwrap_or(now);
            1.0 - (now.duration_since(started).as_secs_f64() / span.max(0.1))
        } else {
            1.0
        };
        if fade_out_progress <= 0.0 {
            manager.current = None;
            *music = None;
        } else {
            *music = Some(Music::new(
                b.track.sound(),
                0,
                0,
                true,
            ));
            *volume = (fade_in_progress * fade_out_progress) as f32 * scale;
        }
    }

    // New combat: only matters when nothing is playing yet.
    if manager.current.is_none() {
        for (_, target, id, landed) in &new_attacks {
            if *target == "minecraft:player" {
                if *landed {
                    manager.potential_target = Some((*id, now));
                }
            } else {
                manager.recent_player_attacks.insert(*id, now);
                if manager.recent_mob_attacks.contains_key(id) {
                    start_battle(manager, *id, true);
                }
            }
        }
        for (_, source) in &new_hurts {
            if health <= 0.0 {
                break;
            }
            if source.contains("player") {
                start_battle(manager, u32::MAX, false);
            } else if source.contains("explosion") || source.contains("badRespawnPoint") || source.contains("bad_respawn_point") {
                let hit = nearby_mobs
                    .iter()
                    .find(|(_, _, d)| *d <= EXPLOSION_RADIUS)
                    .map(|(id, _, _)| *id);
                start_battle(manager, hit.unwrap_or(u32::MAX), true);
            } else {
                let remembered = manager.recent_player_attacks.clone();
                let hit = nearby_mobs
                    .iter()
                    .find(|(id, kind, d)| {
                        *d <= MOB_RADIUS
                            && remembered.get(id).is_some_and(|t| now.duration_since(*t).as_millis() < MUTUAL_MS)
                            && !manager.cfg.disabled_mobs.contains(kind)
                    })
                    .map(|(id, _, _)| *id)
                    .or_else(|| {
                        nearby_mobs
                            .iter()
                            .find(|(id, kind, d)| {
                                *d <= MOB_FALLBACK_RADIUS
                                    && !manager.cfg.disabled_mobs.contains(kind)
                            })
                            .map(|(id, _, _)| *id)
                    });
                if let Some(id) = hit {
                    manager.recent_mob_attacks.insert(id, now);
                    if manager.recent_player_attacks.contains_key(&id) {
                        start_battle(manager, id, true);
                    }
                }
            }
        }
        if manager.cfg.mob_battle && wither_near {
            start_battle(manager, u32::MAX, true);
        }
        // The open player attack confirms within 500 ms.
        if let Some((id, at)) = manager.potential_target {
            if now.duration_since(at).as_millis() > CONFIRM_MS {
                manager.potential_target = None;
            }
        }
    }

    // Retire expired attacker memories.
    let now2 = now;
    manager
        .recent_player_attacks
        .retain(|_, t| now2.duration_since(*t).as_millis() < MUTUAL_MS);
    manager
        .recent_mob_attacks
        .retain(|_, t| now2.duration_since(*t).as_millis() < MUTUAL_MS);
}

fn battle_target(b: &Battle) -> Option<u32> {
    b.target
}

fn start_battle(manager: &mut Manager, target: u32, is_mob: bool) {
    if manager.current.is_some() {
        return;
    }
    manager.current = Some(Battle {
        track: Track::FirstHalf,
        is_mob,
        started: Instant::now(),
        ticks_played: 0,
        synced: false,
        death_fade: false,
        death_started: None,
        target: if target == u32::MAX { None } else { Some(target) },
    });
    manager.potential_target = None;
}


/// Once a tick, from the game-tick hook: play a queued end sound once the
/// battle track has stopped (the original queues it the same way and plays
/// it when the current sound ends).
pub fn game_tick(game: &mut crate::game::Game<'_>) {
    let mut manager = state();
    let manager = manager.get_or_insert_with(Manager::new);
    if let Some(p) = game.player.as_mut() {
        manager.max_health = p.attribute("minecraft:max_health").unwrap_or(20.0);
    }
    if manager.current.is_some() {
        return;
    }
    let Some(end) = manager.pending_end.take() else {
        return;
    };
    if !game.native_window() {
        manager.pending_end = Some(end);
        return;
    }
    let Some(p) = game.player.as_ref() else {
        manager.pending_end = Some(end);
        return;
    };
    let pos = [p.position.x as f64, p.position.y as f64, p.position.z as f64];
    let sound = if end.short { SHORT_END } else { LONG_END };
    let _ = game.native_play_local_sound(sound, pos, manager.scale(false), 1.0, 0);
}

/// `/pvpmusic`: the settings the original keeps in its config screen.
pub fn command_entry(
    game: &mut crate::game::Game<'_>,
    command: &str,
    ran: &mut bool,
) {
    let rest = command.trim();
    let (head, args) = rest.split_once(' ').unwrap_or((rest, ""));
    let head = head.trim_start_matches('/').to_lowercase();
    if !HEADS.contains(&head.as_str()) {
        return;
    }
    *ran = true;
    let mut manager = state();
    let manager = manager.get_or_insert_with(Manager::new);
    let mut parts = args.split_whitespace();
    match parts.next().unwrap_or("status") {
        "status" => {
            game.native_chat(&format!(
                "pvp_music: volume {} fadeout {}s fadein {}s deathfade {} mob {} mobfadeout {}s mobfadein {}s mobdeathfade {} phased {} beat {} threshold {} shortend {} longend {} disabled [{}]",
                manager.cfg.volume as i64, manager.cfg.fade_out, manager.cfg.fade_in,
                manager.cfg.death_fadeout_multiplier,
                if manager.cfg.mob_battle { "on" } else { "off" },
                manager.cfg.mob_fade_out, manager.cfg.mob_fade_in,
                manager.cfg.mob_death_fadeout_multiplier,
                if manager.cfg.phased { "on" } else { "off" },
                manager.cfg.beat_divisor, manager.cfg.phase2_health_threshold,
                manager.cfg.short_end_threshold, manager.cfg.long_end_threshold,
                manager.cfg.disabled_mobs.join(", ")
            ));
        }
        "volume" => { if let Some(v) = parts.next().and_then(|v| v.parse::<f64>().ok()) { manager.cfg.volume = v.clamp(0.0, 100.0); } }
        "fadeout" => { if let Some(v) = parts.next().and_then(|v| v.parse::<f64>().ok()) { manager.cfg.fade_out = v.max(0.0); } }
        "fadein" => { if let Some(v) = parts.next().and_then(|v| v.parse::<f64>().ok()) { manager.cfg.fade_in = v.max(0.0); } }
        "deathfade" => { if let Some(v) = parts.next().and_then(|v| v.parse::<f64>().ok()) { manager.cfg.death_fadeout_multiplier = v.max(1.0); } }
        "mob" => { manager.cfg.mob_battle = matches!(parts.next(), Some("on") | None); }
        "mobfadeout" => { if let Some(v) = parts.next().and_then(|v| v.parse::<f64>().ok()) { manager.cfg.mob_fade_out = v.max(0.0); } }
        "mobfadein" => { if let Some(v) = parts.next().and_then(|v| v.parse::<f64>().ok()) { manager.cfg.mob_fade_in = v.max(0.0); } }
        "mobdeathfade" => { if let Some(v) = parts.next().and_then(|v| v.parse::<f64>().ok()) { manager.cfg.mob_death_fadeout_multiplier = v.max(1.0); } }
        "beat" => { if let Some(v) = parts.next().and_then(|v| v.parse::<i32>().ok()) { manager.cfg.beat_divisor = v.max(1); } }
        "threshold" => { if let Some(v) = parts.next().and_then(|v| v.parse::<f64>().ok()) { manager.cfg.phase2_health_threshold = v.clamp(0.0, 1.0); } }
        "phase1" => { if let Some(v) = parts.next().and_then(|v| v.parse::<f64>().ok()) { manager.cfg.phase1_duration = v.max(1.0); } }
        "phase2" => { if let Some(v) = parts.next().and_then(|v| v.parse::<f64>().ok()) { manager.cfg.phase2_duration = v.max(1.0); } }
        "shortend" => { if let Some(v) = parts.next().and_then(|v| v.parse::<i64>().ok()) { manager.cfg.short_end_threshold = v; } }
        "longend" => { if let Some(v) = parts.next().and_then(|v| v.parse::<i64>().ok()) { manager.cfg.long_end_threshold = v; } }
        "disabled" => {
            let (sub, mob) = (parts.next(), parts.next().map(|m| m.to_lowercase()));
            if let (Some(sub), Some(mob)) = (sub, mob) {
                match sub {
                    "add" => { if !manager.cfg.disabled_mobs.contains(&mob) { manager.cfg.disabled_mobs.push(mob); } }
                    "remove" => { manager.cfg.disabled_mobs.retain(|m| *m != mob); }
                    _ => {}
                }
            }
        }
        _ => { game.native_chat(USAGE); }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_the_original_config() {
        let cfg = Config::default();
        assert_eq!(cfg.fade_out, 60.0);
        assert_eq!(cfg.volume, 50.0);
        assert_eq!(cfg.death_fadeout_multiplier, 250.0);
        assert!(cfg.mob_battle);
        assert_eq!(cfg.mob_fade_out, 20.0);
        assert_eq!(cfg.mob_death_fadeout_multiplier, 400.0);
        assert!(cfg.phased);
        assert_eq!(cfg.phase1_duration, 74.667);
        assert_eq!(cfg.phase2_duration, 85.333);
        assert_eq!(cfg.beat_divisor, 4);
        assert_eq!(cfg.phase2_health_threshold, 0.4);
        assert_eq!(cfg.short_end_threshold, 600);
        assert_eq!(cfg.long_end_threshold, 1200);
        assert_eq!(cfg.disabled_mobs.len(), 6);
    }

    #[test]
    fn a_battle_starts_stopped_and_fades_in() {
        let mut manager = Manager::new();
        start_battle(&mut manager, 7, false);
        let b = manager.current.as_ref().unwrap();
        assert_eq!(b.track, Track::FirstHalf);
        assert_eq!(b.target, Some(7));
        assert!(!b.death_fade);
        assert!(manager.scale(false) <= 0.51);
    }

    #[test]
    fn mob_disabled_kinds_are_listed() {
        let cfg = Config::default();
        assert!(cfg.disabled_mobs.contains(&"silverfish".to_string()));
    }
}
