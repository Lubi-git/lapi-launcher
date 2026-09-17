mod app;
mod config;
mod desktop;
mod graphics;
mod history;
mod i18n;
mod icons;
mod storage;
mod system;
#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;
mod ui;

use std::{
    env,
    io::{self, IsTerminal},
    os::unix::process::CommandExt,
    process::Command,
    time::Duration,
};

use anyhow::{Result, bail, ensure};
use crossterm::{
    event::{
        self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
    },
    execute,
};
use ratatui_image::picker::{Picker, ProtocolType};

use crate::{
    app::App,
    config::{Config, ImageProtocol},
    history::History,
    i18n::Translator,
    icons::Assets,
};

fn main() -> Result<()> {
    let text = Translator::from_environment();
    let mut list = false;
    let mut no_images = false;
    for argument in env::args_os().skip(1) {
        match argument.to_str() {
            Some("--help" | "-h") => {
                println!("{}", text.command_help());
                return Ok(());
            }
            Some("--version" | "-V") => {
                println!("lapi-launcher {}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            Some("--print-default-config") => {
                print!("{}", config::EXAMPLE);
                return Ok(());
            }
            Some("--config") => bail!(text.config_option_message()),
            Some("--list") => list = true,
            Some("--no-images") => no_images = true,
            _ => bail!(text.unknown_option(&argument.to_string_lossy())),
        }
    }
    let mut config = Config::load_standard()?;
    config.images.enabled &= !no_images;
    let text = Translator::new(config.interface.language);
    let catalog = desktop::discover(&config)?;
    if list {
        for application in &catalog.apps {
            println!("{}", application.name);
        }
        return Ok(());
    }
    ensure!(
        io::stdin().is_terminal() && io::stdout().is_terminal(),
        text.interactive_terminal_required()
    );
    let history_path = History::default_path()?;
    let history = History::load(&history_path, text);
    let warning = history.as_ref().err().map(|error| format!("{error:#}"));
    let mut app = App::new(config, catalog, history.unwrap_or_default(), history_path);
    if let Some(warning) = warning {
        app.error(warning);
    }
    let mut terminal = ratatui::try_init()?;
    let guard = TerminalGuard;
    execute!(io::stdout(), EnableMouseCapture, EnableBracketedPaste)?;
    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = execute!(io::stdout(), DisableMouseCapture, DisableBracketedPaste);
        ratatui::restore();
        previous_hook(info);
    }));
    let mut assets = if app.config.images.enabled {
        let legacy = matches!(app.config.images.protocol, ImageProtocol::KittyLegacy)
            || matches!(app.config.images.protocol, ImageProtocol::Auto)
                && env::var("KONSOLE_VERSION")
                    .is_ok_and(|value| value.parse::<u32>().is_ok_and(|version| version >= 220400))
                && env::var_os("TMUX").is_none()
                && env::var_os("STY").is_none();
        let mut picker = match app.config.images.protocol {
            ImageProtocol::Halfblocks => Picker::halfblocks(),
            _ => Picker::from_query_stdio().unwrap_or_else(|_| Picker::halfblocks()),
        };
        match app.config.images.protocol {
            ImageProtocol::Kitty => picker.set_protocol_type(ProtocolType::Kitty),
            ImageProtocol::Sixel => picker.set_protocol_type(ProtocolType::Sixel),
            ImageProtocol::Iterm2 => picker.set_protocol_type(ProtocolType::Iterm2),
            ImageProtocol::Auto
                if picker.protocol_type() == ProtocolType::Halfblocks
                    && env::var_os("WEZTERM_EXECUTABLE").is_some() =>
            {
                picker.set_protocol_type(ProtocolType::Iterm2)
            }
            _ => {}
        }
        picker.set_background_color(Some([24, 24, 37, 255]));
        Some(if legacy {
            Assets::with_legacy(picker, app.config.images.icon_theme.clone(), true, app.text)
        } else {
            Assets::new(picker, app.config.images.icon_theme.clone(), app.text)
        })
    } else {
        None
    };
    let mut redraw = true;
    while !app.quit {
        if let Some(assets) = &mut assets {
            redraw |= assets.poll();
            if let Some(warning) = assets.warning.take() {
                app.error(warning);
            }
        }
        redraw |= app.reap_children();
        if redraw {
            if let Some(assets) = &mut assets {
                assets.placements.begin();
            }
            terminal.draw(|frame| {
                if let Some(assets) = &mut assets {
                    assets.placements.set_viewport(frame.area());
                }
                ui::draw(frame, &mut app, &mut assets);
            })?;
            if let Some(assets) = &mut assets {
                if app.help {
                    assets.placements.begin();
                }
                assets.placements.flush(&mut io::stdout())?;
            }
            redraw = false;
        }
        if event::poll(Duration::from_millis(50))? {
            app.handle(event::read()?)?;
            redraw = true;
        }
    }
    let next_program = app.next_program;
    drop(terminal);
    drop(guard);
    if let Some(program) = next_program {
        return Err(Command::new(program.command()).exec().into());
    }
    Ok(())
}

struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = execute!(io::stdout(), DisableMouseCapture, DisableBracketedPaste);
        ratatui::restore();
    }
}
