use std::{
    collections::HashSet,
    env, fs,
    os::unix::{fs::PermissionsExt, process::CommandExt},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
};

use anyhow::{Context, Result, bail, ensure};
use freedesktop_desktop_entry::{DesktopEntry, Iter};

use crate::config::{self, Config};

#[derive(Debug)]
pub struct Application {
    pub id: String,
    pub name: String,
    pub search_text: String,
    pub entry: DesktopEntry,
    pub on_desktop: bool,
    pub file: Option<PathBuf>,
}

pub struct Catalog {
    pub apps: Vec<Application>,
    pub skipped: usize,
}

pub fn discover(config: &Config) -> Result<Catalog> {
    let mut roots = config.applications.extra_dirs.clone();
    roots.extend(
        config::data_dirs()?
            .into_iter()
            .map(|path| path.join("applications")),
    );
    let locales = locales();
    let desktops: Vec<String> = env::var("XDG_CURRENT_DESKTOP")
        .unwrap_or_default()
        .split(':')
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .collect();
    let mut catalog = scan(roots, &locales, &desktops);
    if let Some(directory) = config::desktop_dir(config)? {
        add_desktop(&mut catalog, &directory, &locales, &desktops)?;
    }
    catalog
        .apps
        .sort_by_cached_key(|app| (app.name.to_lowercase(), app.id.clone()));
    Ok(catalog)
}

fn scan(roots: Vec<PathBuf>, locales: &[String], desktops: &[String]) -> Catalog {
    let mut seen = HashSet::new();
    let mut catalog = Catalog {
        apps: Vec::new(),
        skipped: 0,
    };
    for root in roots {
        let root = root.canonicalize().unwrap_or(root);
        for path in Iter::new(std::iter::once(root.clone())) {
            let id = desktop_id(&root, &path);
            if !seen.insert(id.clone()) {
                continue;
            }
            let entry = match DesktopEntry::from_path(&path, Some(locales)) {
                Ok(entry) => entry,
                Err(_) => {
                    catalog.skipped += 1;
                    continue;
                }
            };
            if !visible(&entry, desktops) {
                continue;
            }
            if entry
                .try_exec()
                .is_some_and(|program| executable(program).is_none())
            {
                continue;
            }
            let Some(name) = entry
                .name(locales)
                .map(|name| name.into_owned())
                .filter(|name| !name.trim().is_empty())
            else {
                catalog.skipped += 1;
                continue;
            };
            let search_text = format!(
                "{} {} {} {} {}",
                name,
                id,
                entry.generic_name(locales).unwrap_or_default(),
                entry.keywords(locales).unwrap_or_default().join(" "),
                entry.exec().unwrap_or_default()
            );
            catalog.apps.push(Application {
                id,
                name,
                search_text,
                entry,
                on_desktop: false,
                file: None,
            });
        }
    }
    catalog
        .apps
        .sort_by_cached_key(|app| (app.name.to_lowercase(), app.id.clone()));
    catalog
}

fn desktop_id(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("-")
}

