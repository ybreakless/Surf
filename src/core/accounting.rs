use chrono::Utc;
use rust_decimal::Decimal;
use uuid::Uuid;

use super::model::{Account, AccountType, Invoice, JournalEntry, JournalLine};

pub struct AccountingEngine;

impl AccountingEngine {
    /// Generates standard chart of accounts
    pub fn default_chart_of_accounts() -> Vec<Account> {
        vec![
            Account {
                id: Uuid::new_v4(),
                code: "1000".to_string(),
                name: "Cash and Bank".to_string(),
                account_type: AccountType::Asset,
                balance: Decimal::ZERO,
            },
            Account {
                id: Uuid::new_v4(),
                code: "1200".to_string(),
                name: "Accounts Receivable".to_string(),
                account_type: AccountType::Asset,
                balance: Decimal::ZERO,
            },
            Account {
                id: Uuid::new_v4(),
                code: "1400".to_string(),
                name: "Inventory".to_string(),
                account_type: AccountType::Asset,
                balance: Decimal::ZERO,
            },
            Account {
                id: Uuid::new_v4(),
                code: "2000".to_string(),
                name: "Accounts Payable".to_string(),
                account_type: AccountType::Liability,
                balance: Decimal::ZERO,
            },
            Account {
                id: Uuid::new_v4(),
                code: "2200".to_string(),
                name: "Sales Tax / VAT Payable".to_string(),
                account_type: AccountType::Liability,
                balance: Decimal::ZERO,
            },
            Account {
                id: Uuid::new_v4(),
                code: "3000".to_string(),
                name: "Owner Equity / Retained Earnings".to_string(),
                account_type: AccountType::Equity,
                balance: Decimal::ZERO,
            },
            Account {
                id: Uuid::new_v4(),
                code: "4000".to_string(),
                name: "Sales Revenue".to_string(),
                account_type: AccountType::Revenue,
                balance: Decimal::ZERO,
            },
            Account {
                id: Uuid::new_v4(),
                code: "5000".to_string(),
                name: "Cost of Goods Sold".to_string(),
                account_type: AccountType::Expense,
                balance: Decimal::ZERO,
            },
            Account {
                id: Uuid::new_v4(),
                code: "6000".to_string(),
                name: "Operating Expenses".to_string(),
                account_type: AccountType::Expense,
                balance: Decimal::ZERO,
            },
        ]
    }

    /// Validates double-entry invariant: Sum(Debits) MUST == Sum(Credits)
    pub fn validate_entry(entry: &JournalEntry) -> Result<(), String> {
        let total_debit: Decimal = entry.lines.iter().map(|l| l.debit).sum();
        let total_credit: Decimal = entry.lines.iter().map(|l| l.credit).sum();

        if total_debit != total_credit {
            return Err(format!(
                "Unbalanced Journal Entry: Debits ({}) != Credits ({})",
                total_debit, total_credit
            ));
        }

        if total_debit <= Decimal::ZERO {
            return Err("Journal entry total must be greater than zero".to_string());
        }

        Ok(())
    }

    /// Automatically generates a balancing journal entry when an Invoice is issued/posted
    /// Debit: Accounts Receivable (1200)
    /// Credit: Sales Revenue (4000)
    /// Credit: Sales Tax Payable (2200) [if tax > 0]
    pub fn create_invoice_journal_entry(
        invoice: &Invoice,
        ar_account: &Account,
        revenue_account: &Account,
        tax_account: &Account,
        sequence: u32,
    ) -> Result<JournalEntry, String> {
        let mut lines = Vec::new();

        // 1. Debit Accounts Receivable for total amount
        lines.push(JournalLine {
            account_id: ar_account.id,
            account_code: ar_account.code.clone(),
            account_name: ar_account.name.clone(),
            debit: invoice.total_amount,
            credit: Decimal::ZERO,
            memo: format!(
                "Invoice {} for {}",
                invoice.invoice_number, invoice.customer_name
            ),
        });

        // 2. Credit Sales Revenue for subtotal
        lines.push(JournalLine {
            account_id: revenue_account.id,
            account_code: revenue_account.code.clone(),
            account_name: revenue_account.name.clone(),
            debit: Decimal::ZERO,
            credit: invoice.subtotal,
            memo: format!("Sales revenue: {}", invoice.invoice_number),
        });

        // 3. Credit Sales Tax Payable if applicable
        if invoice.tax_amount > Decimal::ZERO {
            lines.push(JournalLine {
                account_id: tax_account.id,
                account_code: tax_account.code.clone(),
                account_name: tax_account.name.clone(),
                debit: Decimal::ZERO,
                credit: invoice.tax_amount,
                memo: format!("Tax collected: {}", invoice.invoice_number),
            });
        }

        let entry = JournalEntry {
            id: Uuid::new_v4(),
            entry_number: format!("JE-{:04}", sequence),
            entry_date: invoice.issue_date,
            memo: format!("Posting Invoice {}", invoice.invoice_number),
            reference: Some(invoice.invoice_number.clone()),
            lines,
            created_at: Utc::now(),
        };

        Self::validate_entry(&entry)?;
        Ok(entry)
    }

    /// Automatically generates a balancing journal entry when an Invoice is marked as Paid
    /// Debit: Cash and Bank (1000)
    /// Credit: Accounts Receivable (1200)
    pub fn create_payment_journal_entry(
        invoice: &Invoice,
        cash_account: &Account,
        ar_account: &Account,
        sequence: u32,
    ) -> Result<JournalEntry, String> {
        let lines = vec![
            JournalLine {
                account_id: cash_account.id,
                account_code: cash_account.code.clone(),
                account_name: cash_account.name.clone(),
                debit: invoice.total_amount,
                credit: Decimal::ZERO,
                memo: format!("Payment received for {}", invoice.invoice_number),
            },
            JournalLine {
                account_id: ar_account.id,
                account_code: ar_account.code.clone(),
                account_name: ar_account.name.clone(),
                debit: Decimal::ZERO,
                credit: invoice.total_amount,
                memo: format!("Clear A/R for {}", invoice.invoice_number),
            },
        ];

        let entry = JournalEntry {
            id: Uuid::new_v4(),
            entry_number: format!("JE-{:04}", sequence),
            entry_date: invoice.due_date,
            memo: format!("Payment received: {}", invoice.invoice_number),
            reference: Some(invoice.invoice_number.clone()),
            lines,
            created_at: Utc::now(),
        };

        Self::validate_entry(&entry)?;
        Ok(entry)
    }
}
