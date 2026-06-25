//! csgo-server-scanner — finds CS:GO servers matching your client build (8802).
//!
//! Default: interactive TUI.   `--once`: print a static table and exit.

mod a2s;
mod blocklist;
mod classify;
mod favourites;
mod geo;
mod mode;
mod model;
mod scan;
mod source;
mod tui;

use blocklist::Blocklist;
use favourites::Favourites;
use model::{Badge, ServerInfo};
use std::net::SocketAddrV4;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let once = std::env::args().any(|a| a == "--once");

    let favs = Favourites::load();
    let blocked = Blocklist::load();
    let mut seed: Vec<SocketAddrV4> = favs.addrs();
    if let Ok(a) = scan::KNOWN_GOOD.parse() {
        seed.push(a);
    }

    eprintln!("Scanning… (harvest + A2S)");
    let servers = scan::scan(&seed).await;

    if once {
        print_table(&servers, &favs, &blocked);
        Ok(())
    } else {
        tui::run(servers, favs, blocked).await
    }
}

fn print_table(servers: &[ServerInfo], favs: &Favourites, blocked: &Blocklist) {
    println!(
        "{:<2}{:<6} {:>6} {:>6} {:<12} {:<3} {:<16} {}",
        "", "BADGE", "PING", "PLRS", "MODE", "CC", "MAP", "ADDR"
    );
    println!("{}", "-".repeat(96));
    for s in servers {
        let badge = classify::classify(s);
        let star = if blocked.contains(&s.addr) {
            "✗ "
        } else if favs.contains(&s.addr) {
            "★ "
        } else {
            "  "
        };
        println!(
            "{}{:<6} {:>4}ms {:>5} {:<12} {:<3} {:<16} {}",
            star,
            badge.label(),
            s.latency.as_millis(),
            format!("{}/{}", s.players, s.max_players),
            mode::infer(s),
            if s.country.is_empty() { "??" } else { &s.country },
            trunc(&s.map, 16),
            s.addr,
        );
    }
    let joinable = servers
        .iter()
        .filter(|s| matches!(classify::classify(s), Badge::Match | Badge::Legacy2023))
        .count();
    println!(
        "\n{} live servers, {} joinable for your build (MATCH/2023).",
        servers.len(),
        joinable
    );
}

fn trunc(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let t: String = s.chars().take(max.saturating_sub(1)).collect();
        format!("{t}…")
    }
}
