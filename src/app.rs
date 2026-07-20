use std::collections::HashMap;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::widgets::ListState;

use crate::jakim::{CompanyDetail, SearchPage, SearchResult, CATEGORIES, STATES};
use crate::text_field::TextField;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Search,
    ModeFilter,
    StateFilter,
    CategoryFilter,
    Results,
    /// Ranger's right column: reading/scrolling the live preview of the
    /// highlighted company, or typing an incremental filter over its
    /// product list.
    Preview,
}

/// Which of the portal's search backends to query. `Combined` (the
/// default) runs both concurrently and merges them, badge-tagged, so a
/// single search covers "is this a certified company?" and "is this a
/// certified product?" at once. `Company`/`Product` are still available
/// for when you specifically want one or the other (e.g. to browse a
/// non-food category, which product search can't do).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchMode {
    Combined,
    Company,
    Product,
}

impl SearchMode {
    pub fn label(self) -> &'static str {
        match self {
            SearchMode::Combined => "Combined",
            SearchMode::Company => "Company",
            SearchMode::Product => "Product",
        }
    }

    fn next(self) -> Self {
        match self {
            SearchMode::Combined => SearchMode::Company,
            SearchMode::Company => SearchMode::Product,
            SearchMode::Product => SearchMode::Combined,
        }
    }

    fn prev(self) -> Self {
        match self {
            SearchMode::Combined => SearchMode::Product,
            SearchMode::Product => SearchMode::Company,
            SearchMode::Company => SearchMode::Combined,
        }
    }
}

/// What the caller (main.rs) should do after a key press. Keeps App free of
/// any knowledge of tokio/reqwest — it just describes intent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    None,
    Quit,
    RunSearch,
}

pub enum AppEvent {
    /// Tagged with the generation it was requested under (see
    /// `App::search_generation`), so a slow response for a search the user
    /// has since refined can be dropped instead of clobbering newer results.
    SearchResult(u64, anyhow::Result<SearchPage>),
    /// Tagged with the comp_code it was requested for, so a slow response
    /// for a company the user has since scrolled past can be dropped
    /// instead of clobbering whatever is now selected.
    DetailResult(String, anyhow::Result<CompanyDetail>),
}

pub struct App {
    pub input: TextField,
    pub search_mode: SearchMode,
    pub state_idx: usize,
    pub category_idx: usize,
    pub focus: Focus,

    pub results: Vec<SearchResult>,
    pub list_state: ListState,
    pub page: u32,
    pub total_pages: u32,
    pub total_records: u32,
    pub loading: bool,
    pub error: Option<String>,
    /// Bumped every time a search is kicked off; main.rs tags the request
    /// with the value at spawn time so a response that arrives after a
    /// newer search has already started can be told apart and discarded.
    search_generation: u64,

    /// Live preview of the highlighted company (ranger-style: updates as
    /// the selection moves, no explicit "open" needed).
    pub preview: Option<CompanyDetail>,
    pub preview_loading: bool,
    pub preview_comp_code: Option<String>,
    pub preview_scroll: u16,
    pub preview_cache: HashMap<String, CompanyDetail>,
    /// Set when the selection needs a preview main.rs hasn't fetched yet;
    /// main.rs drains this once per tick and clears it.
    pub pending_preview: Option<(String, String, String)>,

    /// Incremental filter over the *currently previewed* company's
    /// product list (client-side only — the portal has no product search).
    pub product_filter: TextField,
    pub product_filter_active: bool,

    pub should_quit: bool,
    pub searched_once: bool,
}

impl App {
    pub fn new() -> Self {
        Self {
            input: TextField::new(),
            search_mode: SearchMode::Combined,
            state_idx: 0,
            category_idx: 0,
            focus: Focus::Search,
            results: Vec::new(),
            list_state: ListState::default(),
            page: 1,
            total_pages: 0,
            total_records: 0,
            loading: false,
            error: None,
            search_generation: 0,
            preview: None,
            preview_loading: false,
            preview_comp_code: None,
            preview_scroll: 0,
            preview_cache: HashMap::new(),
            pending_preview: None,
            product_filter: TextField::new(),
            product_filter_active: false,
            should_quit: false,
            searched_once: false,
        }
    }

