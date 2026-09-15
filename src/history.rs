use std::{
    fs::{self, OpenOptions},
    io::Write,
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::config;

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct History {
    #[serde(default)]
    pub recent: Vec<String>,
}

impl History {
    pub fn default_path() -> Result<PathBuf> {
        Ok(config::xdg_path("XDG_STATE_HOME", ".local/state")?.join("lapi-launcher/recent.toml"))
    }

    pub fn load(path: &Path) -> Result<Self> {
        match fs::read_to_string(path) {
            Ok(contents) => {
                toml::from_str(&contents).context("El historial de aplicaciones no es válido")
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(error).context("No se pudo leer el historial"),
        }
    }

    pub fn record(&mut self, id: &str, limit: usize) {
        self.recent.retain(|previous| previous != id);
        self.recent.insert(0, id.to_owned());
        self.recent.truncate(limit);
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let parent = path.parent().context("Ruta de historial inválida")?;
        fs::create_dir_all(parent).context("No se pudo crear el directorio del historial")?;
        let temporary = path.with_extension(format!("{}.tmp", std::process::id()));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temporary)
            .context("No se pudo crear el archivo temporal del historial")?;
        let result = (|| -> Result<()> {
            file.write_all(toml::to_string(self)?.as_bytes())?;
            file.sync_all()?;
            fs::rename(&temporary, path)?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result.context("No se pudo guardar el historial")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recents_are_unique_newest_first_and_bounded() {
        let mut history = History::default();
        for id in ["first", "second", "third", "second"] {
            history.record(id, 2);
        }
        assert_eq!(history.recent, ["second", "third"]);
        history.record("first", 0);
        assert!(history.recent.is_empty());
    }

    #[test]
    fn persists_recents_and_reports_corrupted_history() {
        let temporary = crate::test_support::TempDir::new();
        let path = temporary.path.join("state/recent.toml");
        let mut history = History::load(&path).unwrap();
        history.record("example.desktop", 12);
        history.save(&path).unwrap();
        assert_eq!(History::load(&path).unwrap().recent, ["example.desktop"]);
        history.record("another.desktop", 12);
        history.save(&path).unwrap();
        assert_eq!(
            History::load(&path).unwrap().recent,
            ["another.desktop", "example.desktop"]
        );
        fs::write(path, "recent = [invalid").unwrap();
        assert!(History::load(&temporary.path.join("state/recent.toml")).is_err());
    }
}
