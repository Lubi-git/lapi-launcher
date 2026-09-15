use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

use freedesktop_desktop_entry::DesktopEntry;

use crate::{
    app::App,
    config::Config,
    desktop::{Application, Catalog},
    history::History,
};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

pub struct TempDir {
    pub path: PathBuf,
}

impl TempDir {
    pub fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "lapi-test-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self { path }
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

pub fn app(names: &[&str], history_path: PathBuf) -> App {
    let apps = names
        .iter()
        .map(|name| Application {
            id: format!("{name}.desktop"),
            name: (*name).into(),
            search_text: (*name).into(),
            entry: DesktopEntry::from_appid((*name).into()),
            on_desktop: true,
            file: None,
        })
        .collect();
    App::new(
        Config::default(),
        Catalog { apps, skipped: 0 },
        History::default(),
        history_path,
    )
}
