use std::{
    env, fs,
    io::Write,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize, de::Error as _};

use crate::i18n::Language;

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
    pub interface: Interface,
    pub theme: Theme,
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Interface {
    pub language: Language,
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

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Theme {
    pub background: ThemeColor,
    pub foreground: ThemeColor,
    pub muted: RgbColor,
    pub accent: RgbColor,
    pub info: RgbColor,
    pub border: RgbColor,
    pub error: RgbColor,
    pub selection: RgbColor,
    pub header_background: RgbColor,
    pub header_foreground: ThemeColor,
    pub launcher_background: RgbColor,
    pub installer_background: RgbColor,
    pub manager_background: RgbColor,
    pub launcher_foreground: RgbColor,
    pub installer_manager_foreground: RgbColor,
    pub image_background: RgbColor,
    pub border_style: BorderStyle,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            background: ThemeColor::Terminal,
            foreground: ThemeColor::Terminal,
            muted: RgbColor::new(127, 132, 156),
            accent: RgbColor::new(203, 166, 247),
            info: RgbColor::new(137, 220, 235),
            border: RgbColor::new(69, 71, 90),
            error: RgbColor::new(243, 139, 168),
            selection: RgbColor::new(203, 166, 247),
            header_background: RgbColor::new(203, 166, 247),
            header_foreground: ThemeColor::Terminal,
            launcher_background: RgbColor::new(166, 227, 161),
            installer_background: RgbColor::new(220, 38, 38),
            manager_background: RgbColor::new(220, 38, 38),
            launcher_foreground: RgbColor::new(0, 0, 0),
            installer_manager_foreground: RgbColor::new(255, 255, 255),
            image_background: RgbColor::new(24, 24, 37),
            border_style: BorderStyle::Rounded,
        }
    }
}

pub const THEME_FIELD_COUNT: usize = 17;

impl Theme {
    pub const fn field_color(&self, index: usize) -> Option<RgbColor> {
        match index {
            0 => self.background.rgb(),
            1 => self.foreground.rgb(),
            2 => Some(self.muted),
            3 => Some(self.accent),
            4 => Some(self.info),
            5 => Some(self.border),
            6 => Some(self.error),
            7 => Some(self.selection),
            8 => Some(self.header_background),
            9 => self.header_foreground.rgb(),
            10 => Some(self.launcher_background),
            11 => Some(self.launcher_foreground),
            12 => Some(self.installer_background),
            13 => Some(self.manager_background),
            14 => Some(self.installer_manager_foreground),
            15 => Some(self.image_background),
            _ => None,
        }
    }

    pub fn field_value(&self, index: usize) -> String {
        match index {
            0 => self.background.as_config_value(),
            1 => self.foreground.as_config_value(),
            2 => self.muted.as_hex(),
            3 => self.accent.as_hex(),
            4 => self.info.as_hex(),
            5 => self.border.as_hex(),
            6 => self.error.as_hex(),
            7 => self.selection.as_hex(),
            8 => self.header_background.as_hex(),
            9 => self.header_foreground.as_config_value(),
            10 => self.launcher_background.as_hex(),
            11 => self.launcher_foreground.as_hex(),
            12 => self.installer_background.as_hex(),
            13 => self.manager_background.as_hex(),
            14 => self.installer_manager_foreground.as_hex(),
            15 => self.image_background.as_hex(),
            16 => self.border_style.to_string().into(),
            _ => String::new(),
        }
    }

    pub fn set_field_value(
        &mut self,
        index: usize,
        value: &str,
    ) -> std::result::Result<(), String> {
        match index {
            0 => self.background = ThemeColor::parse(value)?,
            1 => self.foreground = ThemeColor::parse(value)?,
            2 => self.muted = RgbColor::parse(value)?,
            3 => self.accent = RgbColor::parse(value)?,
            4 => self.info = RgbColor::parse(value)?,
            5 => self.border = RgbColor::parse(value)?,
            6 => self.error = RgbColor::parse(value)?,
            7 => self.selection = RgbColor::parse(value)?,
            8 => self.header_background = RgbColor::parse(value)?,
            9 => self.header_foreground = ThemeColor::parse(value)?,
            10 => self.launcher_background = RgbColor::parse(value)?,
            11 => self.launcher_foreground = RgbColor::parse(value)?,
            12 => self.installer_background = RgbColor::parse(value)?,
            13 => self.manager_background = RgbColor::parse(value)?,
            14 => self.installer_manager_foreground = RgbColor::parse(value)?,
            15 => self.image_background = RgbColor::parse(value)?,
            16 => self.border_style = BorderStyle::parse(value)?,
            _ => return Err("campo de tema inválido".into()),
        }
        Ok(())
    }

