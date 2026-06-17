//! Reusable scan pipeline shared by the CLI (--once) and the TUI:
//! harvest ip:port -> A2S each concurrently -> sort (badge, then latency).

use crate::classify;
use crate::model::ServerInfo;
use crate::{a2s, geo, source};
use std::net::SocketAddrV4;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;

const A2S_TIMEOUT: Duration = Duration::from_secs(2);
const MAX_CONCURRENT: usize = 256;

/// The user's known-good server — always queried so it shows up even if no source
/// happens to list it on a given day.
pub const KNOWN_GOOD: &str = "51.77.47.244:27015";

/// Run a full scan. `seed` (favourites + known-good) is always queried.
pub async fn scan(seed: &[SocketAddrV4]) -> Vec<ServerInfo> {
    let candidates = source::harvest(seed).await;

    let sem = Arc::new(Semaphore::new(MAX_CONCURRENT));
    let mut tasks = Vec::new();
    for addr in candidates {
        let permit = Arc::clone(&sem);
        tasks.push(tokio::spawn(async move {
            let _p = permit.acquire().await.unwrap();
            a2s::query(addr, A2S_TIMEOUT).await
        }));
    }

    let mut servers: Vec<ServerInfo> = Vec::new();
    for t in tasks {
        if let Ok(Ok(info)) = t.await {
            servers.push(info);
        }
    }

    // Geo-enrich: one batched lookup for all live servers, then stamp each row.
    let ips: Vec<_> = servers.iter().map(|s| *s.addr.ip()).collect();
    let countries = geo::lookup(&ips).await;
    for s in &mut servers {
        if let Some(cc) = countries.get(s.addr.ip()) {
            s.country = cc.clone();
        }
    }

    servers.sort_by(|a, b| {
        classify::classify(a)
            .rank()
            .cmp(&classify::classify(b).rank())
            .then(a.latency.cmp(&b.latency))
    });
    servers
}
