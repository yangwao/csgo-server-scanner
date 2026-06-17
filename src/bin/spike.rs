//! Phase 0 spike — THROWAWAY CODE.
//!
//! Goal: answer the two unknowns that block the whole project before we build it.
//!   1. Does the Steam master server still hand out legacy CS:GO (appid 730) addresses?
//!   2. What do the version / protocol fields of a *known-good* 8802 server look like
//!      on the wire? (target: 51.77.47.244:27015, which the user can actually join)
//!
//! We hand-roll both protocols here so we can see the raw bytes. The real app will
//! likely lean on the `a2s` crate, but the spike must show us the ground truth first.

use std::net::{Ipv4Addr, SocketAddrV4};
use std::time::{Duration, Instant};
use tokio::net::UdpSocket;
use tokio::time::timeout;

/// The user's known-good, joinable server. This is our oracle for "what 8802 reports".
const KNOWN_GOOD: &str = "51.77.47.244:27015";
const MASTER: &str = "hl2master.steampowered.com:27011";
const TIMEOUT: Duration = Duration::from_secs(3);

#[tokio::main]
async fn main() {
    println!("=== A2S_INFO against known-good server {KNOWN_GOOD} ===");
    match query_a2s_info(KNOWN_GOOD).await {
        Ok(info) => info.print(),
        Err(e) => println!("  A2S query failed: {e}"),
    }

    println!("\n=== Steam master server: first page of \\appid\\730 ===");
    match fetch_master_page().await {
        Ok(addrs) => {
            println!("  got {} addresses (showing up to 10):", addrs.len());
            for a in addrs.iter().take(10) {
                println!("    {a}");
            }
        }
        Err(e) => println!("  master query failed: {e}"),
    }
}

// ---------------------------------------------------------------------------
// A2S_INFO  (with the modern challenge handshake)
// ---------------------------------------------------------------------------

struct A2sInfo {
    latency: Duration,
    protocol: u8,
    name: String,
    map: String,
    folder: String,
    game: String,
    appid: u16,
    players: u8,
    max_players: u8,
    bots: u8,
    version: String,
}

impl A2sInfo {
    fn print(&self) {
        println!("  latency:     {} ms", self.latency.as_millis());
        println!("  protocol:    {}", self.protocol);
        println!("  name:        {}", self.name);
        println!("  map:         {}", self.map);
        println!("  folder:      {}", self.folder);
        println!("  game:        {}", self.game);
        println!("  appid:       {}", self.appid);
        println!("  players:     {}/{} ({} bots)", self.players, self.max_players, self.bots);
        println!("  version:     {}   <-- compare this to client build 8802", self.version);
    }
}

async fn query_a2s_info(addr: &str) -> Result<A2sInfo, String> {
    let sock = UdpSocket::bind("0.0.0.0:0").await.map_err(|e| e.to_string())?;
    sock.connect(addr).await.map_err(|e| e.to_string())?;

    // A2S_INFO request payload.
    let base: &[u8] = b"\xFF\xFF\xFF\xFF\x54Source Engine Query\0";

    let started = Instant::now();
    let mut buf = vec![0u8; 4096];
    let resp = a2s_exchange(&sock, base, &mut buf).await?;

    // 0xFF*4 header, then a type byte. 0x41 = challenge, 0x49 = info.
    let mut c = Cursor::new(resp);
    c.skip(4)?; // 0xFF FF FF FF
    let mut kind = c.u8()?;

    // If we got a challenge, re-send base + the 4 challenge bytes.
    if kind == 0x41 {
        let challenge = c.take(4)?;
        let mut req = base.to_vec();
        req.extend_from_slice(challenge);
        let resp2 = a2s_exchange(&sock, &req, &mut buf).await?;
        c = Cursor::new(resp2);
        c.skip(4)?;
        kind = c.u8()?;
    }

    if kind != 0x49 {
        return Err(format!("unexpected A2S response type 0x{kind:02X}"));
    }

    let protocol = c.u8()?;
    let name = c.cstr()?;
    let map = c.cstr()?;
    let folder = c.cstr()?;
    let game = c.cstr()?;
    let appid = c.u16le()?;
    let players = c.u8()?;
    let max_players = c.u8()?;
    let bots = c.u8()?;
    let _server_type = c.u8()?;
    let _environment = c.u8()?;
    let _visibility = c.u8()?;
    let _vac = c.u8()?;
    let version = c.cstr()?;

    Ok(A2sInfo {
        latency: started.elapsed(),
        protocol,
        name,
        map,
        folder,
        game,
        appid,
        players,
        max_players,
        bots,
        version,
    })
}

