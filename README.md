# cekhalal

A terminal UI for searching Malaysia's official JAKIM halal directory
([MyeHalal](https://myehalal.halal.gov.my/)) — check certified companies
and products without leaving the terminal.

There's no public API, so `cekhalal` talks to the same two search backends
the portal's own web pages use (server-rendered HTML) and parses the
results. It only reads from `myehalal.halal.gov.my`; it doesn't submit
anything.

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

There are two independent search **modes**, matching the two tabs the
official site itself has ("Syarikat" / "Produk"):

- **Company** (default) — matches company/premise names
- **Product** — matches food/drink product and brand names directly (e.g.
  searching "Milo" finds the ~170 certified products with Milo in the
  name, across every manufacturer) — this is a separate query the portal
  only exposes when the category is pinned to "Produk Makanan / Minuman",
  so switching to Product mode locks that filter automatically

Selecting a result in either mode opens the same live preview — the full
company certificate, address, and complete product list — since a product
match is really just a pointer into its owning company's certificate.

- **Tab** / **Shift+Tab** — cycle focus: Search → Mode → State → Category
  → Results (**l**/**Enter** from Results also jumps straight into
  Preview)
- **/** — jump to the search box from anywhere (except from inside
  Preview, see below)
- In the **search box**: type a name, then **Enter** to search, **Esc**
  to leave the field without losing your query
- In the **Mode** field: **←/→** (or h/l) to toggle Company/Product,
  **Enter** to re-run the search
- In the **State** / **Category** filters: **←/→** (or h/l) to change,
  **Enter** to re-run the search with the new filter (Category is locked
  in Product mode)
- In the **results list**: **↑/↓** (or j/k) to move (the preview pane
  updates as you go), **l**/**Enter** to focus the preview for
  scrolling/filtering, **n/p** (or PageDown/PageUp) for next/previous
  page, **q**/**Esc** to quit
- In the **preview pane**: **↑/↓** (or j/k) to scroll, **/** to type an
  incremental filter over the company's **product list** (matches name or
  brand, live, like ranger's in-pane search — handy when a certificate
  has 30+ products and you want the one you searched for), **Enter** to
  keep the filter applied and stop typing, **Esc** to clear the filter
  (press again to go back to Results), **h**/**Left** also returns to
  Results
- **Ctrl+C** quits from anywhere

## How it works

- `src/jakim.rs` — HTTP client + HTML scraper for both MyeHalal search
  backends (company directory and product search) and the shared
  per-company detail view (unit-tested against captured real responses in
  `tests/fixtures/`)
- `src/app.rs` — application state and keybinding logic
- `src/ui.rs` — [ratatui](https://ratatui.rs) rendering
- `src/main.rs` — terminal setup and the async event loop (tokio + crossterm)

Two things worth knowing if you're poking at the code:

- Uses `native-tls` (the system's OpenSSL) rather than `rustls`, because
  the JAKIM server's TLS configuration fails a `rustls`-only handshake.
- The product search (`JakimClient::search_products`) requires **both**
  `category=PR` and `ty=PR` in the request — sending either alone throws
  an uncaught PHP fatal error server-side (confirmed against the live
  site). This isn't obvious from the site's own search form, which only
  ever sends `category`; the `ty` switch is set by JS when clicking the
  "Produk" results tab.