fn add_desktop(
    catalog: &mut Catalog,
    directory: &Path,
    locales: &[String],
    desktops: &[String],
) -> Result<()> {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error).context("No se pudo leer el escritorio"),
    };
    for file in entries.flatten() {
        let path = file.path();
        let filename = file.file_name().to_string_lossy().into_owned();
        if filename.starts_with('.') {
            continue;
        }
        let is_launcher = path
            .extension()
            .is_some_and(|extension| extension == "desktop")
            || path.canonicalize().ok().is_some_and(|path| {
                path.extension()
                    .is_some_and(|extension| extension == "desktop")
            });
        if is_launcher {
            let entry = match DesktopEntry::from_path(&path, Some(locales)) {
                Ok(entry) => entry,
                Err(_) => {
                    catalog.skipped += 1;
                    continue;
                }
            };
            if !visible(&entry, desktops) {
                continue;
            }
            let Some(name) = entry.name(locales).map(|name| name.into_owned()) else {
                continue;
            };
            let application = Application {
                id: filename,
                search_text: name.clone(),
                name,
                entry,
                on_desktop: true,
                file: None,
            };
            if let Some(previous) = catalog.apps.iter_mut().find(|previous| {
                previous.id == application.id
                    || previous
                        .entry
                        .path
                        .canonicalize()
                        .ok()
                        .zip(path.canonicalize().ok())
                        .is_some_and(|(left, right)| left == right)
            }) {
                *previous = application;
            } else {
                catalog.apps.push(application);
            }
        } else if path.is_file() || path.is_dir() {
            let name = if path.is_dir() {
                filename
            } else {
                path.file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned()
            };
            let mut entry = DesktopEntry::from_appid(name.clone());
            entry.path = path.clone();
            entry.add_desktop_entry("Icon".into(), file_icon(&path).into());
            catalog.apps.push(Application {
                id: format!("file:{}", path.display()),
                search_text: name.clone(),
                name,
                entry,
                on_desktop: false,
                file: Some(path),
            });
        }
    }
    Ok(())
}

fn file_icon(path: &Path) -> &'static str {
    if path.is_dir() {
        return "folder";
    }
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "png" | "jpg" | "jpeg" | "webp" | "gif" | "svg" => "image-x-generic",
        "pdf" => "application-pdf",
        "zip" | "gz" | "xz" | "7z" | "tar" => "package-x-generic",
        "mp3" | "ogg" | "flac" | "wav" => "audio-x-generic",
        "mp4" | "webm" | "mkv" => "video-x-generic",
        _ => "text-x-generic",
    }
}

fn visible(entry: &DesktopEntry, desktops: &[String]) -> bool {
    if entry.type_() != Some("Application") || entry.hidden() || entry.no_display() {
        return false;
    }
    if entry.exec().is_none() && !entry.dbus_activatable() {
        return false;
    }
    if let Some(only) = entry.only_show_in()
        && !only
            .iter()
            .any(|allowed| desktops.iter().any(|desktop| desktop == allowed))
    {
        return false;
    }
    if let Some(excluded) = entry.not_show_in()
        && excluded
            .iter()
            .any(|blocked| desktops.iter().any(|desktop| desktop == blocked))
    {
        return false;
    }
    true
}

pub fn executable(program: &str) -> Option<PathBuf> {
    let is_executable = |path: &Path| {
        fs::metadata(path)
            .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
    };
    if program.contains('/') {
        let path = PathBuf::from(program);
        return is_executable(&path).then_some(path);
    }
    env::var_os("PATH").and_then(|paths| {
        env::split_paths(&paths)
            .map(|directory| directory.join(program))
            .find(|path| is_executable(path))
    })
}

pub fn launch(app: &Application, config: &Config) -> Result<Child> {
    if let Some(path) = &app.file {
        let mut command = if executable("xdg-open").is_some() {
            Command::new("xdg-open")
        } else {
            let mut command = Command::new("gio");
            command.arg("open");
            command
        };
        return command
            .arg(path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .process_group(0)
            .spawn()
            .with_context(|| format!("No se pudo abrir {}", app.name));
    }
    let entry = &app.entry;
    let mut arguments =
        if entry.dbus_activatable() && !entry.terminal() && executable("gio").is_some() {
            vec![
                "gio".to_owned(),
                "launch".to_owned(),
                entry.path.to_string_lossy().into_owned(),
            ]
        } else {
            exec_arguments(entry, &app.name)?
        };
    if entry.terminal() {
        let mut terminal = terminal_arguments(&config.launcher.terminal)?;
        terminal.append(&mut arguments);
        arguments = terminal;
    }
    let (program, arguments) = arguments.split_first().context("Exec está vacío")?;
    let mut command = Command::new(program);
    command
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0);
    if let Some(directory) = entry.path().filter(|path| !path.is_empty()) {
        command.current_dir(directory);
    }
    command
        .spawn()
        .with_context(|| format!("No se pudo abrir {}", app.name))
}

