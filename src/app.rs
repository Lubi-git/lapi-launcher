use std::{
    path::PathBuf,
    process::Child,
    time::{Duration, Instant},
};

use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind};
use nucleo_matcher::{
    Matcher, Utf32Str,
    pattern::{AtomKind, CaseMatching, Normalization, Pattern},
};
use ratatui::layout::{Position, Rect};

use crate::{
    config::Config,
    desktop::{self, Application, Catalog},
    history::History,
    i18n::Translator,
    system::SystemInfo,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    Desktop,
    Recent,
    Results,
}

impl Section {
    pub fn index(self) -> usize {
        match self {
            Self::Desktop => 0,
            Self::Recent => 1,
            Self::Results => 2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LapiProgram {
    Installer,
    Manager,
}

impl LapiProgram {
    pub const fn command(self) -> &'static str {
        match self {
            Self::Installer => "lapi-installer",
            Self::Manager => "lapi-manager",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Target {
    System(usize),
    SystemDetails(usize),
    Program(LapiProgram),
    Section(Section),
    App(Section, usize),
    Search,
    ContextAction,
    ContextClose,
}

pub struct Hit {
    pub area: Rect,
    pub target: Target,
}

pub struct App {
    pub config: Config,
    pub text: Translator,
    pub apps: Vec<Application>,
    pub filtered: Vec<usize>,
    pub desktop: Vec<usize>,
    pub recent: Vec<usize>,
    pub query: String,
    pub section: Section,
    pub selected: [usize; 3],
    pub offsets: [usize; 3],
    pub columns: [usize; 3],
    pub rows: [usize; 3],
    pub system: SystemInfo,
    pub expanded: [bool; 3],
    pub system_offsets: [usize; 3],
    pub system_rows: [usize; 3],
    pub system_focus: Option<usize>,
    pub content_scroll: u16,
    pub content_height: u16,
    pub content_viewport_height: u16,
    pub hits: Vec<Hit>,
    pub help: bool,
    pub context_menu: Option<usize>,
    pub quit: bool,
    pub next_program: Option<LapiProgram>,
    pub status: String,
    pub status_error: bool,
    history: History,
    history_path: PathBuf,
    matcher: Matcher,
    last_click: Option<(Target, Instant)>,
    children: Vec<(String, Child)>,
}

impl App {
    pub fn new(config: Config, catalog: Catalog, history: History, history_path: PathBuf) -> Self {
        let text = Translator::new(config.interface.language);
        let status = if catalog.skipped > 0 {
            text.skipped_entries(catalog.skipped)
        } else {
            text.ready().into()
        };
        let mut app = Self {
            config,
            text,
            apps: catalog.apps,
            filtered: Vec::new(),
            desktop: Vec::new(),
            recent: Vec::new(),
            query: String::new(),
            section: Section::Desktop,
            selected: [0; 3],
            offsets: [0; 3],
            columns: [1; 3],
            rows: [1; 3],
            system: SystemInfo::read(text),
            expanded: [false; 3],
            system_offsets: [0; 3],
            system_rows: [0; 3],
            system_focus: None,
            content_scroll: 0,
            content_height: 0,
            content_viewport_height: 0,
            hits: Vec::new(),
            help: false,
            context_menu: None,
            quit: false,
            next_program: None,
            status,
            status_error: false,
            history,
            history_path,
            matcher: Matcher::new(nucleo_matcher::Config::DEFAULT),
            last_click: None,
            children: Vec::new(),
        };
        app.filter();
        app
    }

    pub fn indices(&self, section: Section) -> &[usize] {
        match section {
            Section::Desktop => &self.desktop,
            Section::Recent => &self.recent,
            Section::Results => &self.filtered,
        }
    }

    fn filter(&mut self) {
        self.desktop = self
            .apps
            .iter()
            .enumerate()
            .filter(|(_, app)| app.on_desktop)
            .map(|(index, _)| index)
            .collect();
        let pattern = Pattern::new(
            self.query.trim(),
            CaseMatching::Ignore,
            Normalization::Smart,
            AtomKind::Fuzzy,
        );
        let mut buffer = Vec::new();
        let mut scored = Vec::new();
        for (index, app) in self.apps.iter().enumerate() {
            if let Some(score) = pattern.score(
                Utf32Str::new(&app.search_text, &mut buffer),
                &mut self.matcher,
            ) {
                let name_score = pattern
                    .score(Utf32Str::new(&app.name, &mut buffer), &mut self.matcher)
                    .unwrap_or(0);
                scored.push((index, score + name_score * 2));
            }
        }
        scored.sort_by_key(|entry| std::cmp::Reverse(entry.1));
        self.filtered = scored.into_iter().map(|(index, _)| index).collect();
        self.recent = self
            .history
            .recent
            .iter()
            .take(self.config.launcher.recent_limit)
            .filter_map(|id| self.apps.iter().position(|app| app.id == *id))
            .collect();
        self.selected = [0; 3];
        self.offsets = [0; 3];
        if self.searching() {
            self.section = Section::Results;
        } else if self.section == Section::Results {
            self.section = Section::Desktop;
        }
        self.last_click = None;
    }

    pub fn searching(&self) -> bool {
        !self.query.is_empty()
    }

    pub fn set_content_view(&mut self, content_height: u16, viewport_height: u16) {
        self.content_height = content_height;
        self.content_viewport_height = viewport_height;
        self.content_scroll = self.content_scroll.min(self.maximum_content_scroll());
    }

    pub fn handle(&mut self, event: Event) -> Result<()> {
        match event {
            Event::Key(key) if key.kind != KeyEventKind::Release => {
                if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                    self.quit = true;
                    return Ok(());
                }
                if self.help {
                    if matches!(key.code, KeyCode::Esc | KeyCode::F(1) | KeyCode::Enter) {
                        self.help = false;
                    }
                    return Ok(());
                }
                if self.context_menu.is_some() {
                    match key.code {
                        KeyCode::Esc => self.context_menu = None,
                        KeyCode::Enter => self.toggle_desktop_pin(),
                        _ => {}
                    }
                    return Ok(());
                }
                match key.code {
                    KeyCode::Esc if self.query.is_empty() => self.quit = true,
                    KeyCode::Esc => {
                        self.query.clear();
                        self.filter();
                    }
                    KeyCode::F(1) => self.help = true,
                    KeyCode::F(section @ 2..=4) => self.toggle_system(usize::from(section - 2)),
                    KeyCode::F(5) => match desktop::discover(&self.config) {
                        Ok(catalog) => {
                            self.apps = catalog.apps;
                            self.filter();
                            self.status = self.text.applications_updated().into();
                            self.status_error = false;
                        }
                        Err(error) => self.error(format!("{error:#}")),
                    },
                    KeyCode::Tab if self.searching() => self.move_selection(1),
                    KeyCode::BackTab if self.searching() => self.move_selection(-1),
                    KeyCode::Tab | KeyCode::BackTab => {
                        self.section = if self.section == Section::Desktop {
                            Section::Recent
                        } else {
                            Section::Desktop
                        };
                    }
                    KeyCode::PageUp if !self.searching() => self.scroll_content(-6),
                    KeyCode::PageDown if !self.searching() => self.scroll_content(6),
                    KeyCode::Left => self.move_selection(-1),
                    KeyCode::Right => self.move_selection(1),
                    KeyCode::Up | KeyCode::Down
                        if key.modifiers.contains(KeyModifiers::CONTROL) =>
                    {
                        if !self.searching() {
                            self.scroll_content(if key.code == KeyCode::Up { -1 } else { 1 });
                        } else if let Some(index) = self.system_focus {
                            self.scroll_system(index, if key.code == KeyCode::Up { -1 } else { 1 });
                        }
                    }
                    KeyCode::Up => {
                        self.move_selection(-(self.columns[self.section.index()] as isize))
                    }
                    KeyCode::Down => {
                        self.move_selection(self.columns[self.section.index()] as isize)
                    }
                    KeyCode::PageUp => self.move_selection(
                        -((self.columns[self.section.index()] * self.rows[self.section.index()])
                            as isize),
                    ),
                    KeyCode::PageDown => self.move_selection(
                        (self.columns[self.section.index()] * self.rows[self.section.index()])
                            as isize,
                    ),
                    KeyCode::Home => self.selected[self.section.index()] = 0,
                    KeyCode::End => {
                        self.selected[self.section.index()] =
                            self.indices(self.section).len().saturating_sub(1)
                    }
                    KeyCode::Enter => self.launch_selected(),
                    KeyCode::Backspace => {
                        self.query.pop();
                        self.filter();
                    }
                    KeyCode::Char('l' | 'u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        self.query.clear();
                        self.filter();
                    }
                    KeyCode::Char(character)
                        if !key.modifiers.intersects(
                            KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER,
                        ) && self.query.chars().count() < 256
                            && !character.is_control() =>
                    {
                        self.query.push(character);
                        self.section = Section::Desktop;
                        self.filter();
                    }
                    _ => {}
                }
            }
            Event::Paste(text) if !self.help => {
                let remaining = 256usize.saturating_sub(self.query.chars().count());
                self.query.extend(
                    text.chars()
                        .filter(|character| !character.is_control())
                        .take(remaining),
                );
                self.section = Section::Desktop;
                self.filter();
            }
            Event::Mouse(mouse) if !self.help => {
                let target = self
                    .hits
                    .iter()
                    .rev()
                    .find(|hit| hit.area.contains(Position::new(mouse.column, mouse.row)))
                    .map(|hit| hit.target);
                match mouse.kind {
                    MouseEventKind::Down(MouseButton::Left) => {
                        if self.context_menu.is_some() {
                            match target {
                                Some(Target::ContextAction) => self.toggle_desktop_pin(),
                                _ => self.context_menu = None,
                            }
                            return Ok(());
                        }
                        match target {
                            Some(Target::System(index)) => self.toggle_system(index),
                            Some(Target::SystemDetails(index)) => self.system_focus = Some(index),
                            Some(Target::Program(program)) => self.launch_lapi_program(program),
                            Some(Target::Section(section)) => self.section = section,
                            Some(Target::Search) => {
                                self.section = if self.searching() {
                                    Section::Results
                                } else {
                                    Section::Desktop
                                }
                            }
                            Some(target @ Target::App(section, position)) => {
                                self.section = section;
                                self.selected[section.index()] = position;
                                if self.last_click.is_some_and(|(previous, time)| {
                                    previous == target
                                        && time.elapsed() < Duration::from_millis(400)
                                }) {
                                    self.launch_selected();
                                    self.last_click = None;
                                } else {
                                    self.last_click = Some((target, Instant::now()));
                                }
                            }
                            _ => {}
                        }
                        if !matches!(target, Some(Target::App(_, _))) {
                            self.last_click = None;
                        }
                    }
                    MouseEventKind::Down(MouseButton::Right) => {
                        if let Some(Target::App(section, position)) = target
                            && let Some(index) = self.indices(section).get(position).copied()
                            && self.apps[index].file.is_none()
                        {
                            self.context_menu = Some(index);
                            self.last_click = None;
                        }
                    }
                    MouseEventKind::ScrollDown | MouseEventKind::ScrollUp => {
                        let direction = if mouse.kind == MouseEventKind::ScrollDown {
                            1
                        } else {
                            -1
                        };
                        if !self.searching() {
                            self.scroll_content(direction);
                        } else if let Some(Target::System(index) | Target::SystemDetails(index)) =
                            target
                        {
                            self.scroll_system(index, direction);
                        } else {
                            if let Some(Target::App(section, _) | Target::Section(section)) = target
                            {
                                self.section = section;
                            }
                            self.move_selection(
                                direction * self.columns[self.section.index()] as isize,
                            );
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn toggle_system(&mut self, index: usize) {
        self.expanded[index] ^= true;
        self.system_offsets[index] = 0;
        if self.expanded[index] {
            self.system_focus = Some(index);
        } else if self.system_focus == Some(index) {
            self.system_focus = self.expanded.iter().rposition(|expanded| *expanded);
        }
    }

    fn scroll_system(&mut self, index: usize, change: isize) {
        if !self.expanded[index] || self.system_rows[index] == 0 {
            return;
        }
        self.system_focus = Some(index);
        let maximum = self.system.sections[index]
            .details
            .len()
            .saturating_sub(self.system_rows[index]);
        self.system_offsets[index] = self.system_offsets[index]
            .saturating_add_signed(change)
            .min(maximum);
    }

    fn maximum_content_scroll(&self) -> u16 {
        self.content_height
            .saturating_sub(self.content_viewport_height)
    }

    fn scroll_content(&mut self, change: isize) {
        self.content_scroll = if change.is_negative() {
            self.content_scroll
                .saturating_sub(change.unsigned_abs().min(u16::MAX as usize) as u16)
        } else {
            self.content_scroll
                .saturating_add((change as usize).min(u16::MAX as usize) as u16)
                .min(self.maximum_content_scroll())
        };
        self.last_click = None;
    }

    fn launch_lapi_program(&mut self, program: LapiProgram) {
        self.select_lapi_program(program, desktop::executable(program.command()).is_some());
    }

    fn select_lapi_program(&mut self, program: LapiProgram, available: bool) {
        if available {
            self.next_program = Some(program);
            self.quit = true;
        } else {
            self.error(self.text.program_missing(program.command()));
        }
    }

    fn move_selection(&mut self, change: isize) {
        let selected = &mut self.selected[self.section.index()];
        *selected = selected.saturating_add_signed(change);
        self.selected[self.section.index()] = self.selected[self.section.index()]
            .min(self.indices(self.section).len().saturating_sub(1));
        self.last_click = None;
    }

    fn launch_selected(&mut self) {
        let Some(index) = self
            .indices(self.section)
            .get(self.selected[self.section.index()])
            .copied()
        else {
            return;
        };
        match desktop::launch(&self.apps[index], &self.config) {
            Ok(child) => {
                let name = self.apps[index].name.clone();
                self.children.push((name.clone(), child));
                self.history
                    .record(&self.apps[index].id, self.config.launcher.recent_limit);
                self.status = self.text.opening(&name);
                self.status_error = false;
                if let Err(error) = self.history.save(&self.history_path, self.text) {
                    self.error(format!("{error:#}"));
                }
                let selection = self.selected;
                self.filter();
                self.selected = selection;
                self.selected[Section::Recent.index()] = 0;
                self.quit = self.config.launcher.close_on_launch && !self.status_error;
            }
            Err(_) => self.error(self.text.cannot_open(&self.apps[index].name)),
        }
    }

    fn toggle_desktop_pin(&mut self) {
        let Some(index) = self.context_menu else {
            return;
        };
        let name = self.apps[index].name.clone();
        match desktop::set_desktop_pin(&self.apps[index], &self.config)
            .and_then(|()| desktop::discover(&self.config))
        {
            Ok(catalog) => {
                self.apps = catalog.apps;
                self.filter();
                self.status = self.text.desktop_updated(&name);
                self.status_error = false;
            }
            Err(error) => self.error(format!("{error:#}")),
        }
        self.context_menu = None;
    }

    pub fn reap_children(&mut self) -> bool {
        let mut failures = Vec::new();
        self.children
            .retain_mut(|(name, child)| match child.try_wait() {
                Ok(Some(status)) => {
                    if !status.success() {
                        failures.push(self.text.child_failed(name, status));
                    }
                    false
                }
                Ok(None) => true,
                Err(error) => {
                    failures.push(format!("{name}: {error}"));
                    false
                }
            });
        if let Some(error) = failures.pop() {
            self.error(error);
            return true;
        }
        false
    }

    pub fn error(&mut self, message: String) {
        self.status = message;
        self.status_error = true;
    }
}

#[cfg(test)]
mod tests {
    use super::LapiProgram;

    #[test]
    fn missing_lapi_program_keeps_the_launcher_running() {
        let temporary = crate::test_support::TempDir::new();
        let mut app = crate::test_support::app(&[], temporary.path.join("history.toml"));
        app.select_lapi_program(LapiProgram::Installer, false);
        assert!(!app.quit);
        assert_eq!(app.next_program, None);
        assert!(app.status_error);
        assert!(app.status.contains("lapi-installer"));

        app.select_lapi_program(LapiProgram::Manager, true);
        assert!(app.quit);
        assert_eq!(app.next_program, Some(LapiProgram::Manager));
    }
}
