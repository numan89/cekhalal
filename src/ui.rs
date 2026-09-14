use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::{App, Focus, SearchMode};
use crate::jakim::{ResultKind, CATEGORIES, STATES};
use crate::text_field::TextField;

const ACCENT: Color = Color::Green;

pub fn draw(f: &mut Frame, app: &mut App) {
    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Min(3),
            Constraint::Length(2),
        ])
        .split(f.area());

    draw_title(f, root[0]);
    draw_search_row(f, app, root[1]);

    // Ranger-style: results on the left, a live preview of whatever is
    // highlighted on the right — no separate "open" step. The Preview
    // pane only earns its space once there's something a selection could
    // preview; before the first search it'd just be an empty "Nothing
    // selected yet." box, so Results gets the full width instead (same
    // idea as talabulilm's single-pane placeholder before searching).
    if app.searched_once {
        let panes = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(42), Constraint::Percentage(58)])
            .split(root[2]);
        draw_results(f, app, panes[0]);
        draw_preview(f, app, panes[1]);
    } else {
        draw_results(f, app, root[2]);
    }

    draw_status_bar(f, app, root[3]);
}

fn draw_title(f: &mut Frame, area: Rect) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(0), Constraint::Length(12)])
        .split(area);

    let line = Line::from(vec![
        Span::styled(" cekhalal ", Style::default().fg(Color::Black).bg(ACCENT).add_modifier(Modifier::BOLD)),
        Span::raw("  JAKIM MyeHalal directory"),
    ]);
    f.render_widget(Paragraph::new(line), cols[0]);

    let credit = Paragraph::new("by Nu'man")
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Right);
    f.render_widget(credit, cols[1]);
}

fn focus_style(active: bool) -> Style {
    if active {
        Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    }
}

/// Renders a `TextField` with a visible cursor when focused, plain text
/// otherwise — used for both the search box and the product filter.
fn text_field_line(field: &TextField, active: bool) -> Line<'static> {
    if !active {
        return Line::raw(field.as_string());
    }
    let (before, at_cursor, after) = field.render_parts();
    let cursor_span = if at_cursor.is_empty() {
        Span::raw("\u{2588}")
    } else {
        Span::styled(at_cursor, Style::default().add_modifier(Modifier::REVERSED))
    };
    Line::from(vec![Span::raw(before), cursor_span, Span::raw(after)])
}

fn draw_search_row(f: &mut Frame, app: &App, area: Rect) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(38),
            Constraint::Percentage(17),
            Constraint::Percentage(20),
            Constraint::Percentage(25),
        ])
        .split(area);

    let search_active = app.focus == Focus::Search;
    // A non-empty box clears first on Esc; once empty, Esc moves on to
    // Results (see handle_key_search) — the title reflects whichever is
    // about to happen.
    let esc_hint = if !app.input.is_empty() { "Esc to clear" } else { "Esc to quit" };
    let label = match app.search_mode {
        SearchMode::Combined => "Search",
        SearchMode::Company => "Search company",
        SearchMode::Product => "Search product",
    };
    let search_title =
        if search_active { format!("{label} (Enter to run, {esc_hint})") } else { format!("{label} (press / )") };
    let search = Paragraph::new(text_field_line(&app.input, search_active)).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(focus_style(search_active))
            .title(search_title),
    );
    f.render_widget(search, cols[0]);

    let mode_active = app.focus == Focus::ModeFilter;
    let mode_widget = Paragraph::new(app.search_mode.label())
        .style(if app.search_mode == SearchMode::Combined { Style::default().fg(ACCENT) } else { Style::default() })
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(focus_style(mode_active))
                .title("Mode (Alt+1)"),
        );
    f.render_widget(mode_widget, cols[1]);

    let state_active = app.focus == Focus::StateFilter;
    let state_widget = Paragraph::new(STATES[app.state_idx].1).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(focus_style(state_active))
            .title("State (Alt+2)"),
    );
    f.render_widget(state_widget, cols[2]);

    let cat_active = app.focus == Focus::CategoryFilter;
    let category_locked = app.search_mode == SearchMode::Product;
    let (cat_text, cat_title) = if category_locked {
        ("Produk Makanan / Minuman", "Category (fixed for product search)")
    } else {
        (CATEGORIES[app.category_idx].1, "Category (Alt+3)")
    };
    let cat_widget = Paragraph::new(cat_text)
        .style(if category_locked { Style::default().fg(Color::DarkGray) } else { Style::default() })
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(focus_style(cat_active))
                .title(cat_title),
        );
    f.render_widget(cat_widget, cols[3]);
}

