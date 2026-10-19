# Surf — Professional Accounting & Logistics

Surf is a local-first, offline-capable accounting and trade-logistics desktop app
built with Rust + egui + SQLite. It mirrors the workflows of Wave Accounting for
small import/export businesses while staying fully local: SQLite storage,
double-entry journal, and a currency system tuned for Iranian Rial (IRR),
Dirham (AED), Dollar (USD), Euro (EUR), Yuan/RMB (CNY) and Ruble (RUB).

![Surf](assets/Surf-icon.svg)

## Features (current)

- **Dual-entry ledger** — every invoice/payment/Delivery Order auto-post to a
  chart of accounts (Cash, AR, Revenue, Liabilities, Tax).
- **Sales invoices** — auto-numbered per customer postal-zone, with line
  items, per-line discounts, units and tax;留言 status lifecycle (Draft/Sent/Paid/Cancelled).
- **Delivery orders** — generated from invoices with status tracking (Pending → In Transit → Delivered).
- **Customers** — auto customer-codes (initials) + postal-zone routing, per-customer currency.
- **FX engine** — daily free-market IRR rates fetched from tgju.org (USD/EUR/AED/CNY/RUB),
  stored as "1 unit FX = N IRR"; cross-currency conversion via IRR pivot; manual
  rate override with date; full rate history.
- **Reports center** — trial balance (balanced check), income statement,
  balance sheet with net income, aged receivables in Wave-style buckets per
  customer + currency.
- **Recurring billing** — templates (weekly/monthly/quarterly/yearly with
  month-end clamping) auto-generate invoices with double-entry posts; missed
  cycles catch up on next run.
- **Automatic tasks** — one engine runs at app start (also "Run All Now"):
  fetches missing FX rates for today, creates a daily DB snapshot in
  `backups/`, bills due recurring templates, scans overdue invoices; every run
  is logged in an audit table.
- **Device sync (plumbing)** — change-log based sync bundle export/import between devices.
- **Company profile** — legal name, address, TRN tax id, bank/IBAN details used on exports.
- **Exports** — invoice to PNG/PDF via a canvas renderer in-core.

## Running

```bash
cargo run           # launches the desktop app (GUI)
```

First launch creates `surf_data.db` (SQLite, WAL mode) in the working directory.

### Daily FX rate update

```bash
# Manual:
./target/debug/Surf rates-update            # updates ./surf_data.db

# Or via cron at 07:30 every day, with a fixed database location:
30 7 * * * cd $HOME/projects/Surf && ./target/debug/Surf rates-update $HOME/.local/share/surf/surf_data.db
```

Alternatively open the app's **FX Rates** tab and press "Update from tgju"
(runs in a background thread so the UI stays responsive).

### Automatic daily tasks

The app runs a background automation cycle on every launch:
1. Fetch today's missing IRR rates from tgju.org (skipped when current).
2. Snapshot the database to `backups/surf_backup_YYYYMMDD.db` (one per day,
   replaces itself; files older than 30 days are pruned).
3. Generate invoices from due recurring templates (catch-up ≤ 12 cycles).
4. Scan for overdue invoices and log a summary.

All outcomes land in the **Automations** tab log (kept in the DB itself), so
cron-only users can audit it later from the app.

## Build & test

```bash
cargo check --all-targets          # fast compile of lib + tests
cargo test                          # unit + integration tests (23+)
cargo clippy --all-targets          # zero-warning gate
cargo fmt                           # rustfmt (project convention)
```

## Architecture

| Path                  | Scope                                            |
|-----------------------|--------------------------------------------------|
| `src/core/`           | domain: money, fx, model, accounting, numbering, sync |
| `src/db/`             | rusqlite Repository, migrations, persistence     |
| `src/ui/`             | egui screens (dashboard, invoices, ledger, fx….) |
| `src/services/`       | outbound integrations (tgju FX provider)        |
| `src/app.rs`          | top-level navigation + tab wiring                |
| `src/main.rs`         | GUI entrypoint + `rates-update` CLI subcommand    |

Details on the FX data model: [docs/FX.md](docs/FX.md).
Progress & roadmap: [docs/TASKS.md](docs/TASKS.md).
Contributor/agent guide: [AGENTS.md](AGENTS.md).

## Note on Wave parity

Surf is a clean-room reimplementation of accounting behaviors familiar from
Wave Accounting. No Wave source code or assets are used. Wave brand names and
its Payments/Apps features are not present; those were replaced by
local-first, manual and email-based equivalents that fit a Dubai-based
import/export business.
