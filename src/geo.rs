//! Geo-IP country lookup via ip-api.com's free batch endpoint (≤100 IPs/request,
//! ~45 req/min). One batch per 100 servers, run once per scan. Best-effort: any
//! failure just leaves those servers without a country.

use serde_json::Value;
use std::collections::HashMap;
use std::net::Ipv4Addr;

const BATCH_URL: &str = "http://ip-api.com/batch?fields=countryCode,query";

/// Map each IP to its ISO country code. Missing/failed lookups are simply absent.
pub async fn lookup(ips: &[Ipv4Addr]) -> HashMap<Ipv4Addr, String> {
    let mut out = HashMap::new();
    let client = reqwest::Client::new();

    for chunk in ips.chunks(100) {
        let body = format!(
            "[{}]",
            chunk
                .iter()
                .map(|ip| format!("\"{ip}\""))
                .collect::<Vec<_>>()
                .join(",")
        );

        let resp = client
            .post(BATCH_URL)
            .header("content-type", "application/json")
            .body(body)
            .send()
            .await;

        let Ok(resp) = resp else { continue };
        let Ok(text) = resp.text().await else { continue };
        let Ok(Value::Array(items)) = serde_json::from_str::<Value>(&text) else {
            continue;
        };

        for item in items {
            let cc = item.get("countryCode").and_then(Value::as_str);
            let q = item.get("query").and_then(Value::as_str);
            if let (Some(cc), Some(q)) = (cc, q) {
                if let Ok(ip) = q.parse::<Ipv4Addr>() {
                    out.insert(ip, cc.to_string());
                }
            }
        }
    }
    out
}
