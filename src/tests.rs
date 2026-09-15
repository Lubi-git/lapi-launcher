use std::{
    fs,
    time::{Duration, Instant},
};

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend, layout::Rect};
use ratatui_image::picker::Picker;

use crate::{
    app::{LapiProgram, Section, Target},
    config::Config,
    icons::{Assets, Source},
    test_support::{self, TempDir},
    ui,
};

fn key(code: KeyCode) -> Event {
    Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
}

#[test]
fn fuzzy_search_handles_accents_and_empty_results_without_stale_selection() {
    let temporary = TempDir::new();
    let mut app = test_support::app(
        &["Firefox", "Música", "Terminal"],
        temporary.path.join("history.toml"),
    );
    app.handle(Event::Paste("musica".into())).unwrap();
    assert_eq!(app.filtered, [1]);
    app.handle(key(KeyCode::End)).unwrap();
    app.handle(Event::Paste("zzz".into())).unwrap();
    assert!(app.filtered.is_empty());
    app.handle(key(KeyCode::Enter)).unwrap();
    assert!(!app.quit);
    app.handle(key(KeyCode::Esc)).unwrap();
    assert_eq!(app.filtered.len(), 3);
    app.handle(Event::Paste("FFX".into())).unwrap();
    assert_eq!(app.filtered, [0]);
    assert_eq!(app.section, Section::Results);
}

#[test]
fn search_replaces_groups_with_alias_only_vertical_results() {
    let temporary = TempDir::new();
    let mut app = test_support::app(
        &["Editor personal", "Editor global"],
        temporary.path.join("history.toml"),
    );
    app.apps[0].id = "internal-user-name.desktop".into();
    app.apps[1].id = "internal-system-name.desktop".into();
    app.apps[1].on_desktop = false;
    app.handle(Event::Paste("Editor".into())).unwrap();
    let mut terminal = Terminal::new(TestBackend::new(100, 40)).unwrap();
    terminal
        .draw(|frame| ui::draw(frame, &mut app, &mut None))
        .unwrap();
    assert!(
        !app.hits
            .iter()
            .any(|hit| matches!(hit.target, Target::Section(_)))
    );
    let rows: Vec<_> = app
        .hits
        .iter()
        .filter(|hit| matches!(hit.target, Target::App(Section::Results, _)))
        .collect();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].area.x, rows[1].area.x);
    assert!(rows[1].area.y > rows[0].area.y);
    let contents: String = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    assert!(contents.contains("Editor personal"));
    assert!(contents.contains("Editor global"));
    assert!(!contents.contains("internal-"));
    assert!(!contents.contains(".desktop"));
    assert!(!contents.contains("Recent"));
    app.handle(key(KeyCode::Tab)).unwrap();
    assert_eq!(app.section, Section::Results);
    assert_eq!(app.selected[Section::Results.index()], 1);
    app.handle(key(KeyCode::Esc)).unwrap();
    assert_eq!(app.section, Section::Desktop);
    assert_eq!(app.desktop, [0]);
    terminal
        .draw(|frame| ui::draw(frame, &mut app, &mut None))
        .unwrap();
    let contents: String = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect();
    assert!(!contents.contains("Editor global"));
    assert!(!contents.contains(".desktop"));
    assert!(contents.contains("Recent"));
}

#[test]
fn search_scroll_keeps_the_selected_result_visible_at_small_sizes() {
    let temporary = TempDir::new();
    let names: Vec<String> = (0..40).map(|index| format!("Editor {index}")).collect();
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    let mut app = test_support::app(&names, temporary.path.join("history.toml"));
    app.handle(Event::Paste("Editor".into())).unwrap();
    for (width, height) in [(40, 20), (80, 24), (100, 40)] {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        app.handle(key(KeyCode::End)).unwrap();
        terminal
            .draw(|frame| ui::draw(frame, &mut app, &mut None))
            .unwrap();
        assert!(
            app.hits
                .iter()
                .any(|hit| hit.target == Target::App(Section::Results, 39))
        );
        for hit in &app.hits {
            assert_eq!(
                hit.area.intersection(Rect::new(0, 0, width, height)),
                hit.area
            );
        }
    }
}

