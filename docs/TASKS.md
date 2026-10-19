# Surf Build Roadmap (Wave-parity)

Status key: [x] done · [~] partially wired · [ ] pending

## M0 — Foundations & Currency Core (current sprint)

- [x] Rust + egui + SQLite skeleton: dark theme app shell, top-tab IA
- [x] Double-entry chart of accounts + auto-posting on invoice/payment
- [x] Customers with auto codes & postal-zone routing; per-customer selection
- [x] Delivery Orders generated from invoices, status flow
- [x] `core/money.rs`: Currency enum (IRR/AED/USD/EUR/CNY/RUB), grouping, precision, Toman display
- [x] `core/fx.rs`: FxRate model, IRR-pivot `convert`, `resolve_rate`, `format_rate`, `parse_rate_str`
- [x] DB migrations (`0001_fx_rates`) + `upsert_fx_rate` / `get_all_fx_rates` / `get_latest_fx_rates`
- [x] Base-currency + company-profile persistence in `app_settings`
- [x] `services/fx_provider.rs`: tgju.org fetch/parse (fixture-tested), background worker + `rates-update` CLI
- [x] FX Rates tab: status banner, rate table, manual entry override, converter, history
- [x] Currency-aware formatting across dashboard/ledger/invoices/customers
- [x] Company Profile tab with Dubai-first defaults + stamp path
- [x] Email/communications center tab (drafting/printing view)
- [x] Tests: money/fx units, tgju fixture parse, rate persistence + cross conversion, profile persistence
- [x] Reports center (premium parity): trial balance, income statement, balance sheet, aged receivables (currency-safe buckets), overview metrics
- [x] Recurring billing: templates (weekly/monthly/quarterly/yearly, month-end clamping), catch-up generation with auto journal posting + numbering
- [x] Automations engine: startup auto-run of FX top-up, dated DB snapshot (VACUUM INTO, 30-day retention), recurring billing, overdue scan, log UI + Run All Now
- [ ] Docs polish (this file, AGENTS.md)

## M1 — Document FX rates + recurring billing

- [ ] Store per-invoice exchange-rate snapshot at issue date; display both equivalent amounts
- [ ] Realized FX gain/loss account wiring on payments (1101/1110-style accounts)
- [x] Recurring invoices (template approval, auto-bill dates) — catch-up cycles capped at 12/run
- [x] Estimates→invoice conversion; duplicate invoice (not yet — see M2)
- [ ] Uninvoice transactions page, manual journal add UI (attach files)

## M2 — Payments, bills & payroll (self-serve instead of Wave Payments)

- [ ] Sales payments list + accept-bank-transfer records
- [ ] Unpaid bills, purchase orders, vendor credits
- [ ] Customer statements (aging summary 30/60/90), balance sheet, cash flow, transaction report exports
- [ ] Account balances/adjustments UI, multi-currency swollen balances view

## M3 — Logistics depth for import/export

- [ ] Shipment packages: weight/dims/customs fields, per-DO checklist
- [ ] Air/sea/route manifest selection, packing list doc, Commercial Invoice doc,
    Certificate of Origin doc, or Free Zone certificate attachments
- [ ] Border/customs fee capture + duty accounting posts

## M4 — Inventory & integrations

- [ ] Item catalog with COGS/average cost, stock movements ledger
- [ ] Order pipeline integration: shop/order sync, webhooks, Open API endpoints
- [ ] Email sending (SMTP config + templates), attachments, statement scheduling

## M5 — Hardening & sync

- [ ] Atomic posting rewrite (`BEGIN IMMEDIATE` around JE + balances)
- [ ] Sequence tables instead of `count(*)`; CHECK constraints; indexes audit
- [ ] Device sync conflict UI, delta-only bundles, signed export
- [ ] Release build (LTO on), iconography, printable templates, accessibility pass
- [ ] Update paths + signed build instructions for Windows/macOS/Linux
