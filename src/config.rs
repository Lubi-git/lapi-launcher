use std::{
    env, fs,
    io::Write,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize, de::Error as _};

pub const EXAMPLE: &str = include_str!("../config.example.toml");
pub const OFFICIAL_LOGO: &[u8] = include_bytes!("../resources/logo.png");
const CONFIG_FILE: &str = "config.toml";
const CONFIG_DIRECTORY: &str = "lapi-launcher";
const GLOBAL_CONFIG_DIRECTORY: &str = "/etc/lapi-launcher";
const LOGO_FILE: &str = "logo.png";

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub logo: Logo,
    pub images: Images,
    pub launcher: Launcher,
    pub applications: Applications,
    pub buttons: Buttons,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Logo {
    pub source: LogoSource,
    pub path: Option<PathBuf>,
    pub width: u16,
    pub height: u16,
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LogoSource {
    #[default]
    Builtin,
    Os,
    Desktop,
}

impl Default for Logo {
    fn default() -> Self {
        Self {
            source: LogoSource::Builtin,
            path: Some(PathBuf::from(LOGO_FILE)),
            width: 22,
            height: 7,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Images {
    pub enabled: bool,
    pub protocol: ImageProtocol,
    pub icon_theme: Option<String>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ImageProtocol {
    #[default]
    Auto,
    Halfblocks,
    Kitty,
    #[serde(rename = "kitty-legacy")]
    KittyLegacy,
    Sixel,
    Iterm2,
}

impl Default for Images {
    fn default() -> Self {
        Self {
            enabled: true,
            protocol: ImageProtocol::Auto,
            icon_theme: None,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Launcher {
    pub terminal: Vec<String>,
    pub recent_limit: usize,
    pub close_on_launch: bool,
}

impl Default for Launcher {
    fn default() -> Self {
        Self {
            terminal: Vec::new(),
            recent_limit: 12,
            close_on_launch: false,
        }
    }
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Applications {
    pub extra_dirs: Vec<PathBuf>,
    pub desktop_dir: Option<PathBuf>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Buttons {
    pub launcher_background: RgbColor,
    pub installer_background: RgbColor,
    pub manager_background: RgbColor,
    pub launcher_foreground: RgbColor,
    pub installer_manager_foreground: RgbColor,
}

impl Default for Buttons {
    fn default() -> Self {
        Self {
            launcher_background: RgbColor::new(166, 227, 161),
            installer_background: RgbColor::new(220, 38, 38),
            manager_background: RgbColor::new(220, 38, 38),
            launcher_foreground: RgbColor::new(0, 0, 0),
            installer_manager_foreground: RgbColor::new(255, 255, 255),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RgbColor {
    red: u8,
    green: u8,
    blue: u8,
}

impl RgbColor {
    pub const fn new(red: u8, green: u8, blue: u8) -> Self {
        Self { red, green, blue }
    }

    pub const fn components(self) -> (u8, u8, u8) {
        (self.red, self.green, self.blue)
    }

    fn parse(value: &str) -> std::result::Result<Self, String> {
        let value = value
            .strip_prefix('#')
            .ok_or_else(|| "debe usar el formato #RRGGBB".to_owned())?;
        if value.len() != 6 {
            return Err("debe usar exactamente seis dígitos hexadecimales".into());
        }
        let component = |offset| {
            u8::from_str_radix(&value[offset..offset + 2], 16)
                .map_err(|_| "debe usar el formato #RRGGBB".to_owned())
        };
        Ok(Self::new(component(0)?, component(2)?, component(4)?))
    }
}

impl Serialize for RgbColor {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&format!(
            "#{:02x}{:02x}{:02x}",
            self.red, self.green, self.blue
        ))
    }
}

impl<'de> Deserialize<'de> for RgbColor {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(&value).map_err(D::Error::custom)
    }
}

impl Config {
    pub fn load_standard() -> Result<Self> {
        let user = user_config_path()?;
        let global = global_config_path();
        let legacy = legacy_config_path()?;
        let config = load_preferred_config(&user, &global, &legacy)?;
        if user.exists() {
            write_official_logo_if_missing(&user)?;
        }
        Ok(config)
    }

    pub fn load(path: &Path) -> Result<Self> {
        let mut value = config_value(path)?;
        resolve_config_paths(&mut value, path)?;
        Self::from_value(value, &path.display().to_string(), config_directory(path)?)
    }
}

pub fn home() -> Result<PathBuf> {
    env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .context("HOME debe contener una ruta absoluta")
}

pub fn xdg_path(variable: &str, fallback: &str) -> Result<PathBuf> {
    match env::var_os(variable)
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
    {
        Some(path) => Ok(path),
        None => Ok(home()?.join(fallback)),
    }
}

pub fn user_config_path() -> Result<PathBuf> {
    Ok(user_config_path_for(&xdg_path(
        "XDG_CONFIG_HOME",
        ".config",
    )?))
}

pub fn global_config_path() -> PathBuf {
    PathBuf::from(GLOBAL_CONFIG_DIRECTORY).join(CONFIG_FILE)
}

fn legacy_config_path() -> Result<PathBuf> {
    let executable = env::current_exe().context("No se pudo localizar el ejecutable de Lapi")?;
    executable
        .parent()
        .filter(|directory| !directory.as_os_str().is_empty())
        .map(|directory| directory.join(CONFIG_FILE))
        .context("El ejecutable de Lapi no tiene un directorio válido")
}

fn user_config_path_for(config_home: &Path) -> PathBuf {
    config_home.join(CONFIG_DIRECTORY).join(CONFIG_FILE)
}

fn load_preferred_config(user: &Path, global: &Path, legacy: &Path) -> Result<Config> {
    if user.exists() {
        if global.exists() {
            return load_layered_config(global, user);
        }
        return Config::load(user);
    }
    if global.exists() {
        return Config::load(global);
    }
    if legacy.exists() {
        let mut config = Config::load(legacy)?;
        migrate_legacy_config(user, legacy, &mut config)?;
        return Config::load(user);
    }
    create_default_config(user)?;
    Config::load(user)
}

fn load_layered_config(global: &Path, user: &Path) -> Result<Config> {
    let mut global_value = config_value(global)?;
    let mut user_value = config_value(user)?;
    resolve_config_paths(&mut global_value, global)?;
    resolve_config_paths(&mut user_value, user)?;
    merge_config_values(&mut global_value, user_value);
    Config::from_value(
        global_value,
        &format!("{} y {}", global.display(), user.display()),
        config_directory(user)?,
    )
}

fn config_value(path: &Path) -> Result<toml::Value> {
    let contents =
        fs::read_to_string(path).with_context(|| format!("No se pudo leer {}", path.display()))?;
    toml::from_str(&contents).with_context(|| format!("Configuración inválida: {}", path.display()))
}

fn resolve_config_paths(value: &mut toml::Value, path: &Path) -> Result<()> {
    let base = path.parent().unwrap_or(Path::new("."));
    let Some(config) = value.as_table_mut() else {
        return Ok(());
    };
    if let Some(path) = config
        .get_mut("logo")
        .and_then(toml::Value::as_table_mut)
        .and_then(|logo| logo.get_mut("path"))
    {
        resolve_config_path(path, base)?;
    }
    let Some(applications) = config
        .get_mut("applications")
        .and_then(toml::Value::as_table_mut)
    else {
        return Ok(());
    };
    if let Some(path) = applications.get_mut("desktop_dir") {
        resolve_config_path(path, base)?;
    }
    if let Some(directories) = applications
        .get_mut("extra_dirs")
        .and_then(toml::Value::as_array_mut)
    {
        for directory in directories {
            resolve_config_path(directory, base)?;
        }
    }
    Ok(())
}

fn resolve_config_path(value: &mut toml::Value, base: &Path) -> Result<()> {
    let toml::Value::String(path) = value else {
        return Ok(());
    };
    *path = resolve_path(Path::new(path), base)?
        .to_string_lossy()
        .into_owned();
    Ok(())
}

fn merge_config_values(base: &mut toml::Value, overlay: toml::Value) {
    match (base, overlay) {
        (toml::Value::Table(base), toml::Value::Table(overlay)) => {
            for (key, value) in overlay {
                if let Some(current) = base.get_mut(&key) {
                    merge_config_values(current, value);
                } else {
                    base.insert(key, value);
                }
            }
        }
        (base, overlay) => *base = overlay,
    }
}

impl Config {
    fn from_value(value: toml::Value, source: &str, base: &Path) -> Result<Self> {
        let mut config: Self = value
            .try_into()
            .with_context(|| format!("Configuración inválida: {source}"))?;
        if let Some(logo) = &mut config.logo.path {
            *logo = resolve_path(logo, base)?;
        }
        for directory in &mut config.applications.extra_dirs {
            *directory = resolve_path(directory, base)?;
        }
        if let Some(directory) = &mut config.applications.desktop_dir {
            *directory = resolve_path(directory, base)?;
        }
        ensure!(
            (8..=60).contains(&config.logo.width),
            "logo.width debe estar entre 8 y 60"
        );
        ensure!(
            (3..=16).contains(&config.logo.height),
            "logo.height debe estar entre 3 y 16"
        );
        ensure!(
            config.launcher.recent_limit <= 100,
            "launcher.recent_limit no puede superar 100"
        );
        ensure!(
            config.launcher.terminal.iter().all(|arg| !arg.is_empty()),
            "launcher.terminal contiene un argumento vacío"
        );
        Ok(config)
    }
}

fn create_default_config(path: &Path) -> Result<()> {
    write_config_if_missing(path, EXAMPLE)?;
    write_official_logo_if_missing(path)
}

fn migrate_legacy_config(path: &Path, legacy: &Path, config: &mut Config) -> Result<()> {
    let legacy_logo = logo_path(legacy)?;
    let logo_is_official_or_missing = match fs::read(&legacy_logo) {
        Ok(bytes) => bytes == OFFICIAL_LOGO,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => true,
        Err(_) => false,
    };
    if config.logo.path.as_deref() == Some(legacy_logo.as_path()) && logo_is_official_or_missing {
        config.logo.path = Some(PathBuf::from(format!("./{LOGO_FILE}")));
    }
    let contents =
        toml::to_string_pretty(config).context("No se pudo preparar la configuración")?;
    write_config_if_missing(path, &contents)?;
    write_official_logo_if_missing(path)
}

fn write_config_if_missing(path: &Path, contents: &str) -> Result<()> {
    let directory = config_directory(path)?;
    fs::create_dir_all(directory)
        .with_context(|| format!("No se pudo crear {}", directory.display()))?;
    match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
    {
        Ok(mut file) => {
            file.write_all(contents.as_bytes())
                .with_context(|| format!("No se pudo escribir {}", path.display()))?;
            file.sync_all()
                .with_context(|| format!("No se pudo guardar {}", path.display()))
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
        Err(error) => Err(error).with_context(|| {
            format!(
                "No se pudo crear {}. El directorio de configuración debe permitir escritura",
                path.display()
            )
        }),
    }
}

fn write_official_logo_if_missing(config_path: &Path) -> Result<()> {
    let path = logo_path(config_path)?;
    let directory = config_directory(config_path)?;
    fs::create_dir_all(directory)
        .with_context(|| format!("No se pudo crear {}", directory.display()))?;
    match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
    {
        Ok(mut file) => {
            file.write_all(OFFICIAL_LOGO)
                .with_context(|| format!("No se pudo escribir {}", path.display()))?;
            file.sync_all()
                .with_context(|| format!("No se pudo guardar {}", path.display()))
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
        Err(error) => Err(error)
            .with_context(|| format!("No se pudo crear el logo oficial {}", path.display())),
    }
}

fn config_directory(path: &Path) -> Result<&Path> {
    path.parent()
        .filter(|directory| !directory.as_os_str().is_empty())
        .context("La ruta de configuración no tiene un directorio válido")
}

fn logo_path(config_path: &Path) -> Result<PathBuf> {
    Ok(config_directory(config_path)?.join(LOGO_FILE))
}

pub fn desktop_dir(config: &Config) -> Result<Option<PathBuf>> {
    if let Some(path) = &config.applications.desktop_dir {
        return Ok(Some(path.clone()));
    }
    let home = home()?;
    let user_dirs = xdg_path("XDG_CONFIG_HOME", ".config")?.join("user-dirs.dirs");
    let contents = fs::read_to_string(user_dirs).unwrap_or_default();
    if let Some(path) = parse_desktop_dir(&contents, &home) {
        return Ok((path != home).then_some(path));
    }
    Ok(Some(if home.join("Escritorio").is_dir() {
        home.join("Escritorio")
    } else {
        home.join("Desktop")
    }))
}

fn parse_desktop_dir(contents: &str, home: &Path) -> Option<PathBuf> {
    let value = contents
        .lines()
        .filter_map(|line| line.trim().split_once('='))
        .find(|(key, _)| key.trim() == "XDG_DESKTOP_DIR")?
        .1
        .trim();
    let value = value.strip_prefix('"')?.strip_suffix('"')?;
    let mut decoded = String::new();
    let mut characters = value.chars();
    while let Some(character) = characters.next() {
        if character == '\\' {
            decoded.push(characters.next()?);
        } else {
            decoded.push(character);
        }
    }
    if decoded == "$HOME" {
        return Some(home.to_owned());
    }
    if let Some(relative) = decoded.strip_prefix("$HOME/") {
        return Some(home.join(relative));
    }
    let path = PathBuf::from(decoded);
    path.is_absolute().then_some(path)
}

pub fn data_dirs() -> Result<Vec<PathBuf>> {
    let mut directories = vec![xdg_path("XDG_DATA_HOME", ".local/share")?];
    let system_dirs = env::var_os("XDG_DATA_DIRS")
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    directories.extend(env::split_paths(&system_dirs).filter(|path| path.is_absolute()));
    directories.extend([
        home()?.join(".local/share/flatpak/exports/share"),
        PathBuf::from("/var/lib/flatpak/exports/share"),
        PathBuf::from("/var/lib/snapd/desktop"),
    ]);
    let mut seen = std::collections::HashSet::new();
    directories.retain(|path| seen.insert(path.clone()));
    Ok(directories)
}

fn resolve_path(path: &Path, base: &Path) -> Result<PathBuf> {
    if path.as_os_str().is_empty() {
        bail!("Las rutas de configuración no pueden estar vacías");
    }
    if let Ok(relative) = path.strip_prefix("~") {
        return Ok(home()?.join(relative));
    }
    Ok(if path.is_absolute() {
        path.to_owned()
    } else {
        base.join(path)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_path_supports_xdg_localization_spaces_and_disabled_directories() {
        let home = Path::new("/home/example");
        assert_eq!(
            parse_desktop_dir("XDG_DESKTOP_DIR=\"$HOME/Mi Escritorio\"", home),
            Some(home.join("Mi Escritorio"))
        );
        assert_eq!(
            parse_desktop_dir("XDG_DESKTOP_DIR=\"/data/Desk\"", home),
            Some(PathBuf::from("/data/Desk"))
        );
        assert_eq!(
            parse_desktop_dir("XDG_DESKTOP_DIR=\"$HOME\"", home),
            Some(home.to_owned())
        );
        assert_eq!(
            parse_desktop_dir("XDG_DESKTOP_DIR=\"$(touch file)\"", home),
            None
        );
    }

    #[test]
    fn user_config_has_priority_over_global_config() {
        let temporary = crate::test_support::TempDir::new();
        let user = user_config_path_for(&temporary.path.join("user-config"));
        let global = temporary.path.join("etc/lapi-launcher/config.toml");
        let legacy = temporary.path.join("legacy/config.toml");
        fs::create_dir_all(global.parent().unwrap()).unwrap();
        fs::write(
            &global,
            "[logo]\nwidth = 30\nheight = 10\n\n[buttons]\nmanager_background = '#102030'\n",
        )
        .unwrap();
        assert_eq!(
            load_preferred_config(&user, &global, &legacy)
                .unwrap()
                .logo
                .width,
            30
        );

        fs::create_dir_all(user.parent().unwrap()).unwrap();
        fs::write(&user, "[logo]\nwidth = 40\n").unwrap();
        let config = load_preferred_config(&user, &global, &legacy).unwrap();
        assert_eq!(config.logo.width, 40);
        assert_eq!(config.logo.height, 10);
        assert_eq!(config.buttons.manager_background.components(), (16, 32, 48));
    }

    #[test]
    fn creates_a_default_user_config_when_no_config_exists() {
        let temporary = crate::test_support::TempDir::new();
        let user = user_config_path_for(&temporary.path.join("user-config"));
        let global = temporary.path.join("etc/lapi-launcher/config.toml");
        let legacy = temporary.path.join("legacy/config.toml");
        let config = load_preferred_config(&user, &global, &legacy).unwrap();
        assert_eq!(config.logo.width, 22);
        assert_eq!(
            config.logo.path,
            Some(user.parent().unwrap().join(LOGO_FILE))
        );
        assert_eq!(fs::read_to_string(&user).unwrap(), EXAMPLE);
        assert_eq!(
            fs::read(user.parent().unwrap().join(LOGO_FILE)).unwrap(),
            OFFICIAL_LOGO
        );
    }

    #[test]
    fn partial_config_uses_a_logo_next_to_its_configuration_file() {
        let temporary = crate::test_support::TempDir::new();
        let path = temporary.path.join("nested/config.toml");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "[buttons]\nlauncher_background = '#102030'\n").unwrap();

        assert_eq!(
            Config::load(&path).unwrap().logo.path,
            Some(temporary.path.join("nested/logo.png"))
        );
    }

    #[test]
    fn default_config_never_overwrites_an_existing_logo() {
        let temporary = crate::test_support::TempDir::new();
        let path = temporary.path.join("user-config/config.toml");
        let logo = path.parent().unwrap().join(LOGO_FILE);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&logo, b"custom-logo").unwrap();

        create_default_config(&path).unwrap();
        assert_eq!(fs::read(logo).unwrap(), b"custom-logo");
    }

    #[test]
    fn migrates_legacy_config_to_the_user_path_with_resolved_paths() {
        let temporary = crate::test_support::TempDir::new();
        let user = user_config_path_for(&temporary.path.join("user-config"));
        let global = temporary.path.join("etc/lapi-launcher/config.toml");
        let legacy = temporary.path.join("legacy/config.toml");
        fs::create_dir_all(legacy.parent().unwrap()).unwrap();
        fs::write(&legacy, "[logo]\npath = 'logo.png'\n").unwrap();

        let config = load_preferred_config(&user, &global, &legacy).unwrap();
        let user_logo = user.parent().unwrap().join(LOGO_FILE);
        assert_eq!(config.logo.path, Some(user_logo.clone()));
        assert_eq!(
            Config::load(&user).unwrap().logo.path,
            Some(user_logo.clone())
        );
        assert_eq!(fs::read(user_logo).unwrap(), OFFICIAL_LOGO);
    }
}
