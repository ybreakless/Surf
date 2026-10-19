//! Startup automations: daily FX top-up, DB backups, recurring billing,
//! and overdue scanning. Runs off-thread so the UI stays responsive.

use chrono::Duration;
use chrono::Utc;
use rust_decimal::Decimal;
use std::sync::mpsc::{self, Receiver};
use std::thread;
use uuid::Uuid;

use crate::core::accounting::AccountingEngine;
use crate::core::fx::resolve_rate;
use crate::core::model::{Invoice, InvoiceItem, InvoiceStatus, RecurringInvoice};
use crate::core::money::Currency;
use crate::core::numbering::NumberingEngine;
use crate::db::repository::Repository;

const MAX_CATCHUP_PER_RUN: usize = 12;
const BACKUP_RETENTION_DAYS: u64 = 30;

/// Fires when the background automation run completes with a human summary.
pub enum AutomationMsg {
    Done(String),
}

/// Executes every automation once, logging each outcome and returning a summary.
pub fn run_all_blocking(repo: &Repository) -> String {
    Vec::from_iter([
        run_fx_topup(repo),
        run_backup(repo),
        run_recurring_billing(repo),
        run_overdue_scan(repo),
    ])
    .join(" \u{00B7} ")
}

/// Ensures today's free-market IRR rates exist; fetches missing ones from tgju.org.
pub fn run_fx_topup(repo: &Repository) -> String {
    let today = Utc::now().date_naive();
    let rates = repo.get_all_fx_rates().unwrap_or_default();
    let missing: Vec<&str> = Currency::ALL
        .iter()
        .filter(|c| **c != Currency::IRR)
        .filter(|c| resolve_rate(**c, today, &rates).is_none())
        .map(|c| c.code())
        .collect();

    if missing.is_empty() {
        let msg = "FX rates already current".to_string();
        let _ = repo.insert_automation_log("fx_topup", true, &msg);
        return format!("\u{2714} {msg}");
    }

    match super::fx_provider::update_rates_blocking(repo) {
        Ok((saved, failed)) => {
            let failed_note = if failed.is_empty() {
                String::new()
            } else {
                format!(", {} failed", failed.join(", "))
            };
            let msg = format!(
                "FX update stored {saved} currencies{failed_note} (missing: {})",
                missing.join(", ")
            );
            let _ = repo.insert_automation_log("fx_topup", saved > 0, &msg);
            format!("\u{2714} FX top-up: {saved} stored")
        }
        Err(e) => {
            let msg = format!("FX update failed: {e}");
            let _ = repo.insert_automation_log("fx_topup", false, &msg);
            "\u{26A0} FX top-up failed".to_string()
        }
    }
}

/// Creates today's offline snapshot (one per day, in a `backups/` folder next to the DB).
pub fn run_backup(repo: &Repository) -> String {
    let dir = repo
        .db_path()
        .parent()
        .unwrap_or(std::path::Path::new("."))
        .join("backups");
    match repo.backup_into(&dir) {
        Ok(path) => {
            prune_backups(&dir);
            let msg = format!("Database snapshot: {}", path.display());
            let _ = repo.insert_automation_log("backup", true, &msg);
            format!(
                "\u{2714} Backup: {}",
                path.file_name().unwrap_or_default().to_string_lossy()
            )
        }
        Err(e) => {
            let msg = format!("Backup failed: {e}");
            let _ = repo.insert_automation_log("backup", false, &msg);
            "\u{26A0} Backup failed".to_string()
        }
    }
}

