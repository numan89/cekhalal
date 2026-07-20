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

The layout is [ranger](https://github.com/ranger/ranger)-style: a results
list on the left and a **live preview** pane on the right that updates as
you move the selection — no "open" step needed, and each company's full
detail (once fetched) is cached for the session so revisiting it is
instant.

- **Tab** / **Shift+Tab** — cycle focus: Search → State → Category →
  Results (**l**/**Enter** from Results also jumps straight into Preview)
- **/** — jump to the search box from anywhere (except from inside
  Preview, see below)
- In the **search box**: type a company name, then **Enter** to search,
  **Esc** to leave the field without losing your query
- In the **State** / **Category** filters: **←/→** (or h/l) to change,
  **Enter** to re-run the search with the new filter
- In the **results list**: **↑/↓** (or j/k) to move (the preview pane
  updates as you go), **l**/**Enter** to focus the preview for
  scrolling/filtering, **n/p** (or PageDown/PageUp) for next/previous
  page, **q**/**Esc** to quit
- In the **preview pane**: **↑/↓** (or j/k) to scroll, **/** to type an
  incremental filter over that company's **product list** (matches name
  or brand, live, like ranger's in-pane search — e.g. type "milo" to jump
  straight to a MILO product buried in a 30-product certificate), **Enter**
  to keep the filter applied and stop typing, **Esc** to clear the filter
  (press again to go back to Results), **h**/**Left** also returns to
  Results
- **Ctrl+C** quits from anywhere

Note: the search box matches on **company name**, the same as the
official site — it won't find a product by brand alone (e.g. searching
"Milo" finds nothing, because the portal itself doesn't index products for
search). What it does have is the full product list for every company,
which is what the preview pane's `/` filter is for — pick a company, then
narrow down to the product you're after.

## How it works

- `src/jakim.rs` — HTTP client + HTML scraper for the MyeHalal directory
  search and per-company detail views (unit-tested against captured real
  responses in `tests/fixtures/`)
- `src/app.rs` — application state and keybinding logic
- `src/ui.rs` — [ratatui](https://ratatui.rs) rendering
- `src/main.rs` — terminal setup and the async event loop (tokio + crossterm)

Uses `native-tls` (the system's OpenSSL) rather than `rustls`, because the
JAKIM server's TLS configuration fails a `rustls`-only handshake.
