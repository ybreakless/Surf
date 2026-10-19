use chrono::{DateTime, Datelike, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AccountType {
    Asset,     // Debit normal
    Liability, // Credit normal
    Equity,    // Credit normal
    Revenue,   // Credit normal
    Expense,   // Debit normal
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    pub account_type: AccountType,
    pub balance: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Customer {
    pub id: Uuid,
    pub name: String,
    pub code: String, // e.g. "ARF" derived or assigned
    pub email: String,
    pub phone: String,
    pub address: String,
    pub city: String,
    pub country: String,
    pub postal_code: String,
    pub postal_zone: String, // Normalized regional postal zone, e.g. "100" or "DXB"
    pub currency: String,
    pub notes: String,
    pub created_at: DateTime<Utc>,
}

/// Whether a catalog item is sold to customers, bought from vendors, or both.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProductKind {
    Sale,
    Purchase,
    Both,
}

impl ProductKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            ProductKind::Sale => "Sale",
            ProductKind::Purchase => "Purchase",
            ProductKind::Both => "Both",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        match s {
            "Purchase" => ProductKind::Purchase,
            "Both" => ProductKind::Both,
            _ => ProductKind::Sale,
        }
    }

    /// True when the item can appear on sales documents (invoices/estimates).
    pub fn sellable(&self) -> bool {
        matches!(self, ProductKind::Sale | ProductKind::Both)
    }

    /// True when the item can appear on purchase documents (bills).
    pub fn purchasable(&self) -> bool {
        matches!(self, ProductKind::Purchase | ProductKind::Both)
    }
}

/// A catalog item (good or service) with default pricing and linked GL accounts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Product {
    pub id: Uuid,
    pub name: String,
    pub kind: ProductKind,
    pub unit: String,
    pub default_price: Decimal,
    pub tax_rate: Decimal,
    /// Income account code used on sales documents (e.g. "4000").
    pub income_account_code: String,
    /// Expense account code used on purchase documents (e.g. "6000").
    pub expense_account_code: String,
    pub active: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vendor {
    pub id: Uuid,
    pub name: String,
    pub code: String,
    pub email: String,
    pub phone: String,
    pub address: String,
    pub city: String,
    pub country: String,
    pub postal_code: String,
    pub currency: String,
    pub payment_terms_days: u32,
    pub notes: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BillStatus {
    Unpaid,
    Partial,
    Paid,
}

impl BillStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            BillStatus::Unpaid => "Unpaid",
            BillStatus::Partial => "Partial",
            BillStatus::Paid => "Paid",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        match s {
            "Partial" => BillStatus::Partial,
            "Paid" => BillStatus::Paid,
            _ => BillStatus::Unpaid,
        }
    }
}

/// A supplier bill awaiting payment (accounts payable document).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VendorBill {
    pub id: Uuid,
    pub bill_number: String,
    pub vendor_id: Uuid,
    pub vendor_name: String,
    pub issue_date: NaiveDate,
    pub due_date: NaiveDate,
    pub status: BillStatus,
    pub items: Vec<InvoiceItem>,
    pub subtotal: Decimal,
    pub tax_rate: Decimal,
    pub tax_amount: Decimal,
    pub total_amount: Decimal,
    pub currency: String,
    pub notes: String,
    pub journal_entry_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InvoiceStatus {
    Draft,
    Sent,
    Paid,
    Cancelled,
}

