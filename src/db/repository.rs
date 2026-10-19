use chrono::{DateTime, NaiveDate, Utc};
use rusqlite::{Connection, Result, params};
use rust_decimal::Decimal;
use std::path::Path;
use std::str::FromStr;
use uuid::Uuid;

use crate::core::accounting::AccountingEngine;
use crate::core::fx::{FxRate, RateSource};
use crate::core::model::{
    Account, AccountType, BillStatus, CompanyProfile, Customer, DeliveryItem, DeliveryOrder,
    DeliveryStatus, Invoice, InvoiceItem, InvoiceStatus, JournalEntry, JournalLine, Product,
    ProductKind, RecurringInvoice, Vendor, VendorBill,
};
use crate::core::money::Currency;
use crate::core::sync::{ChangeRecord, SyncAction, SyncBundle};
use std::path::PathBuf;

pub struct Repository {
    conn: Connection,
    db_path: std::path::PathBuf,
    pub device_id: String,
}

impl Repository {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let db_path = path.as_ref().to_path_buf();
        let conn = Connection::open(&db_path)?;
        let mut repo = Self::init_common(conn)?;
        repo.db_path = db_path;
        Ok(repo)
    }

    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    pub fn in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        Self::init_common(conn)
    }

    fn init_common(conn: Connection) -> Result<Self> {
        conn.busy_timeout(std::time::Duration::from_millis(5000))?;
        let _ = conn.pragma_update(None, "journal_mode", "WAL");
        let _ = conn.pragma_update(None, "foreign_keys", "ON");
        let mut repo = Self {
            conn,
            db_path: std::path::PathBuf::from(":memory:"),
            device_id: String::new(),
        };
        repo.init_schema()?;
        repo.run_migrations()?;
        repo.load_or_create_device_id()?;
        repo.seed_default_accounts()?;
        Ok(repo)
    }

    fn init_schema(&self) -> Result<()> {
        self.conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS app_settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS accounts (
                id TEXT PRIMARY KEY,
                code TEXT NOT NULL UNIQUE,
                name TEXT NOT NULL,
                account_type TEXT NOT NULL,
                balance TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS customers (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                code TEXT NOT NULL,
                email TEXT,
                phone TEXT,
                address TEXT,
                city TEXT,
                country TEXT,
                postal_code TEXT,
                postal_zone TEXT,
                currency TEXT NOT NULL,
                notes TEXT,
                created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS invoices (
                id TEXT PRIMARY KEY,
                invoice_number TEXT NOT NULL UNIQUE,
                customer_id TEXT NOT NULL,
                customer_name TEXT NOT NULL,
                postal_code TEXT,
                postal_zone TEXT,
                issue_date TEXT NOT NULL,
                due_date TEXT NOT NULL,
                status TEXT NOT NULL,
                items_json TEXT NOT NULL,
                subtotal TEXT NOT NULL,
                tax_rate TEXT NOT NULL,
                tax_amount TEXT NOT NULL,
                total_amount TEXT NOT NULL,
                currency TEXT NOT NULL,
                notes TEXT,
                journal_entry_id TEXT,
                created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS delivery_orders (
                id TEXT PRIMARY KEY,
                delivery_number TEXT NOT NULL UNIQUE,
                invoice_id TEXT,
                invoice_number TEXT,
                customer_id TEXT NOT NULL,
                customer_name TEXT NOT NULL,
                delivery_address TEXT,
                postal_code TEXT,
                postal_zone TEXT,
                delivery_date TEXT NOT NULL,
                status TEXT NOT NULL,
                items_json TEXT NOT NULL,
                driver_notes TEXT,
                created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS journal_entries (
                id TEXT PRIMARY KEY,
                entry_number TEXT NOT NULL UNIQUE,
                entry_date TEXT NOT NULL,
                memo TEXT NOT NULL,
                reference TEXT,
                lines_json TEXT NOT NULL,
                created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS sync_changes (
                change_id TEXT PRIMARY KEY,
                device_id TEXT NOT NULL,
                entity_type TEXT NOT NULL,
                entity_id TEXT NOT NULL,
                action TEXT NOT NULL,
                payload_json TEXT NOT NULL,
                timestamp_ms INTEGER NOT NULL
            );
            "#,
        )?;
        Ok(())
    }

    /// Numbered migrations applied on top of `init_schema`. Each runs exactly once
    /// inside its own transaction; `schema_version` in app_settings tracks progress.
    const MIGRATIONS: &'static [(&'static str, &'static str)] = &[
        (
            "0001_fx_rates",
            r#"
            CREATE TABLE IF NOT EXISTS fx_rates (
                id TEXT PRIMARY KEY,
                base_code TEXT NOT NULL,
                quote_code TEXT NOT NULL,
                rate TEXT NOT NULL,
                as_of_date TEXT NOT NULL,
                source TEXT NOT NULL,
                source_url TEXT,
                raw_value TEXT,
                fetched_at TEXT,
                created_at TEXT NOT NULL,
                UNIQUE (quote_code, as_of_date)
            );
            CREATE INDEX IF NOT EXISTS idx_fx_rates_quote_date
                ON fx_rates (quote_code, as_of_date DESC);
            "#,
        ),
        (
            "0002_recurring_invoices",
            r#"
            CREATE TABLE IF NOT EXISTS recurring_invoices (
                id TEXT PRIMARY KEY,
                template_name TEXT NOT NULL,
                customer_id TEXT NOT NULL,
                frequency TEXT NOT NULL,
                next_run TEXT NOT NULL,
                tax_rate TEXT NOT NULL DEFAULT '0',
                item_description TEXT NOT NULL DEFAULT '',
                item_qty TEXT NOT NULL DEFAULT '1',
                item_unit TEXT NOT NULL DEFAULT 'Pcs',
                item_unit_price TEXT NOT NULL DEFAULT '0',
                active INTEGER NOT NULL DEFAULT 1,
                last_run TEXT,
                created_at TEXT NOT NULL
            );
            "#,
        ),
        (
            "0003_automation_log",
            r#"
            CREATE TABLE IF NOT EXISTS automation_log (
                id TEXT PRIMARY KEY,
                task TEXT NOT NULL,
                ok INTEGER NOT NULL,
                result TEXT NOT NULL,
                ran_at TEXT NOT NULL
            );
            "#,
        ),
        (
            "0004_purchases",
            r#"
            CREATE TABLE IF NOT EXISTS vendors (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                code TEXT NOT NULL,
                email TEXT,
                phone TEXT,
                address TEXT,
                city TEXT,
                country TEXT,
                postal_code TEXT,
                currency TEXT NOT NULL,
                payment_terms_days INTEGER NOT NULL DEFAULT 30,
                notes TEXT,
                created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS products (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                kind TEXT NOT NULL,
                unit TEXT NOT NULL DEFAULT 'Pcs',
                default_price TEXT NOT NULL DEFAULT '0',
                tax_rate TEXT NOT NULL DEFAULT '0',
                income_account_code TEXT NOT NULL DEFAULT '4000',
                expense_account_code TEXT NOT NULL DEFAULT '6000',
                active INTEGER NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS vendor_bills (
                id TEXT PRIMARY KEY,
                bill_number TEXT NOT NULL UNIQUE,
                vendor_id TEXT NOT NULL,
                vendor_name TEXT NOT NULL,
                issue_date TEXT NOT NULL,
                due_date TEXT NOT NULL,
                status TEXT NOT NULL,
                items_json TEXT NOT NULL,
                subtotal TEXT NOT NULL,
                tax_rate TEXT NOT NULL,
                tax_amount TEXT NOT NULL,
                total_amount TEXT NOT NULL,
                currency TEXT NOT NULL,
                notes TEXT,
                journal_entry_id TEXT,
                created_at TEXT NOT NULL
            );
            "#,
        ),
    ];

    fn run_migrations(&self) -> Result<()> {
        for (name, sql) in Self::MIGRATIONS {
            let applied: bool = self.conn.query_row(
                "SELECT count(*) FROM app_settings WHERE key = 'migration' || ?1",
                params![name],
                |r| r.get::<_, i64>(0),
            )? > 0;
            if applied {
                continue;
            }
            self.conn.execute_batch(
                "BEGIN IMMEDIATE;
                 SAVEPOINT mig;",
            )?;
            let result = self
                .conn
                .execute_batch(sql)
                .and_then(|_| {
                    self.conn.execute(
                        "INSERT INTO app_settings (key, value) VALUES ('migration' || ?1, 'ok')",
                        params![name],
                    )
                })
                .and_then(|_| self.conn.execute("COMMIT", []));
            match result {
                Ok(_) => {}
                Err(e) => {
                    let _ = self.conn.execute_batch("ROLLBACK");
                    return Err(e);
                }
            }
        }
        Ok(())
    }

    // --- Company Settings ---

    /// Generic key-value setting read; returns None when unset.
    pub fn get_setting(&self, key: &str) -> Option<String> {
        self.conn
            .query_row(
                "SELECT value FROM app_settings WHERE key = ?1",
                params![key],
                |r| r.get::<_, String>(0),
            )
            .ok()
    }

    /// Generic key-value setting write (upsert).
    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO app_settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn get_base_currency(&self) -> String {
        self.conn
            .query_row(
                "SELECT value FROM app_settings WHERE key = 'base_currency'",
                [],
                |r| r.get::<_, String>(0),
            )
            .unwrap_or_else(|_| Currency::IRR.code().to_string())
    }

    pub fn set_base_currency(&self, code: &str) -> Result<()> {
        let normalized = Currency::parse(code)
            .map(|c| c.code().to_string())
            .unwrap_or_else(|| Currency::IRR.code().to_string());
        self.conn.execute(
            "INSERT INTO app_settings (key, value) VALUES ('base_currency', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![normalized],
        )?;
        Ok(())
    }

    pub fn get_company_profile(&self) -> CompanyProfile {
        self.conn
            .query_row(
                "SELECT value FROM app_settings WHERE key = 'company_profile'",
                [],
                |r| r.get::<_, String>(0),
            )
            .ok()
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default()
    }

    pub fn save_company_profile(&self, profile: &CompanyProfile) -> Result<()> {
        let json = serde_json::to_string(profile).unwrap_or_default();
        self.conn.execute(
            "INSERT INTO app_settings (key, value) VALUES ('company_profile', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![json],
        )?;
        Ok(())
    }

    // --- Recurring billing templates ---

    pub fn insert_recurring_invoice(&self, t: &RecurringInvoice) -> Result<()> {
        self.conn.execute(
            "INSERT INTO recurring_invoices (id, template_name, customer_id, frequency, next_run, tax_rate, item_description, item_qty, item_unit, item_unit_price, active, last_run, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                t.id.to_string(),
                t.template_name,
                t.customer_id.to_string(),
                t.frequency.as_str(),
                t.next_run.to_string(),
                t.tax_rate.to_string(),
                t.item_description,
                t.item_qty.to_string(),
                t.item_unit,
                t.item_unit_price.to_string(),
                t.active as i64,
                t.last_run.map(|d| d.to_string()),
                t.created_at.to_rfc3339()
            ],
        )?;
        Ok(())
    }

    pub fn get_recurring_invoices(&self) -> Result<Vec<RecurringInvoice>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, template_name, customer_id, frequency, next_run, tax_rate, item_description, item_qty, item_unit, item_unit_price, active, last_run, created_at
             FROM recurring_invoices ORDER BY next_run ASC, created_at ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            let id_str: String = row.get(0)?;
            let cust_str: String = row.get(2)?;
            let freq_str: String = row.get(3)?;
            let next_str: String = row.get(4)?;
            let last_str: Option<String> = row.get(11)?;
            let created_str: String = row.get(12)?;
            Ok(RecurringInvoice {
                id: Uuid::parse_str(&id_str).unwrap_or_default(),
                template_name: row.get(1)?,
                customer_id: Uuid::parse_str(&cust_str).unwrap_or_default(),
                frequency: crate::core::model::Frequency::parse(&freq_str),
                next_run: NaiveDate::parse_from_str(&next_str, "%Y-%m-%d")
                    .unwrap_or_else(|_| Utc::now().date_naive()),
                tax_rate: Decimal::from_str(&row.get::<_, String>(5).unwrap_or_default())
                    .unwrap_or(Decimal::ZERO),
                item_description: row.get(6)?,
                item_qty: Decimal::from_str(&row.get::<_, String>(7).unwrap_or_default())
                    .unwrap_or(Decimal::ONE),
                item_unit: row.get(8)?,
                item_unit_price: Decimal::from_str(&row.get::<_, String>(9).unwrap_or_default())
                    .unwrap_or(Decimal::ZERO),
                active: row.get::<_, i64>(10)? != 0,
                last_run: last_str.and_then(|s| NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok()),
                created_at: DateTime::parse_from_rfc3339(&created_str)
                    .map(|d| d.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
            })
        })?;
        let mut res = Vec::new();
        for r in rows {
            res.push(r?);
        }
        Ok(res)
    }

    /// Marks a template as having generated its invoice: advances next_run and last_run.
    pub fn mark_recurring_run(&self, id: Uuid, next_run: NaiveDate) -> Result<()> {
        self.conn.execute(
            "UPDATE recurring_invoices SET next_run = ?2, last_run = ?3 WHERE id = ?1",
            params![
                id.to_string(),
                next_run.to_string(),
                Utc::now().date_naive().to_string()
            ],
        )?;
        Ok(())
    }

    pub fn set_recurring_active(&self, id: Uuid, active: bool) -> Result<()> {
        self.conn.execute(
            "UPDATE recurring_invoices SET active = ?2 WHERE id = ?1",
            params![id.to_string(), active as i64],
        )?;
        Ok(())
    }

    // --- Automation log ---

    pub fn insert_automation_log(&self, task: &str, ok: bool, result: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO automation_log (id, task, ok, result, ran_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                Uuid::new_v4().to_string(),
                task,
                ok as i64,
                result,
                Utc::now().to_rfc3339()
            ],
        )?;
        Ok(())
    }

    pub fn get_automation_logs(&self, limit: usize) -> Result<Vec<(String, bool, String, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT task, ok, result, ran_at FROM automation_log ORDER BY ran_at DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok((
                row.get(0)?,
                row.get::<_, i64>(1)? != 0,
                row.get(2)?,
                row.get(3)?,
            ))
        })?;
        let mut res = Vec::new();
        for r in rows {
            res.push(r?);
        }
        Ok(res)
    }

    /// Offline snapshot of the whole database via SQLite VACUUM INTO.
    /// Name is date-based so one run per day replaces itself.
    pub fn backup_into(&self, dir: &Path) -> std::result::Result<PathBuf, String> {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        let name = format!(
            "surf_backup_{}.db",
            Utc::now().date_naive().format("%Y%m%d")
        );
        let target = dir.join(&name);
        if target.exists() {
            std::fs::remove_file(&target).map_err(|e| e.to_string())?;
        }
        self.conn
            .execute(
                "VACUUM INTO ?1",
                params![target.to_string_lossy().to_string()],
            )
            .map_err(|e| e.to_string())?;
        Ok(target)
    }

    // --- FX Rates ---

    pub fn upsert_fx_rate(&self, rate: &FxRate) -> Result<()> {
        self.conn.execute(
            "INSERT INTO fx_rates (id, base_code, quote_code, rate, as_of_date, source, source_url, raw_value, fetched_at, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
             ON CONFLICT(quote_code, as_of_date) DO UPDATE SET
                id = excluded.id,
                base_code = excluded.base_code,
                rate = excluded.rate,
                source = excluded.source,
                source_url = excluded.source_url,
                raw_value = excluded.raw_value,
                fetched_at = excluded.fetched_at,
                created_at = excluded.created_at",
            params![
                rate.id.to_string(),
                rate.base.code(),
                rate.quote.code(),
                rate.rate.to_string(),
                rate.as_of_date.to_string(),
                rate.source.as_str(),
                rate.source_url,
                rate.raw_value,
                rate.fetched_at.map(|t| t.to_rfc3339()),
                rate.created_at.to_rfc3339()
            ],
        )?;
        Ok(())
    }

    pub fn get_all_fx_rates(&self) -> Result<Vec<FxRate>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, base_code, quote_code, rate, as_of_date, source, source_url, raw_value, fetched_at, created_at
             FROM fx_rates ORDER BY as_of_date DESC, created_at DESC LIMIT 500",
        )?;

        let rows = stmt.query_map([], |row| {
            let id_str: String = row.get(0)?;
            let base_str: String = row.get(1)?;
            let quote_str: String = row.get(2)?;
            let rate_str: String = row.get(3)?;
            let date_str: String = row.get(4)?;
            let src_str: String = row.get(5)?;
            let created_str: String = row.get(9)?;
            let fetched_str: Option<String> = row.get(8)?;
            Ok(FxRate {
                id: Uuid::parse_str(&id_str).unwrap_or_else(|_| Uuid::new_v4()),
                base: Currency::parse(&base_str).unwrap_or(Currency::IRR),
                quote: Currency::parse(&quote_str).unwrap_or(Currency::IRR),
                rate: Decimal::from_str(&rate_str).unwrap_or(Decimal::ZERO),
                as_of_date: NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")
                    .unwrap_or_else(|_| Utc::now().date_naive()),
                source: RateSource::parse(&src_str),
                source_url: row.get(6)?,
                raw_value: row.get(7)?,
                fetched_at: fetched_str
                    .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                    .map(|d| d.with_timezone(&Utc)),
                created_at: DateTime::parse_from_rfc3339(&created_str)
                    .map(|d| d.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
            })
        })?;

        let mut res = Vec::new();
        for r in rows {
            res.push(r?);
        }
        Ok(res)
    }

    /// The freshest stored rate for each currency (used by UI/converter).
    pub fn get_latest_fx_rates(&self) -> Result<Vec<FxRate>> {
        let all = self.get_all_fx_rates()?;
        let mut latest: Vec<FxRate> = Vec::new();
        for c in Currency::ALL {
            if let Some(r) = all.iter().find(|r| r.quote == c) {
                latest.push(r.clone());
            }
        }
        Ok(latest)
    }

    fn load_or_create_device_id(&mut self) -> Result<()> {
        let mut stmt = self
            .conn
            .prepare("SELECT value FROM app_settings WHERE key = 'device_id'")?;
        let mut rows = stmt.query([])?;
        if let Some(row) = rows.next()? {
            self.device_id = row.get(0)?;
        } else {
            let id = format!("surf-{}", &Uuid::new_v4().to_string()[..8]);
            self.conn.execute(
                "INSERT INTO app_settings (key, value) VALUES ('device_id', ?1)",
                params![id],
            )?;
            self.device_id = id;
        }
        Ok(())
    }

    fn seed_default_accounts(&self) -> Result<()> {
        let count: i64 = self
            .conn
            .query_row("SELECT count(*) FROM accounts", [], |r| r.get(0))?;
        if count == 0 {
            let defaults = AccountingEngine::default_chart_of_accounts();
            for acc in defaults {
                self.insert_account(&acc, false)?;
            }
        }
        Ok(())
    }

    // --- Sync Record Helper ---
    fn record_sync_change(
        &self,
        entity_type: &str,
        entity_id: &str,
        action: SyncAction,
        payload_json: &str,
    ) -> Result<()> {
        let action_str = match action {
            SyncAction::Upsert => "UPSERT",
            SyncAction::Delete => "DELETE",
        };
        let change_id = Uuid::new_v4().to_string();
        let now_ms = Utc::now().timestamp_millis();
        self.conn.execute(
            "INSERT INTO sync_changes (change_id, device_id, entity_type, entity_id, action, payload_json, timestamp_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                change_id,
                self.device_id,
                entity_type,
                entity_id,
                action_str,
                payload_json,
                now_ms
            ],
        )?;
        Ok(())
    }

    // --- Accounts ---
    pub fn insert_account(&self, acc: &Account, track_sync: bool) -> Result<()> {
        let type_str = format!("{:?}", acc.account_type);
        self.conn.execute(
            "INSERT OR REPLACE INTO accounts (id, code, name, account_type, balance) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                acc.id.to_string(),
                acc.code,
                acc.name,
                type_str,
                acc.balance.to_string()
            ],
        )?;

        if track_sync {
            let json = serde_json::to_string(acc).unwrap_or_default();
            self.record_sync_change("account", &acc.id.to_string(), SyncAction::Upsert, &json)?;
        }
        Ok(())
    }

    pub fn get_accounts(&self) -> Result<Vec<Account>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, code, name, account_type, balance FROM accounts ORDER BY code ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            let id_str: String = row.get(0)?;
            let code: String = row.get(1)?;
            let name: String = row.get(2)?;
            let type_str: String = row.get(3)?;
            let balance_str: String = row.get(4)?;

            let account_type = match type_str.as_str() {
                "Asset" => AccountType::Asset,
                "Liability" => AccountType::Liability,
                "Equity" => AccountType::Equity,
                "Revenue" => AccountType::Revenue,
                _ => AccountType::Expense,
            };

            Ok(Account {
                id: Uuid::parse_str(&id_str).unwrap_or_default(),
                code,
                name,
                account_type,
                balance: Decimal::from_str(&balance_str).unwrap_or(Decimal::ZERO),
            })
        })?;

        let mut res = Vec::new();
        for r in rows {
            res.push(r?);
        }
        Ok(res)
    }

    // --- Customers ---
    pub fn insert_customer(&self, c: &Customer, track_sync: bool) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO customers (id, name, code, email, phone, address, city, country, postal_code, postal_zone, currency, notes, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                c.id.to_string(),
                c.name,
                c.code,
                c.email,
                c.phone,
                c.address,
                c.city,
                c.country,
                c.postal_code,
                c.postal_zone,
                c.currency,
                c.notes,
                c.created_at.to_rfc3339()
            ],
        )?;

        if track_sync {
            let json = serde_json::to_string(c).unwrap_or_default();
            self.record_sync_change("customer", &c.id.to_string(), SyncAction::Upsert, &json)?;
        }
        Ok(())
    }

    pub fn get_customers(&self) -> Result<Vec<Customer>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, code, email, phone, address, city, country, postal_code, postal_zone, currency, notes, created_at FROM customers ORDER BY name ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            let id_str: String = row.get(0)?;
            let created_at_str: String = row.get(12)?;
            Ok(Customer {
                id: Uuid::parse_str(&id_str).unwrap_or_default(),
                name: row.get(1)?,
                code: row.get(2)?,
                email: row.get(3)?,
                phone: row.get(4)?,
                address: row.get(5)?,
                city: row.get(6)?,
                country: row.get(7)?,
                postal_code: row.get(8)?,
                postal_zone: row.get(9)?,
                currency: row.get(10)?,
                notes: row.get(11)?,
                created_at: DateTime::parse_from_rfc3339(&created_at_str)
                    .map(|d| d.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
            })
        })?;

        let mut res = Vec::new();
        for r in rows {
            res.push(r?);
        }
        Ok(res)
    }

    // --- Invoices ---
    pub fn insert_invoice(&self, inv: &Invoice, track_sync: bool) -> Result<()> {
        let items_json = serde_json::to_string(&inv.items).unwrap_or_default();
        self.conn.execute(
            "INSERT OR REPLACE INTO invoices (id, invoice_number, customer_id, customer_name, postal_code, postal_zone, issue_date, due_date, status, items_json, subtotal, tax_rate, tax_amount, total_amount, currency, notes, journal_entry_id, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)",
            params![
                inv.id.to_string(),
                inv.invoice_number,
                inv.customer_id.to_string(),
                inv.customer_name,
                inv.postal_code,
                inv.postal_zone,
                inv.issue_date.to_string(),
                inv.due_date.to_string(),
                inv.status.as_str(),
                items_json,
                inv.subtotal.to_string(),
                inv.tax_rate.to_string(),
                inv.tax_amount.to_string(),
                inv.total_amount.to_string(),
                inv.currency,
                inv.notes,
                inv.journal_entry_id.map(|id| id.to_string()),
                inv.created_at.to_rfc3339()
            ],
        )?;

        if track_sync {
            let json = serde_json::to_string(inv).unwrap_or_default();
            self.record_sync_change("invoice", &inv.id.to_string(), SyncAction::Upsert, &json)?;
        }
        Ok(())
    }

    pub fn get_invoices(&self) -> Result<Vec<Invoice>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, invoice_number, customer_id, customer_name, postal_code, postal_zone, issue_date, due_date, status, items_json, subtotal, tax_rate, tax_amount, total_amount, currency, notes, journal_entry_id, created_at FROM invoices ORDER BY issue_date DESC, created_at DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            let id_str: String = row.get(0)?;
            let cust_id_str: String = row.get(2)?;
            let issue_str: String = row.get(6)?;
            let due_str: String = row.get(7)?;
            let status_str: String = row.get(8)?;
            let items_json: String = row.get(9)?;
            let subtotal_str: String = row.get(10)?;
            let tax_rate_str: String = row.get(11)?;
            let tax_amount_str: String = row.get(12)?;
            let total_str: String = row.get(13)?;
            let journal_id_opt: Option<String> = row.get(16)?;
            let created_at_str: String = row.get(17)?;

            let items: Vec<InvoiceItem> = serde_json::from_str(&items_json).unwrap_or_default();

            Ok(Invoice {
                id: Uuid::parse_str(&id_str).unwrap_or_default(),
                invoice_number: row.get(1)?,
                customer_id: Uuid::parse_str(&cust_id_str).unwrap_or_default(),
                customer_name: row.get(3)?,
                postal_code: row.get(4)?,
                postal_zone: row.get(5)?,
                issue_date: NaiveDate::parse_from_str(&issue_str, "%Y-%m-%d")
                    .unwrap_or_else(|_| Utc::now().date_naive()),
                due_date: NaiveDate::parse_from_str(&due_str, "%Y-%m-%d")
                    .unwrap_or_else(|_| Utc::now().date_naive()),
                status: InvoiceStatus::from_str(&status_str),
                items,
                subtotal: Decimal::from_str(&subtotal_str).unwrap_or(Decimal::ZERO),
                tax_rate: Decimal::from_str(&tax_rate_str).unwrap_or(Decimal::ZERO),
                tax_amount: Decimal::from_str(&tax_amount_str).unwrap_or(Decimal::ZERO),
                total_amount: Decimal::from_str(&total_str).unwrap_or(Decimal::ZERO),
                currency: row.get(14)?,
                notes: row.get(15)?,
                journal_entry_id: journal_id_opt.and_then(|s| Uuid::parse_str(&s).ok()),
                created_at: DateTime::parse_from_rfc3339(&created_at_str)
                    .map(|d| d.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
            })
        })?;

        let mut res = Vec::new();
        for r in rows {
            res.push(r?);
        }
        Ok(res)
    }

    pub fn next_invoice_sequence(&self, zone: &str, cust_code: &str) -> Result<u32> {
        let pattern = format!("%-{}-{}-%", zone, cust_code);
        let count: i64 = self.conn.query_row(
            "SELECT count(*) FROM invoices WHERE invoice_number LIKE ?1",
            params![pattern],
            |r| r.get(0),
        )?;
        Ok((count as u32) + 1)
    }

    // --- Delivery Orders ---
    pub fn insert_delivery_order(&self, d: &DeliveryOrder, track_sync: bool) -> Result<()> {
        let items_json = serde_json::to_string(&d.items).unwrap_or_default();
        self.conn.execute(
            "INSERT OR REPLACE INTO delivery_orders (id, delivery_number, invoice_id, invoice_number, customer_id, customer_name, delivery_address, postal_code, postal_zone, delivery_date, status, items_json, driver_notes, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                d.id.to_string(),
                d.delivery_number,
                d.invoice_id.map(|id| id.to_string()),
                d.invoice_number,
                d.customer_id.to_string(),
                d.customer_name,
                d.delivery_address,
                d.postal_code,
                d.postal_zone,
                d.delivery_date.to_string(),
                d.status.as_str(),
                items_json,
                d.driver_notes,
                d.created_at.to_rfc3339()
            ],
        )?;

        if track_sync {
            let json = serde_json::to_string(d).unwrap_or_default();
            self.record_sync_change(
                "delivery_order",
                &d.id.to_string(),
                SyncAction::Upsert,
                &json,
            )?;
        }
        Ok(())
    }

    pub fn get_delivery_orders(&self) -> Result<Vec<DeliveryOrder>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, delivery_number, invoice_id, invoice_number, customer_id, customer_name, delivery_address, postal_code, postal_zone, delivery_date, status, items_json, driver_notes, created_at FROM delivery_orders ORDER BY delivery_date DESC, created_at DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            let id_str: String = row.get(0)?;
            let inv_id_opt: Option<String> = row.get(2)?;
            let cust_id_str: String = row.get(4)?;
            let date_str: String = row.get(9)?;
            let status_str: String = row.get(10)?;
            let items_json: String = row.get(11)?;
            let created_at_str: String = row.get(13)?;

            let items: Vec<DeliveryItem> = serde_json::from_str(&items_json).unwrap_or_default();

            Ok(DeliveryOrder {
                id: Uuid::parse_str(&id_str).unwrap_or_default(),
                delivery_number: row.get(1)?,
                invoice_id: inv_id_opt.and_then(|s| Uuid::parse_str(&s).ok()),
                invoice_number: row.get(3)?,
                customer_id: Uuid::parse_str(&cust_id_str).unwrap_or_default(),
                customer_name: row.get(5)?,
                delivery_address: row.get(6)?,
                postal_code: row.get(7)?,
                postal_zone: row.get(8)?,
                delivery_date: NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")
                    .unwrap_or_else(|_| Utc::now().date_naive()),
                status: DeliveryStatus::from_str(&status_str),
                items,
                driver_notes: row.get(12)?,
                created_at: DateTime::parse_from_rfc3339(&created_at_str)
                    .map(|d| d.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
            })
        })?;

        let mut res = Vec::new();
        for r in rows {
            res.push(r?);
        }
        Ok(res)
    }

    pub fn next_delivery_sequence(&self, zone: &str, cust_code: &str) -> Result<u32> {
        let pattern = format!("%-{}-{}-%", zone, cust_code);
        let count: i64 = self.conn.query_row(
            "SELECT count(*) FROM delivery_orders WHERE delivery_number LIKE ?1",
            params![pattern],
            |r| r.get(0),
        )?;
        Ok((count as u32) + 1)
    }

    // --- Vendors ---
    pub fn insert_vendor(&self, v: &Vendor, track_sync: bool) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO vendors (id, name, code, email, phone, address, city, country, postal_code, currency, payment_terms_days, notes, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                v.id.to_string(),
                v.name,
                v.code,
                v.email,
                v.phone,
                v.address,
                v.city,
                v.country,
                v.postal_code,
                v.currency,
                v.payment_terms_days as i64,
                v.notes,
                v.created_at.to_rfc3339()
            ],
        )?;

        if track_sync {
            let json = serde_json::to_string(v).unwrap_or_default();
            self.record_sync_change("vendor", &v.id.to_string(), SyncAction::Upsert, &json)?;
        }
        Ok(())
    }

    pub fn get_vendors(&self) -> Result<Vec<Vendor>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, code, email, phone, address, city, country, postal_code, currency, payment_terms_days, notes, created_at FROM vendors ORDER BY name ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            let id_str: String = row.get(0)?;
            let created_at_str: String = row.get(12)?;
            Ok(Vendor {
                id: Uuid::parse_str(&id_str).unwrap_or_default(),
                name: row.get(1)?,
                code: row.get(2)?,
                email: row.get(3)?,
                phone: row.get(4)?,
                address: row.get(5)?,
                city: row.get(6)?,
                country: row.get(7)?,
                postal_code: row.get(8)?,
                currency: row.get(9)?,
                payment_terms_days: row.get::<_, i64>(10)? as u32,
                notes: row.get(11)?,
                created_at: DateTime::parse_from_rfc3339(&created_at_str)
                    .map(|d| d.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
            })
        })?;

        let mut res = Vec::new();
        for r in rows {
            res.push(r?);
        }
        Ok(res)
    }

    // --- Products & Services ---
    pub fn insert_product(&self, p: &Product, track_sync: bool) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO products (id, name, kind, unit, default_price, tax_rate, income_account_code, expense_account_code, active, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                p.id.to_string(),
                p.name,
                p.kind.as_str(),
                p.unit,
                p.default_price.to_string(),
                p.tax_rate.to_string(),
                p.income_account_code,
                p.expense_account_code,
                p.active as i64,
                p.created_at.to_rfc3339()
            ],
        )?;

        if track_sync {
            let json = serde_json::to_string(p).unwrap_or_default();
            self.record_sync_change("product", &p.id.to_string(), SyncAction::Upsert, &json)?;
        }
        Ok(())
    }

    pub fn get_products(&self) -> Result<Vec<Product>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, kind, unit, default_price, tax_rate, income_account_code, expense_account_code, active, created_at FROM products ORDER BY name ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            let id_str: String = row.get(0)?;
            let kind_str: String = row.get(2)?;
            let created_at_str: String = row.get(9)?;
            Ok(Product {
                id: Uuid::parse_str(&id_str).unwrap_or_default(),
                name: row.get(1)?,
                kind: ProductKind::from_str(&kind_str),
                unit: row.get(3)?,
                default_price: Decimal::from_str(&row.get::<_, String>(4)?)
                    .unwrap_or(Decimal::ZERO),
                tax_rate: Decimal::from_str(&row.get::<_, String>(5)?).unwrap_or(Decimal::ZERO),
                income_account_code: row.get(6)?,
                expense_account_code: row.get(7)?,
                active: row.get::<_, i64>(8)? != 0,
                created_at: DateTime::parse_from_rfc3339(&created_at_str)
                    .map(|d| d.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
            })
        })?;

        let mut res = Vec::new();
        for r in rows {
            res.push(r?);
        }
        Ok(res)
    }

    // --- Vendor Bills ---
    pub fn insert_vendor_bill(&self, b: &VendorBill, track_sync: bool) -> Result<()> {
        let items_json = serde_json::to_string(&b.items).unwrap_or_default();
        self.conn.execute(
            "INSERT OR REPLACE INTO vendor_bills (id, bill_number, vendor_id, vendor_name, issue_date, due_date, status, items_json, subtotal, tax_rate, tax_amount, total_amount, currency, notes, journal_entry_id, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
            params![
                b.id.to_string(),
                b.bill_number,
                b.vendor_id.to_string(),
                b.vendor_name,
                b.issue_date.to_string(),
                b.due_date.to_string(),
                b.status.as_str(),
                items_json,
                b.subtotal.to_string(),
                b.tax_rate.to_string(),
                b.tax_amount.to_string(),
                b.total_amount.to_string(),
                b.currency,
                b.notes,
                b.journal_entry_id.map(|id| id.to_string()),
                b.created_at.to_rfc3339()
            ],
        )?;

        if track_sync {
            let json = serde_json::to_string(b).unwrap_or_default();
            self.record_sync_change("vendor_bill", &b.id.to_string(), SyncAction::Upsert, &json)?;
        }
        Ok(())
    }

    pub fn get_vendor_bills(&self) -> Result<Vec<VendorBill>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, bill_number, vendor_id, vendor_name, issue_date, due_date, status, items_json, subtotal, tax_rate, tax_amount, total_amount, currency, notes, journal_entry_id, created_at FROM vendor_bills ORDER BY issue_date DESC, created_at DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            let id_str: String = row.get(0)?;
            let vendor_id_str: String = row.get(2)?;
            let issue_str: String = row.get(4)?;
            let due_str: String = row.get(5)?;
            let status_str: String = row.get(6)?;
            let items_json: String = row.get(7)?;
            let subtotal_str: String = row.get(8)?;
            let tax_rate_str: String = row.get(9)?;
            let tax_amount_str: String = row.get(10)?;
            let total_str: String = row.get(11)?;
            let journal_id_opt: Option<String> = row.get(14)?;
            let created_at_str: String = row.get(15)?;

            let items: Vec<InvoiceItem> = serde_json::from_str(&items_json).unwrap_or_default();

            Ok(VendorBill {
                id: Uuid::parse_str(&id_str).unwrap_or_default(),
                bill_number: row.get(1)?,
                vendor_id: Uuid::parse_str(&vendor_id_str).unwrap_or_default(),
                vendor_name: row.get(3)?,
                issue_date: NaiveDate::parse_from_str(&issue_str, "%Y-%m-%d")
                    .unwrap_or_else(|_| Utc::now().date_naive()),
                due_date: NaiveDate::parse_from_str(&due_str, "%Y-%m-%d")
                    .unwrap_or_else(|_| Utc::now().date_naive()),
                status: BillStatus::from_str(&status_str),
                items,
                subtotal: Decimal::from_str(&subtotal_str).unwrap_or(Decimal::ZERO),
                tax_rate: Decimal::from_str(&tax_rate_str).unwrap_or(Decimal::ZERO),
                tax_amount: Decimal::from_str(&tax_amount_str).unwrap_or(Decimal::ZERO),
                total_amount: Decimal::from_str(&total_str).unwrap_or(Decimal::ZERO),
                currency: row.get(12)?,
                notes: row.get(13)?,
                journal_entry_id: journal_id_opt.and_then(|s| Uuid::parse_str(&s).ok()),
                created_at: DateTime::parse_from_rfc3339(&created_at_str)
                    .map(|d| d.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
            })
        })?;

        let mut res = Vec::new();
        for r in rows {
            res.push(r?);
        }
        Ok(res)
    }

    pub fn next_bill_sequence(&self) -> Result<u32> {
        let count: i64 = self
            .conn
            .query_row("SELECT count(*) FROM vendor_bills", [], |r| r.get(0))?;
        Ok((count as u32) + 1)
    }

    // --- Journal Entries ---
    pub fn insert_journal_entry(&self, je: &JournalEntry, track_sync: bool) -> Result<()> {
        let lines_json = serde_json::to_string(&je.lines).unwrap_or_default();
        self.conn.execute(
            "INSERT OR REPLACE INTO journal_entries (id, entry_number, entry_date, memo, reference, lines_json, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                je.id.to_string(),
                je.entry_number,
                je.entry_date.to_string(),
                je.memo,
                je.reference,
                lines_json,
                je.created_at.to_rfc3339()
            ],
        )?;

        // Update account balances based on lines
        for line in &je.lines {
            if let Ok(mut accs) = self.get_accounts()
                && let Some(acc) = accs.iter_mut().find(|a| a.id == line.account_id)
            {
                match acc.account_type {
                    AccountType::Asset | AccountType::Expense => {
                        acc.balance += line.debit - line.credit;
                    }
                    AccountType::Liability | AccountType::Equity | AccountType::Revenue => {
                        acc.balance += line.credit - line.debit;
                    }
                }
                let _ = self.insert_account(acc, track_sync);
            }
        }

        if track_sync {
            let json = serde_json::to_string(je).unwrap_or_default();
            self.record_sync_change(
                "journal_entry",
                &je.id.to_string(),
                SyncAction::Upsert,
                &json,
            )?;
        }
        Ok(())
    }

    pub fn get_journal_entries(&self) -> Result<Vec<JournalEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, entry_number, entry_date, memo, reference, lines_json, created_at FROM journal_entries ORDER BY entry_date DESC, created_at DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            let id_str: String = row.get(0)?;
            let date_str: String = row.get(2)?;
            let lines_json: String = row.get(5)?;
            let created_at_str: String = row.get(6)?;

            let lines: Vec<JournalLine> = serde_json::from_str(&lines_json).unwrap_or_default();

            Ok(JournalEntry {
                id: Uuid::parse_str(&id_str).unwrap_or_default(),
                entry_number: row.get(1)?,
                entry_date: NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")
                    .unwrap_or_else(|_| Utc::now().date_naive()),
                memo: row.get(3)?,
                reference: row.get(4)?,
                lines,
                created_at: DateTime::parse_from_rfc3339(&created_at_str)
                    .map(|d| d.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
            })
        })?;

        let mut res = Vec::new();
        for r in rows {
            res.push(r?);
        }
        Ok(res)
    }

    pub fn next_journal_sequence(&self) -> Result<u32> {
        let count: i64 = self
            .conn
            .query_row("SELECT count(*) FROM journal_entries", [], |r| r.get(0))?;
        Ok((count as u32) + 1)
    }

    // --- Sync Operations ---
    pub fn get_all_sync_changes(&self) -> Result<Vec<ChangeRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT change_id, device_id, entity_type, entity_id, action, payload_json, timestamp_ms FROM sync_changes ORDER BY timestamp_ms ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            let change_id_str: String = row.get(0)?;
            let action_str: String = row.get(4)?;
            let action = match action_str.as_str() {
                "DELETE" => SyncAction::Delete,
                _ => SyncAction::Upsert,
            };

            Ok(ChangeRecord {
                change_id: Uuid::parse_str(&change_id_str).unwrap_or_default(),
                device_id: row.get(1)?,
                entity_type: row.get(2)?,
                entity_id: row.get(3)?,
                action,
                payload_json: row.get(5)?,
                timestamp_ms: row.get(6)?,
            })
        })?;

        let mut res = Vec::new();
        for r in rows {
            res.push(r?);
        }
        Ok(res)
    }

    pub fn export_sync_bundle(&self) -> Result<SyncBundle> {
        let changes = self.get_all_sync_changes()?;
        Ok(SyncBundle::new(self.device_id.clone(), changes))
    }

    pub fn apply_sync_bundle(&self, bundle: &SyncBundle) -> Result<usize> {
        let mut applied_count = 0;
        for change in &bundle.changes {
            // Check if change already applied locally
            let exists: i64 = self.conn.query_row(
                "SELECT count(*) FROM sync_changes WHERE change_id = ?1",
                params![change.change_id.to_string()],
                |r| r.get(0),
            )?;
            if exists > 0 {
                continue;
            }

            match change.entity_type.as_str() {
                "customer" => {
                    if let Ok(c) = serde_json::from_str::<Customer>(&change.payload_json) {
                        self.insert_customer(&c, false)?;
                        applied_count += 1;
                    }
                }
                "invoice" => {
                    if let Ok(inv) = serde_json::from_str::<Invoice>(&change.payload_json) {
                        self.insert_invoice(&inv, false)?;
                        applied_count += 1;
                    }
                }
                "delivery_order" => {
                    if let Ok(d) = serde_json::from_str::<DeliveryOrder>(&change.payload_json) {
                        self.insert_delivery_order(&d, false)?;
                        applied_count += 1;
                    }
                }
                "journal_entry" => {
                    if let Ok(je) = serde_json::from_str::<JournalEntry>(&change.payload_json) {
                        self.insert_journal_entry(&je, false)?;
                        applied_count += 1;
                    }
                }
                "account" => {
                    if let Ok(acc) = serde_json::from_str::<Account>(&change.payload_json) {
                        self.insert_account(&acc, false)?;
                        applied_count += 1;
                    }
                }
                "vendor" => {
                    if let Ok(v) = serde_json::from_str::<Vendor>(&change.payload_json) {
                        self.insert_vendor(&v, false)?;
                        applied_count += 1;
                    }
                }
                "product" => {
                    if let Ok(p) = serde_json::from_str::<Product>(&change.payload_json) {
                        self.insert_product(&p, false)?;
                        applied_count += 1;
                    }
                }
                "vendor_bill" => {
                    if let Ok(b) = serde_json::from_str::<VendorBill>(&change.payload_json) {
                        self.insert_vendor_bill(&b, false)?;
                        applied_count += 1;
                    }
                }
                _ => {}
            }

            // Save change record to local log so we don't reapply
            let action_str = match change.action {
                SyncAction::Upsert => "UPSERT",
                SyncAction::Delete => "DELETE",
            };
            self.conn.execute(
                "INSERT OR REPLACE INTO sync_changes (change_id, device_id, entity_type, entity_id, action, payload_json, timestamp_ms)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    change.change_id.to_string(),
                    change.device_id,
                    change.entity_type,
                    change.entity_id,
                    action_str,
                    change.payload_json,
                    change.timestamp_ms
                ],
            )?;
        }
        Ok(applied_count)
    }
}
