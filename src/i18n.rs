use std::env;

use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    #[default]
    Auto,
    #[serde(rename = "es")]
    Spanish,
    #[serde(rename = "en")]
    English,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Locale {
    Spanish,
    English,
}

#[derive(Clone, Copy)]
pub struct Translator(Locale);

impl Translator {
    pub fn new(language: Language) -> Self {
        Self(match language {
            Language::Auto => locale_from_environment(),
            Language::Spanish => Locale::Spanish,
            Language::English => Locale::English,
        })
    }

    pub fn from_environment() -> Self {
        Self(locale_from_environment())
    }

    pub const fn command_help(self) -> &'static str {
        match self.0 {
            Locale::Spanish => {
                "Lapi Launcher · aplicaciones Linux en tu terminal\n\nUso: lapi-launcher [opciones]\n\n  Configuración          Usuario: $XDG_CONFIG_HOME/lapi-launcher/config.toml (o ~/.config)\n                         Global: /etc/lapi-launcher/config.toml\n  --list                 Listar aplicaciones sin abrir la TUI\n  --no-images            Usar iniciales en lugar de imágenes\n  --print-default-config Imprimir configuración de ejemplo\n  --version              Mostrar versión\n  --help                 Mostrar esta ayuda"
            }
            Locale::English => {
                "Lapi Launcher · Linux applications in your terminal\n\nUsage: lapi-launcher [options]\n\n  Configuration          User: $XDG_CONFIG_HOME/lapi-launcher/config.toml (or ~/.config)\n                         Global: /etc/lapi-launcher/config.toml\n  --list                 List applications without opening the TUI\n  --no-images            Use initials instead of images\n  --print-default-config Print the example configuration\n  --version              Show the version\n  --help                 Show this help"
            }
        }
    }

    pub const fn config_option_message(self) -> &'static str {
        match self.0 {
            Locale::Spanish => {
                "Lapi busca primero la configuración de usuario y después /etc/lapi-launcher/config.toml"
            }
            Locale::English => {
                "Lapi checks the user configuration first and then /etc/lapi-launcher/config.toml"
            }
        }
    }

    pub fn unknown_option(self, option: &str) -> String {
        match self.0 {
            Locale::Spanish => format!("Opción desconocida: {option}. Usa --help"),
            Locale::English => format!("Unknown option: {option}. Use --help"),
        }
    }

    pub const fn interactive_terminal_required(self) -> &'static str {
        match self.0 {
            Locale::Spanish => "Ejecuta Lapi dentro de un terminal interactivo, o usa --list",
            Locale::English => "Run Lapi in an interactive terminal, or use --list",
        }
    }

    pub const fn is_spanish(self) -> bool {
        matches!(self.0, Locale::Spanish)
    }

    pub const fn compact_terminal(self) -> &'static str {
        match self.0 {
            Locale::Spanish => "LAPI\nAmplía el terminal a 40 × 20.\nEsc / Ctrl+C: salir",
            Locale::English => "LAPI\nExpand the terminal to 40 × 20.\nEsc / Ctrl+C: quit",
        }
    }

    pub const fn tagline(self) -> &'static str {
        match self.0 {
            Locale::Spanish => "  LANZADOR DE APLICACIONES",
            Locale::English => "  APPLICATION LAUNCHER",
        }
    }

    pub const fn footer_home(self) -> &'static str {
        match self.0 {
            Locale::Spanish => {
                "Rueda / PgUp / PgDn desplazar  ↑↓←→ mover  Tab sección  F1 ayuda  Esc salir"
            }
            Locale::English => {
                "Wheel / PgUp / PgDn scroll  ↑↓←→ move  Tab section  F1 help  Esc quit"
            }
        }
    }

    pub const fn footer_search(self) -> &'static str {
        match self.0 {
            Locale::Spanish => "↑↓ mover  Enter abrir  Esc volver  F1 ayuda",
            Locale::English => "↑↓ move  Enter open  Esc back  F1 help",
        }
    }

    pub const fn footer_default(self) -> &'static str {
        match self.0 {
            Locale::Spanish => "↑↓←→ mover  Enter abrir  Tab sección  F1 ayuda  Esc salir",
            Locale::English => "↑↓←→ move  Enter open  Tab section  F1 help  Esc quit",
        }
    }

    pub const fn system_information(self) -> &'static str {
        match self.0 {
            Locale::Spanish => "Información del sistema",
            Locale::English => "System information",
        }
    }

    pub const fn system_information_scroll(self) -> &'static str {
        match self.0 {
            Locale::Spanish => "Información del sistema · Ctrl+↑↓ / rueda",
            Locale::English => "System information · Ctrl+↑↓ / wheel",
        }
    }

    pub const fn search(self) -> &'static str {
        match self.0 {
            Locale::Spanish => " Buscar ",
            Locale::English => " Search ",
        }
    }

    pub const fn search_placeholder(self) -> &'static str {
        match self.0 {
            Locale::Spanish => " Busca una aplicación o archivo…",
            Locale::English => " Search for an application or file…",
        }
    }

    pub const fn desktop(self) -> &'static str {
        match self.0 {
            Locale::Spanish => "Escritorio",
            Locale::English => "Desktop",
        }
    }

    pub const fn recent(self) -> &'static str {
        match self.0 {
            Locale::Spanish => "Recientes",
            Locale::English => "Recent",
        }
    }

    pub const fn no_matches_clear(self) -> &'static str {
        match self.0 {
            Locale::Spanish => "  Sin coincidencias. Esc limpia la búsqueda.",
            Locale::English => "  No matches. Esc clears the search.",
        }
    }

    pub const fn no_matches_back(self) -> &'static str {
        match self.0 {
            Locale::Spanish => "  Sin coincidencias. Esc vuelve al escritorio.",
            Locale::English => "  No matches. Esc returns to the desktop.",
        }
    }

    pub const fn no_recent(self) -> &'static str {
        match self.0 {
            Locale::Spanish => "  Las aplicaciones que abras aparecerán aquí.",
            Locale::English => "  Applications you open will appear here.",
        }
    }

    pub const fn no_desktop(self) -> &'static str {
        match self.0 {
            Locale::Spanish => "  No hay accesos a aplicaciones en tu escritorio.",
            Locale::English => "  There are no application entries on your desktop.",
        }
    }

    pub const fn help_title(self) -> &'static str {
        match self.0 {
            Locale::Spanish => " Lapi · controles ",
            Locale::English => " Lapi · controls ",
        }
    }

    pub const fn launcher_button(self) -> &'static str {
        match self.0 {
            Locale::Spanish => " LANZADOR ",
            Locale::English => " LAUNCHER ",
        }
    }

    pub const fn installer_button(self) -> &'static str {
        match self.0 {
            Locale::Spanish => " INSTALADOR ",
            Locale::English => " INSTALLER ",
        }
    }

    pub const fn manager_button(self) -> &'static str {
        match self.0 {
            Locale::Spanish => " GESTOR ",
            Locale::English => " MANAGER ",
        }
    }

    pub const fn help(self) -> [&'static str; 15] {
        match self.0 {
            Locale::Spanish => [
                "Escribe             Buscar aplicaciones (fuzzy)",
                "↑ ↓ ← →             Mover la selección",
                "Enter / doble clic  Abrir aplicación",
                "Tab / Shift+Tab     Cambiar Escritorio / Recientes",
                "PgUp / PgDown       Avanzar por páginas",
                "Home / End          Primera / última aplicación",
                "Rueda / PgUp / PgDn Desplazar la vista",
                "F2 / F3 / F4        Expandir sistema / entorno / PC",
                "Ctrl+↑ / Ctrl+↓     Desplazar la vista",
                "INSTALADOR / GESTOR   Ejecutar herramientas Lapi",
                "F5                  Volver a leer aplicaciones",
                "Ctrl+L / Ctrl+U     Limpiar búsqueda",
                "Esc                 Limpiar búsqueda; después salir",
                "Ctrl+C              Salir",
                "F1 / Esc / Enter    Cerrar esta ayuda",
            ],
            Locale::English => [
                "Type                Search applications (fuzzy)",
                "↑ ↓ ← →             Move selection",
                "Enter / double click Open application",
                "Tab / Shift+Tab     Switch Desktop / Recent",
                "PgUp / PgDown       Move through pages",
                "Home / End          First / last application",
                "Wheel / PgUp / PgDn Scroll the view",
                "F2 / F3 / F4        Expand system / desktop / PC",
                "Ctrl+↑ / Ctrl+↓     Scroll the view",
                "INSTALLER / MANAGER Run Lapi tools",
                "F5                  Reload applications",
                "Ctrl+L / Ctrl+U     Clear search",
                "Esc                 Clear search; then quit",
                "Ctrl+C              Quit",
                "F1 / Esc / Enter    Close this help",
            ],
        }
    }

    pub fn skipped_entries(self, count: usize) -> String {
        match self.0 {
            Locale::Spanish => format!("Se omitieron {count} accesos inválidos"),
            Locale::English => format!("Skipped {count} invalid entries"),
        }
    }

    pub const fn ready(self) -> &'static str {
        match self.0 {
            Locale::Spanish => "Listo para abrir aplicaciones",
            Locale::English => "Ready to open applications",
        }
    }

    pub const fn applications_updated(self) -> &'static str {
        match self.0 {
            Locale::Spanish => "Aplicaciones actualizadas",
            Locale::English => "Applications updated",
        }
    }

    pub fn program_missing(self, program: &str) -> String {
        match self.0 {
            Locale::Spanish => {
                format!("No se encontró {program} en PATH; Lapi Launcher sigue activo")
            }
            Locale::English => {
                format!("{program} was not found in PATH; Lapi Launcher remains active")
            }
        }
    }

    pub fn opening(self, name: &str) -> String {
        match self.0 {
            Locale::Spanish => format!("Abriendo {name}"),
            Locale::English => format!("Opening {name}"),
        }
    }

    pub fn cannot_open(self, name: &str) -> String {
        match self.0 {
            Locale::Spanish => format!("No se pudo abrir {name}"),
            Locale::English => format!("Could not open {name}"),
        }
    }

    pub const fn configured_logo_unavailable(self) -> &'static str {
        match self.0 {
            Locale::Spanish => "No se pudo cargar el logo configurado",
            Locale::English => "Could not load the configured logo",
        }
    }

    pub const fn invalid_history(self) -> &'static str {
        match self.0 {
            Locale::Spanish => "El historial de aplicaciones no es válido",
            Locale::English => "The application history is invalid",
        }
    }

    pub const fn cannot_read_history(self) -> &'static str {
        match self.0 {
            Locale::Spanish => "No se pudo leer el historial",
            Locale::English => "Could not read the application history",
        }
    }

    pub const fn invalid_history_path(self) -> &'static str {
        match self.0 {
            Locale::Spanish => "Ruta de historial inválida",
            Locale::English => "Invalid history path",
        }
    }

    pub const fn cannot_create_history_directory(self) -> &'static str {
        match self.0 {
            Locale::Spanish => "No se pudo crear el directorio del historial",
            Locale::English => "Could not create the history directory",
        }
    }

    pub const fn cannot_create_history_file(self) -> &'static str {
        match self.0 {
            Locale::Spanish => "No se pudo crear el archivo temporal del historial",
            Locale::English => "Could not create the temporary history file",
        }
    }

    pub const fn cannot_save_history(self) -> &'static str {
        match self.0 {
            Locale::Spanish => "No se pudo guardar el historial",
            Locale::English => "Could not save the application history",
        }
    }

    pub fn child_failed(self, name: &str, status: impl std::fmt::Display) -> String {
        match self.0 {
            Locale::Spanish => format!("{name} terminó con {status}"),
            Locale::English => format!("{name} exited with {status}"),
        }
    }

    pub const fn unavailable(self) -> &'static str {
        match self.0 {
            Locale::Spanish => "No disponible",
            Locale::English => "Unavailable",
        }
    }

    pub const fn operating_system(self) -> &'static str {
        if self.is_spanish() {
            "Sistema operativo"
        } else {
            "Operating system"
        }
    }
    pub const fn desktop_environment(self) -> &'static str {
        if self.is_spanish() {
            "Entorno de escritorio"
        } else {
            "Desktop environment"
        }
    }
    pub const fn model(self) -> &'static str {
        if self.is_spanish() { "Modelo" } else { "Model" }
    }
    pub const fn threads(self) -> &'static str {
        if self.is_spanish() {
            "Hilos"
        } else {
            "Threads"
        }
    }
    pub const fn total_memory(self) -> &'static str {
        if self.is_spanish() {
            "RAM total"
        } else {
            "Total RAM"
        }
    }
    pub const fn architecture(self) -> &'static str {
        if self.is_spanish() {
            "Arquitectura"
        } else {
            "Architecture"
        }
    }
    pub const fn uptime_label(self) -> &'static str {
        if self.is_spanish() {
            "Tiempo activo"
        } else {
            "Uptime"
        }
    }
    pub const fn session(self) -> &'static str {
        if self.is_spanish() {
            "Sesión"
        } else {
            "Session"
        }
    }
    pub const fn protocol(self) -> &'static str {
        if self.is_spanish() {
            "Protocolo"
        } else {
            "Protocol"
        }
    }
    pub const fn drive(self) -> &'static str {
        if self.is_spanish() { "Unidad" } else { "Drive" }
    }
    pub const fn used_total(self) -> &'static str {
        if self.is_spanish() {
            "Usado / total"
        } else {
            "Used / total"
        }
    }
    pub const fn available(self) -> &'static str {
        if self.is_spanish() {
            "Disponible"
        } else {
            "Available"
        }
    }
    pub const fn disk(self) -> &'static str {
        if self.is_spanish() { "Disco" } else { "Disk" }
    }
    pub const fn disk_personal(self) -> &'static str {
        if self.is_spanish() {
            "Disco personal"
        } else {
            "Personal disk"
        }
    }
    pub const fn disk_system(self) -> &'static str {
        if self.is_spanish() {
            "Disco del sistema"
        } else {
            "System disk"
        }
    }
    pub const fn model_unavailable(self) -> &'static str {
        if self.is_spanish() {
            "Modelo no disponible"
        } else {
            "Model unavailable"
        }
    }
    pub const fn capacity_unavailable(self) -> &'static str {
        if self.is_spanish() {
            "capacidad no disponible"
        } else {
            "capacity unavailable"
        }
    }
    pub const fn type_unavailable(self) -> &'static str {
        if self.is_spanish() {
            "tipo no disponible"
        } else {
            "type unavailable"
        }
    }

    pub fn drive_number(self, index: usize) -> String {
        format!("{} {}", self.drive(), index + 1)
    }
    pub fn uptime(self, hours: u64, minutes: u64) -> String {
        format!("{hours} h {minutes} min")
    }
    pub fn physical_capacity(self, bytes: u64) -> String {
        if bytes >= 1_000_000_000_000 {
            if self.is_spanish() {
                format!("{:.2} TB físicos", bytes as f64 / 1_000_000_000_000.0)
            } else {
                format!("{:.2} TB physical", bytes as f64 / 1_000_000_000_000.0)
            }
        } else if self.is_spanish() {
            format!("{:.1} GB físicos", bytes as f64 / 1_000_000_000.0)
        } else {
            format!("{:.1} GB physical", bytes as f64 / 1_000_000_000.0)
        }
    }
}

fn locale_from_environment() -> Locale {
    let mut locales: Vec<String> = env::var("LANGUAGE")
        .unwrap_or_default()
        .split(':')
        .map(str::to_owned)
        .collect();
    locales.extend(
        ["LC_ALL", "LC_MESSAGES", "LANG"]
            .into_iter()
            .filter_map(|name| env::var(name).ok()),
    );
    if locales.iter().any(|locale| locale.starts_with("es")) {
        Locale::Spanish
    } else {
        Locale::English
    }
}

#[cfg(test)]
mod tests {
    use super::{Language, Translator};

    #[test]
    fn explicit_language_overrides_the_environment() {
        assert_eq!(Translator::new(Language::Spanish).search(), " Buscar ");
        assert_eq!(Translator::new(Language::English).search(), " Search ");
        assert_eq!(
            Translator::new(Language::English).operating_system(),
            "Operating system"
        );
    }
}
