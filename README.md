# cekhalal

A terminal UI for searching Malaysia's official JAKIM halal directory
([MyeHalal](https://myehalal.halal.gov.my/)) — check certified companies,
premises, and products without leaving the terminal.

There's no public API, so `cekhalal` talks to the same search endpoint the
portal's own web page uses (server-rendered HTML) and parses the results.
It only reads from `myehalal.halal.gov.my`; it doesn't submit anything.

## Build & run

```sh
cargo build --release
./target/release/cekhalal
```

or just `cargo run` during development.

## Usage

- **Tab** / **Shift+Tab** — cycle focus: Search → State → Category → Results
- **/** — jump straight to the search box from anywhere
- In the **search box**: type a company name, then **Enter** to search,
  **Esc** to leave the field without losing your query
- In the **State** / **Category** filters: **←/→** (or h/l) to change,
  **Enter** to re-run the search with the new filter
- In the **results list**: **↑/↓** (or j/k) to move, **Enter** to open full
  certificate details (address, phone, officers, product list), **n/p** (or
  PageDown/PageUp) for next/previous page, **q**/**Esc** to quit
- In the **detail view**: **↑/↓** to scroll, **Esc**/**q**/**Enter** to close
- **Ctrl+C** quits from anywhere

Note: the search matches on **company name**, the same as the official
site — it won't find a product by brand alone (e.g. searching "Milo" finds
nothing, but "Nestle" surfaces it as a listed product under Nestle's
certificate).

## How it works

- `src/jakim.rs` — HTTP client + HTML scraper for the MyeHalal directory
  search and per-company detail views (unit-tested against captured real
  responses in `tests/fixtures/`)
- `src/app.rs` — application state and keybinding logic
- `src/ui.rs` — [ratatui](https://ratatui.rs) rendering
- `src/main.rs` — terminal setup and the async event loop (tokio + crossterm)

Uses `native-tls` (the system's OpenSSL) rather than `rustls`, because the
JAKIM server's TLS configuration fails a `rustls`-only handshake.