/// Send `payload`, wait for one datagram, return the slice we received.
async fn a2s_exchange<'a>(
    sock: &UdpSocket,
    payload: &[u8],
    buf: &'a mut [u8],
) -> Result<&'a [u8], String> {
    sock.send(payload).await.map_err(|e| e.to_string())?;
    let n = timeout(TIMEOUT, sock.recv(buf))
        .await
        .map_err(|_| "timed out".to_string())?
        .map_err(|e| e.to_string())?;
    Ok(&buf[..n])
}

// ---------------------------------------------------------------------------
// Steam master server query
// ---------------------------------------------------------------------------

async fn fetch_master_page() -> Result<Vec<SocketAddrV4>, String> {
    let sock = UdpSocket::bind("0.0.0.0:0").await.map_err(|e| e.to_string())?;
    sock.connect(MASTER).await.map_err(|e| e.to_string())?;

    // 0x31 = request, 0xFF = all regions, seed "0.0.0.0:0", filter \appid\730
    let mut req: Vec<u8> = vec![0x31, 0xFF];
    req.extend_from_slice(b"0.0.0.0:0\0");
    req.extend_from_slice(b"\\appid\\730\0");

    sock.send(&req).await.map_err(|e| e.to_string())?;

    let mut buf = vec![0u8; 4096];
    let n = timeout(TIMEOUT, sock.recv(&mut buf))
        .await
        .map_err(|_| "timed out".to_string())?
        .map_err(|e| e.to_string())?;

    // Response: 0xFF FF FF FF 66 0A, then 6-byte entries (IP big-endian, port big-endian).
    // Terminated by 0.0.0.0:0.
    let data = &buf[..n];
    if data.len() < 6 || &data[..6] != b"\xFF\xFF\xFF\xFF\x66\x0A" {
        return Err(format!("unexpected master header: {:02X?}", &data[..data.len().min(6)]));
    }

    let mut out = Vec::new();
    for chunk in data[6..].chunks_exact(6) {
        let ip = Ipv4Addr::new(chunk[0], chunk[1], chunk[2], chunk[3]);
        let port = u16::from_be_bytes([chunk[4], chunk[5]]);
        if ip.is_unspecified() && port == 0 {
            break; // end-of-list sentinel
        }
        out.push(SocketAddrV4::new(ip, port));
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// tiny byte cursor
// ---------------------------------------------------------------------------

struct Cursor<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn new(data: &'a [u8]) -> Self {
        Cursor { data, pos: 0 }
    }
    fn skip(&mut self, n: usize) -> Result<(), String> {
        self.take(n).map(|_| ())
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        if self.pos + n > self.data.len() {
            return Err("unexpected end of buffer".into());
        }
        let s = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }
    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }
    fn u16le(&mut self) -> Result<u16, String> {
        let b = self.take(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }
    fn cstr(&mut self) -> Result<String, String> {
        let start = self.pos;
        while self.pos < self.data.len() && self.data[self.pos] != 0 {
            self.pos += 1;
        }
        let s = String::from_utf8_lossy(&self.data[start..self.pos]).into_owned();
        self.pos += 1; // consume the null terminator
        Ok(s)
    }
}