fn terminal_arguments(configured: &[String]) -> Result<Vec<String>> {
    if !configured.is_empty() {
        return Ok(configured.to_vec());
    }
    let candidates: &[&[&str]] = &[
        &["xdg-terminal-exec"],
        &["konsole", "-e"],
        &["kitty", "--"],
        &["wezterm", "start", "--"],
        &["alacritty", "-e"],
        &["foot", "--"],
        &["gnome-terminal", "--"],
        &["xfce4-terminal", "-x"],
        &["x-terminal-emulator", "-e"],
        &["xterm", "-e"],
    ];
    for candidate in candidates {
        if executable(candidate[0]).is_some() {
            return Ok(candidate
                .iter()
                .map(|argument| (*argument).to_owned())
                .collect());
        }
    }
    bail!("Configura launcher.terminal para abrir aplicaciones de terminal")
}

fn exec_arguments(entry: &DesktopEntry, name: &str) -> Result<Vec<String>> {
    let exec = entry
        .exec()
        .context("La aplicación requiere activación D-Bus; instala gio")?;
    let tokens = tokenize(&unescape_value(exec)?)?;
    let mut arguments = Vec::new();
    for token in tokens {
        match token.as_str() {
            "%f" | "%F" | "%u" | "%U" | "%d" | "%D" | "%n" | "%N" | "%v" | "%m" => continue,
            "%i" => {
                if let Some(icon) = entry.icon() {
                    arguments.extend(["--icon".to_owned(), icon.to_owned()]);
                }
            }
            "%c" => arguments.push(name.to_owned()),
            "%k" => arguments.push(entry.path.to_string_lossy().into_owned()),
            _ => {
                let mut expanded = String::new();
                let mut characters = token.chars();
                while let Some(character) = characters.next() {
                    if character != '%' {
                        expanded.push(character);
                        continue;
                    }
                    match characters.next() {
                        Some('%') => expanded.push('%'),
                        Some('c') => expanded.push_str(name),
                        Some('k') => expanded.push_str(&entry.path.to_string_lossy()),
                        Some(code) => bail!("Código Exec no válido en un argumento: %{code}"),
                        None => bail!("Código Exec incompleto"),
                    }
                }
                arguments.push(expanded);
            }
        }
    }
    ensure!(
        arguments
            .first()
            .is_some_and(|program| !program.is_empty() && !program.contains('=')),
        "Ejecutable .desktop inválido"
    );
    Ok(arguments)
}

fn unescape_value(value: &str) -> Result<String> {
    let mut decoded = String::new();
    let mut characters = value.chars();
    while let Some(character) = characters.next() {
        if character != '\\' {
            decoded.push(character);
            continue;
        }
        match characters.next().context("Escape incompleto en Exec")? {
            's' => decoded.push(' '),
            'n' => decoded.push('\n'),
            't' => decoded.push('\t'),
            'r' => decoded.push('\r'),
            '\\' => decoded.push('\\'),
            escaped => {
                decoded.push('\\');
                decoded.push(escaped);
            }
        }
    }
    Ok(decoded)
}

fn tokenize(exec: &str) -> Result<Vec<String>> {
    let mut arguments = Vec::new();
    let mut argument = String::new();
    let mut quoted = false;
    let mut started = false;
    let mut characters = exec.chars();
    while let Some(character) = characters.next() {
        match character {
            '"' => {
                quoted = !quoted;
                started = true;
            }
            '\\' => {
                let escaped = characters.next().context("Escape incompleto en Exec")?;
                if quoted && !matches!(escaped, '"' | '`' | '$' | '\\') {
                    argument.push('\\');
                }
                argument.push(escaped);
                started = true;
            }
            whitespace if whitespace.is_ascii_whitespace() && !quoted => {
                if started {
                    arguments.push(std::mem::take(&mut argument));
                    started = false;
                }
            }
            character => {
                argument.push(character);
                started = true;
            }
        }
    }
    ensure!(!quoted, "Comillas sin cerrar en Exec");
    if started {
        arguments.push(argument);
    }
    Ok(arguments)
}

