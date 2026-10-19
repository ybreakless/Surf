//! Live FX rate provider backed by tgju.org (Iranian free-market rates).
//!
//! tgju publishes profile pages per instrument:
//!   https://www.tgju.org/profile/price_dollar_rl  (USD)
//!   https://www.tgju.org/profile/price_eur        (EUR)
//!   https://www.tgju.org/profile/price_aed        (AED)
//!   https://www.tgju.org/profile/price_cny        (CNY)
//!   https://www.tgju.org/profile/price_rub        (RUB)
//!
//! Each page contains a span like:
//!   <span class="price" data-col="info.last_trade.PDrCotVal">2,638,000</span>
//! that holds the latest free-market value in IRR for one unit of the currency.

use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use uuid::Uuid;

use crate::core::fx::{FxRate, RateSource};
use crate::core::money::Currency;
use crate::db::repository::Repository;

/// (currency, tgju profile slug)
pub const TGJU_SYMBOLS: [(&str, &str); 5] = [
    ("USD", "price_dollar_rl"),
    ("EUR", "price_eur"),
    ("AED", "price_aed"),
    ("CNY", "price_cny"),
    ("RUB", "price_rub"),
];

const TGJU_BASE_URL: &str = "https://www.tgju.org/profile";
const USER_AGENT: &str = "Surf/0.1 (+local accounting desktop app)";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Debug)]
pub enum FxUpdateMsg {
    Progress(String),
    Done(Result<(usize, Vec<String>), String>),
}

/// Parses the tgju profile HTML of one currency page.
/// Returns IRR-per-unit rate (integer) or an error string for diagnostics.
pub fn parse_tgju_html(html: &str) -> Result<i64, String> {
    // Match the span regardless of attribute order around data-col.
    let re = regex::Regex::new(r#"data-col="info\.last_trade\.PDrCotVal"[^>]*>\s*([0-9][0-9,]*)"#)
        .map_err(|e| format!("regex error: {e}"))?;

    let captures = re
        .captures_iter(html)
        .next()
        .ok_or_else(|| "price span not found (page layout changed or blocked)".to_string())?;

    let raw = &captures[1];
    let val: i64 = raw
        .replace(',', "")
        .parse::<i64>()
        .map_err(|e| format!("could not parse '{raw}' as positive int: {e}"))?;
    if val > 0 {
        Ok(val)
    } else {
        Err(format!("parsed non-positive rate: {val}"))
    }
}

/// Fetches one currency's current free-market IRR rate from tgju.org.
/// Blocking network call - use only from worker threads or CLI.
pub fn fetch_tgju_rate(slug: &str) -> Result<(i64, String), String> {
    let url = format!("{TGJU_BASE_URL}/{slug}");
    let agent = ureq::AgentBuilder::new()
        .timeout(REQUEST_TIMEOUT)
        .user_agent(USER_AGENT)
        .build();

    let response = agent
        .get(&url)
        .call()
        .map_err(|e| format!("fetch {url}: {e}"))?;

    let html = response
        .into_string()
        .map_err(|e| format!("read {url}: {e}"))?;

    let rate = parse_tgju_html(&html)?;
    Ok((rate, url))
}

/// Blocking one-shot updater used by the `rates-update` CLI subcommand.
pub fn update_rates_blocking(repo: &Repository) -> Result<(usize, Vec<String>), String> {
    let today = chrono::Utc::now().date_naive();
    let mut saved = 0usize;
    let mut failed = Vec::new();

    for (code, slug) in TGJU_SYMBOLS {
        let currency = Currency::parse(code).ok_or("bad symbol")?;
        match fetch_tgju_rate(slug) {
            Ok((iri_rate, url)) => {
                let fx = FxRate {
                    id: Uuid::new_v4(),
                    base: Currency::IRR,
                    quote: currency,
                    rate: rust_decimal::Decimal::from(iri_rate),
                    as_of_date: today,
                    source: RateSource::Tgju,
                    source_url: Some(url),
                    raw_value: Some(iri_rate.to_string()),
                    fetched_at: Some(chrono::Utc::now()),
                    created_at: chrono::Utc::now(),
                };
                match repo.upsert_fx_rate(&fx) {
                    Ok(_) => {
                        saved += 1;
                        println!("{}: {} IRR", currency.code(), iri_rate);
                    }
                    Err(e) => failed.push(format!("{}: store error ({})", currency.code(), e)),
                }
            }
            Err(e) => failed.push(format!("{}: {}", currency.code(), e)),
        }
        // Be a polite scraper: one request per second.
        thread::sleep(Duration::from_secs(1));
    }

    if failed.len() == TGJU_SYMBOLS.len() && saved == 0 {
        return Err(format!(
            "all currency fetches failed: {}",
            failed.join("; ")
        ));
    }
    Ok((saved, failed))
}

/// Spawns the GUI background worker so the UI never blocks on network calls.
pub fn spawn_fx_update_worker(db_path: std::path::PathBuf) -> mpsc::Receiver<FxUpdateMsg> {
    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {
        let result = Repository::new(&db_path)
            .map_err(|e| e.to_string())
            .and_then(|repo| update_rates_blocking(&repo));
        let _ = tx.send(FxUpdateMsg::Done(result));
    });

    rx
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE_AED: &str = r#"<html><body>
    <div class="info-panel">
      <span class="symbol-price-header">دلار آمریکا</span>
      <table><tbody>
        <tr><td>بازار</td><td><span>آزاد</span></td></tr>
        <tr><td>آخرین قیمت</td><td><span class="price" data-col="info.last_trade.PDrCotVal">721,260</span></td></tr>
        <tr><td>بهترین خرید</td><td><span data-col="info.best_sell.value">721,000</span></td></tr>
      </tbody></table>
    </div></body></html>"#;

    #[test]
    fn parses_fixture_price() {
        assert_eq!(parse_tgju_html(FIXTURE_AED).unwrap(), 721_260);
    }

    #[test]
    fn rejects_empty_html() {
        assert!(parse_tgju_html("<html></html>").is_err());
    }

    #[test]
    fn symbol_table_has_five_currencies() {
        assert_eq!(TGJU_SYMBOLS.len(), 5);
        assert!(
            TGJU_SYMBOLS
                .iter()
                .all(|(c, s)| { Currency::parse(c).is_some() && !s.is_empty() })
        );
    }
}
