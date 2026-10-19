use chrono::{Datelike, Utc};

/// Intelligent numbering engine for Delivery Orders and Invoices
/// derives codes based on customer identity, geographical routing, and postal zones.
pub struct NumberingEngine;

impl NumberingEngine {
    /// Auto-generates a concise customer code (e.g. "Aref Logistics" -> "AL", "Acme" -> "ACM")
    pub fn derive_customer_code(name: &str) -> String {
        let clean = name.trim();
        if clean.is_empty() {
            return "CUS".to_string();
        }

        let words: Vec<&str> = clean.split_whitespace().filter(|w| !w.is_empty()).collect();

        if words.len() >= 2 {
            let code: String = words
                .iter()
                .filter_map(|w| w.chars().find(|c| c.is_alphanumeric()))
                .take(4)
                .collect();
            let upper = code.to_uppercase();
            if upper.len() >= 2 {
                return upper;
            }
        }

        // Single word fallback
        let alphanumeric: String = clean
            .chars()
            .filter(|c| c.is_alphanumeric())
            .take(3)
            .collect();

        let upper = alphanumeric.to_uppercase();
        if upper.is_empty() {
            "CUS".to_string()
        } else {
            upper
        }
    }

    /// Normalizes postal code into a regional logistics / routing zone
    /// e.g. "10001" -> "100", "94103-1234" -> "941", "SW1A 1AA" -> "SW1A", "DXB-04" -> "DXB04"
    pub fn derive_postal_zone(postal_code: &str, city: &str) -> String {
        let trimmed_pc = postal_code.trim();
        if trimmed_pc.is_empty() {
            // Fall back to city initials if no postal code
            let city_clean: String = city
                .chars()
                .filter(|c| c.is_alphanumeric())
                .take(3)
                .collect();
            let upper = city_clean.to_uppercase();
            return if upper.is_empty() {
                "GEN".to_string()
            } else {
                upper
            };
        }

        // Check if postal code is alphanumeric or numeric
        let clean: String = trimmed_pc.chars().filter(|c| c.is_alphanumeric()).collect();

        if clean.chars().all(|c| c.is_ascii_digit()) {
            // Numeric zip code (like US 5-digit zip, France 5-digit, Germany 5-digit)
            if clean.len() >= 5 {
                clean[..3].to_string()
            } else if !clean.is_empty() {
                clean
            } else {
                "GEN".to_string()
            }
        } else {
            // Alphanumeric postal code (e.g. UK SW1A 1AA, Canada M5V 2T6)
            let take_len = clean.len().min(4);
            clean[..take_len].to_uppercase()
        }
    }

    /// Formats an Invoice Number based on postal zone, customer code, year, and sequence
    /// Pattern: INV-{ZONE}-{CUST}-{YY}{SEQ:03}
    /// Example: INV-100-ARF-26001
    pub fn format_invoice_number(zone: &str, cust_code: &str, sequence: u32) -> String {
        let current_year = Utc::now().year() % 100; // e.g. 26
        let z = if zone.trim().is_empty() {
            "GEN"
        } else {
            zone.trim()
        };
        let c = if cust_code.trim().is_empty() {
            "CUS"
        } else {
            cust_code.trim()
        };
        format!("INV-{}-{}-{:02}{:03}", z, c, current_year, sequence)
    }

    /// Formats a Delivery Order Number based on postal zone, customer code, year, and sequence
    /// Pattern: DO-{ZONE}-{CUST}-{YY}{SEQ:03}
    /// Example: DO-100-ARF-26001
    pub fn format_delivery_number(zone: &str, cust_code: &str, sequence: u32) -> String {
        let current_year = Utc::now().year() % 100;
        let z = if zone.trim().is_empty() {
            "GEN"
        } else {
            zone.trim()
        };
        let c = if cust_code.trim().is_empty() {
            "CUS"
        } else {
            cust_code.trim()
        };
        format!("DO-{}-{}-{:02}{:03}", z, c, current_year, sequence)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_customer_code_derivation() {
        assert_eq!(
            NumberingEngine::derive_customer_code("Aref Logistics"),
            "AL"
        );
        assert_eq!(
            NumberingEngine::derive_customer_code("Atlas Trading Global"),
            "ATG"
        );
        assert_eq!(NumberingEngine::derive_customer_code("Acme"), "ACM");
        assert_eq!(NumberingEngine::derive_customer_code(""), "CUS");
    }

    #[test]
    fn test_postal_zone_derivation() {
        assert_eq!(
            NumberingEngine::derive_postal_zone("10001", "New York"),
            "100"
        );
        assert_eq!(
            NumberingEngine::derive_postal_zone("94103-1234", "San Francisco"),
            "941"
        );
        assert_eq!(
            NumberingEngine::derive_postal_zone("SW1A 1AA", "London"),
            "SW1A"
        );
        assert_eq!(NumberingEngine::derive_postal_zone("", "Dubai"), "DUB");
    }
}