#[test]
fn arrows_and_tab_navigate_independent_sections() {
    let temporary = TempDir::new();
    let mut app = test_support::app(
        &["One", "Two", "Three", "Four", "Five"],
        temporary.path.join("history.toml"),
    );
    app.columns[0] = 3;
    app.handle(key(KeyCode::Down)).unwrap();
    assert_eq!(app.selected[0], 3);
    app.handle(key(KeyCode::Right)).unwrap();
    assert_eq!(app.selected[0], 4);
    app.handle(key(KeyCode::Down)).unwrap();
    assert_eq!(app.selected[0], 4);
    app.handle(key(KeyCode::Tab)).unwrap();
    assert_eq!(app.section, Section::Recent);
    app.handle(key(KeyCode::Down)).unwrap();
    assert_eq!(app.selected, [4, 0, 0]);
}

#[test]
fn resizing_and_expanding_sections_keeps_controls_inside_the_terminal() {
    let temporary = TempDir::new();
    let names: Vec<String> = (0..80).map(|index| format!("App {index:02}")).collect();
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    let mut app = test_support::app(&names, temporary.path.join("history.toml"));
    for (width, height) in [(0, 0), (12, 8), (40, 20), (80, 24), (100, 40), (140, 55)] {
        for expanded in [false, true] {
            app.expanded = [expanded; 3];
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            terminal
                .draw(|frame| ui::draw(frame, &mut app, &mut None))
                .unwrap();
            let screen = Rect::new(0, 0, width, height);
            for hit in &app.hits {
                assert_eq!(hit.area.intersection(screen), hit.area, "{width}x{height}");
            }
            if width >= 40 && height >= 20 {
                app.handle(key(KeyCode::PageDown)).unwrap();
                terminal
                    .draw(|frame| ui::draw(frame, &mut app, &mut None))
                    .unwrap();
                assert!(app.content_scroll <= app.content_height);
            }
        }
    }
}

#[test]
fn system_expansion_pushes_groups_down_and_keeps_them_reachable() {
    let temporary = TempDir::new();
    let mut app = test_support::app(
        &["Firefox", "Editor", "Terminal", "Files", "Music", "Video"],
        temporary.path.join("history.toml"),
    );
    app.recent = app.desktop.clone();
    let mut terminal = Terminal::new(TestBackend::new(100, 40)).unwrap();
    terminal
        .draw(|frame| ui::draw(frame, &mut app, &mut None))
        .unwrap();
    let search = app
        .hits
        .iter()
        .find(|hit| hit.target == Target::Search)
        .unwrap()
        .area;
    let desktop = app
        .hits
        .iter()
        .find(|hit| hit.target == Target::Section(Section::Desktop))
        .unwrap()
        .area;
    let recent = app
        .hits
        .iter()
        .find(|hit| hit.target == Target::Section(Section::Recent))
        .unwrap()
        .area;
    assert_eq!(desktop.height, 8);
    assert_eq!(recent.height, 8);
    assert_eq!(recent.y - desktop.y, 9);
    assert_eq!(app.rows[Section::Desktop.index()], 1);
    assert_eq!(app.rows[Section::Recent.index()], 1);
    assert!(
        app.hits
            .iter()
            .any(|hit| hit.target == Target::Program(LapiProgram::Installer))
    );
    assert!(
        app.hits
            .iter()
            .any(|hit| hit.target == Target::Program(LapiProgram::Manager))
    );

    app.handle(key(KeyCode::F(4))).unwrap();
    terminal
        .draw(|frame| ui::draw(frame, &mut app, &mut None))
        .unwrap();
    let expanded_search = app
        .hits
        .iter()
        .find(|hit| hit.target == Target::Search)
        .unwrap()
        .area;
    assert!(expanded_search.y > search.y);
    assert!(app.content_height > app.content_viewport_height);
    app.handle(key(KeyCode::PageDown)).unwrap();
    app.handle(key(KeyCode::PageDown)).unwrap();
    terminal
        .draw(|frame| ui::draw(frame, &mut app, &mut None))
        .unwrap();
    let desktop = app
        .hits
        .iter()
        .find(|hit| hit.target == Target::Section(Section::Desktop))
        .unwrap()
        .area;
    let recent = app
        .hits
        .iter()
        .find(|hit| hit.target == Target::Section(Section::Recent))
        .unwrap()
        .area;
    assert_eq!(recent.y - desktop.y, 9);
    assert_eq!(app.rows[Section::Desktop.index()], 1);
    assert_eq!(app.rows[Section::Recent.index()], 1);
    for hit in &app.hits {
        assert_eq!(hit.area.intersection(Rect::new(0, 0, 100, 40)), hit.area);
    }
}

