use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::widgets::ListState;

use crate::jakim::{CompanyDetail, SearchPage, SearchResult, CATEGORIES, STATES};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Search,
    StateFilter,
    CategoryFilter,
    Results,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Browse,
    Detail,
}

/// What the caller (main.rs) should do after a key press. Keeps App free of
/// any knowledge of tokio/reqwest — it just describes intent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    None,
    Quit,
    RunSearch,
    OpenDetail,
}

pub enum AppEvent {
    SearchResult(anyhow::Result<SearchPage>),
    DetailResult(anyhow::Result<CompanyDetail>),
}

pub struct App {
    pub input: String,
    pub state_idx: usize,
    pub category_idx: usize,
    pub focus: Focus,
    pub mode: Mode,

    pub results: Vec<SearchResult>,
    pub list_state: ListState,
    pub page: u32,
    pub total_pages: u32,
    pub total_records: u32,
    pub loading: bool,
    pub error: Option<String>,

    pub detail: Option<CompanyDetail>,
    pub detail_loading: bool,
    pub detail_scroll: u16,

    pub should_quit: bool,
    pub searched_once: bool,
}

impl App {
    pub fn new() -> Self {
        Self {
            input: String::new(),
            state_idx: 0,
            category_idx: 0,
            focus: Focus::Search,
            mode: Mode::Browse,
            results: Vec::new(),
            list_state: ListState::default(),
            page: 1,
            total_pages: 0,
            total_records: 0,
            loading: false,
            error: None,
            detail: None,
            detail_loading: false,
            detail_scroll: 0,
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

    pub fn selected_result(&self) -> Option<&SearchResult> {
        self.list_state.selected().and_then(|i| self.results.get(i))
    }

    pub fn apply_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::SearchResult(Ok(page)) => {
                self.loading = false;
                self.error = None;
                self.page = page.page.max(1);
                self.total_pages = page.total_pages;
                self.total_records = page.total_records;
                self.results = page.results;
                self.searched_once = true;
                if self.results.is_empty() {
                    self.list_state.select(None);
                } else {
                    self.list_state.select(Some(0));
                }
            }
            AppEvent::SearchResult(Err(e)) => {
                self.loading = false;
                self.error = Some(format!("{e:#}"));
            }
            AppEvent::DetailResult(Ok(detail)) => {
                self.detail_loading = false;
                self.detail_scroll = 0;
                self.detail = Some(detail);
            }
            AppEvent::DetailResult(Err(e)) => {
                self.detail_loading = false;
                self.mode = Mode::Browse;
                self.error = Some(format!("{e:#}"));
            }
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Action {
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return Action::Quit;
        }

        match self.mode {
            Mode::Detail => self.handle_key_detail(key),
            Mode::Browse => self.handle_key_browse(key),
        }
    }

    fn handle_key_detail(&mut self, key: KeyEvent) -> Action {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Enter => {
                self.mode = Mode::Browse;
                self.detail = None;
                self.detail_loading = false;
            }
            KeyCode::Down | KeyCode::Char('j') => self.detail_scroll = self.detail_scroll.saturating_add(1),
            KeyCode::Up | KeyCode::Char('k') => self.detail_scroll = self.detail_scroll.saturating_sub(1),
            KeyCode::PageDown => self.detail_scroll = self.detail_scroll.saturating_add(10),
            KeyCode::PageUp => self.detail_scroll = self.detail_scroll.saturating_sub(10),
            _ => {}
        }
        Action::None
    }

    fn handle_key_browse(&mut self, key: KeyEvent) -> Action {
        if key.code == KeyCode::Char('/') && self.focus != Focus::Search {
            self.focus = Focus::Search;
            return Action::None;
        }

        match self.focus {
            Focus::Search => return self.handle_key_search(key),
            Focus::StateFilter => return self.handle_key_state_filter(key),
            Focus::CategoryFilter => return self.handle_key_category_filter(key),
            Focus::Results => return self.handle_key_results(key),
        }
    }

    fn handle_key_search(&mut self, key: KeyEvent) -> Action {
        match key.code {
            KeyCode::Esc => self.focus = Focus::Results,
            KeyCode::Tab => self.focus = Focus::StateFilter,
            KeyCode::BackTab => self.focus = Focus::Results,
            KeyCode::Enter => {
                self.page = 1;
                return Action::RunSearch;
            }
            KeyCode::Backspace => {
                self.input.pop();
            }
            KeyCode::Char(c) => self.input.push(c),
            _ => {}
        }
        Action::None
    }

    fn handle_key_state_filter(&mut self, key: KeyEvent) -> Action {
        match key.code {
            KeyCode::Esc => self.focus = Focus::Results,
            KeyCode::Tab => self.focus = Focus::CategoryFilter,
            KeyCode::BackTab => self.focus = Focus::Search,
            KeyCode::Left | KeyCode::Char('h') => {
                self.state_idx = self.state_idx.checked_sub(1).unwrap_or(STATES.len() - 1);
            }
            KeyCode::Right | KeyCode::Char('l') => {
                self.state_idx = (self.state_idx + 1) % STATES.len();
            }
            KeyCode::Enter => {
                self.page = 1;
                return Action::RunSearch;
            }
            _ => {}
        }
        Action::None
    }

    fn handle_key_category_filter(&mut self, key: KeyEvent) -> Action {
        match key.code {
            KeyCode::Esc => self.focus = Focus::Results,
            KeyCode::Tab => self.focus = Focus::Results,
            KeyCode::BackTab => self.focus = Focus::StateFilter,
            KeyCode::Left | KeyCode::Char('h') => {
                self.category_idx = self.category_idx.checked_sub(1).unwrap_or(CATEGORIES.len() - 1);
            }
            KeyCode::Right | KeyCode::Char('l') => {
                self.category_idx = (self.category_idx + 1) % CATEGORIES.len();
            }
            KeyCode::Enter => {
                self.page = 1;
                return Action::RunSearch;
            }
            _ => {}
        }
        Action::None
    }

    fn handle_key_results(&mut self, key: KeyEvent) -> Action {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => return Action::Quit,
            KeyCode::Tab => self.focus = Focus::Search,
            KeyCode::BackTab => self.focus = Focus::CategoryFilter,
            KeyCode::Down | KeyCode::Char('j') => self.move_selection(1),
            KeyCode::Up | KeyCode::Char('k') => self.move_selection(-1),
            KeyCode::Enter => {
                if self.selected_result().is_some() {
                    return Action::OpenDetail;
                }
            }
            KeyCode::PageDown | KeyCode::Char('n') => {
                if self.page < self.total_pages.max(1) {
                    self.page += 1;
                    return Action::RunSearch;
                }
            }
            KeyCode::PageUp | KeyCode::Char('p') => {
                if self.page > 1 {
                    self.page -= 1;
                    return Action::RunSearch;
                }
            }
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
    }
}
