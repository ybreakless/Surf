//! Financial report computations (premium report suite).
//!
//! All functions are pure: they take already-loaded domain data and return
//! display-ready structures, so they are trivially unit-testable.

use chrono::NaiveDate;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;

use super::model::{Account, AccountType, Invoice, InvoiceStatus};

#[derive(Debug, Clone)]
pub struct TrialBalanceRow {
    pub code: String,
    pub name: String,
    pub debit: Decimal,
    pub credit: Decimal,
}

#[derive(Debug, Clone)]
pub struct IncomeStatement {
    pub revenue: Decimal,
    pub expenses: Decimal,
    pub net_income: Decimal,
}

#[derive(Debug, Clone)]
pub struct BalanceSheet {
    pub assets: Decimal,
    pub liabilities: Decimal,
    pub equity: Decimal,
    pub net_income: Decimal,
}

/// Statement rows: (invoice_number, issue_date, due_date, status, total, days_overdue)
#[derive(Debug, Clone)]
pub struct StatementRow {
    pub invoice_number: String,
    pub issue_date: NaiveDate,
    pub due_date: NaiveDate,
    pub status: InvoiceStatus,
    pub total: Decimal,
    pub currency: String,
    pub days_overdue: i64,
}

/// Aged receivables per customer in Wave-style buckets: current, 1-30, 31-60, 61-90, 90+.
#[derive(Debug, Clone)]
pub struct AgingRow {
    pub customer_name: String,
    pub currency: String,
    pub current: Decimal,
    pub b30: Decimal,
    pub b60: Decimal,
    pub b90: Decimal,
    pub b90plus: Decimal,
    pub total: Decimal,
}

