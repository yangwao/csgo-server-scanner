//! Interactive terminal UI (ratatui): scroll, sort, filter, favourite, rescan.

use crate::blocklist::Blocklist;
use crate::favourites::Favourites;
use crate::model::{Badge, ServerInfo};
use crate::{classify, mode, scan};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState};
use ratatui::Frame;
use std::net::SocketAddrV4;
use std::time::Duration;

#[derive(Clone, Copy, PartialEq)]
enum Sort {
    Relevance, // badge rank, then ping
    Ping,
    Players,
}

#[derive(Clone, Copy, PartialEq)]
enum Filter {
    All,
    Joinable, // Match + Legacy2023
    Favourites,
}

struct App {
    servers: Vec<ServerInfo>,
    favs: Favourites,
    blocked: Blocklist,
    sort: Sort,
    filter: Filter,
    status: String,
}

impl App {
    /// Indices into `servers` to display, after filter + sort.
    fn view(&self) -> Vec<usize> {
        let mut idx: Vec<usize> = (0..self.servers.len())
            .filter(|&i| {
                let s = &self.servers[i];
                match self.filter {
                    Filter::All => true,
                    Filter::Joinable => {
                        matches!(classify::classify(s), Badge::Match | Badge::Legacy2023)
                    }
                    Filter::Favourites => self.favs.contains(&s.addr),
                }
            })
            .collect();

        idx.sort_by(|&a, &b| {
            let (sa, sb) = (&self.servers[a], &self.servers[b]);
            match self.sort {
                Sort::Relevance => classify::classify(sa)
                    .rank()
                    .cmp(&classify::classify(sb).rank())
                    .then(sa.latency.cmp(&sb.latency)),
                Sort::Ping => sa.latency.cmp(&sb.latency),
                Sort::Players => sb.players.cmp(&sa.players),
            }
        });
        idx
    }
}

/// Seed addresses always worth querying: the known-good server + every favourite.
fn seed(favs: &Favourites) -> Vec<SocketAddrV4> {
    let mut v = favs.addrs();
    if let Ok(a) = scan::KNOWN_GOOD.parse() {
        v.push(a);
    }
    v
}

pub async fn run(
    servers: Vec<ServerInfo>,
    favs: Favourites,
    blocked: Blocklist,
) -> std::io::Result<()> {
    let mut app = App {
        status: format!("{} servers", servers.len()),
        servers,
        favs,
        blocked,
        sort: Sort::Relevance,
        filter: Filter::All,
    };
    let mut state = TableState::default().with_selected(Some(0));

    let mut terminal = ratatui::init();
    let result = loop {
        let view = app.view();
        clamp(&mut state, view.len());
        if let Err(e) = terminal.draw(|f| ui(f, &app, &view, &mut state)) {
            break Err(e);
        }

        if !event::poll(Duration::from_millis(250))? {
            continue;
        }
        let Event::Key(key) = event::read()? else { continue };
        if key.kind != KeyEventKind::Press {
            continue;
        }
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => break Ok(()),
            KeyCode::Down | KeyCode::Char('j') => move_sel(&mut state, view.len(), 1),
            KeyCode::Up | KeyCode::Char('k') => move_sel(&mut state, view.len(), -1),
            KeyCode::Char('s') => {
                app.sort = match app.sort {
                    Sort::Relevance => Sort::Ping,
                    Sort::Ping => Sort::Players,
                    Sort::Players => Sort::Relevance,
                };
            }
            KeyCode::Char('m') => {
                app.filter = match app.filter {
                    Filter::All => Filter::Joinable,
                    Filter::Joinable => Filter::Favourites,
                    Filter::Favourites => Filter::All,
                };
            }
            KeyCode::Char('f') => {
                if let Some(&i) = state.selected().and_then(|sel| view.get(sel)) {
                    let addr = app.servers[i].addr;
                    let now = app.favs.toggle(addr);
                    app.status = format!("{} {}", if now { "★ favourited" } else { "☆ unfavourited" }, addr);
                }
            }
            KeyCode::Char('b') => {
                if let Some(&i) = state.selected().and_then(|sel| view.get(sel)) {
                    let addr = app.servers[i].addr;
                    let now = app.blocked.toggle(addr);
                    app.status = format!("{} {}", if now { "✗ blocked" } else { "✓ unblocked" }, addr);
                }
            }
            KeyCode::Char('r') => {
                app.status = "Scanning… (please wait)".into();
                let view = app.view();
                let _ = terminal.draw(|f| ui(f, &app, &view, &mut state));
                let s = seed(&app.favs);
                app.servers = scan::scan(&s).await;
                app.status = format!("{} servers (rescanned)", app.servers.len());
            }
            _ => {}
        }
    };
    ratatui::restore();
    result
}

