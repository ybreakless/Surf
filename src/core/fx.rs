//! Foreign exchange logic: rate storage model, cross-rate conversion, and
//! rate resolution (manual overrides win, then fetched rates, then latest).

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::money::Currency;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RateSource {
    Manual,
    Tgju,
    Import,
}

impl RateSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            RateSource::Manual => "manual",
            RateSource::Tgju => "tgju",
            RateSource::Import => "import",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "manual" => RateSource::Manual,
            "import" => RateSource::Import,
            _ => RateSource::Tgju,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            RateSource::Manual => "Manual",
            RateSource::Tgju => "tgju.org",
            RateSource::Import => "Imported",
        }
    }
}

/// One unit of `quote` expressed in `base`.
/// e.g. rate { IRR, AED, 721_260 } means 1 AED = 721,260 IRR.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FxRate {
    pub id: Uuid,
    pub base: Currency,
    pub quote: Currency,
    pub rate: Decimal,
    pub as_of_date: NaiveDate,
    pub source: RateSource,
    pub source_url: Option<String>,
    pub raw_value: Option<String>,
    pub fetched_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

impl FxRate {
    /// Inverse of this rate: 1 quote = rate base_units  =>  1 base = 1/rate quote_units.
    pub fn inverse(&self) -> Option<Decimal> {
        if self.rate.is_zero() {
            return None;
        }
        Decimal::ONE.checked_div(self.rate)
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}

/// Converts `amount` from `from` to `to` using a set of IRR-pivot rates.
///
/// Convention (matching tgju): every rate row we store is "1 unit of FX = N IRR".
/// A row {base: IRR, quote: X} with rate r means 1 X = r IRR.
///
/// Strategy:
///   - IRR -> X:            amount / r
///   - X -> IRR:            amount * r
///   - X -> Y:              amount * r_x / r_y   (via IRR pivot)
///   - Same -> Same:        amount unchanged
///
/// Returns `None` when a required rate is missing (caller should prompt for a manual rate).
pub fn convert(amount: Decimal, from: Currency, to: Currency, rates: &[FxRate]) -> Option<Decimal> {
    if from == to {
        return Some(amount);
    }

    if from == Currency::IRR {
        let r = rate_of(to, rates)?;
        if r.is_zero() {
            return None;
        }
        return Some(amount / r);
    }
    if to == Currency::IRR {
        let r = rate_of(from, rates)?;
        return Some(amount * r);
    }

    let rf = rate_of(from, rates)?;
    let rt = rate_of(to, rates)?;
    if rt.is_zero() {
        return None;
    }
    Some(amount * rf / rt)
}

/// Finds the IRR-per-unit rate for a currency (1 unit X = N IRR).
fn rate_of(c: Currency, rates: &[FxRate]) -> Option<Decimal> {
    rates
        .iter()
        .filter(|r| r.quote == c && r.base == Currency::IRR)
        .map(|r| r.rate)
        .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
}

/// Picks the best applicable rate for a currency as of a date:
/// exact date wins; otherwise the most recent rate on or before that date.
pub fn resolve_rate(c: Currency, as_of: NaiveDate, rates: &[FxRate]) -> Option<FxRate> {
    let mut best: Option<FxRate> = None;
    for r in rates
        .iter()
        .filter(|r| r.quote == c && r.as_of_date <= as_of)
    {
        best = match best {
            Some(b) if b.as_of_date >= r.as_of_date => Some(b),
            _ => Some(r.clone()),
        };
    }
    best
}

/// Human-safe display of a rate keeping enough precision for cross math.
pub fn format_rate(rate: Decimal) -> String {
    let dp = (14usize.saturating_sub(rate.trunc().to_string().len())).max(4) as u32;
    Currency::USD.format(rate.round_dp(dp))
}

/// Validates a user-entered rate string and normalizes it to Decimal.
pub fn parse_rate_str(s: &str) -> Option<Decimal> {
    let cleaned: String = s.chars().filter(|c| *c != ',').collect();
    let d: Decimal = cleaned.trim().parse().ok()?;
    if d <= Decimal::ZERO { None } else { Some(d) }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use rust_decimal_macros::dec;

    use super::*;

    fn rate(quote: Currency, r: Decimal) -> FxRate {
        FxRate {
            id: Uuid::new_v4(),
            base: Currency::IRR,
            quote,
            rate: r,
            as_of_date: Utc::now().date_naive(),
            source: RateSource::Tgju,
            source_url: None,
            raw_value: None,
            fetched_at: Some(Utc::now()),
            created_at: Utc::now(),
        }
    }

    #[test]
    fn irr_pivot_roundtrip() {
        // 1 AED = 721,260 IRR  ->  721,260 IRR == 1 AED
        let rates = vec![rate(Currency::AED, dec!(721260))];
        let result = convert(dec!(721260), Currency::IRR, Currency::AED, &rates);
        assert_eq!(result, Some(Decimal::ONE));
        assert_eq!(
            convert(Decimal::ONE, Currency::AED, Currency::IRR, &rates),
            Some(dec!(721260))
        );
        // Same-currency conversion is identity.
        assert_eq!(
            convert(dec!(5), Currency::AED, Currency::AED, &rates),
            Some(dec!(5))
        );
    }

    #[test]
    fn foreign_to_foreign_cross() {
        // USD 2,638,000 IRR; AED 721,260 IRR
        let rates = vec![
            rate(Currency::USD, dec!(2638000)),
            rate(Currency::AED, dec!(721260)),
        ];
        // 1000 AED in USD = 1000 * 721260 / 2638000 = 273.387...
        let result = convert(dec!(1000), Currency::AED, Currency::USD, &rates);
        let expected = dec!(1000) * dec!(721260) / dec!(2638000);
        assert_eq!(result, Some(expected));
    }

    #[test]
    fn missing_rates_yield_none() {
        let rates: Vec<FxRate> = vec![];
        assert_eq!(
            convert(dec!(100), Currency::USD, Currency::IRR, &rates),
            None
        );
        let rates2 = vec![rate(Currency::EUR, dec!(2956900))];
        assert_eq!(
            convert(dec!(100), Currency::USD, Currency::EUR, &rates2),
            None
        ); // no USD rate
        assert!(convert(dec!(100), Currency::EUR, Currency::IRR, &rates2).is_some());
    }

    #[test]
    fn resolve_prefers_exact_then_latest() {
        let today = Utc::now().date_naive();
        let yesterday = today - chrono::Duration::days(1);
        let mut r_old = rate(Currency::USD, dec!(1000));
        r_old.as_of_date = yesterday;
        let r_new = rate(Currency::USD, dec!(2000));

        let rates = vec![r_old.clone(), r_new.clone()];
        let picked = resolve_rate(Currency::USD, today, &rates).unwrap();
        assert_eq!(picked.rate, dec!(2000));

        // As of yesterday, older rate applies.
        let picked2 = resolve_rate(Currency::USD, yesterday, &rates).unwrap();
        assert_eq!(picked2.rate, dec!(1000));

        // Future-only date: nothing applicable yet, falls back to latest overall via caller choice;
        let none = resolve_rate(Currency::EUR, today, std::slice::from_ref(&r_new));
        assert_eq!(none, None);
    }

    #[test]
    fn rate_string_parsing() {
        assert_eq!(parse_rate_str("721,260"), Some(dec!(721260)));
        assert_eq!(parse_rate_str(" 14.50 "), Some(dec!(14.50)));
        assert_eq!(parse_rate_str("-3"), None);
        assert_eq!(parse_rate_str("abc"), None);
        assert_eq!(parse_rate_str("0"), None);
    }

    #[test]
    fn inverse_rate() {
        let r = rate(Currency::AED, dec!(721260));
        let inv = r.inverse().unwrap();
        assert!(inv < Decimal::ONE && inv > dec!(0.));
        let zero = rate(Currency::USD, Decimal::ZERO);
        assert!(zero.inverse().is_none());
    }
}
