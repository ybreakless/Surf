# AGENTS.md — project guide for AI agents & contributors

## Project

Surf is a local-first Wave-parity accounting/logistics desktop app:
Rust 2024 edition, `eframe/egui 0.37` UI, `rusqlite 0.40` (bundled SQLite,
WAL, foreign_keys ON), `rust_decimal` for all money.

Workspace root for the CLI: always run cargo from the project root.

## Commands

```bash
cargo check --all-targets     # fast compile gate
cargo test                    # unit + integration tests
cargo clippy --all-targets    # must be ZERO warnings before commit
cargo fmt                     # project-style; run before commit
./target/debug/Surf           # GUI app
./target/debug/Surf rates-update [db]  # fetch today's tgju.org FX rates
```

## Tools installed for this project

- `rust-analyzer` via rustup toolchain component
  (`~/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/rust-analyzer`).
- `cargo-audit` (dependency CVE scanner) and `cargo-watch` (rebuild loop) are
  installed in `~/.local/bin`:
  `cargo audit` run against Cargo.lock during dependency changes;
  `cargo watch -x 'test --test core_tests'` for fast edit-test loops.
- Keep to stock cargo otherwise; do not add build-time plugins.

## Conventions & invariants (do not break)

1. **No floats for money.** Use `rust_decimal::Decimal` end to end.
2. **IR Rial is canonical.** DB stores Rials; Toman display = `/10` at render
   time only (`core::money::format_toman`).
3. **FX convention:** every stored rate means *1 FX unit = N IRR*; cross rates
   flow through the IRR pivot (`core::fx::convert`); one row per
   (quote_code, as_of_date) via upsert; fetched rows may be overridden by
   manual rows for the same day.
4. **Migrations:** add `("NNNN_name", sql)` to `Repository::MIGRATIONS`;
   never edit an applied migration string; `init_schema` + all migrations
   must be idempotent.
5. **egui 0.37 idioms in this codebase:** `egui::Frame::new()`,
   `CornerRadius::same(n)` (i32), `Margin::symmetric(w, h)` i32 args,
   `ComboBox::from_id_salt`, and the eframe 0.37 `App` trait method takes
   `ui`, **not** `update(...)`+CentralPanel — the shell already draws the top
   bar; screens just render into `ui`.
6. **Dark theme palette (`ui/theme.rs`)**: `Theme::PRIMARY = rgb(46,125,230)`,
   BG_CARD 28,30,36, TEXT_MUTED 150,155,170. Use these, don't hardcode colors.
7. **Language**: code, identifiers, UI strings Spanish/English mix is not
   allowed — English UI copy, except currency symbols. Persian toman labels
   show as `… T`.
8. **Tests live beside code** (`#[cfg(test)]`) plus `tests/core_tests.rs` for
   cross-module flows: invoice→journal posting, sync roundtrip, FX
   persistence, tgju HTML fixture parse. Every new core fn gets a test.
9. **Zero-warning gate**: `cargo clippy --all-targets` and `cargo check` must
   both be quiet before you finish a task.
10. **No commits unless the user explicitly asks.** Current repo status:
    everything is untracked, no baseline commit yet.

## Parallel-agents advisory

Another agent (Antigravity CLI) may edit this repo concurrently. Before
editing a file, re-read it; after finishing, re-run the full gate. Files most
likely to collide: `src/ui/*.rs`, `src/core/model.rs`, `Cargo.toml`.

## FX provider quick facts

- tgju profile pages: `price_dollar_rl`, `price_eur`, `price_aed`,
  `price_cny`, `price_rub`; price marker
  `<span class="price" data-col="info.last_trade.PDrCotVal">N</span>`.
- Sequential fetches only (1/s), 20s timeout, UA `Surf/0.1`.
- GUI path never blocks: `services::fx_provider::spawn_fx_update_worker`
  propagates `FxUpdateMsg` via channel; CLI path uses `update_rates_blocking`.

## Domain vocabulary

- Invoice, Delivery Order (DO), Customer, General Ledger, Journal Entry,
  Device Sync, Company Profile, FX Rates.
- Chart of accounts seeds live in `core/accounting.rs`
  (`default_chart_of_accounts`): 1000 Cash, 1200 AR, 2200 Sales Tax, 4000 Revenue.
- Document numbering: `INV-<zone>-<CCC>-<seq>` (see `core/numbering.rs`).