fn result_badge(kind: ResultKind) -> Span<'static> {
    match kind {
        ResultKind::Company => Span::styled(" CO ", Style::default().fg(Color::Black).bg(Color::Green).add_modifier(Modifier::BOLD)),
        ResultKind::Product => Span::styled(" PR ", Style::default().fg(Color::Black).bg(Color::Magenta).add_modifier(Modifier::BOLD)),
    }
}

fn draw_results(f: &mut Frame, app: &mut App, area: Rect) {
    let results_active = app.focus == Focus::Results;
    let show_badges = app.results_search_mode == SearchMode::Combined;

    let body: Vec<ListItem> = if app.loading {
        vec![ListItem::new("Searching MyeHalal directory...")]
    } else if let Some(err) = &app.error {
        vec![ListItem::new(Line::from(Span::styled(
            format!("Error: {err}"),
            Style::default().fg(Color::Red),
        )))]
    } else if !app.searched_once {
        vec![ListItem::new(
            "Type a name and press Enter to search\n\
             \u{2014} companies + products, matched at once and tagged CO / PR below.",
        )]
    } else if app.results.is_empty() {
        vec![ListItem::new("No results for this search.")]
    } else {
        app.results
            .iter()
            .map(|r| {
                let soonest = r.expiry_dates.first().map(String::as_str).unwrap_or("-");
                // "> " on every row, not just the selected one (via
                // highlight_symbol, which only draws on the selected
                // row) — a stable per-row marker rather than something
                // that only appears once a row happens to be selected.
                let name_line = if show_badges {
                    Line::from(vec![
                        Span::raw("> "),
                        result_badge(r.kind),
                        Span::raw(" "),
                        Span::styled(r.name.clone(), Style::default().add_modifier(Modifier::BOLD)),
                    ])
                } else {
                    Line::from(vec![Span::raw("> "), Span::styled(r.name.clone(), Style::default().add_modifier(Modifier::BOLD))])
                };
                let mut lines = vec![name_line];
                if !r.address.is_empty() {
                    let prefix = if r.kind == ResultKind::Product { "Company: " } else { "" };
                    lines.push(Line::from(Span::styled(
                        format!("  {prefix}{}", r.address),
                        Style::default().fg(Color::Cyan).add_modifier(Modifier::DIM),
                    )));
                }
                if !r.brand.is_empty() {
                    lines.push(Line::from(Span::styled(
                        format!("  Brand: {}", r.brand),
                        Style::default().fg(Color::Yellow),
                    )));
                }
                let expiry_line = if r.kind == ResultKind::Product {
                    format!("  expires {soonest}")
                } else {
                    format!(
                        "  expiry {soonest}  ({} cert entr{})",
                        r.expiry_dates.len(),
                        if r.expiry_dates.len() == 1 { "y" } else { "ies" }
                    )
                };
                lines.push(Line::from(Span::styled(expiry_line, Style::default().fg(Color::Cyan))));
                ListItem::new(lines)
            })
            .collect()
    };

    let title = if app.total_records > 0 {
        format!(
            "Results ({} total, page {}/{})",
            app.total_records,
            app.page,
            app.total_pages.max(1)
        )
    } else {
        "Results".to_string()
    };

    let list = List::new(body)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(focus_style(results_active))
                .title(title),
        )
        // REVERSED instead of a flat DarkGray background: rows here carry
        // their own CO/PR badge background (green/magenta), and a fixed
        // bg color would stomp that, leaving barely-visible black-on-
        // DarkGray text. REVERSED swaps whatever fg/bg a cell already
        // has, so the badge stays legible on the selected row too.
        .highlight_style(Style::default().add_modifier(Modifier::BOLD | Modifier::REVERSED));

    f.render_stateful_widget(list, area, &mut app.list_state);
}

