# csgo-server-scanner

A terminal scanner for **legacy CS:GO** servers — finds servers matching your client
build (8802 / version `1.38.8.1`), measures live latency, and shows players, map,
inferred game mode, and country, with favourites you can persist.

Built because CS2 replaced CS:GO on app id `730`, so the public browser mixes the two.
This tool harvests server addresses, queries each one directly over UDP (A2S), and
**badges** which are joinable on your build.

## Features

- **Discovery** — scrapes server addresses from GameTracker, gamemonitoring.net and
  gs4u.net (paginated, deduped). Sources are pluggable: each is just a URL + a shared
  `ip:port` regex.
- **Live data via A2S_INFO** — latency, players, map, version (with the modern
  challenge handshake).
- **Build badges** — `MATCH` (exact `1.38.8.1`), `2023` (the `1.38.x` family),
  `csgo?` (older legacy), `—` (CS2 / not legacy). CS2 shares the `csgo` gamedir *and*
  protocol 17, so badging keys on the version line.
- **Mode** inferred from map prefix + server name (`surf_`, `jb_`, `de_`, …).
- **Country** via ip-api batch geo-IP.
- **Favourites** persisted to `favourites.txt` (one `ip:port` per line, hand-editable).
- **TUI** (ratatui): scroll, sort (relevance/ping/players), filter
  (all/joinable/favourites), favourite, rescan.

## Usage

```sh
cargo run                 # interactive TUI (needs a terminal ≥ ~100 cols)
cargo run -- --once       # print a static table and exit
```

TUI keys: `↑↓`/`jk` move · `f` favourite · `s` sort · `m` filter · `r` rescan · `q` quit.

## Layout

| file | role |
|------|------|
| `src/source.rs` | paginated address harvesting |
| `src/a2s.rs` | A2S_INFO UDP query (challenge handshake) |
| `src/classify.rs` | build-match badge logic |
| `src/mode.rs` | game-mode heuristic |
| `src/geo.rs` | country lookup |
| `src/scan.rs` | the shared harvest → A2S → geo → sort pipeline |
| `src/tui.rs` | ratatui interface |
| `src/bin/spike.rs` | throwaway protocol spike (kept for reference) |
