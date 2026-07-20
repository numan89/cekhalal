use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::{App, Focus, Mode};
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
    draw_results(f, app, root[2]);
    draw_status_bar(f, app, root[3]);

    if app.mode == Mode::Detail {
        draw_detail_popup(f, app);
    }
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
            "Type a company, product, or brand name and press Enter to search.\n\
             Tab cycles Search / State / Category / Results. Press ? for full help.",
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
                    format!("  Nearest expiry: {soonest}  ({} cert entr{})", r.expiry_dates.len(), if r.expiry_dates.len() == 1 { "y" } else { "ies" }),
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

fn draw_status_bar(f: &mut Frame, app: &App, area: Rect) {
    let help = match app.focus {
        Focus::Search => "type to search  Enter search  Esc leave field  Tab next  Ctrl+C quit",
        Focus::StateFilter | Focus::CategoryFilter => "\u{2190}/\u{2192} change  Enter search  Tab next  Esc results  Ctrl+C quit",
        Focus::Results => "\u{2191}/\u{2193} move  Enter details  n/p page  / search  q quit",
    };
    f.render_widget(
        Paragraph::new(help).style(Style::default().fg(Color::DarkGray)),
        area,
    );
}

fn draw_detail_popup(f: &mut Frame, app: &App) {
    let area = centered_rect(80, 80, f.area());
    f.render_widget(Clear, area);

    if app.detail_loading {
        let p = Paragraph::new("Loading company details...")
            .block(Block::default().borders(Borders::ALL).title("Details"));
        f.render_widget(p, area);
        return;
    }

    let Some(detail) = &app.detail else {
        let p = Paragraph::new("No details available.")
            .block(Block::default().borders(Borders::ALL).title("Details"));
        f.render_widget(p, area);
        return;
    };

    let mut lines: Vec<Line> = Vec::new();
    let field = |label: &str, value: &str| -> Line<'static> {
        Line::from(vec![
            Span::styled(format!("{label:<12}"), Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)),
            Span::raw(value.to_string()),
        ])
    };

    lines.push(field("Name", &detail.name));
    lines.push(field("Address", &detail.address));
    lines.push(field("State", &detail.state));
    lines.push(field("Phone", &detail.phone));
    lines.push(field("Fax", &detail.fax));
    lines.push(field("Email", &detail.email));
    lines.push(field("Website", &detail.website));
    if !detail.reference_no.is_empty() {
        lines.push(field("Reference", &detail.reference_no.join(", ")));
    }
    if !detail.officers.is_empty() {
        lines.push(field("Officers", &detail.officers.join(", ")));
    }

    lines.push(Line::raw(""));
    lines.push(Line::from(Span::styled(
        format!("Product / Menu List ({})", detail.products.len()),
        Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
    )));
    if detail.products.is_empty() {
        lines.push(Line::raw("  (none listed)"));
    } else {
        for (i, p) in detail.products.iter().enumerate() {
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
        .scroll((app.detail_scroll, 0))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(ACCENT))
                .title("Certificate details (\u{2191}/\u{2193} scroll, Esc/q close)")
                .title_alignment(Alignment::Left),
        );
    f.render_widget(p, area);
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1])[1]
}