impl InvoiceStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            InvoiceStatus::Draft => "Draft",
            InvoiceStatus::Sent => "Sent",
            InvoiceStatus::Paid => "Paid",
            InvoiceStatus::Cancelled => "Cancelled",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        match s {
            "Sent" => InvoiceStatus::Sent,
            "Paid" => InvoiceStatus::Paid,
            "Cancelled" => InvoiceStatus::Cancelled,
            _ => InvoiceStatus::Draft,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvoiceItem {
    pub id: Uuid,
    pub description: String,
    pub quantity: Decimal,
    pub unit: String,
    pub unit_price: Decimal,
    pub discount_pct: Decimal,
    pub amount: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanyProfile {
    pub legal_name: String,
    pub trading_name: String,
    pub address: String,
    pub city: String,
    pub country: String,
    pub postal_code: String,
    pub tax_id: String,
    pub phone: String,
    pub email: String,
    pub website: String,
    pub bank_name: String,
    pub bank_iban: String,
    pub bank_swift: String,
    pub logo_path: String,
    pub stamp_path: String,
}

impl Default for CompanyProfile {
    fn default() -> Self {
        Self {
            legal_name: "Surf Commercial Services LLC".to_string(),
            trading_name: "Surf Logistics & Trading".to_string(),
            address: "Financial Center Blvd, Suite 402".to_string(),
            city: "Dubai".to_string(),
            country: "United Arab Emirates".to_string(),
            postal_code: "00000".to_string(),
            tax_id: "TRN-100294819200003".to_string(),
            phone: "+971 4 392 1000".to_string(),
            email: "billing@surfacctg.com".to_string(),
            website: "www.surfacctg.com".to_string(),
            bank_name: "Emirates NBD Bank".to_string(),
            bank_iban: "AE070260001029384756101".to_string(),
            bank_swift: "EBILAEADXXX".to_string(),
            logo_path: String::new(),
            stamp_path: "/home/otto/Downloads/stamp_compressed.pdf".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Invoice {
    pub id: Uuid,
    pub invoice_number: String,
    pub customer_id: Uuid,
    pub customer_name: String,
    pub postal_code: String,
    pub postal_zone: String,
    pub issue_date: NaiveDate,
    pub due_date: NaiveDate,
    pub status: InvoiceStatus,
    pub items: Vec<InvoiceItem>,
    pub subtotal: Decimal,
    pub tax_rate: Decimal, // Percentage, e.g. 5.0
    pub tax_amount: Decimal,
    pub total_amount: Decimal,
    pub currency: String,
    pub notes: String,
    pub journal_entry_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeliveryStatus {
    Pending,
    InTransit,
    Delivered,
    Cancelled,
}

impl DeliveryStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            DeliveryStatus::Pending => "Pending",
            DeliveryStatus::InTransit => "In Transit",
            DeliveryStatus::Delivered => "Delivered",
            DeliveryStatus::Cancelled => "Cancelled",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        match s {
            "InTransit" | "In Transit" => DeliveryStatus::InTransit,
            "Delivered" => DeliveryStatus::Delivered,
            "Cancelled" => DeliveryStatus::Cancelled,
            _ => DeliveryStatus::Pending,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeliveryItem {
    pub id: Uuid,
    pub description: String,
    pub quantity: Decimal,
    pub unit: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeliveryOrder {
    pub id: Uuid,
    pub delivery_number: String,
    pub invoice_id: Option<Uuid>,
    pub invoice_number: Option<String>,
    pub customer_id: Uuid,
    pub customer_name: String,
    pub delivery_address: String,
    pub postal_code: String,
    pub postal_zone: String,
    pub delivery_date: NaiveDate,
    pub status: DeliveryStatus,
    pub items: Vec<DeliveryItem>,
    pub driver_notes: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JournalLine {
    pub account_id: Uuid,
    pub account_code: String,
    pub account_name: String,
    pub debit: Decimal,
    pub credit: Decimal,
    pub memo: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JournalEntry {
    pub id: Uuid,
    pub entry_number: String,
    pub entry_date: NaiveDate,
    pub memo: String,
    pub reference: Option<String>,
    pub lines: Vec<JournalLine>,
    pub created_at: DateTime<Utc>,
}

/// Billing cadence of a recurring invoice template.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Frequency {
    Weekly,
    Monthly,
    Quarterly,
    Yearly,
}

impl Frequency {
    pub fn as_str(&self) -> &'static str {
        match self {
            Frequency::Weekly => "weekly",
            Frequency::Monthly => "monthly",
            Frequency::Quarterly => "quarterly",
            Frequency::Yearly => "yearly",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "weekly" => Frequency::Weekly,
            "quarterly" => Frequency::Quarterly,
            "yearly" => Frequency::Yearly,
            _ => Frequency::Monthly,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Frequency::Weekly => "Every week",
            Frequency::Monthly => "Every month",
            Frequency::Quarterly => "Every quarter",
            Frequency::Yearly => "Every year",
        }
    }

    /// The next billing date strictly after/at cadence from `from`.
    /// Month-based cadences clamp the day to the last day of the target month
    /// (e.g. Jan 31 monthly -> Feb 28).
    pub fn next_date(self, from: NaiveDate) -> NaiveDate {
        match self {
            Frequency::Weekly => from + chrono::Duration::weeks(1),
            Frequency::Monthly => add_months(from, 1),
            Frequency::Quarterly => add_months(from, 3),
            Frequency::Yearly => add_months(from, 12),
        }
    }
}

/// Adds `n` calendar months, clamping to the target month's last day.
fn add_months(from: NaiveDate, n: i32) -> NaiveDate {
    let mut year = from.year();
    let mut month = from.month0() as i32 + n;
    while month >= 12 {
        month -= 12;
        year += 1;
    }
    let (target_year, target_month) = (year, month as u32 + 1);
    let last_day = days_in_month(target_year, target_month);
    let day = from.day().min(last_day);
    NaiveDate::from_ymd_opt(target_year, target_month, day).unwrap_or(from)
}

fn is_leap(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap(year) => 29,
        2 => 28,
        _ => 28,
    }
}

/// A recurring billing template that auto-generates sales invoices.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecurringInvoice {
    pub id: Uuid,
    pub template_name: String,
    pub customer_id: Uuid,
    pub frequency: Frequency,
    pub next_run: NaiveDate,
    pub tax_rate: Decimal,
    pub item_description: String,
    pub item_qty: Decimal,
    pub item_unit: String,
    pub item_unit_price: Decimal,
    pub active: bool,
    pub last_run: Option<NaiveDate>,
    pub created_at: DateTime<Utc>,
}

impl RecurringInvoice {
    /// One line item built from the template fields.
    pub fn line_total(&self) -> Decimal {
        self.item_qty * self.item_unit_price
    }
}

#[cfg(test)]
mod frequency_tests {
    use super::*;

    #[test]
    fn monthend_clamping() {
        let d = NaiveDate::from_ymd_opt(2026, 1, 31).unwrap();
        assert_eq!(
            Frequency::Monthly.next_date(d),
            NaiveDate::from_ymd_opt(2026, 2, 28).unwrap()
        );
        assert_eq!(
            Frequency::Monthly.next_date(NaiveDate::from_ymd_opt(2026, 2, 28).unwrap()),
            NaiveDate::from_ymd_opt(2026, 3, 28).unwrap()
        );
        assert_eq!(
            Frequency::Quarterly.next_date(NaiveDate::from_ymd_opt(2026, 10, 31).unwrap()),
            NaiveDate::from_ymd_opt(2027, 1, 31).unwrap()
        );
        assert_eq!(
            Frequency::Yearly.next_date(NaiveDate::from_ymd_opt(2024, 2, 29).unwrap()),
            NaiveDate::from_ymd_opt(2025, 2, 28).unwrap()
        );
    }

    #[test]
    fn weekly_advance() {
        let d = NaiveDate::from_ymd_opt(2026, 10, 19).unwrap();
        assert_eq!(
            Frequency::Weekly.next_date(d),
            NaiveDate::from_ymd_opt(2026, 10, 26).unwrap()
        );
    }
}