fn locales() -> Vec<String> {
    let mut locales: Vec<String> = env::var("LANGUAGE")
        .unwrap_or_default()
        .split(':')
        .filter(|locale| !locale.is_empty())
        .map(str::to_owned)
        .collect();
    for variable in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(locale) = env::var(variable)
            && !locale.is_empty()
        {
            locales.push(locale.split('.').next().unwrap_or(&locale).to_owned());
            break;
        }
    }
    let fallbacks: Vec<String> = locales
        .iter()
        .filter_map(|locale| {
            locale
                .split_once('_')
                .map(|(language, _)| language.to_owned())
        })
        .collect();
    locales.extend(fallbacks);
    locales
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(exec: &str) -> DesktopEntry {
        let mut entry = DesktopEntry::from_appid("example".into());
        entry.path = PathBuf::from("/apps/Example App.desktop");
        entry.add_desktop_entry("Type".into(), "Application".into());
        entry.add_desktop_entry("Exec".into(), exec.into());
        entry
    }

    #[test]
    fn launch_preserves_quoted_paths_empty_arguments_and_literal_shell_syntax() {
        let entry = entry(r#""/opt/My App/bin" "two words" "" "$(touch /tmp/unwanted)" %U"#);
        assert_eq!(
            exec_arguments(&entry, "Example").unwrap(),
            ["/opt/My App/bin", "two words", "", "$(touch /tmp/unwanted)"]
        );
    }

    #[test]
    fn expands_desktop_fields_without_splitting_names() {
        let mut entry = entry("example %i %c %k %% %f");
        entry.add_desktop_entry("Icon".into(), "example-icon".into());
        assert_eq!(
            exec_arguments(&entry, "Mi aplicación").unwrap(),
            [
                "example",
                "--icon",
                "example-icon",
                "Mi aplicación",
                "/apps/Example App.desktop",
                "%"
            ]
        );
    }

    #[test]
    fn rejects_malformed_exec() {
        for exec in [
            "",
            "\"unfinished",
            "app %Z",
            "app %",
            "ENV=value app",
            "app trailing\\",
        ] {
            assert!(exec_arguments(&entry(exec), "Example").is_err(), "{exec}");
        }
    }

    #[test]
    fn respects_visibility_and_desktop_membership() {
        let mut entry = entry("example");
        entry.add_desktop_entry("OnlyShowIn".into(), "KDE;GNOME;".into());
        assert!(visible(&entry, &["KDE".into()]));
        assert!(!visible(&entry, &["XFCE".into()]));
        entry.add_desktop_entry("NotShowIn".into(), "KDE;".into());
        assert!(!visible(&entry, &["KDE".into()]));
        entry.add_desktop_entry("Hidden".into(), "true".into());
        assert!(!visible(&entry, &["GNOME".into()]));
    }

    #[test]
    fn nested_desktop_ids_use_relative_paths() {
        assert_eq!(
            desktop_id(Path::new("/apps"), Path::new("/apps/vendor/tool.desktop")),
            "vendor-tool.desktop"
        );
    }

    #[test]
    fn desktop_membership_comes_from_the_user_folder_and_search_includes_files() {
        let temporary = crate::test_support::TempDir::new();
        let system = temporary.path.join("applications");
        let desktop = temporary.path.join("Escritorio");
        fs::create_dir_all(&system).unwrap();
        fs::create_dir_all(&desktop).unwrap();
        let launcher = "[Desktop Entry]\nType=Application\nName=Installed Alias\nExec=example\n";
        fs::write(system.join("internal.desktop"), launcher).unwrap();
        fs::write(
            system.join("global.desktop"),
            launcher.replace("Installed Alias", "Global Alias"),
        )
        .unwrap();
        fs::write(
            desktop.join("internal.desktop"),
            launcher.replace("Installed Alias", "Desktop Alias"),
        )
        .unwrap();
        fs::write(desktop.join("Notes.txt"), "A note").unwrap();
        fs::write(desktop.join(".private.txt"), "Private").unwrap();
        let mut catalog = scan(vec![system], &[], &[]);
        add_desktop(&mut catalog, &desktop, &[], &[]).unwrap();
        assert_eq!(catalog.apps.len(), 3);
        let shortcuts: Vec<_> = catalog.apps.iter().filter(|app| app.on_desktop).collect();
        assert_eq!(shortcuts.len(), 1);
        assert_eq!(shortcuts[0].name, "Desktop Alias");
        assert_eq!(shortcuts[0].entry.path, desktop.join("internal.desktop"));
        let note = catalog.apps.iter().find(|app| app.name == "Notes").unwrap();
        assert_eq!(
            note.file.as_deref(),
            Some(desktop.join("Notes.txt").as_path())
        );
        assert!(!note.on_desktop);
    }

    #[test]
    fn decodes_desktop_value_escapes_before_argument_quoting() {
        let entry = entry(r#"app "two\swords" "a\\\\b" "a\\"b""#);
        assert_eq!(
            exec_arguments(&entry, "Example").unwrap(),
            ["app", "two words", "a\\b", "a\"b"]
        );
    }

    #[test]
    fn user_entries_override_system_entries_even_when_hidden() {
        let temporary = crate::test_support::TempDir::new();
        let user = temporary.path.join("user");
        let system = temporary.path.join("system");
        fs::create_dir_all(user.join("nested")).unwrap();
        fs::create_dir_all(system.join("nested")).unwrap();
        let normal = "[Desktop Entry]\nType=Application\nName=Example\nExec=example\n";
        fs::write(system.join("nested/app.desktop"), normal).unwrap();
        fs::write(
            user.join("nested/app.desktop"),
            format!("{normal}Hidden=true\n"),
        )
        .unwrap();
        fs::write(system.join("shown.desktop"), normal).unwrap();
        fs::write(system.join("broken.desktop"), "not a desktop file").unwrap();
        let catalog = scan(vec![user, system], &[], &[]);
        assert_eq!(catalog.apps.len(), 1);
        assert_eq!(catalog.apps[0].id, "shown.desktop");
    }

    #[test]
    fn launching_honors_working_directory_and_does_not_interpret_shell_arguments() {
        let temporary = crate::test_support::TempDir::new();
        let recorder = temporary.path.join("record arguments");
        fs::write(
            &recorder,
            "#!/bin/sh\npwd > working-directory\nprintf '%s\\n' \"$@\" > arguments\n",
        )
        .unwrap();
        fs::set_permissions(&recorder, fs::Permissions::from_mode(0o700)).unwrap();
        let mut entry = entry(&format!(
            "\"{}\" \"two words\" \"$(touch unexpected)\" %U",
            recorder.display()
        ));
        entry.add_desktop_entry("Path".into(), temporary.path.to_string_lossy().into_owned());
        let app = Application {
            id: "test.desktop".into(),
            name: "Test".into(),
            search_text: "Test".into(),
            entry,
            on_desktop: true,
            file: None,
        };
        assert!(
            launch(&app, &Config::default())
                .unwrap()
                .wait()
                .unwrap()
                .success()
        );
        assert_eq!(
            fs::read_to_string(temporary.path.join("arguments")).unwrap(),
            "two words\n$(touch unexpected)\n"
        );
        assert_eq!(
            fs::read_to_string(temporary.path.join("working-directory"))
                .unwrap()
                .trim(),
            temporary.path.to_str().unwrap()
        );
        assert!(!temporary.path.join("unexpected").exists());
    }
}
