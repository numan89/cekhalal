# cekhalal

A terminal UI for searching Malaysia's official JAKIM halal directory
([MyeHalal](https://myehalal.halal.gov.my/)) — check certified companies
and products without leaving the terminal.

There's no public API, so `cekhalal` talks to the same two search backends
the portal's own web pages use (server-rendered HTML) and parses the
results. It only reads from `halal.gov.my` (MyeHalal now lives under the
main Halal Malaysia Portal); it doesn't submit anything.

## Install

**Arch Linux (AUR):** `yay -S cekhalal` or `paru -S cekhalal` (or
manually: `git clone https://aur.archlinux.org/cekhalal.git && cd cekhalal
&& makepkg -si`). The `PKGBUILD` is maintained in
[`packaging/aur/`](packaging/aur/) in this repo.

**From source (any platform with Rust):**

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

By default, a single search checks **both** companies and products at
once (matching the two tabs the official site itself has, "Syarikat" /
"Produk", just merged into one list). Each row is badge-tagged **CO**
(green) or **PR** (magenta) so you can tell at a glance which kind of
match it is — selecting either opens the same live preview, since a
product match is really just a pointer into its owning company's
certificate.

Mode/State/Category are all optional refinements, off to the side of the
main search box — you never have to touch them:

- **Mode** — `Combined` (default), or lock to just `Company` or `Product`
  if you specifically want one (e.g. `Company` to search a non-food
  category, which product search can't do — it only ever covers food/drink)
- **State** — narrow to one Malaysian state
- **Category** — narrow companies to one certification category (locked
  to "Produk Makanan / Minuman" automatically in `Product` mode, since
  the portal requires that)

### Keys

The main flow is just two stops — **Tab** (either direction, Shift+Tab
is the same here) switches between **Search** and **Results**. Preview
is a step *off* that loop, not part of it: it only ever gets focus when
you press **Enter** on a highlighted result, and from inside Preview,
**Tab** always takes you straight back to **Search** (not Results) while
**Enter** always takes you back to **Results** (not Search) — so Tab and
Enter never leave you guessing which of the two you'll land on.

Mode/State/Category live off to the side and are one shortcut away from
*anywhere*, regardless of which of the above you're currently in:

- **Alt+1** — Mode (Combined / Company / Product)
- **Alt+2** — State
- **Alt+3** — Category

Once jumped to one of those, **←/→** (or h/l) changes its value, **Enter**
runs the search and drops you back at Results, and **Esc** (or Tab)
also returns to Results without searching.

- **/** — jump to the search box from anywhere (except from inside
  Preview, see below)
- In the **search box** — full line editing, not just append/backspace:
  - type to insert at the cursor; **←/→** move the cursor; **Home**/**End**
    (the physical Home/End keys) jump the cursor to the start/end of the
    line — handy after using ←/→ or Alt+←/→ to edit in the middle of a
    query, so you don't have to hold an arrow key to get back to the end
  - **Alt+Backspace** or **Ctrl+Backspace** delete the previous word,
    **Alt+D** the next word (terminals disagree on which modifier they
    report for word-delete chords, so both Alt and Ctrl are accepted)
  - **Ctrl+W** clears the entire field, not just one word — a faster
    "start over" than repeated word-deletes
  - **Alt+←/→** (or **Alt+B**/**Alt+F**) move by word; **Ctrl+A**/**Ctrl+E**
    also jump to start/end
  - **Ctrl+U** kills from the cursor to the start of the line, **Ctrl+K**
    to the end
  - **Enter** to search, **Esc** to leave the field without losing your query
- In the **results list**: **↑/↓** (or j/k) to move (the preview pane
  updates as you go), **Enter** on a result to focus the preview,
  **n/p** (or PageDown/PageUp) for next/previous page, **q**/**Esc** to quit
- In the **preview pane**: **↑/↓** (or j/k) to scroll, **/** to type an
  incremental filter over the company's **product list** (matches name or
  brand, live, like ranger's in-pane search — handy when a certificate
  has 30+ products and you want the one you searched for), **Enter**
  while typing the filter keeps it applied and stops typing; **Enter**
  otherwise (or **h**/**Esc**) returns to Results
- **Ctrl+C** quits from anywhere

## How it works

- `src/jakim.rs` — HTTP client + HTML scraper for both MyeHalal search
  backends (company directory and product search) and the shared
  per-company detail view (unit-tested against captured real responses in
  `tests/fixtures/`)
- `src/text_field.rs` — the cursor-based line editor behind the search box
  and product filter (unit-tested)
- `src/app.rs` — application state and keybinding logic
- `src/ui.rs` — [ratatui](https://ratatui.rs) rendering
- `src/main.rs` — terminal setup and the async event loop (tokio + crossterm)

A few things worth knowing if you're poking at the code:

- **Combined search runs concurrently**, not sequentially —
  `JakimClient::search_combined` fires the company and product queries
  together via `tokio::join!` rather than awaiting one after the other,
  so a combined search costs about the same wall-clock time as either
  query alone (measured ~2x speedup over doing them back to back).
- **Stale responses are discarded.** Every search bumps a generation
  counter; if you refine your query again before the first request comes
  back, the late response is tagged with the old generation and silently
  dropped instead of overwriting what you're now looking at.
- **gzip transfer compression** is enabled on the HTTP client — the
  search HTML responses are 30–40KB uncompressed, so this cuts real
  transfer time, on top of the concurrency win above.
- Uses `native-tls` (the system's OpenSSL) rather than `rustls`, because
  the JAKIM server's TLS configuration fails a `rustls`-only handshake.
- The product search (`JakimClient::search_products`) requires **both**
  `category=PR` and `ty=PR` in the request — sending either alone throws
  an uncaught PHP fatal error server-side (confirmed against the live
  site). This isn't obvious from the site's own search form, which only
  ever sends `category`; the `ty` switch is set by JS when clicking the
  "Produk" results tab.

## Author

Made by [Muhammad Nu'man](https://github.com/numan89).

## License

[MIT](LICENSE)
