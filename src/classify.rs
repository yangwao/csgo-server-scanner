//! Badge classification — the heart of the matcher.
//!
//! Encodes the user's decision: older legacy CS:GO builds are *probably* joinable
//! from an 8802 client, so we surface them as candidates (OtherCsgo) rather than
//! hiding them — but we still distinguish the exact known-good build and the 2023
//! final family so the confidence is visible.
//!
//! Ground truth from the Phase 0 spike (51.77.47.244:27015):
//!   version "1.38.8.1", protocol 17, folder "csgo".

use crate::model::{Badge, ServerInfo, ORACLE_VERSION};

/// Decide which badge a server gets relative to the user's client build.
pub fn classify(s: &ServerInfo) -> Badge {
    // Both CS:GO and CS2 use the "csgo" gamedir, so folder can't separate them.
    // A non-csgo gamedir is some other game entirely.
    if s.folder != "csgo" {
        return Badge::NotCsgo;
    }
    // IMPORTANT (learned from live data): CS2 ALSO uses gamedir "csgo" AND network
    // protocol 17 — neither separates it from legacy CS:GO. The only clean signal is
    // the version line: legacy CS:GO is the "1.3x" series (1.38.x final), CS2 is "1.4x"
    // (e.g. 1.41.6.5). So gate purely on the version prefix.
    let is_legacy = s.version.starts_with("1.3");
    if !is_legacy {
        return Badge::NotCsgo;
    }

    if s.version == ORACLE_VERSION {
        Badge::Match // exact build we KNOW is joinable
    } else if s.version.starts_with("1.38.") {
        Badge::Legacy2023 // 2023 final family — the favoured build line
    } else {
        Badge::OtherCsgo // older legacy build — the bet: probably joinable
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddrV4;
    use std::time::Duration;

    fn srv(version: &str, protocol: u8, folder: &str) -> ServerInfo {
        ServerInfo {
            addr: "127.0.0.1:27015".parse::<SocketAddrV4>().unwrap(),
            latency: Duration::from_millis(50),
            protocol,
            name: "test".into(),
            map: "de_dust2".into(),
            folder: folder.into(),
            game: "Counter-Strike: Global Offensive".into(),
            players: 5,
            max_players: 12,
            bots: 0,
            version: version.into(),
            country: String::new(),
        }
    }

    #[test]
    fn known_good_server_is_match() {
        // The user's actual joinable server.
        assert_eq!(classify(&srv("1.38.8.1", 17, "csgo")), Badge::Match);
    }

    #[test]
    fn other_2023_build_is_legacy_family() {
        assert_eq!(classify(&srv("1.38.7.3", 17, "csgo")), Badge::Legacy2023);
    }

    #[test]
    fn older_csgo_is_candidate() {
        assert_eq!(classify(&srv("1.37.5.0", 17, "csgo")), Badge::OtherCsgo);
    }

    #[test]
    fn cs2_shaped_server_is_not_csgo() {
        // Real CS2 (observed live): gamedir "csgo" AND protocol 17 — only the
        // version "1.4x" line gives it away.
        assert_eq!(classify(&srv("1.41.6.5", 17, "csgo")), Badge::NotCsgo);
    }

    #[test]
    fn other_game_is_not_csgo() {
        assert_eq!(classify(&srv("1.38.8.1", 17, "tf")), Badge::NotCsgo);
    }
}