/// Keeps only the newest backups.
fn prune_backups(dir: &std::path::Path) {
    let cutoff = Utc::now().date_naive() - Duration::days(BACKUP_RETENTION_DAYS as i64);
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries.flatten() {
            let name_date = e
                .file_name()
                .to_string_lossy()
                .strip_prefix("surf_backup_")
                .and_then(|s| s.strip_suffix(".db"))
                .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y%m%d").ok());
            if let Some(d) = name_date
                && d < cutoff
            {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
}

/// Generates invoices for every active template whose billing date has arrived,
/// advances its `next_run` (catching up at most [`MAX_CATCHUP_PER_RUN`] cycles).
pub fn run_recurring_billing(repo: &Repository) -> String {
    let today = Utc::now().date_naive();
    let templates = repo.get_recurring_invoices().unwrap_or_default();
    let mut generated = 0usize;
    let mut errors = 0usize;

    for tpl in templates.iter().filter(|t| t.active && t.next_run <= today) {
        let mut date = tpl.next_run;
        let mut cycles = 0usize;
        while date <= today && cycles < MAX_CATCHUP_PER_RUN {
            match generate_invoice_from_template(repo, tpl, date) {
                Ok(number) => {
                    generated += 1;
                    let msg = format!(
                        "Recurring invoice {} generated from template '{}'",
                        number, tpl.template_name
                    );
                    let _ = repo.insert_automation_log("recurring_billing", true, &msg);
                }
                Err(e) => {
                    errors += 1;
                    let msg = format!("Template '{}' failed: {}", tpl.template_name, e);
                    let _ = repo.insert_automation_log("recurring_billing", false, &msg);
                    break;
                }
            }
            date = tpl.frequency.next_date(date);
            cycles += 1;
        }
        if cycles > 0 {
            let _ = repo.mark_recurring_run(tpl.id, date);
        }
    }

    if errors > 0 {
        format!("\u{26A0} Recurring: {generated} generated, {errors} errors")
    } else if generated > 0 {
        format!("\u{2714} Recurring billing: {generated} invoices generated")
    } else {
        String::from("\u{2714} Recurring billing: nothing due")
    }
}

/// Builds + posts one invoice (with double-entry journal) from a template.
fn generate_invoice_from_template(
    repo: &Repository,
    tpl: &RecurringInvoice,
    bill_date: chrono::NaiveDate,
) -> std::result::Result<String, String> {
    let customers = repo.get_customers().unwrap_or_default();
    let cust = customers
        .iter()
        .find(|c| c.id == tpl.customer_id)
        .ok_or_else(|| "customer not found".to_string())?;

    let seq = repo
        .next_invoice_sequence(&cust.postal_zone, &cust.code)
        .unwrap_or(1);
    let number = NumberingEngine::format_invoice_number(&cust.postal_zone, &cust.code, seq);

    let qty = tpl.item_qty;
    let price = tpl.item_unit_price;
    let line_total = qty * price;
    let subtotal = line_total;
    let tax_amount = subtotal * (tpl.tax_rate / Decimal::from(100));
    let total_amount = subtotal + tax_amount;

    let item = InvoiceItem {
        id: Uuid::new_v4(),
        description: tpl.item_description.clone(),
        quantity: qty,
        unit: tpl.item_unit.clone(),
        unit_price: price,
        discount_pct: Decimal::ZERO,
        amount: line_total,
    };

    let mut invoice = Invoice {
        id: Uuid::new_v4(),
        invoice_number: number.clone(),
        customer_id: cust.id,
        customer_name: cust.name.clone(),
        postal_code: cust.postal_code.clone(),
        postal_zone: cust.postal_zone.clone(),
        issue_date: bill_date,
        due_date: bill_date + Duration::days(30), // Net 30 like interactive flow
        status: InvoiceStatus::Sent,
        items: vec![item],
        subtotal,
        tax_rate: tpl.tax_rate,
        tax_amount,
        total_amount,
        currency: cust.currency.clone(),
        notes: format!(
            "Auto-generated from recurring template '{}'",
            tpl.template_name
        ),
        journal_entry_id: None,
        created_at: Utc::now(),
    };

    let accounts = repo.get_accounts().unwrap_or_default();
    let ar = accounts.iter().find(|a| a.code == "1200").cloned();
    let rev = accounts.iter().find(|a| a.code == "4000").cloned();
    let tax = accounts.iter().find(|a| a.code == "2200").cloned();

    if let (Some(ar), Some(rev), Some(tax)) = (ar, rev, tax) {
        let seq = repo.next_journal_sequence().unwrap_or(1);
        if let Ok(mut je) =
            AccountingEngine::create_invoice_journal_entry(&invoice, &ar, &rev, &tax, seq)
        {
            je.reference = Some(invoice.invoice_number.clone());
            repo.insert_journal_entry(&je, true)
                .map_err(|e| format!("journal: {e}"))?;
            invoice.journal_entry_id = Some(je.id);
        }
    }

    repo.insert_invoice(&invoice, true)
        .map_err(|e| format!("invoice: {e}"))?;
    Ok(number.clone())
}

/// Counts unpaid invoices already past their due date (informational scan).
pub fn run_overdue_scan(repo: &Repository) -> String {
    let today = Utc::now().date_naive();
    let invoices = repo.get_invoices().unwrap_or_default();
    let overdue: Vec<&Invoice> = invoices
        .iter()
        .filter(|i| i.status == InvoiceStatus::Sent && i.due_date < today)
        .collect();

    let listing = overdue
        .iter()
        .take(5)
        .map(|i| i.invoice_number.clone())
        .collect::<Vec<_>>()
        .join(", ");

    let msg = if overdue.is_empty() {
        "No overdue invoices".to_string()
    } else {
        format!(
            "{} overdue invoice(s): {}{}",
            overdue.len(),
            listing,
            if overdue.len() > 5 { "\u{2026}" } else { "" }
        )
    };
    let _ = repo.insert_automation_log("overdue_scan", true, &msg);

    if overdue.is_empty() {
        "\u{2714} Overdue: none".to_string()
    } else {
        format!("\u{26A0} Overdue: {} invoice(s)", overdue.len())
    }
}

/// Spawns the worker that runs all automations then reports back.
pub fn spawn_automations_worker(db_path: std::path::PathBuf) -> Receiver<AutomationMsg> {
    let (tx, rx) = mpsc::channel();

    thread::Builder::new()
        .name("surf-automations".to_string())
        .spawn(move || {
            let result = Repository::new(&db_path)
                .map_err(|e| e.to_string())
                .map(|repo| run_all_blocking(&repo));
            let summary = match result {
                Ok(s) => s,
                Err(e) => format!("\u{26A0} Automations engine failed: {e}"),
            };
            let _ = tx.send(AutomationMsg::Done(summary));
        })
        .expect("spawn automations worker");

    rx
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::model::{Customer, Frequency, RecurringInvoice};
    use rust_decimal_macros::dec;

    fn customer() -> Customer {
        Customer {
            id: Uuid::new_v4(),
            name: "Automated Trading LLC".to_string(),
            code: "ATL".to_string(),
            email: String::new(),
            phone: String::new(),
            address: String::new(),
            city: String::new(),
            country: String::new(),
            postal_code: "10001".to_string(),
            postal_zone: "100".to_string(),
            currency: "USD".to_string(),
            notes: String::new(),
            created_at: Utc::now(),
        }
    }

    fn template(next_run: chrono::NaiveDate, customer_id: Uuid) -> RecurringInvoice {
        RecurringInvoice {
            id: Uuid::new_v4(),
            template_name: "Monthly Hosting".to_string(),
            customer_id,
            frequency: Frequency::Monthly,
            next_run,
            tax_rate: dec!(5),
            item_description: "Hosting & support retainer".to_string(),
            item_qty: dec!(1),
            item_unit: "Mo".to_string(),
            item_unit_price: dec!(1000),
            active: true,
            last_run: None,
            created_at: Utc::now(),
        }
    }

    #[test]
    fn recurring_generates_invoice_advances_and_logs() {
        let repo = Repository::in_memory().unwrap();
        let cust = customer();
        repo.insert_customer(&cust, true).unwrap();
        let past = Utc::now().date_naive() - Duration::days(35);
        let tpl = template(past, cust.id);
        repo.insert_recurring_invoice(&tpl).unwrap();

        let summary = run_recurring_billing(&repo);

        assert!(summary.contains("2 invoices generated"), "{summary}");
        let invoices = repo.get_invoices().unwrap();
        assert_eq!(invoices.len(), 2);
        for inv in &invoices {
            assert_eq!(inv.total_amount, dec!(1050)); // 1000 + 5% tax
            assert_eq!(inv.status, InvoiceStatus::Sent);
            assert_eq!(inv.currency, "USD");
            assert!(inv.journal_entry_id.is_some());
        }

        // Two billed cycles: AR debited, revenue credited each time.
        let accounts = repo.get_accounts().unwrap();
        let ar = accounts.iter().find(|a| a.code == "1200").unwrap();
        let rev = accounts.iter().find(|a| a.code == "4000").unwrap();
        assert_eq!(ar.balance, dec!(2100));
        assert_eq!(rev.balance, dec!(2000));

        // Template advanced to a strictly-future date (35 days back -> +25d ahead).
        let after = repo.get_recurring_invoices().unwrap();
        assert!(after[0].next_run > Utc::now().date_naive());

        // Logs recorded
        let logs = repo.get_automation_logs(10).unwrap();
        assert!(!logs.is_empty());
        assert!(
            logs.iter()
                .any(|(task, ok, _, _)| *task == "recurring_billing" && *ok)
        );
    }

    #[test]
    fn backup_creates_snapshot_file() {
        let dir = std::env::temp_dir().join(format!("surf-backup-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let repo = Repository::in_memory().unwrap();
        // in-memory DBs can still VACUUM INTO a real file
        let path = repo.backup_into(&dir).unwrap();
        assert!(path.exists());
        assert!(path.metadata().unwrap().len() > 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn overdue_scan_reports_unpaid_past_due() {
        let repo = Repository::in_memory().unwrap();
        let cust = customer();
        repo.insert_customer(&cust, true).unwrap();
        let base = Utc::now().date_naive();
        let customers = repo.get_customers().unwrap();
        let cust = &customers[0];
        let seq = repo
            .next_invoice_sequence(&cust.postal_zone, &cust.code)
            .unwrap();
        let number = NumberingEngine::format_invoice_number(&cust.postal_zone, &cust.code, seq);
        let invoice = Invoice {
            id: Uuid::new_v4(),
            invoice_number: number,
            customer_id: cust.id,
            customer_name: cust.name.clone(),
            postal_code: cust.postal_code.clone(),
            postal_zone: cust.postal_zone.clone(),
            issue_date: base,
            due_date: base - Duration::days(5),
            status: InvoiceStatus::Sent,
            items: vec![],
            subtotal: dec!(10),
            tax_rate: dec!(0),
            tax_amount: dec!(0),
            total_amount: dec!(10),
            currency: "USD".to_string(),
            notes: String::new(),
            journal_entry_id: None,
            created_at: Utc::now(),
        };
        repo.insert_invoice(&invoice, false).unwrap();

        let summary = run_overdue_scan(&repo);
        assert!(summary.contains("1 invoice(s)"), "{summary}");
        let logs = repo.get_automation_logs(10).unwrap();
        assert!(
            logs.iter()
                .any(|(t, _, r, _)| t == "overdue_scan" && r.contains("overdue invoice"))
        );
    }

    #[test]
    fn fx_topup_skips_when_current() {
        let repo = Repository::in_memory().unwrap();
        let today = Utc::now().date_naive();
        for c in Currency::ALL.iter().filter(|c| **c != Currency::IRR) {
            let fx = crate::core::fx::FxRate {
                id: Uuid::new_v4(),
                base: Currency::IRR,
                quote: *c,
                rate: dec!(1000),
                as_of_date: today,
                source: crate::core::fx::RateSource::Manual,
                source_url: None,
                raw_value: None,
                fetched_at: Some(Utc::now()),
                created_at: Utc::now(),
            };
            repo.upsert_fx_rate(&fx).unwrap();
        }
        let summary = run_fx_topup(&repo);
        assert!(summary.contains("already current"), "{summary}");
    }

    #[test]
    fn inactive_templates_do_not_generate() {
        let repo = Repository::in_memory().unwrap();
        let cust = customer();
        repo.insert_customer(&cust, true).unwrap();
        let mut tpl = template(Utc::now().date_naive(), cust.id);
        tpl.active = false;
        repo.insert_recurring_invoice(&tpl).unwrap();
        let summary = run_recurring_billing(&repo);
        assert!(summary.contains("nothing due"), "{summary}");
        assert!(repo.get_invoices().unwrap().is_empty());
    }
}