#[test]
fn config_resolves_relative_logo_paths_and_rejects_invalid_options() {
    let temporary = TempDir::new();
    let path = temporary.path.join("config.toml");
    fs::write(&path, "[logo]\npath = 'logo.png'\n").unwrap();
    assert_eq!(
        Config::load(&path).unwrap().logo.path.unwrap(),
        temporary.path.join("logo.png")
    );
    for invalid in [
        "[logo]\nheight = 0",
        "[logo]\nsource = 'unknown'",
        "[images]\nprotocol = 'unknown'",
        "[launcher]\nterminal = ['']",
        "[buttons]\nlauncher_background = 'red'",
        "[logo]\npaht = 'typo.png'",
    ] {
        fs::write(&path, invalid).unwrap();
        assert!(Config::load(&path).is_err(), "{invalid}");
    }
    fs::write(
        &path,
        "[buttons]\nlauncher_background = '#102030'\ninstaller_background = '#405060'\nmanager_background = '#708090'\nlauncher_foreground = '#a0b0c0'\ninstaller_manager_foreground = '#d0e0f0'\n",
    )
    .unwrap();
    let buttons = Config::load(&path).unwrap().buttons;
    assert_eq!(buttons.launcher_background.components(), (16, 32, 48));
    assert_eq!(buttons.installer_background.components(), (64, 80, 96));
    assert_eq!(buttons.manager_background.components(), (112, 128, 144));
    assert_eq!(buttons.launcher_foreground.components(), (160, 176, 192));
    assert_eq!(
        buttons.installer_manager_foreground.components(),
        (208, 224, 240)
    );
    fs::write(&path, crate::config::EXAMPLE).unwrap();
    Config::load(&path).unwrap();
}

#[test]
fn image_worker_renders_a_png_and_reports_a_missing_logo() {
    let temporary = TempDir::new();
    let path = temporary.path.join("logo.png");
    image::RgbaImage::from_pixel(16, 16, image::Rgba([255, 100, 50, 255]))
        .save(&path)
        .unwrap();
    let mut assets = Assets::new(Picker::halfblocks(), None);
    let mut terminal = Terminal::new(TestBackend::new(20, 10)).unwrap();
    let source = Source::File(path);
    let mut rendered = false;
    let deadline = Instant::now() + Duration::from_secs(5);
    while !rendered && Instant::now() < deadline {
        assets.poll();
        terminal
            .draw(|frame| {
                rendered = assets.render(frame, "logo".into(), source.clone(), frame.area());
            })
            .unwrap();
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(rendered, "El worker no devolvió la imagen");
    terminal
        .draw(|frame| {
            assets.render(
                frame,
                "missing".into(),
                Source::File(temporary.path.join("missing.png")),
                frame.area(),
            );
        })
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while assets.warning.is_none() && Instant::now() < deadline {
        assets.poll();
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(assets.warning.is_some());
}

#[test]
fn successful_launch_updates_recents_and_failed_launch_does_not() {
    let temporary = TempDir::new();
    let history_path = temporary.path.join("history.toml");
    let mut app = test_support::app(&["Working", "Missing"], history_path.clone());
    app.apps[0]
        .entry
        .add_desktop_entry("Exec".into(), "/usr/bin/true".into());
    app.apps[1].entry.add_desktop_entry(
        "Exec".into(),
        temporary
            .path
            .join("missing-executable")
            .to_string_lossy()
            .into_owned(),
    );
    app.handle(key(KeyCode::Enter)).unwrap();
    assert_eq!(app.recent, [0]);
    assert!(!app.status_error);
    assert_eq!(
        crate::history::History::load(&history_path).unwrap().recent,
        ["Working.desktop"]
    );
    app.handle(key(KeyCode::Right)).unwrap();
    app.handle(key(KeyCode::Enter)).unwrap();
    assert!(app.status_error);
    assert_eq!(app.recent, [0]);
    let deadline = Instant::now() + Duration::from_millis(50);
    while Instant::now() < deadline {
        app.reap_children();
        std::thread::sleep(Duration::from_millis(5));
    }
}