fn ui(f: &mut Frame, app: &App, view: &[usize], state: &mut TableState) {
    let chunks = Layout::vertical([Constraint::Min(1), Constraint::Length(3)]).split(f.area());

    let header = Row::new(["", "BADGE", "PING", "PLRS", "MODE", "CC", "MAP", "ADDR", "NAME"])
        .style(Style::new().bold().underlined());

    let rows = view.iter().map(|&i| {
        let s = &app.servers[i];
        let badge = classify::classify(s);
        let color = match badge {
            Badge::Match => Color::Green,
            Badge::Legacy2023 => Color::Cyan,
            Badge::OtherCsgo => Color::Yellow,
            Badge::NotCsgo => Color::DarkGray,
        };
        let is_blocked = app.blocked.contains(&s.addr);
        // ✗ (couldn't connect) takes precedence over ★ (favourite): the blocklist is
        // the more recent "this one is actually broken" signal.
        let (marker, marker_color) = if is_blocked {
            ("✗", Color::Red)
        } else if app.favs.contains(&s.addr) {
            ("★", Color::Yellow)
        } else {
            (" ", Color::Yellow)
        };
        let cc = if s.country.is_empty() { "??" } else { &s.country };
        Row::new(vec![
            Cell::from(marker).style(Style::new().fg(marker_color)),
            Cell::from(badge.label()),
            Cell::from(format!("{}ms", s.latency.as_millis())),
            Cell::from(format!("{}/{}", s.players, s.max_players)),
            Cell::from(mode::infer(s)),
            Cell::from(cc),
            Cell::from(trunc(&s.map, 16)),
            Cell::from(s.addr.to_string()),
            Cell::from(trunc(&s.name, 30)),
        ])
        .style(row_style(color, is_blocked))
    });

    let widths = [
        Constraint::Length(1),
        Constraint::Length(5),
        Constraint::Length(6),
        Constraint::Length(6),
        Constraint::Length(11),
        Constraint::Length(2),
        Constraint::Length(16),
        Constraint::Length(21),
        Constraint::Min(10),
    ];
    let table = Table::new(rows, widths)
        .header(header)
        .block(Block::default().borders(Borders::ALL).title(" CS:GO servers — build 8802 (1.38.8.1) "))
        .row_highlight_style(Style::new().add_modifier(Modifier::REVERSED))
        .highlight_symbol("▶ ");
    f.render_stateful_widget(table, chunks[0], state);

    let sort = match app.sort {
        Sort::Relevance => "relevance",
        Sort::Ping => "ping",
        Sort::Players => "players",
    };
    let filter = match app.filter {
        Filter::All => "all",
        Filter::Joinable => "joinable",
        Filter::Favourites => "favourites",
    };
    let help = Line::from(vec![
        "↑↓".bold(), " move  ".into(),
        "f".bold(), " favourite  ".into(),
        "b".bold(), " block  ".into(),
        "s".bold(), format!(" sort:{sort}  ").into(),
        "m".bold(), format!(" filter:{filter}  ").into(),
        "r".bold(), " rescan  ".into(),
        "q".bold(), " quit".into(),
    ]);
    let footer = Paragraph::new(vec![help, Line::from(app.status.clone())])
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(footer, chunks[1]);
}

/// Style for one table row.
///
/// `badge_color` is the per-badge colour (green = MATCH, cyan = 2023, …) already
/// chosen by the caller. `is_blocked` is true for anti-favourited servers — the ones
/// you couldn't connect to and want de-emphasised but still visible.
///
/// TODO(you): decide how a blocked row should look. The non-blocked case must stay
/// `Style::new().fg(badge_color)` so normal rows are unchanged. For blocked rows,
/// pick a treatment that says "ignore me" without hiding the badge signal entirely.
/// Things you can reach for on a ratatui `Style`:
///   .add_modifier(Modifier::DIM)          // fade the whole row
///   .add_modifier(Modifier::CROSSED_OUT)  // strike-through (terminal-dependent)
///   .fg(Color::DarkGray)                  // override colour, drops the badge hue
/// Trade-off: DIM keeps the green/cyan badge hue (you still see it *was* a MATCH),
/// while forcing DarkGray reads as "dead" but throws away that colour information.
fn row_style(badge_color: Color, is_blocked: bool) -> Style {
    if is_blocked {
        // TODO(you): replace this placeholder with your chosen blocked-row treatment.
        Style::new().fg(badge_color)
    } else {
        Style::new().fg(badge_color)
    }
}

fn clamp(state: &mut TableState, len: usize) {
    if len == 0 {
        state.select(None);
    } else if state.selected().map_or(true, |s| s >= len) {
        state.select(Some(len - 1));
    } else if state.selected().is_none() {
        state.select(Some(0));
    }
}

fn move_sel(state: &mut TableState, len: usize, delta: isize) {
    if len == 0 {
        return;
    }
    let cur = state.selected().unwrap_or(0) as isize;
    let next = (cur + delta).rem_euclid(len as isize) as usize;
    state.select(Some(next));
}

fn trunc(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let t: String = s.chars().take(max.saturating_sub(1)).collect();
        format!("{t}…")
    }
}
