//! Infer a server's game mode from signals we already have (map name + server name).
//!
//! A2S_INFO doesn't report mode, and A2S_RULES is unreliable for community servers,
//! so we read the conventions the CS:GO community already follows: map prefixes
//! (`surf_`, `jb_`, `ze_`, `de_`, ...) and name keywords. This is a heuristic — tune
//! the rules/order below to taste; the first match wins.

use crate::model::ServerInfo;

pub fn infer(s: &ServerInfo) -> &'static str {
    // Workshop maps arrive as "workshop/<id>/<actual_map>" — match on the basename.
    let map = s.map.rsplit('/').next().unwrap_or(&s.map).to_lowercase();
    let name = s.name.to_lowercase();
    let has = |needle: &str| map.starts_with(needle);
    let named = |needle: &str| name.contains(needle);

    // Map-prefix is the strongest signal; fall back to name keywords.
    if has("surf_") {
        "Surf"
    } else if has("bhop_") {
        "Bhop"
    } else if has("kz_") || has("climb_") {
        "KZ/Climb"
    } else if has("jb_") || has("ba_jail") || named("jail") {
        "Jailbreak"
    } else if has("ze_") {
        "Zombie Esc"
    } else if has("zm_") || named("zombie") {
        "Zombie"
    } else if has("aim_") {
        "Aim"
    } else if has("awp_") || named("awp") {
        "AWP"
    } else if has("gg_") || named("gungame") || named("arms race") {
        "GunGame"
    } else if has("am_") || named("retake") {
        "Retake/Arena"
    } else if named("deathmatch") || named("ffa") || named(" dm ") || named("[dm]") {
        "Deathmatch"
    } else if has("de_") {
        "Bomb"
    } else if has("cs_") {
        "Hostage"
    } else {
        "?"
    }
}
