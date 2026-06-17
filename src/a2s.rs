//! A2S_INFO querying — handles the modern challenge handshake and measures latency.
//! Proven against the user's known-good server in the Phase 0 spike.

use crate::model::ServerInfo;
use std::net::SocketAddrV4;
use std::time::{Duration, Instant};
use tokio::net::UdpSocket;
use tokio::time::timeout;

/// Query one server's A2S_INFO. Returns latency-measured server info, or an error
/// (timeout / unreachable / malformed response).
pub async fn query(addr: SocketAddrV4, to: Duration) -> Result<ServerInfo, String> {
    let sock = UdpSocket::bind("0.0.0.0:0").await.map_err(|e| e.to_string())?;
    sock.connect(addr).await.map_err(|e| e.to_string())?;

    let base: &[u8] = b"\xFF\xFF\xFF\xFF\x54Source Engine Query\0";

    let started = Instant::now();
    let mut buf = vec![0u8; 4096];
    let resp = exchange(&sock, base, &mut buf, to).await?;

    let mut c = Cursor::new(resp);
    c.skip(4)?; // 0xFF FF FF FF
    let mut kind = c.u8()?;

    // 0x41 = challenge: re-send base + the 4 challenge bytes.
    if kind == 0x41 {
        let challenge = c.take(4)?;
        let mut req = base.to_vec();
        req.extend_from_slice(challenge);
        let resp2 = exchange(&sock, &req, &mut buf, to).await?;
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
    let _appid = c.u16le()?;
    let players = c.u8()?;
    let max_players = c.u8()?;
    let bots = c.u8()?;
    let _server_type = c.u8()?;
    let _environment = c.u8()?;
    let _visibility = c.u8()?;
    let _vac = c.u8()?;
    let version = c.cstr()?;

    Ok(ServerInfo {
        addr,
        latency: started.elapsed(),
        protocol,
        name,
        map,
        folder,
        game,
        players,
        max_players,
        bots,
        version,
        country: String::new(), // filled by the geo pass after scanning
    })
}

async fn exchange<'a>(
    sock: &UdpSocket,
    payload: &[u8],
    buf: &'a mut [u8],
    to: Duration,
) -> Result<&'a [u8], String> {
    sock.send(payload).await.map_err(|e| e.to_string())?;
    let n = timeout(to, sock.recv(buf))
        .await
        .map_err(|_| "timed out".to_string())?
        .map_err(|e| e.to_string())?;
    Ok(&buf[..n])
}

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
        self.pos += 1; // consume null terminator
        Ok(s)
    }
}
