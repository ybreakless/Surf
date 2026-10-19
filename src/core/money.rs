//! Currency definitions, precision, and display formatting.
//!
//! Money is always stored as [`rust_decimal::Decimal`]. Formatting adapts to
//! each currency's conventions (IRR has no minor unit; Toman display divides
//! by 10 without ever changing stored values).

use rust_decimal::Decimal;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Currency {
    IRR,
    AED,
    USD,
    EUR,
    CNY,
    RUB,
}

impl Currency {
    pub const ALL: [Currency; 6] = [
        Currency::IRR,
        Currency::AED,
        Currency::USD,
        Currency::EUR,
        Currency::CNY,
        Currency::RUB,
    ];

    pub fn code(&self) -> &'static str {
        match self {
            Currency::IRR => "IRR",
            Currency::AED => "AED",
            Currency::USD => "USD",
            Currency::EUR => "EUR",
            Currency::CNY => "CNY",
            Currency::RUB => "RUB",
        }
    }

    /// Human display label used across the UI.
    pub fn label(&self) -> &'static str {
        match self {
            Currency::IRR => "Iranian Rial",
            Currency::AED => "UAE Dirham",
            Currency::USD => "US Dollar",
            Currency::EUR => "Euro",
            Currency::CNY => "Chinese Yuan (RMB)",
            Currency::RUB => "Russian Ruble",
        }
    }

    pub fn symbol(&self) -> &'static str {
        match self {
            Currency::IRR => "IRR",
            Currency::AED => "AED",
            Currency::USD => "$",
            Currency::EUR => "€",
            Currency::CNY => "¥",
            Currency::RUB => "₽",
        }
    }

    /// Fractional digits used for display and entry rounding.
    /// IRR has no minor unit in practice.
    pub const fn decimals(&self) -> u32 {
        match self {
            Currency::IRR => 0,
            _ => 2,
        }
    }

    pub fn parse(code: &str) -> Option<Currency> {
        Currency::ALL
            .iter()
            .copied()
            .find(|c| c.code().eq_ignore_ascii_case(code.trim()))
    }

    /// Rounds to the currency's canonical precision (e.g. IRR -> whole rials).
    pub fn round(&self, amount: Decimal) -> Decimal {
        amount.round_dp(self.decimals())
    }

    /// Formats an amount with thousands separators and correct precision.
    pub fn format(&self, amount: Decimal) -> String {
        let amount = amount.round_dp(self.decimals());
        let neg = amount.is_sign_negative();
        let abs = amount.abs();
        // Split into integer + fractional textual parts.
        let s = abs.to_string();
        let (int_part, frac_part) = match s.split_once('.') {
            Some((i, f)) => (i.to_string(), Some(f.to_string())),
            None => (s, None),
        };

        // Group integer digits in threes from the right.
        let mut grouped = String::new();
        let chars: Vec<char> = int_part.chars().collect();
        for (idx, ch) in chars.iter().enumerate() {
            if idx > 0 && (chars.len() - idx).is_multiple_of(3) {
                grouped.push(',');
            }
            grouped.push(*ch);
        }

        let decimals = self.decimals() as usize;
        let mut out = if neg { "-" } else { "" }.to_string();
        out.push_str(&grouped);
        if decimals > 0 {
            out.push('.');
            let mut frac = frac_part.unwrap_or_default();
            while frac.len() < decimals {
                frac.push('0');
            }
            if frac.len() > decimals {
                frac.truncate(decimals);
            }
            out.push_str(&frac);
        }
        out
    }

    /// "symbol 1,234.00" (e.g. "$ 1,234.00", "IRR 2,638,000").
    pub fn format_money(&self, amount: Decimal) -> String {
        format!("{} {}", self.symbol(), self.format(amount))
    }
}

/// The accounting base currency of the company; IRR is the pivot of the FX model.
impl Default for Currency {
    fn default() -> Self {
        Currency::IRR
    }
}

/// Display helper for IRR: shows Toman (IRR / 10) as many Iranian users expect.
/// This is display-only; the database and ledger always store Rials.
pub fn format_toman(assert_rial: Decimal, display: bool) -> String {
    let suffix = if display { " T" } else { "" };
    if display {
        let t = assert_rial / Decimal::from(10);
        format!("{}{}", Currency::IRR.format(t), suffix)
    } else {
        format!("{}{}", Currency::IRR.format(assert_rial), suffix)
    }
}

/// Formats money where the currency code is only known as a string (DB values).
/// Unknown codes fall back to a generic "1,234.56 XXX" form with 2 decimals.
pub fn format_money_by_code(code: &str, amount: Decimal) -> String {
    match Currency::parse(code) {
        Some(c) => c.format_money(amount),
        None => format!("{} {}", amount.round_dp(2), code),
    }
}

#[cfg(test)]
mod tests {
    use rust_decimal_macros::dec;

    use super::*;

    #[test]
    fn grouping_and_decimals() {
        assert_eq!(Currency::USD.format(dec!(1234.5)), "1,234.50");
        assert_eq!(Currency::USD.format(dec!(12345678.901)), "12,345,678.90");
        assert_eq!(Currency::IRR.format(dec!(2638000)), "2,638,000");
        assert_eq!(Currency::IRR.format(dec!(2638000.57)), "2,638,001"); // rounds to whole rial
        assert_eq!(Currency::EUR.format(dec!(-987.654)), "-987.65");
    }

    #[test]
    fn money_strings() {
        assert_eq!(Currency::AED.format_money(dec!(1000)), "AED 1,000.00");
        assert_eq!(Currency::USD.format_money(dec!(12.5)), "$ 12.50");
    }

    #[test]
    fn parse_codes() {
        assert_eq!(Currency::parse("usd"), Some(Currency::USD));
        assert_eq!(Currency::parse("CNY"), Some(Currency::CNY));
        assert_eq!(Currency::parse("GBP"), None);
        assert!(Currency::ALL.contains(&Currency::IRR));
    }

    #[test]
    fn toman_display_only() {
        let r = Decimal::from(2_638_000);
        assert_eq!(format_toman(r, true), "263,800 T");
        assert_eq!(format_toman(r, false), "2,638,000");
    }
}

#[cfg(test)]
mod proptests {
    use super::*;

    /// Currency formatting must round-trip: parse(formatted) == rounded(amount).
    #[test]
    fn format_roundtrip() {
        for c in Currency::ALL {
            let raw = Decimal::new(12345678, c.decimals());
            let formatted = c.format(raw);
            let cleaned: String = formatted.chars().filter(|ch| *ch != ',').collect();
            let parsed: Decimal = cleaned.parse().unwrap_or_default();
            assert_eq!(c.round(parsed), c.round(raw), "currency {}", c.code());
        }
    }
}
