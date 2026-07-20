use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::{App, Focus};
use crate::jakim::{CATEGORIES, STATES};

const ACCENT: Color = Color::Green;

pub fn draw(f: &mut Frame, app: &mut App) {
    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Min(3),
            Constraint::Length(1),
        ])
        .split(f.area());

    draw_title(f, root[0]);
    draw_search_row(f, app, root[1]);

    // Ranger-style: results on the left, a live preview of whatever is
    // highlighted on the right — no separate "open" step.
    let panes = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(42), Constraint::Percentage(58)])
        .split(root[2]);
    draw_results(f, app, panes[0]);
    draw_preview(f, app, panes[1]);

    draw_status_bar(f, app, root[3]);
}

fn draw_title(f: &mut Frame, area: Rect) {
    let line = Line::from(vec![
        Span::styled(" cekhalal ", Style::default().fg(Color::Black).bg(ACCENT).add_modifier(Modifier::BOLD)),
        Span::raw("  JAKIM MyeHalal directory, from your terminal"),
    ]);
    f.render_widget(Paragraph::new(line), area);
}

fn focus_style(active: bool) -> Style {
    if active {
        Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    }
}

fn draw_search_row(f: &mut Frame, app: &App, area: Rect) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
        ])
        .split(area);

    let search_active = app.focus == Focus::Search;
    let search_title = if search_active { "Search (Enter to run, Esc to leave)" } else { "Search (press / )" };
    let cursor = if search_active { "█" } else { "" };
    let search_text = format!("{}{}", app.input, cursor);
    let search = Paragraph::new(search_text).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(focus_style(search_active))
            .title(search_title),
    );
    f.render_widget(search, cols[0]);

    let state_active = app.focus == Focus::StateFilter;
    let state_widget = Paragraph::new(STATES[app.state_idx].1).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(focus_style(state_active))
            .title("State (\u{2190}/\u{2192})"),
    );
    f.render_widget(state_widget, cols[1]);

    let cat_active = app.focus == Focus::CategoryFilter;
    let cat_widget = Paragraph::new(CATEGORIES[app.category_idx].1).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(focus_style(cat_active))
            .title("Category (\u{2190}/\u{2192})"),
    );
    f.render_widget(cat_widget, cols[2]);
}

fn draw_results(f: &mut Frame, app: &mut App, area: Rect) {
    let results_active = app.focus == Focus::Results;

    let body: Vec<ListItem> = if app.loading {
        vec![ListItem::new("Searching MyeHalal directory...")]
    } else if let Some(err) = &app.error {
        vec![ListItem::new(Line::from(Span::styled(
            format!("Error: {err}"),
            Style::default().fg(Color::Red),
        )))]
    } else if !app.searched_once {
        vec![ListItem::new(
            "Type a company name and press Enter to search.\n\
             Tab cycles Search / State / Category / Results.\n\
             The preview pane on the right shows products live as you move.",
        )]
    } else if app.results.is_empty() {
        vec![ListItem::new("No results for this search.")]
    } else {
        app.results
            .iter()
            .map(|r| {
                let soonest = r.expiry_dates.first().map(String::as_str).unwrap_or("-");
                let mut lines = vec![Line::from(Span::styled(
                    r.name.clone(),
                    Style::default().add_modifier(Modifier::BOLD),
                ))];
                if !r.address.is_empty() {
                    lines.push(Line::from(Span::styled(
                        format!("  {}", r.address),
                        Style::default().fg(Color::DarkGray),
                    )));
                }
                if !r.brand.is_empty() {
                    lines.push(Line::from(Span::styled(
                        format!("  Brand: {}", r.brand),
                        Style::default().fg(Color::Yellow),
                    )));
                }
                lines.push(Line::from(Span::styled(
                    format!(
                        "  expiry {soonest}  ({} cert entr{})",
                        r.expiry_dates.len(),
                        if r.expiry_dates.len() == 1 { "y" } else { "ies" }
                    ),
                    Style::default().fg(Color::Cyan),
                )));
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
        .highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD))
        .highlight_symbol("> ");

    f.render_stateful_widget(list, area, &mut app.list_state);
}

fn draw_preview(f: &mut Frame, app: &App, area: Rect) {
    let preview_active = app.focus == Focus::Preview;
    let title = if app.product_filter_active {
        format!("Filter products: {}\u{2588}", app.product_filter)
    } else if !app.product_filter.is_empty() {
        format!("Preview \u{2014} filtered by \"{}\" (Esc to clear)", app.product_filter)
    } else {
        "Preview  (l/Enter to focus, / filters products, h/Esc back)".to_string()
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

    let filter = app.product_filter.to_lowercase();
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
        format!("Products ({}/{} match \"{}\")", matches.len(), detail.products.len(), app.product_filter)
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
                Span::styled(format!("  {:>3}. ", i + 1), Style::default().fg(Color::DarkGray)),
                Span::raw(p.name.clone()),
                Span::styled(brand, Style::default().fg(Color::Cyan)),
            ]));
            if !p.expiry.is_empty() {
                lines.push(Line::from(Span::styled(
                    format!("        expires {}", p.expiry),
                    Style::default().fg(Color::DarkGray),
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

fn draw_status_bar(f: &mut Frame, app: &App, area: Rect) {
    let help = match app.focus {
        Focus::Search => "type to search  Enter search  Esc leave field  Tab next  Ctrl+C quit",
        Focus::StateFilter | Focus::CategoryFilter => "\u{2190}/\u{2192} change  Enter search  Tab next  Esc results  Ctrl+C quit",
        Focus::Results => "\u{2191}/\u{2193} move  l/Enter preview  n/p page  / search  q quit",
        Focus::Preview if app.product_filter_active => "type to filter products  Enter apply  Esc cancel",
        Focus::Preview => "\u{2191}/\u{2193} scroll  / filter products  h/Esc back  q quit",
    };
    f.render_widget(
        Paragraph::new(help).style(Style::default().fg(Color::DarkGray)),
        area,
    );
}
