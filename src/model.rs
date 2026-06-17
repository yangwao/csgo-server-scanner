//! Core data types shared across the scanner.

use std::net::SocketAddrV4;
use std::time::Duration;

/// The golden value from your known-good, joinable server (51.77.47.244:27015).
/// A2S reports the `steam.inf` PatchVersion string here — NOT the build int 8802.
pub const ORACLE_VERSION: &str = "1.38.8.1";

/// What A2S_INFO told us about a single live server.
#[derive(Debug, Clone)]
pub struct ServerInfo {
    pub addr: SocketAddrV4,
    pub latency: Duration,
    pub protocol: u8,
    pub name: String,
    pub map: String,
    /// Game directory, e.g. "csgo". Our coarse "is this even CS:GO" signal.
    pub folder: String,
    pub game: String,
    pub players: u8,
    pub max_players: u8,
    pub bots: u8,
    /// PatchVersion string, e.g. "1.38.8.1".
    pub version: String,
    /// ISO country code from geo-IP, filled in after the A2S pass (empty until then).
    pub country: String,
}

/// How a server relates to YOUR client build. We show all servers and badge them,
/// rather than hiding non-matches (your chosen UX).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Badge {
    /// Same build your client runs — should be directly joinable.
    Match,
    /// The 2023 final CS:GO family (1.38.x) but not your exact build — likely joinable.
    Legacy2023,
    /// A CS:GO server on some other/older version.
    OtherCsgo,
    /// Not legacy CS:GO at all (e.g. CS2, or a non-csgo gamedir).
    NotCsgo,
}

impl Badge {
    /// Sort priority: lower = more interesting (shown first).
    pub fn rank(self) -> u8 {
        match self {
            Badge::Match => 0,
            Badge::Legacy2023 => 1,
            Badge::OtherCsgo => 2,
            Badge::NotCsgo => 3,
        }
    }

    /// Short tag for table display.
    pub fn label(self) -> &'static str {
        match self {
            Badge::Match => "MATCH",
            Badge::Legacy2023 => "2023",
            Badge::OtherCsgo => "csgo?",
            Badge::NotCsgo => "—",
        }
    }
}
