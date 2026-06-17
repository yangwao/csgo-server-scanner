//! Address discovery. Every source reduces to the same job: GET a page and regex
//! `ip:port` candidates out of it. No per-site table parsing — A2S gives us the
//! authoritative live data later. Adding a source = one more entry below.

use regex::Regex;
use std::collections::BTreeSet;
use std::net::SocketAddrV4;

const USER_AGENT: &str =
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 \
     (KHTML, like Gecko) Chrome/120.0 Safari/537.36";

/// A scrapable source: a URL template with `{page}` and how many pages to walk.
struct Source {
    name: &'static str,
    template: &'static str,
    pages: u32,
}

/// Validated pagination params: GameTracker uses `searchpge`, the others `page`.
const SOURCES: &[Source] = &[
    Source {
        name: "gametracker",
        template: "https://www.gametracker.com/search/csgo/?searchipp=50&searchpge={page}",
        pages: 5,
    },
    Source {
        name: "gamemonitoring",
        template: "https://gamemonitoring.net/counter-strike-global-offensive/servers?page={page}",
        pages: 3,
    },
    Source {
        name: "gs4u",
        template: "https://gs4u.net/en/csgo?page={page}",
        pages: 3,
    },
];

/// Harvest a deduped, sorted set of candidate addresses from all sources and pages.
/// `seed` addresses (favourites, known-good servers) are always included so they get
/// queried even if no source lists them right now. Failing pages are logged, not fatal.
pub async fn harvest(seed: &[SocketAddrV4]) -> BTreeSet<SocketAddrV4> {
    let re = Regex::new(r"\b(?:\d{1,3}\.){3}\d{1,3}:\d{2,5}\b").unwrap();
    let client = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .build()
        .expect("build http client");

    let mut found: BTreeSet<SocketAddrV4> = seed.iter().copied().collect();
    for src in SOURCES {
        let before = found.len();
        for page in 1..=src.pages {
            let url = src.template.replace("{page}", &page.to_string());
            match fetch(&client, &url).await {
                Ok(body) => {
                    for m in re.find_iter(&body) {
                        if let Ok(addr) = m.as_str().parse::<SocketAddrV4>() {
                            found.insert(addr);
                        }
                    }
                }
                Err(e) => eprintln!("  {} p{page}: skipped ({e})", src.name),
            }
        }
        eprintln!("  {}: +{} addresses", src.name, found.len() - before);
    }
    found
}

async fn fetch(client: &reqwest::Client, url: &str) -> Result<String, String> {
    let resp = client.get(url).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    resp.text().await.map_err(|e| e.to_string())
}