fn draw_preview(f: &mut Frame, app: &App, area: Rect) {
    let preview_active = app.focus == Focus::Preview;
    let filter_text = app.product_filter.as_string();
    let title = if app.product_filter_active {
        let mut spans = vec![Span::raw("Filter products: ")];
        spans.extend(text_field_line(&app.product_filter, true).spans);
        Line::from(spans)
    } else if !filter_text.is_empty() {
        Line::raw(format!("Preview \u{2014} filtered by \"{filter_text}\" (Esc to clear)"))
    } else {
        Line::raw("Preview (Enter from Results to focus, / filters products)")
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(focus_style(preview_active))
        .title(title);

    if app.selected_result().is_none() {
        f.render_widget(Paragraph::new("Nothing selected yet.").block(block), area);
        return;
    }
    if app.preview_loading {
        f.render_widget(Paragraph::new("Loading...").block(block), area);
        return;
    }
    let Some(detail) = &app.preview else {
        f.render_widget(Paragraph::new("No details available.").block(block), area);
        return;
    };

    let mut lines: Vec<Line> = Vec::new();
    let field = |label: &str, value: &str| -> Line<'static> {
        Line::from(vec![
            Span::styled(format!("{label:<10}"), Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)),
            Span::raw(value.to_string()),
        ])
    };

    lines.push(field("Name", &detail.name));
    lines.push(field("Address", &detail.address));
    lines.push(field("State", &detail.state));
    lines.push(field("Phone", &detail.phone));
    lines.push(field("Email", &detail.email));
    lines.push(field("Website", &detail.website));
    if !detail.reference_no.is_empty() {
        lines.push(field("Reference", &detail.reference_no.join(", ")));
    }

    let filter = filter_text.to_lowercase();
    let matches: Vec<_> = detail
        .products
        .iter()
        .filter(|p| {
            filter.is_empty()
                || p.name.to_lowercase().contains(&filter)
                || p.brand.to_lowercase().contains(&filter)
        })
        .collect();

    lines.push(Line::raw(""));
    let heading = if filter.is_empty() {
        format!("Products ({})", detail.products.len())
    } else {
        format!("Products ({}/{} match \"{filter_text}\")", matches.len(), detail.products.len())
    };
    lines.push(Line::from(Span::styled(
        heading,
        Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
    )));

    if detail.products.is_empty() {
        lines.push(Line::raw("  (none listed)"));
    } else if matches.is_empty() {
        lines.push(Line::raw("  (no products match this filter)"));
    } else {
        for (i, p) in matches.iter().enumerate() {
            let brand = if p.brand.is_empty() { String::new() } else { format!(" [{}]", p.brand) };
            lines.push(Line::from(vec![
                Span::styled(format!("  {:>3}. ", i + 1), Style::default().fg(Color::Cyan).add_modifier(Modifier::DIM)),
                Span::raw(p.name.clone()),
                Span::styled(brand, Style::default().fg(Color::Cyan)),
            ]));
            if !p.expiry.is_empty() {
                lines.push(Line::from(Span::styled(
                    format!("        expires {}", p.expiry),
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::DIM),
                )));
            }
        }
    }

    let text = Text::from(lines);
    let p = Paragraph::new(text)
        .wrap(Wrap { trim: false })
        .scroll((app.preview_scroll, 0))
        .block(block);
    f.render_widget(p, area);
}

/// The area is 2 rows tall (matching talabulilm's own status bar height)
/// even though this is one line of text — a single-row `Paragraph`
/// doesn't wrap, so anything past the terminal's width would otherwise
/// be silently clipped; the most important keys (Esc/Enter/Tab/quit) go
/// first so those survive on a narrow terminal regardless.
fn draw_status_bar(f: &mut Frame, app: &App, area: Rect) {
    let help = match app.focus {
        // The must-have core (type to search / Tab / Esc / Alt+←/→
        // word / Alt+Backspace / Ctrl+C), worded identically to
        // talabulilm's own Search hint. Enter is already covered by the
        // search box's own title.
        // "Tab → Results" only once there's actually a search to switch
        // to — showing it before that just points at an empty pane.
        Focus::Search if app.searched_once => {
            "type to search  Tab \u{2192} Results  Esc clear/quit  Alt+\u{2190}/\u{2192} word  Alt+Backspace del word"
        }
        Focus::Search => "type to search  Esc clear/quit  Alt+\u{2190}/\u{2192} word  Alt+Backspace del word",
        Focus::ModeFilter | Focus::StateFilter | Focus::CategoryFilter => "\u{2190}/\u{2192} change  Enter search  Tab/Esc back",
        // n/p paging and Enter → Preview only make sense once there's
        // actually something to page through or preview.
        Focus::Results if app.searched_once => "\u{2191}/\u{2193} move  Enter \u{2192} Preview  n/p page  Tab/Esc back  / search",
        Focus::Results => "\u{2191}/\u{2193} move  Tab/Esc back  / search",
        Focus::Preview if app.product_filter_active => "type to filter products  Enter apply  Esc cancel",
        Focus::Preview => "\u{2191}/\u{2193} scroll  / filter products  Esc/h \u{2192} Results  Tab \u{2192} Search",
    };
    f.render_widget(Paragraph::new(help).style(Style::default().fg(Color::DarkGray)), area);
}
