//! "Anti-favourites": servers you couldn't connect to (e.g. no-steam servers that
//! reject STEAM validation). Persisted as plain text (one `ip:port` per line) so the
//! file stays human-editable, mirroring `favourites.rs`.

use std::collections::BTreeSet;
use std::fs;
use std::net::SocketAddrV4;

const FILE: &str = "blocklist.txt";

#[derive(Default)]
pub struct Blocklist {
    set: BTreeSet<SocketAddrV4>,
}

impl Blocklist {
    /// Load from disk. A missing or malformed file just yields an empty set —
    /// the blocklist should never block startup.
    pub fn load() -> Self {
        let set = fs::read_to_string(FILE)
            .unwrap_or_default()
            .lines()
            .filter_map(|l| l.trim().parse::<SocketAddrV4>().ok())
            .collect();
        Blocklist { set }
    }

    fn save(&self) {
        let body = self
            .set
            .iter()
            .map(|a| a.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        let _ = fs::write(FILE, body);
    }

    /// Flip blocked state for an address, persist immediately, return new state.
    pub fn toggle(&mut self, addr: SocketAddrV4) -> bool {
        let now_blocked = if self.set.remove(&addr) {
            false
        } else {
            self.set.insert(addr);
            true
        };
        self.save();
        now_blocked
    }

    pub fn contains(&self, addr: &SocketAddrV4) -> bool {
        self.set.contains(addr)
    }
}