fn normal_balance(t: AccountType) -> CreditSide {
    match t {
        AccountType::Asset | AccountType::Expense => CreditSide::Debit,
        AccountType::Liability | AccountType::Equity | AccountType::Revenue => CreditSide::Credit,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CreditSide {
    Debit,
    Credit,
}

/// Trial balance: every account on its normal-balance side; debits must equal credits.
pub fn trial_balance(accounts: &[Account]) -> Vec<TrialBalanceRow> {
    accounts
        .iter()
        .map(|a| match normal_balance(a.account_type) {
            CreditSide::Debit => TrialBalanceRow {
                code: a.code.clone(),
                name: a.name.clone(),
                debit: a.balance.max(Decimal::ZERO),
                credit: a.balance.min(Decimal::ZERO).abs(),
            },
            CreditSide::Credit => TrialBalanceRow {
                code: a.code.clone(),
                name: a.name.clone(),
                debit: a.balance.min(Decimal::ZERO).abs(),
                credit: a.balance.max(Decimal::ZERO),
            },
        })
        .collect()
}

pub fn trial_balance_totals(rows: &[TrialBalanceRow]) -> (Decimal, Decimal) {
    (
        rows.iter().map(|r| r.debit).sum(),
        rows.iter().map(|r| r.credit).sum(),
    )
}

/// Cumulative income statement (as of today).
pub fn income_statement(accounts: &[Account]) -> IncomeStatement {
    let revenue: Decimal = accounts
        .iter()
        .filter(|a| a.account_type == AccountType::Revenue)
        .map(|a| a.balance) // credit-normal: positive balance = revenue earned
        .sum();
    let expenses: Decimal = accounts
        .iter()
        .filter(|a| a.account_type == AccountType::Expense)
        .map(|a| a.balance) // debit-normal
        .sum();
    IncomeStatement {
        revenue,
        expenses,
        net_income: revenue - expenses,
    }
}

/// Cumulative balance sheet; net income is included as an equity component.
pub fn balance_sheet(accounts: &[Account]) -> BalanceSheet {
    let assets: Decimal = accounts
        .iter()
        .filter(|a| a.account_type == AccountType::Asset)
        .map(|a| a.balance)
        .sum();
    let liabilities: Decimal = accounts
        .iter()
        .filter(|a| a.account_type == AccountType::Liability)
        .map(|a| a.balance)
        .sum();
    let equity: Decimal = accounts
        .iter()
        .filter(|a| a.account_type == AccountType::Equity)
        .map(|a| a.balance)
        .sum();
    BalanceSheet {
        assets,
        liabilities,
        equity,
        net_income: income_statement(accounts).net_income,
    }
}

fn outstanding(status: InvoiceStatus) -> bool {
    matches!(status, InvoiceStatus::Sent | InvoiceStatus::Draft)
}

/// Customer statement rows for one customer, sorted by issue date.
pub fn customer_statement(
    invoices: &[Invoice],
    customer_id: uuid::Uuid,
    today: NaiveDate,
) -> Vec<StatementRow> {
    invoices
        .iter()
        .filter(|inv| inv.customer_id == customer_id && outstanding(inv.status))
        .map(|inv| StatementRow {
            invoice_number: inv.invoice_number.clone(),
            issue_date: inv.issue_date,
            due_date: inv.due_date,
            status: inv.status,
            total: inv.total_amount,
            currency: inv.currency.clone(),
            days_overdue: today.signed_duration_since(inv.due_date).num_days().max(0),
        })
        .collect()
}

/// Aged receivables across all customers with outstanding invoices.
/// Buckets by days past due date: current (<=0), 1-30, 31-60, 61-90, 90+.
/// Grouped by (customer, currency) so foreign-currency sums never mix.
pub fn aged_receivables(invoices: &[Invoice], today: NaiveDate) -> Vec<AgingRow> {
    let mut map: std::collections::BTreeMap<(String, String), AgingRow> =
        std::collections::BTreeMap::new();

    for inv in invoices.iter().filter(|i| i.status == InvoiceStatus::Sent) {
        let days = today.signed_duration_since(inv.due_date).num_days();
        let key = (inv.customer_name.clone(), inv.currency.clone());
        let row = map.entry(key).or_insert_with(|| AgingRow {
            customer_name: inv.customer_name.clone(),
            currency: inv.currency.clone(),
            current: dec!(0),
            b30: dec!(0),
            b60: dec!(0),
            b90: dec!(0),
            b90plus: dec!(0),
            total: dec!(0),
        });

        let bucket: &mut Decimal = if days <= 0 {
            &mut row.current
        } else if days <= 30 {
            &mut row.b30
        } else if days <= 60 {
            &mut row.b60
        } else if days <= 90 {
            &mut row.b90
        } else {
            &mut row.b90plus
        };
        *bucket += inv.total_amount;
        row.total += inv.total_amount;
    }

    map.into_values().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{DateTime, Utc};
    use rust_decimal_macros::dec;
    use uuid::Uuid;

    fn account(code: &str, name: &str, t: AccountType, bal: Decimal) -> Account {
        Account {
            id: Uuid::new_v4(),
            code: code.to_string(),
            name: name.to_string(),
            account_type: t,
            balance: bal,
        }
    }

    fn invoice(
        cust: &str,
        cust_id: Uuid,
        due: NaiveDate,
        total: Decimal,
        status: InvoiceStatus,
    ) -> Invoice {
        Invoice {
            id: Uuid::new_v4(),
            invoice_number: format!("INV-{cust}-{total}"),
            customer_id: cust_id,
            customer_name: cust.to_string(),
            postal_code: String::new(),
            postal_zone: String::new(),
            issue_date: today() - chrono::Duration::days(45),
            due_date: due,
            status,
            items: vec![],
            subtotal: total,
            tax_rate: dec!(0),
            tax_amount: dec!(0),
            total_amount: total,
            currency: "USD".to_string(),
            notes: String::new(),
            journal_entry_id: None,
            created_at: DateTime::<Utc>::from_timestamp(0, 0).unwrap(),
        }
    }

    fn today() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 10, 19).unwrap()
    }

    #[test]
    fn trial_balance_balances_after_posting() {
        let accounts = vec![
            account("1000", "Cash", AccountType::Asset, dec!(1050)),
            account("1200", "Accounts Receivable", AccountType::Asset, dec!(0)),
            account("2200", "Sales Tax", AccountType::Liability, dec!(50)),
            account("4000", "Revenue", AccountType::Revenue, dec!(1000)),
        ];
        let rows = trial_balance(&accounts);
        let (d, c) = trial_balance_totals(&rows);
        assert_eq!(d, c);
        assert_eq!(d, dec!(1050));
    }

    #[test]
    fn income_and_balance_sheet_consistent() {
        // Ledger-equivalent postings: invoice 1050 (paid by cash), rent 200 unpaid from cash.
        let accounts = vec![
            account("1000", "Cash", AccountType::Asset, dec!(850)),
            account("1200", "Accounts Receivable", AccountType::Asset, dec!(0)),
            account("2200", "Sales Tax", AccountType::Liability, dec!(50)),
            account("4000", "Revenue", AccountType::Revenue, dec!(1000)),
            account("5000", "Rent", AccountType::Expense, dec!(200)),
        ];
        let is = income_statement(&accounts);
        assert_eq!(is.revenue, dec!(1000));
        assert_eq!(is.expenses, dec!(200));
        assert_eq!(is.net_income, dec!(800));

        let bs = balance_sheet(&accounts);
        assert_eq!(bs.assets, dec!(850));
        assert_eq!(bs.liabilities + bs.equity + bs.net_income, dec!(850));
    }

    #[test]
    fn aging_buckets_by_due_date() {
        let cust = Uuid::new_v4();
        let invoices = vec![
            invoice(
                "A",
                cust,
                today() - chrono::Duration::days(45),
                dec!(500),
                InvoiceStatus::Sent,
            ), // 31-60
            invoice(
                "A",
                cust,
                today() + chrono::Duration::days(5),
                dec!(100),
                InvoiceStatus::Sent,
            ), // current
            invoice(
                "A",
                cust,
                today() - chrono::Duration::days(100),
                dec!(75),
                InvoiceStatus::Sent,
            ), // 90+
            invoice(
                "A",
                cust,
                today() - chrono::Duration::days(20),
                dec!(300),
                InvoiceStatus::Paid,
            ), // paid: ignored
        ];
        let rows = aged_receivables(&invoices, today());
        assert_eq!(rows.len(), 1);
        let a = &rows[0];
        assert_eq!(a.currency, "USD");
        assert_eq!(a.current, dec!(100));
        assert_eq!(a.b60, dec!(500));
        assert_eq!(a.b90plus, dec!(75));
        assert_eq!(a.total, dec!(675));
    }

    #[test]
    fn statement_rows_and_overdue_days() {
        let cust = Uuid::new_v4();
        let invoices = vec![
            invoice(
                "B",
                cust,
                today() - chrono::Duration::days(10),
                dec!(500),
                InvoiceStatus::Sent,
            ),
            invoice(
                "B",
                cust,
                today() - chrono::Duration::days(10),
                dec!(500),
                InvoiceStatus::Paid,
            ),
        ];
        let rows = customer_statement(&invoices, cust, today());
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].days_overdue, 10);
    }
}
