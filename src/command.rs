//! `/pvpmusic`: the settings the original keeps in its config screen,
//! driven by the client command hook. The hook's answer returns the lines to
//! show; the wiring in `lib.rs` sends them through the client text host.
use crate::state::{Config, Manager, STATE, HEADS, USAGE};

/// Handle one `/pvpmusic` or `/pvm` command. `ran` says the command was ours.
/// Returns the lines to show the player.
pub fn command_entry(command: &str, ran: &mut bool) -> Vec<String> {
    let rest = command.trim();
    let (head, args) = rest.split_once(' ').unwrap_or((rest, ""));
    let head = head.trim_start_matches('/').to_lowercase();
    if !HEADS.contains(&head.as_str()) {
        return Vec::new();
    }
    *ran = true;
    let mut out = Vec::new();
    let mut manager = STATE.lock().unwrap_or_else(|e| e.into_inner());
    let manager = manager.get_or_insert_with(Manager::new);
    let mut parts = args.split_whitespace();
    match parts.next().unwrap_or("status") {
        "status" => {
            out.push(format!(
                "pvp_music: volume {} fadeout {}s fadein {}s deathfade {} mob {} mobfadeout {}s mobfadein {}s mobdeathfade {} phased {} beat {} threshold {} shortend {} longend {} disabled [{}]",
                manager.cfg.volume as i64,
                manager.cfg.fade_out,
                manager.cfg.fade_in,
                manager.cfg.death_fadeout_multiplier,
                if manager.cfg.mob_battle { "on" } else { "off" },
                manager.cfg.mob_fade_out,
                manager.cfg.mob_fade_in,
                manager.cfg.mob_death_fadeout_multiplier,
                if manager.cfg.phased { "on" } else { "off" },
                manager.cfg.beat_divisor,
                manager.cfg.phase2_health_threshold,
                manager.cfg.short_end_threshold,
                manager.cfg.long_end_threshold,
                manager.cfg.disabled_mobs.join(", ")
            ));
        }
        "volume" => {
            if let Some(v) = parts.next().and_then(|v| v.parse::<f64>().ok()) {
                manager.cfg.volume = v.clamp(0.0, 100.0);
            }
        }
        "fadeout" => {
            if let Some(v) = parts.next().and_then(|v| v.parse::<f64>().ok()) {
                manager.cfg.fade_out = v.max(0.0);
            }
        }
        "fadein" => {
            if let Some(v) = parts.next().and_then(|v| v.parse::<f64>().ok()) {
                manager.cfg.fade_in = v.max(0.0);
            }
        }
        "deathfade" => {
            if let Some(v) = parts.next().and_then(|v| v.parse::<f64>().ok()) {
                manager.cfg.death_fadeout_multiplier = v.max(1.0);
            }
        }
        "mob" => {
            manager.cfg.mob_battle = matches!(parts.next(), Some("on") | None);
        }
        "mobfadeout" => {
            if let Some(v) = parts.next().and_then(|v| v.parse::<f64>().ok()) {
                manager.cfg.mob_fade_out = v.max(0.0);
            }
        }
        "mobfadein" => {
            if let Some(v) = parts.next().and_then(|v| v.parse::<f64>().ok()) {
                manager.cfg.mob_fade_in = v.max(0.0);
            }
        }
        "mobdeathfade" => {
            if let Some(v) = parts.next().and_then(|v| v.parse::<f64>().ok()) {
                manager.cfg.mob_death_fadeout_multiplier = v.max(1.0);
            }
        }
        "beat" => {
            if let Some(v) = parts.next().and_then(|v| v.parse::<i32>().ok()) {
                manager.cfg.beat_divisor = v.max(1);
            }
        }
        "threshold" => {
            if let Some(v) = parts.next().and_then(|v| v.parse::<f64>().ok()) {
                manager.cfg.phase2_health_threshold = v.clamp(0.0, 1.0);
            }
        }
        "phase1" => {
            if let Some(v) = parts.next().and_then(|v| v.parse::<f64>().ok()) {
                manager.cfg.phase1_duration = v.max(1.0);
            }
        }
        "phase2" => {
            if let Some(v) = parts.next().and_then(|v| v.parse::<f64>().ok()) {
                manager.cfg.phase2_duration = v.max(1.0);
            }
        }
        "shortend" => {
            if let Some(v) = parts.next().and_then(|v| v.parse::<i64>().ok()) {
                manager.cfg.short_end_threshold = v;
            }
        }
        "longend" => {
            if let Some(v) = parts.next().and_then(|v| v.parse::<i64>().ok()) {
                manager.cfg.long_end_threshold = v;
            }
        }
        "disabled" => {
            let (sub, mob) = (parts.next(), parts.next().map(|m| m.to_lowercase()));
            if let (Some(sub), Some(mob)) = (sub, mob) {
                match sub {
                    "add" => {
                        if !manager.cfg.disabled_mobs.contains(&mob) {
                            manager.cfg.disabled_mobs.push(mob);
                        }
                    }
                    "remove" => {
                        manager.cfg.disabled_mobs.retain(|m| *m != mob);
                    }
                    _ => {}
                }
            }
        }
        _ => {
            out.push(USAGE.to_string());
        }
    }
    out
}

/// The command hook sees every slash command; this names ours.
pub fn is_our_command(command: &str) -> bool {
    let rest = command.trim();
    let (head, _) = rest.split_once(' ').unwrap_or((rest, ""));
    let head = head.trim_start_matches('/').to_lowercase();
    HEADS.contains(&head.as_str())
}

#[allow(dead_code)]
fn _cfg_reference(_: &Config) {}
