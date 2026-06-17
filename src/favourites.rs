//! Favourite servers, persisted as plain text (one `ip:port` per line) so the file
//! stays human-editable and we avoid a serialization dependency.

use std::collections::BTreeSet;
use std::fs;
use std::net::SocketAddrV4;

const FILE: &str = "favourites.txt";

#[derive(Default)]
pub struct Favourites {
    set: BTreeSet<SocketAddrV4>,
}

impl Favourites {
    /// Load from disk. A missing or malformed file just yields an empty set —
    /// favourites should never block startup.
    pub fn load() -> Self {
        let set = fs::read_to_string(FILE)
            .unwrap_or_default()
            .lines()
            .filter_map(|l| l.trim().parse::<SocketAddrV4>().ok())
            .collect();
        Favourites { set }
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

    /// Flip favourite state for an address, persist immediately, return new state.
    pub fn toggle(&mut self, addr: SocketAddrV4) -> bool {
        let now_fav = if self.set.remove(&addr) {
            false
        } else {
            self.set.insert(addr);
            true
        };
        self.save();
        now_fav
    }

    pub fn contains(&self, addr: &SocketAddrV4) -> bool {
        self.set.contains(addr)
    }

    pub fn addrs(&self) -> Vec<SocketAddrV4> {
        self.set.iter().copied().collect()
    }
}
