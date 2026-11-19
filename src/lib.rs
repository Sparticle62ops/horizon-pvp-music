//! PVP Music for Horizon: battle music during combat. The mod API wiring.
//!
//! The hooks gather what the client can see (the camera player, the mobs in
//! reach, our attack clicks and hurt ticks) into a [`Snapshot`], and the
//! state machine in `state.rs` runs the original's behavior against it. The
//! reads the client cannot see degrade the way the port's README lists, and
//! each one carries its tracking issue.
use mod_api::{
    self, ClientAttackTargetEvent, ClientHost, ClientTickPreEvent, Command, CommandEvent, EntityRef,
    Flow, LivingRef, MusicEvent, RegisterClientCommandsEvent, SoundPlay, Text,
};

mod command;
mod state;

use state::{MobFacts, PlayerFacts, Snapshot};

/// The reach of the mob reads around the player (the state machine filters
/// by its own radii inside this; the widest is the wither's 30).
const MOB_REACH: f64 = 32.0;

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl mod_api::Hooks for Mod {
    /// Once a client tick: run the battle state machine and set the track
    /// and its volume, or leave the game's own music when no battle is on.
    fn music(&self, host: &dyn ClientHost, event: &mut MusicEvent) {
        let snap = snapshot(host);
        state::music_tick(&snap, &mut event.music, &mut event.volume);
    }

    /// A client attack click: record each target as an attack candidate.
    /// The API hears the click before the server confirms the hit, so the
    /// record carries `landed: true` the moment the click happens (the
    /// original waited for the landed hit; the degradation is in the README).
    fn client_attack_target(&self, host: &mut dyn ClientHost, event: &mut ClientAttackTargetEvent) -> Flow {
        let game_time = game_time(host);
        for target in event.targets.iter() {
            let kind = target
                .kind(host)
                .map(|k| k.id(host))
                .unwrap_or_else(|| "unknown".to_string());
            state::record_attack(game_time, kind, target.id());
        }
        Flow::Continue
    }

    /// Once a client tick: refresh the max-health read, detect a health
    /// drop as a hurt (the client has no hurt event; the degradation is in
    /// the README) and play a queued end sound once the battle stopped.
    fn client_tick_pre(&self, host: &mut dyn ClientHost, _event: &mut ClientTickPreEvent) {
        let player = host.camera_entity();
        if let Some(player) = player.as_ref() {
            if let Some(living) = host.living(*player) {
                let health = living.health(host).unwrap_or(0.0);
                let max = living.max_health(host).unwrap_or(20.0) as f64;
                state::set_max_health(max);
                if state::health_dropped(health) {
                    let game_time = game_time(host);
                    state::record_hurt(game_time, "unknown".to_string());
                }
            }
        }
        if let Some(short) = state::tick_end(player.is_some()) {
            let sound = if short { state::SHORT_END } else { state::LONG_END };
            let play = SoundPlay::ui(sound);
            let _ = host.play_local_sound(&play, 0);
        }
    }

    /// Register `/pvpmusic` and its `pvm` short form.
    fn register_client_commands(&self, _host: &mut dyn ClientHost, event: &mut RegisterClientCommandsEvent) {
        event.commands.push(Command::new("pvpmusic [status | volume <0-100> | fadeout <s> | fadein <s> | deathfade <x> | mob on|off | mobfadeout <s> | mobfadein <s> | mobdeathfade <x> | threshold <0-1> | beat <n> | phase1 <s> | phase2 <s> | shortend <t> | longend <t> | disabled add|remove <mob>]"));
        event.commands.push(Command::new("pvm [status | volume <0-100> | fadeout <s> | fadein <s> | deathfade <x> | mob on|off | mobfadeout <s> | mobfadein <s> | mobdeathfade <x> | threshold <0-1> | beat <n> | phase1 <s> | phase2 <s> | shortend <t> | longend <t> | disabled add|remove <mob>]"));
    }

    /// `/pvpmusic …`: run the settings, show the answer as a chat line and
    /// take the command so the game does not run it.
    fn command(&self, host: &mut dyn ClientHost, event: &mut CommandEvent) -> Flow {
        if !command::is_our_command(event.command) {
            return Flow::Continue;
        }
        let mut ran = false;
        for line in command::command_entry(event.command, &mut ran) {
            let _ = host.chat_line(Text::literal(line));
        }
        if ran {
            Flow::Cancel
        } else {
            Flow::Continue
        }
    }
}

/// The player the camera follows (the local player; None on the menus).
fn camera_player(host: &dyn ClientHost) -> Option<EntityRef> {
    host.camera_entity()
}

fn living(host: &dyn ClientHost, entity: EntityRef) -> Option<LivingRef> {
    host.living(entity)
}

fn game_time(host: &dyn ClientHost) -> i64 {
    camera_player(host)
        .and_then(|p| p.level(host))
        .and_then(|level| level.game_time(host))
        .unwrap_or(0)
}

/// The mobs in reach of the player: id, type id, position and health.
fn mobs_around(host: &dyn ClientHost, player: &EntityRef) -> Vec<MobFacts> {
    let Some(pos) = player.position(host) else {
        return Vec::new();
    };
    let Some(level) = player.level(host) else {
        return Vec::new();
    };
    let Some(list) = level.entities_in(
        host,
        [pos[0] - MOB_REACH, pos[1] - MOB_REACH, pos[2] - MOB_REACH],
        [pos[0] + MOB_REACH, pos[1] + MOB_REACH, pos[2] + MOB_REACH],
    ) else {
        return Vec::new();
    };
    let mut mobs = Vec::new();
    for entity in list {
        if entity.id() == player.id() {
            continue;
        }
        // Living entities only: the state machine cares about health.
        let Some(living) = living(host, entity) else {
            continue;
        };
        let health = living.health(host).unwrap_or(0.0);
        let kind = entity
            .kind(host)
            .map(|k| k.id(host))
            .unwrap_or_else(|| "unknown".to_string());
        let position = entity.position(host).unwrap_or([0.0; 3]);
        mobs.push(MobFacts {
            id: entity.id(),
            kind,
            position,
            health,
        });
    }
    mobs
}

/// Gather everything the state machine reads this tick.
fn snapshot(host: &dyn ClientHost) -> Snapshot {
    let player = camera_player(host);
    let (position, health) = player
        .as_ref()
        .map(|p| {
            (
                p.position(host).unwrap_or([0.0; 3]),
                host.living(*p).and_then(|l| l.health(host)).unwrap_or(0.0),
            )
        })
        .unwrap_or(([0.0; 3], 0.0));
    let mobs = player.as_ref().map(|p| mobs_around(host, p)).unwrap_or_default();
    let (attacks, hurts) = state::drain_pending();
    Snapshot {
        player: Some(PlayerFacts { position, health }),
        mobs,
        attacks,
        hurts,
    }
}