    pub fn cycle_field(&mut self, index: usize, direction: isize) {
        match index {
            0 => cycle_theme_color(&mut self.background, direction),
            1 => cycle_theme_color(&mut self.foreground, direction),
            2 => cycle_rgb_color(&mut self.muted, direction),
            3 => cycle_rgb_color(&mut self.accent, direction),
            4 => cycle_rgb_color(&mut self.info, direction),
            5 => cycle_rgb_color(&mut self.border, direction),
            6 => cycle_rgb_color(&mut self.error, direction),
            7 => cycle_rgb_color(&mut self.selection, direction),
            8 => cycle_rgb_color(&mut self.header_background, direction),
            9 => cycle_theme_color(&mut self.header_foreground, direction),
            10 => cycle_rgb_color(&mut self.launcher_background, direction),
            11 => cycle_rgb_color(&mut self.launcher_foreground, direction),
            12 => cycle_rgb_color(&mut self.installer_background, direction),
            13 => cycle_rgb_color(&mut self.manager_background, direction),
            14 => cycle_rgb_color(&mut self.installer_manager_foreground, direction),
            15 => cycle_rgb_color(&mut self.image_background, direction),
            16 => self.border_style = self.border_style.cycle(direction),
            _ => {}
        }
    }
}

const THEME_PALETTE: [RgbColor; 12] = [
    RgbColor::new(24, 24, 37),
    RgbColor::new(69, 71, 90),
    RgbColor::new(127, 132, 156),
    RgbColor::new(203, 166, 247),
    RgbColor::new(137, 220, 235),
    RgbColor::new(166, 227, 161),
    RgbColor::new(243, 139, 168),
    RgbColor::new(249, 226, 175),
    RgbColor::new(250, 179, 135),
    RgbColor::new(137, 180, 250),
    RgbColor::new(255, 255, 255),
    RgbColor::new(0, 0, 0),
];

fn cycle_rgb_color(color: &mut RgbColor, direction: isize) {
    let index = THEME_PALETTE
        .iter()
        .position(|candidate| candidate == color);
    let next = match (index, direction.is_negative()) {
        (Some(index), true) => (index + THEME_PALETTE.len() - 1) % THEME_PALETTE.len(),
        (Some(index), false) => (index + 1) % THEME_PALETTE.len(),
        (None, true) => THEME_PALETTE.len() - 1,
        (None, false) => 0,
    };
    *color = THEME_PALETTE[next];
}

fn cycle_theme_color(color: &mut ThemeColor, direction: isize) {
    if let ThemeColor::Rgb(value) = color {
        cycle_rgb_color(value, direction);
    } else {
        *color = ThemeColor::Rgb(if direction.is_negative() {
            *THEME_PALETTE.last().expect("theme palette is not empty")
        } else {
            THEME_PALETTE[0]
        });
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

    pub fn as_hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.red, self.green, self.blue)
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
        serializer.serialize_str(&self.as_hex())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeColor {
    Terminal,
    Rgb(RgbColor),
}

impl ThemeColor {
    pub const fn rgb(self) -> Option<RgbColor> {
        match self {
            Self::Terminal => None,
            Self::Rgb(color) => Some(color),
        }
    }

    fn parse(value: &str) -> std::result::Result<Self, String> {
        if value.eq_ignore_ascii_case("terminal") {
            Ok(Self::Terminal)
        } else {
            Ok(Self::Rgb(RgbColor::parse(value)?))
        }
    }

    pub fn as_config_value(self) -> String {
        self.rgb()
            .map(RgbColor::as_hex)
            .unwrap_or_else(|| "terminal".into())
    }
}

impl Serialize for ThemeColor {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.as_config_value())
    }
}

impl<'de> Deserialize<'de> for ThemeColor {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(&value).map_err(D::Error::custom)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum BorderStyle {
    Plain,
    #[default]
    Rounded,
    Double,
    Thick,
}

impl BorderStyle {
    fn parse(value: &str) -> std::result::Result<Self, String> {
        match value.to_ascii_lowercase().as_str() {
            "plain" => Ok(Self::Plain),
            "rounded" => Ok(Self::Rounded),
            "double" => Ok(Self::Double),
            "thick" => Ok(Self::Thick),
            _ => Err("debe ser plain, rounded, double o thick".into()),
        }
    }

    pub const fn cycle(self, direction: isize) -> Self {
        let styles = [Self::Plain, Self::Rounded, Self::Double, Self::Thick];
        let index = match self {
            Self::Plain => 0,
            Self::Rounded => 1,
            Self::Double => 2,
            Self::Thick => 3,
        };
        styles[if direction.is_negative() {
            (index + styles.len() - 1) % styles.len()
        } else {
            (index + 1) % styles.len()
        }]
    }

