# FX System Design

## Money representation

- All monetary values are `rust_decimal::Decimal` (no floats).
- Every currency has a canonical decimal precision: IRR = 0 (whole Rials),
  everything else = 2.
- The database stores **Rial amounts**, never Toman. Toman is a display-only
  convenience (`core::money::format_toman`, divides by 10 at render time).

## Currencies

`core::money::Currency` supports exactly: `IRR, AED, USD, EUR, CNY, RUB`.

## Rate model

A stored rate is **"1 unit of foreign currency = N IRR"** — matching how tgju
publishes prices.

```
fx_rates (
  id TEXT PK,
  base_code  TEXT,      -- always 'IRR' in current data
  quote_code TEXT,      -- USD / EUR / AED / CNY / RUB
  rate TEXT,            -- N IRR per 1 quote unit ("1 AED = 721260 IRR")
  as_of_date TEXT,      -- YYYY-MM-DD the rate applies to
  source TEXT,          -- manual | tgju | import
  source_url TEXT, raw_value TEXT, fetched_at TEXT, created_at TEXT,
  UNIQUE(quote_code, as_of_date)
)
```

- One row per (currency, date): a fresh tgju fetch or a manual entry **replaces**
  the row for that pair (upsert), so the current-day value is unambiguous.
- Cross conversion goes through the IRR pivot:
  `X → Y = amount * rate(X) / rate(Y)`.
- `core::fx::convert` implements this; `resolve_rate(date)` picks the exact
  date or the closest prior one.

## Data source: tgju.org

`src/services/fx_provider.rs` fetches one HTTPS profile page per currency:

| Currency | URL                                                       |
|----------|-----------------------------------------------------------|
| USD      | https://www.tgju.org/profile/price_dollar_rl              |
| EUR      | https://www.tgju.org/profile/price_eur                    |
| AED      | https://www.tgju.org/profile/price_aed                    |
| CNY      | https://www.tgju.org/profile/price_cny                    |
| RUB      | https://www.tgju.org/profile/price_rub                    |

The page contains a span `data-col="info.last_trade.PDrCotVal"` whose text
(e.g. `2,638,000`) is the free-market price. Parsed with a regex tolerant of
attribute order. `robots.txt` on tgju.org does not forbid profile pages.

Policy: sequential requests, 1/second, 20s timeout, Desktop user-agent
"Surf/0.1". Failures degrade gracefully — partial results are stored, the
error list is surfaced in the UI banner and CLI stderr,
the app stays functional in IRR.

## Where rates are used

- FX Rates tab (rate table + manual entry + converter + history).
- Dashboard/ledger/banner formatting via `get_base_currency()`.
- Future (M1+): stored per-document exchange-rate snapshots, multi-currency
  journal entries and realized/unrealized FX gain-loss accounts.