    pub fn state_code(&self) -> &'static str {
        STATES[self.state_idx].0
    }

    pub fn category_code(&self) -> &'static str {
        CATEGORIES[self.category_idx].0
    }

    pub fn search_generation(&self) -> u64 {
        self.search_generation
    }

    pub fn selected_result(&self) -> Option<&SearchResult> {
        self.list_state.selected().and_then(|i| self.results.get(i))
    }

    pub fn apply_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::SearchResult(generation, result) => {
                if generation != self.search_generation {
                    return; // stale: a newer search has already started
                }
                self.loading = false;
                match result {
                    Ok(page) => {
                        self.error = None;
                        self.page = page.page.max(1);
                        self.total_pages = page.total_pages;
                        self.total_records = page.total_records;
                        self.results = page.results;
                        self.searched_once = true;
                        if self.results.is_empty() {
                            self.list_state.select(None);
                            self.clear_preview();
                        } else {
                            self.list_state.select(Some(0));
                            self.sync_preview_for_selection();
                        }
                    }
                    Err(e) => self.error = Some(format!("{e:#}")),
                }
            }
            AppEvent::DetailResult(comp_code, Ok(detail)) => {
                self.preview_cache.insert(comp_code.clone(), detail.clone());
                if self.preview_comp_code.as_deref() == Some(comp_code.as_str()) {
                    self.preview = Some(detail);
                    self.preview_loading = false;
                }
            }
            AppEvent::DetailResult(comp_code, Err(e)) => {
                if self.preview_comp_code.as_deref() == Some(comp_code.as_str()) {
                    self.preview_loading = false;
                    self.error = Some(format!("{e:#}"));
                }
            }
        }
    }

    fn clear_preview(&mut self) {
        self.preview = None;
        self.preview_loading = false;
        self.preview_comp_code = None;
        self.product_filter.clear();
        self.product_filter_active = false;
    }

    /// Ensures `preview` reflects whatever is currently highlighted:
    /// reuse the cache if we've seen this company before, otherwise queue
    /// a fetch for main.rs to pick up.
    fn sync_preview_for_selection(&mut self) {
        let Some(r) = self.selected_result() else {
            self.clear_preview();
            return;
        };
        let comp_code = r.comp_code.clone();
        let type_ = r.type_.clone();
        let ty = r.ty.clone();
        if self.preview_comp_code.as_deref() == Some(comp_code.as_str())
            && (self.preview.is_some() || self.preview_loading)
        {
            return;
        }

        self.preview_scroll = 0;
        self.product_filter.clear();
        self.product_filter_active = false;
        self.preview_comp_code = Some(comp_code.clone());

        if let Some(cached) = self.preview_cache.get(&comp_code) {
            self.preview = Some(cached.clone());
            self.preview_loading = false;
        } else {
            self.preview = None;
            self.preview_loading = true;
            self.pending_preview = Some((comp_code, type_, ty));
        }
    }

    /// Marks a new search as starting: bumps the generation (so any
    /// in-flight response becomes stale) and returns the action for
    /// main.rs to actually spawn it.
    fn trigger_search(&mut self) -> Action {
        self.search_generation += 1;
        self.loading = true;
        self.error = None;
        Action::RunSearch
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Action {
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return Action::Quit;
        }

        // Mode/State/Category are one Alt+digit away from anywhere, so the
        // main Tab cycle can stay a simple three-stop loop (Search →
        // Results → Preview) without also having to pass through these.
        if key.modifiers.contains(KeyModifiers::ALT) {
            match key.code {
                KeyCode::Char('1') => {
                    self.focus = Focus::ModeFilter;
                    return Action::None;
                }
                KeyCode::Char('2') => {
                    self.focus = Focus::StateFilter;
                    return Action::None;
                }
                KeyCode::Char('3') => {
                    self.focus = Focus::CategoryFilter;
                    return Action::None;
                }
                _ => {}
            }
        }

        if key.code == KeyCode::Char('/') && self.focus != Focus::Search && self.focus != Focus::Preview {
            self.focus = Focus::Search;
            return Action::None;
        }

        match self.focus {
            Focus::Search => self.handle_key_search(key),
            Focus::ModeFilter => self.handle_key_mode_filter(key),
            Focus::StateFilter => self.handle_key_state_filter(key),
            Focus::CategoryFilter => self.handle_key_category_filter(key),
            Focus::Results => self.handle_key_results(key),
            Focus::Preview => self.handle_key_preview(key),
        }
    }

    fn handle_key_search(&mut self, key: KeyEvent) -> Action {
        match key.code {
            KeyCode::Esc => {
                self.focus = Focus::Results;
                return Action::None;
            }
            KeyCode::Tab => {
                self.focus = Focus::Results;
                return Action::None;
            }
            KeyCode::BackTab => {
                self.focus = Focus::Preview;
                return Action::None;
            }
            KeyCode::Enter => {
                self.page = 1;
                return self.trigger_search();
            }
            _ => {}
        }
        self.input.handle_key(key);
        Action::None
    }

    fn handle_key_mode_filter(&mut self, key: KeyEvent) -> Action {
        match key.code {
            KeyCode::Esc | KeyCode::Tab | KeyCode::BackTab => self.focus = Focus::Results,
            KeyCode::Left | KeyCode::Char('h') => self.search_mode = self.search_mode.prev(),
            KeyCode::Right | KeyCode::Char('l') => self.search_mode = self.search_mode.next(),
            KeyCode::Enter => {
                self.page = 1;
                self.focus = Focus::Results;
                return self.trigger_search();
            }
            _ => {}
        }
        Action::None
    }

    fn handle_key_state_filter(&mut self, key: KeyEvent) -> Action {
        match key.code {
            KeyCode::Esc | KeyCode::Tab | KeyCode::BackTab => self.focus = Focus::Results,
            KeyCode::Left | KeyCode::Char('h') => {
                self.state_idx = self.state_idx.checked_sub(1).unwrap_or(STATES.len() - 1);
            }
            KeyCode::Right | KeyCode::Char('l') => {
                self.state_idx = (self.state_idx + 1) % STATES.len();
            }
            KeyCode::Enter => {
                self.page = 1;
                self.focus = Focus::Results;
                return self.trigger_search();
            }
            _ => {}
        }
        Action::None
    }

    fn handle_key_category_filter(&mut self, key: KeyEvent) -> Action {
        let category_locked = self.search_mode == SearchMode::Product;
        match key.code {
            KeyCode::Esc | KeyCode::Tab | KeyCode::BackTab => self.focus = Focus::Results,
            KeyCode::Left | KeyCode::Char('h') if !category_locked => {
                self.category_idx = self.category_idx.checked_sub(1).unwrap_or(CATEGORIES.len() - 1);
            }
            KeyCode::Right | KeyCode::Char('l') if !category_locked => {
                self.category_idx = (self.category_idx + 1) % CATEGORIES.len();
            }
            KeyCode::Enter => {
                self.page = 1;
                self.focus = Focus::Results;
                return self.trigger_search();
            }
            _ => {}
        }
        Action::None
    }

    fn handle_key_results(&mut self, key: KeyEvent) -> Action {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => return Action::Quit,
            KeyCode::Tab => self.focus = Focus::Preview,
            KeyCode::BackTab => self.focus = Focus::Search,
            KeyCode::Down | KeyCode::Char('j') => self.move_selection(1),
            KeyCode::Up | KeyCode::Char('k') => self.move_selection(-1),
            KeyCode::Right | KeyCode::Char('l') | KeyCode::Enter => {
                if self.selected_result().is_some() {
                    self.focus = Focus::Preview;
                }
            }
            KeyCode::PageDown | KeyCode::Char('n') => {
                if self.page < self.total_pages.max(1) {
                    self.page += 1;
                    return self.trigger_search();
                }
            }
            KeyCode::PageUp | KeyCode::Char('p') => {
                if self.page > 1 {
                    self.page -= 1;
                    return self.trigger_search();
                }
            }
            _ => {}
        }
        Action::None
    }

    fn handle_key_preview(&mut self, key: KeyEvent) -> Action {
        if self.product_filter_active {
            match key.code {
                KeyCode::Esc => {
                    self.product_filter_active = false;
                    self.product_filter.clear();
                    self.preview_scroll = 0;
                }
                KeyCode::Enter => {
                    self.product_filter_active = false;
                }
                _ => {
                    if self.product_filter.handle_key(key) {
                        self.preview_scroll = 0;
                    }
                }
            }
            return Action::None;
        }

        match key.code {
            KeyCode::Char('q') => return Action::Quit,
            KeyCode::Tab => self.focus = Focus::Search,
            KeyCode::BackTab => self.focus = Focus::Results,
            KeyCode::Esc | KeyCode::Left | KeyCode::Char('h') => {
                if !self.product_filter.is_empty() {
                    self.product_filter.clear();
                    self.preview_scroll = 0;
                } else {
                    self.focus = Focus::Results;
                }
            }
            KeyCode::Char('/') => self.product_filter_active = true,
            KeyCode::Down | KeyCode::Char('j') => self.preview_scroll = self.preview_scroll.saturating_add(1),
            KeyCode::Up | KeyCode::Char('k') => self.preview_scroll = self.preview_scroll.saturating_sub(1),
            KeyCode::PageDown => self.preview_scroll = self.preview_scroll.saturating_add(10),
            KeyCode::PageUp => self.preview_scroll = self.preview_scroll.saturating_sub(10),
            _ => {}
        }
        Action::None
    }

    fn move_selection(&mut self, delta: i32) {
        if self.results.is_empty() {
            return;
        }
        let len = self.results.len() as i32;
        let current = self.list_state.selected().unwrap_or(0) as i32;
        let next = (current + delta).rem_euclid(len);
        self.list_state.select(Some(next as usize));
        self.sync_preview_for_selection();
    }
}