    pub const fn to_string(self) -> &'static str {
        match self {
            Self::Plain => "plain",
            Self::Rounded => "rounded",
            Self::Double => "double",
            Self::Thick => "thick",
        }
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
        migrate_button_theme(&mut value);
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
    migrate_button_theme(&mut global_value);
    migrate_button_theme(&mut user_value);
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

fn migrate_button_theme(value: &mut toml::Value) {
    let Some(config) = value.as_table_mut() else {
        return;
    };
    let Some(buttons) = config
        .remove("buttons")
        .and_then(|value| value.as_table().cloned())
    else {
        return;
    };
    let theme = config
        .entry("theme")
        .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));
    let Some(theme) = theme.as_table_mut() else {
        return;
    };
    for (name, color) in buttons {
        theme.entry(name).or_insert(color);
    }
}

impl Config {
    fn from_value(mut value: toml::Value, source: &str, base: &Path) -> Result<Self> {
        migrate_button_theme(&mut value);
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

pub fn save_user_theme(base: &Theme, theme: &Theme) -> Result<()> {
    let path = user_config_path()?;
    save_user_theme_to(&path, base, theme)
}

fn save_user_theme_to(path: &Path, base: &Theme, theme: &Theme) -> Result<()> {
    let mut value = if path.exists() {
        config_value(path)?
    } else {
        toml::Value::Table(toml::map::Map::new())
    };
    migrate_button_theme(&mut value);
    let Some(config) = value.as_table_mut() else {
        bail!("La configuración de usuario debe ser una tabla TOML")
    };
    let base = toml::Value::try_from(base).context("No se pudo preparar el tema base")?;
    let changed = toml::Value::try_from(theme).context("No se pudo preparar el tema")?;
    let Some(base) = base.as_table() else {
        bail!("El tema base debe ser una tabla TOML")
    };
    let Some(changed) = changed.as_table() else {
        bail!("El tema debe ser una tabla TOML")
    };
    let user_theme = config
        .entry("theme")
        .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));
    let Some(user_theme) = user_theme.as_table_mut() else {
        bail!("El tema de usuario debe ser una tabla TOML")
    };
    for (name, color) in changed {
        if base.get(name) != Some(color) {
            user_theme.insert(name.clone(), color.clone());
        }
    }
    let contents =
        toml::to_string_pretty(&value).context("No se pudo preparar la configuración")?;
    write_config(path, &contents)?;
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

fn write_config(path: &Path, contents: &str) -> Result<()> {
    let directory = config_directory(path)?;
    fs::create_dir_all(directory)
        .with_context(|| format!("No se pudo crear {}", directory.display()))?;
    let temporary = directory.join(format!(".{CONFIG_FILE}.{}.tmp", std::process::id()));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .with_context(|| format!("No se pudo crear {}", temporary.display()))?;
    let result = (|| {
        file.write_all(contents.as_bytes())
            .with_context(|| format!("No se pudo escribir {}", temporary.display()))?;
        file.sync_all()
            .with_context(|| format!("No se pudo guardar {}", temporary.display()))?;
        fs::rename(&temporary, path)
            .with_context(|| format!("No se pudo actualizar {}", path.display()))
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
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
        assert_eq!(config.theme.manager_background.components(), (16, 32, 48));
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

    #[test]
    fn theme_supports_terminal_colors_and_cycles_the_palette() {
        let mut theme = Theme::default();
        theme.set_field_value(0, "#102030").unwrap();
        assert_eq!(theme.background.rgb().unwrap().components(), (16, 32, 48));
        theme.set_field_value(0, "terminal").unwrap();
        assert_eq!(theme.background, ThemeColor::Terminal);
        theme.cycle_field(0, 1);
        assert_ne!(theme.background, ThemeColor::Terminal);
        theme.set_field_value(16, "double").unwrap();
        assert_eq!(theme.border_style, BorderStyle::Double);
    }

    #[test]
    fn saving_a_theme_preserves_unmodified_user_and_global_values() {
        let temporary = crate::test_support::TempDir::new();
        let path = temporary.path.join("user-config/config.toml");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "[theme]\nmuted = '#102030'\n").unwrap();
        let base = Theme::default();
        let mut changed = base.clone();
        changed.accent = RgbColor::new(1, 2, 3);

        save_user_theme_to(&path, &base, &changed).unwrap();

        let value = config_value(&path).unwrap();
        let theme = value.get("theme").and_then(toml::Value::as_table).unwrap();
        assert_eq!(
            theme.get("muted").and_then(toml::Value::as_str),
            Some("#102030")
        );
        assert_eq!(
            theme.get("accent").and_then(toml::Value::as_str),
            Some("#010203")
        );
        assert!(!theme.contains_key("background"));
        assert_eq!(
            fs::read(path.parent().unwrap().join(LOGO_FILE)).unwrap(),
            OFFICIAL_LOGO
        );
    }
}
