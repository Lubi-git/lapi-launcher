use ratatui::{
    Frame,
    layout::{Constraint, Layout, Margin, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
};

use crate::{
    app::{App, Hit, LapiProgram, Section, Target},
    config::{LogoSource, RgbColor},
    icons::{Assets, Source},
};

pub const BACKGROUND: Color = Color::Reset;
const FOREGROUND: Color = Color::Reset;
const MUTED: Color = Color::Rgb(127, 132, 156);
const ACCENT: Color = Color::Rgb(203, 166, 247);
const CYAN: Color = Color::Rgb(137, 220, 235);
const BORDER: Color = Color::Rgb(69, 71, 90);
const ERROR: Color = Color::Rgb(243, 139, 168);
const SEARCH_HEIGHT: u16 = 4;
const GRID_HEIGHT: u16 = 8;
const GROUP_GAP: u16 = 1;

pub fn draw(frame: &mut Frame, app: &mut App, assets: &mut Option<Assets>) {
    let screen = frame.area();
    frame.render_widget(Block::new().fg(FOREGROUND), screen);
    app.hits.clear();
    app.system_rows = [0; 3];
    if screen.width < 40 || screen.height < 20 {
        frame.render_widget(
            Paragraph::new(app.text.compact_terminal())
                .fg(ACCENT)
                .wrap(Wrap { trim: true }),
            screen,
        );
        return;
    }
    let width = screen.width.saturating_sub(4).min(104);
    let area = Rect::new(
        screen.x + (screen.width - width) / 2,
        screen.y,
        width,
        screen.height,
    );
    if app.searching() {
        draw_search(frame, app, assets, area);
    } else {
        draw_home(frame, app, assets, area);
    }
    if app.help {
        render_help(frame, screen, app.text);
    }
    if let Some(index) = app.context_menu {
        render_context_menu(frame, app, assets, screen, index);
    }
}

fn render_context_menu(
    frame: &mut Frame,
    app: &mut App,
    assets: &mut Option<Assets>,
    screen: Rect,
    index: usize,
) {
    let width = screen.width.saturating_sub(4).min(56);
    let area = Rect::new(
        screen.x + (screen.width - width) / 2,
        screen.y + screen.height.saturating_sub(10) / 2,
        width,
        10,
    );
    let application = &app.apps[index];
    frame.render_widget(Clear, area);
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .title(app.text.application_actions())
        .border_style(Style::new().fg(ACCENT));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let icon = Rect::new(inner.x + 2, inner.y + 1, 7, 4);
    if let (Some(assets), Some(name)) = (assets.as_mut(), application.entry.icon()) {
        assets.render(
            frame,
            format!("context:{}", application.id),
            Source::Icon(name.into()),
            icon,
        );
    }
    frame.render_widget(
        Paragraph::new(application.name.as_str())
            .fg(ACCENT)
            .bold()
            .wrap(Wrap { trim: true }),
        Rect::new(inner.x + 11, inner.y + 1, inner.width.saturating_sub(13), 2),
    );
    let action = if application.on_desktop {
        app.text.remove_from_desktop()
    } else {
        app.text.pin_to_desktop()
    };
    let action_area = Rect::new(inner.x + 2, inner.y + 5, inner.width.saturating_sub(4), 1);
    frame.render_widget(
        Paragraph::new(action).centered().style(
            Style::new()
                .fg(ACCENT)
                .add_modifier(Modifier::REVERSED | Modifier::BOLD),
        ),
        action_area,
    );
    frame.render_widget(
        Paragraph::new(app.text.context_hint()).centered().fg(MUTED),
        Rect::new(inner.x + 1, inner.y + 7, inner.width.saturating_sub(2), 1),
    );
    app.hits.push(Hit {
        area,
        target: Target::ContextClose,
    });
    app.hits.push(Hit {
        area: action_area,
        target: Target::ContextAction,
    });
}

fn draw_home(frame: &mut Frame, app: &mut App, assets: &mut Option<Assets>, area: Rect) {
    let viewport = Rect::new(area.x, area.y, area.width, area.height.saturating_sub(2));
    let footer = Rect::new(area.x, viewport.bottom(), area.width, 2);
    let content_height = home_content_height(app);
    app.set_content_view(content_height, viewport.height);
    let scroll = app.content_scroll;
    let mut document_y = 0;

    if let Some(header) = document_area(viewport, scroll, document_y, 2) {
        render_header(frame, app.text, header);
    }
    document_y += 2;
    if let Some(logo) = document_area(viewport, scroll, document_y, app.config.logo.height) {
        render_logo(frame, app, assets, logo);
    }
    document_y += app.config.logo.height;
    document_y = render_system_document(frame, app, viewport, scroll, document_y);
    if let Some(tools) = document_area(viewport, scroll, document_y, 1) {
        render_lapi_buttons(frame, app, tools);
    }
    document_y += 1 + GROUP_GAP;
    if let Some(search) = document_area(viewport, scroll, document_y, SEARCH_HEIGHT) {
        render_search(frame, app, search);
    }
    document_y += SEARCH_HEIGHT;
    if let Some(desktop) = document_area(viewport, scroll, document_y, GRID_HEIGHT) {
        render_grid(frame, app, assets, Section::Desktop, desktop);
    }
    document_y += GRID_HEIGHT + GROUP_GAP;
    if let Some(recent) = document_area(viewport, scroll, document_y, GRID_HEIGHT) {
        render_grid(frame, app, assets, Section::Recent, recent);
    }
    render_footer(frame, app, footer, true);
}

fn draw_search(frame: &mut Frame, app: &mut App, assets: &mut Option<Assets>, area: Rect) {
    let details = system_details_height(app);
    let logo_height = compact_logo_height(area.height, app.config.logo.height, details);
    let system_height = (4 + details).min(area.height.saturating_sub(logo_height + 14));
    let [header, logo, system, tools, search, results, footer] = Layout::vertical([
        Constraint::Length(2),
        Constraint::Length(logo_height),
        Constraint::Length(system_height + 1),
        Constraint::Length(1),
        Constraint::Length(SEARCH_HEIGHT),
        Constraint::Min(4),
        Constraint::Length(2),
    ])
    .areas(area);
    render_header(frame, app.text, header);
    render_logo(frame, app, assets, logo);
    render_system_compact(frame, app, system);
    render_lapi_buttons(frame, app, tools);
    render_search(frame, app, search);
    render_results(frame, app, assets, results);
    render_footer(frame, app, footer, false);
}

fn render_header(frame: &mut Frame, text: crate::i18n::Translator, area: Rect) {
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" LAPI ", Style::new().bg(ACCENT).fg(BACKGROUND).bold()),
            Span::styled(text.tagline(), Style::new().fg(MUTED)),
        ])),
        area,
    );
}

fn render_footer(frame: &mut Frame, app: &App, area: Rect, scrollable: bool) {
    let [keys, status] =
        Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).areas(area);
    frame.render_widget(
        Paragraph::new(if scrollable {
            app.text.footer_home()
        } else if app.searching() {
            app.text.footer_search()
        } else {
            app.text.footer_default()
        })
        .fg(MUTED),
        keys,
    );
    frame.render_widget(
        Paragraph::new(app.status.as_str()).fg(if app.status_error { ERROR } else { CYAN }),
        status,
    );
}

fn compact_logo_height(terminal_height: u16, configured_height: u16, details: u16) -> u16 {
    if details == 0 || terminal_height >= 31 + configured_height {
        configured_height.min(terminal_height.saturating_sub(16))
    } else {
        1
    }
}

fn system_details_height(app: &App) -> u16 {
    app.system
        .sections
        .iter()
        .enumerate()
        .filter(|(index, _)| app.expanded[*index])
        .map(|(_, section)| section.details.len() as u16)
        .sum()
}

fn home_content_height(app: &App) -> u16 {
    2 + app.config.logo.height
        + 4
        + system_details_height(app)
        + 1
        + GROUP_GAP
        + SEARCH_HEIGHT
        + GRID_HEIGHT
        + GROUP_GAP
        + GRID_HEIGHT
}

fn document_area(viewport: Rect, scroll: u16, document_y: u16, height: u16) -> Option<Rect> {
    let visible_y = document_y.checked_sub(scroll)?;
    (visible_y.saturating_add(height) <= viewport.height)
        .then(|| Rect::new(viewport.x, viewport.y + visible_y, viewport.width, height))
}

fn render_lapi_buttons(frame: &mut Frame, app: &mut App, area: Rect) {
    let [launcher, installer, manager] = Layout::horizontal([
        Constraint::Percentage(33),
        Constraint::Percentage(34),
        Constraint::Percentage(33),
    ])
    .areas(area);
    let colors = &app.config.buttons;
    for (button, label, background, foreground, target) in [
        (
            launcher,
            app.text.launcher_button(),
            terminal_color(colors.launcher_background),
            terminal_color(colors.launcher_foreground),
            None,
        ),
        (
            installer,
            app.text.installer_button(),
            terminal_color(colors.installer_background),
            terminal_color(colors.installer_manager_foreground),
            Some(LapiProgram::Installer),
        ),
        (
            manager,
            app.text.manager_button(),
            terminal_color(colors.manager_background),
            terminal_color(colors.installer_manager_foreground),
            Some(LapiProgram::Manager),
        ),
    ] {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                label,
                Style::new().bg(background).fg(foreground).bold(),
            )))
            .centered(),
            button,
        );
        if let Some(program) = target {
            app.hits.push(Hit {
                area: button,
                target: Target::Program(program),
            });
        }
    }
}

fn terminal_color(color: RgbColor) -> Color {
    let (red, green, blue) = color.components();
    Color::Rgb(red, green, blue)
}

fn render_logo(frame: &mut Frame, app: &App, assets: &mut Option<Assets>, area: Rect) {
    if area.height <= 1 {
        frame.render_widget(Paragraph::new("l a p i").centered().fg(ACCENT).bold(), area);
        return;
    }
    let width = app.config.logo.width.min(area.width);
    let logo = Rect::new(
        area.x + (area.width - width) / 2,
        area.y,
        width,
        area.height,
    );
    let inner = logo;
    let source =
        app.config
            .logo
            .path
            .clone()
            .map(Source::File)
            .or_else(|| match app.config.logo.source {
                LogoSource::Builtin => None,
                LogoSource::Os => Some(Source::Icon(app.system.os_icon.clone())),
                LogoSource::Desktop => Some(Source::Icon(app.system.desktop_icon.clone())),
            });
    if let (Some(assets), Some(source)) = (assets.as_mut(), source)
        && assets.render(frame, "logo".into(), source, inner)
    {
        return;
    }
    let wordmark = if inner.height >= 3 {
        vec![
            Line::from("╷  ╭─╮ ╭─╮ ╷"),
            Line::from("│  ├─┤ ├─╯ │"),
            Line::from("╰─ ╵ ╵ ╵   ╵"),
        ]
    } else {
        vec![Line::from("L A P I")]
    };
    let height = wordmark.len() as u16;
    let text_area = Rect::new(
        inner.x,
        inner.y + inner.height.saturating_sub(height) / 2,
        inner.width,
        height.min(inner.height),
    );
    frame.render_widget(Paragraph::new(wordmark).centered().fg(ACCENT), text_area);
}

fn render_system_document(
    frame: &mut Frame,
    app: &mut App,
    viewport: Rect,
    scroll: u16,
    mut document_y: u16,
) -> u16 {
    if let Some(title) = document_area(viewport, scroll, document_y, 1) {
        frame.render_widget(
            Paragraph::new(app.text.system_information()).fg(MUTED),
            title,
        );
    }
    document_y += 1;
    if !app.system_focus.is_some_and(|index| app.expanded[index]) {
        app.system_focus = app.expanded.iter().rposition(|expanded| *expanded);
    }
    for (index, section) in app.system.sections.iter().enumerate() {
        app.system_rows[index] = if app.expanded[index] {
            section.details.len()
        } else {
            0
        };
        app.system_offsets[index] = 0;
        if let Some(header) = document_area(viewport, scroll, document_y, 1) {
            let branch = if index == 2 { "└" } else { "├" };
            let indicator = if app.expanded[index] { "▾" } else { "▸" };
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled(format!("{branch} "), Style::new().fg(BORDER)),
                    Span::styled(
                        format!("{indicator} {} ", section.label),
                        Style::new().fg(CYAN),
                    ),
                    Span::styled(
                        format!("› {}", section.summary),
                        Style::new().fg(FOREGROUND),
                    ),
                ])),
                header,
            );
            app.hits.push(Hit {
                area: header,
                target: Target::System(index),
            });
        }
        document_y += 1;
        if app.expanded[index] {
            for (label, value) in &section.details {
                if let Some(detail) = document_area(viewport, scroll, document_y, 1) {
                    frame.render_widget(
                        Paragraph::new(format!("│    {label}: {value}")).fg(MUTED),
                        detail,
                    );
                    app.hits.push(Hit {
                        area: detail,
                        target: Target::SystemDetails(index),
                    });
                }
                document_y += 1;
            }
        }
    }
    document_y
}

fn render_system_compact(frame: &mut Frame, app: &mut App, area: Rect) {
    if area.height < 4 {
        return;
    }
    let mut remaining = usize::from(area.height - 4);
    while remaining > 0 {
        let mut allocated = false;
        for (index, section) in app.system.sections.iter().enumerate() {
            if remaining > 0
                && app.expanded[index]
                && app.system_rows[index] < section.details.len()
            {
                app.system_rows[index] += 1;
                remaining -= 1;
                allocated = true;
            }
        }
        if !allocated {
            break;
        }
    }
    if !app.system_focus.is_some_and(|index| app.expanded[index]) {
        app.system_focus = app.expanded.iter().rposition(|expanded| *expanded);
    }
    let scrollable = app
        .system
        .sections
        .iter()
        .enumerate()
        .any(|(index, section)| {
            app.expanded[index] && section.details.len() > app.system_rows[index]
        });
    frame.render_widget(
        Paragraph::new(if scrollable {
            app.text.system_information_scroll()
        } else {
            app.text.system_information()
        })
        .fg(MUTED),
        Rect::new(area.x, area.y, area.width, 1),
    );
    let mut row = area.y + 1;
    for (index, section) in app.system.sections.iter().enumerate() {
        let header = Rect::new(area.x, row, area.width, 1);
        let capacity = app.system_rows[index];
        let maximum = section.details.len().saturating_sub(capacity);
        app.system_offsets[index] = if app.expanded[index] {
            app.system_offsets[index].min(maximum)
        } else {
            0
        };
        let offset = app.system_offsets[index];
        let mut title_area = header;
        if app.expanded[index] && maximum > 0 {
            let position = format!(
                "{}–{}/{} {}{}",
                offset + 1,
                offset + capacity,
                section.details.len(),
                if offset > 0 { "↑" } else { "" },
                if offset < maximum { "↓" } else { "" },
            );
            let width = Line::from(position.as_str())
                .width()
                .min(usize::from(header.width)) as u16;
            let position_area = Rect::new(header.right() - width, header.y, width, 1);
            title_area.width = title_area.width.saturating_sub(width + 1);
            frame.render_widget(
                Paragraph::new(position)
                    .right_aligned()
                    .fg(if app.system_focus == Some(index) {
                        ACCENT
                    } else {
                        MUTED
                    }),
                position_area,
            );
        }
        let branch = if index == 2 { "└" } else { "├" };
        let indicator = if app.expanded[index] { "▾" } else { "▸" };
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(format!("{branch} "), Style::new().fg(BORDER)),
                Span::styled(
                    format!("{indicator} {} ", section.label),
                    Style::new().fg(CYAN),
                ),
                Span::styled(
                    format!("› {}", section.summary),
                    Style::new().fg(FOREGROUND),
                ),
            ])),
            title_area,
        );
        app.hits.push(Hit {
            area: header,
            target: Target::System(index),
        });
        row += 1;
        if app.expanded[index] {
            if capacity > 0 {
                app.hits.push(Hit {
                    area: Rect::new(area.x, row, area.width, capacity as u16),
                    target: Target::SystemDetails(index),
                });
            }
            for (label, value) in section.details.iter().skip(offset).take(capacity) {
                frame.render_widget(
                    Paragraph::new(format!("│    {label}: {value}")).fg(MUTED),
                    Rect::new(area.x, row, area.width, 1),
                );
                row += 1;
            }
        }
    }
}

fn render_search(frame: &mut Frame, app: &mut App, area: Rect) {
    let area = Rect::new(area.x, area.y, area.width, area.height.min(3));
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(ACCENT))
        .title(Line::from(vec![
            Span::styled(app.text.search(), Style::new().fg(ACCENT)),
            Span::styled(
                format!("{} / {} ", app.filtered.len(), app.apps.len()),
                Style::new().fg(MUTED),
            ),
        ]));
    let inner = block.inner(area);
    let line = if app.query.is_empty() {
        Line::from(Span::styled(
            app.text.search_placeholder(),
            Style::new().fg(MUTED),
        ))
    } else {
        Line::from(format!(" {}", app.query))
    };
    let scroll = line
        .width()
        .saturating_sub(usize::from(inner.width.saturating_sub(1)))
        .min(u16::MAX as usize) as u16;
    frame.render_widget(Paragraph::new(line).block(block).scroll((0, scroll)), area);
    app.hits.push(Hit {
        area,
        target: Target::Search,
    });
}

fn render_grid(
    frame: &mut Frame,
    app: &mut App,
    assets: &mut Option<Assets>,
    section: Section,
    area: Rect,
) {
    if area.is_empty() {
        return;
    }
    let active = app.section == section;
    let label = if section == Section::Desktop {
        app.text.desktop()
    } else {
        app.text.recent()
    };
    let indices = app.indices(section).to_vec();
    let title = Line::from(vec![
        Span::styled(if active { "▸ " } else { "  " }, Style::new().fg(ACCENT)),
        Span::styled(
            format!("{label} "),
            Style::new()
                .fg(if active { ACCENT } else { FOREGROUND })
                .bold(),
        ),
        Span::styled(format!("{}", indices.len()), Style::new().fg(MUTED)),
    ]);
    let header = Rect::new(area.x, area.y, area.width, 1);
    app.hits.push(Hit {
        area,
        target: Target::Section(section),
    });
    let body = Rect::new(
        area.x,
        area.y + 1,
        area.width,
        area.height.saturating_sub(1),
    );
    if body.is_empty() {
        return;
    }
    if indices.is_empty() {
        frame.render_widget(Paragraph::new(title), header);
        let text = if !app.query.is_empty() {
            app.text.no_matches_clear()
        } else if section == Section::Recent {
            app.text.no_recent()
        } else {
            app.text.no_desktop()
        };
        frame.render_widget(
            Paragraph::new(text).fg(MUTED).wrap(Wrap { trim: false }),
            body,
        );
        return;
    }
    let compact = body.height < 7;
    let tile_height = if compact { body.height.min(3) } else { 7 };
    let column_count = (body.width / 18).max(1);
    let row_count = (body.height / tile_height).max(1);
    let tile_width = body.width / column_count;
    let section_index = section.index();
    let columns = usize::from(column_count);
    let rows = usize::from(row_count);
    app.columns[section_index] = columns;
    app.rows[section_index] = rows;
    app.selected[section_index] = app.selected[section_index].min(indices.len() - 1);
    let selected_row = app.selected[section_index] / columns;
    let offset = &mut app.offsets[section_index];
    if selected_row < *offset {
        *offset = selected_row;
    }
    if selected_row >= *offset + rows {
        *offset = selected_row + 1 - rows;
    }
    let start = *offset * columns;
    let mut title_area = header;
    if indices.len() > rows * columns {
        let page = format!(
            "{}–{} / {} ",
            start + 1,
            (start + rows * columns).min(indices.len()),
            indices.len()
        );
        let width = Line::from(page.as_str())
            .width()
            .min(usize::from(header.width)) as u16;
        let page_area = Rect::new(header.right() - width, header.y, width, 1);
        title_area.width = title_area.width.saturating_sub(width);
        frame.render_widget(Paragraph::new(page).right_aligned().fg(MUTED), page_area);
    }
    frame.render_widget(Paragraph::new(title), title_area);
    for (position, app_index) in indices.iter().enumerate().skip(start).take(rows * columns) {
        let relative = position - start;
        let tile = Rect::new(
            body.x + (relative % columns) as u16 * tile_width,
            body.y + (relative / columns) as u16 * tile_height,
            tile_width.saturating_sub(1),
            tile_height,
        );
        let selected = active && position == app.selected[section_index];
        let application = &app.apps[*app_index];
        let color = if selected { ACCENT } else { BORDER };
        if compact {
            let text = vec![Line::from(Span::styled(
                format!("{} {}", if selected { "▸" } else { " " }, application.name),
                Style::new()
                    .fg(if selected { ACCENT } else { FOREGROUND })
                    .bold(),
            ))];
            frame.render_widget(Paragraph::new(text).bg(BACKGROUND), tile);
        } else {
            let icon_width = tile.width.min(12);
            let icon_box = Rect::new(
                tile.x + (tile.width - icon_width) / 2,
                tile.y,
                icon_width,
                5,
            );
            let border = Block::bordered()
                .border_type(BorderType::Rounded)
                .border_style(Style::new().fg(color));
            let image_area = border.inner(icon_box);
            frame.render_widget(border, icon_box);
            let rendered =
                if let (Some(assets), Some(icon)) = (assets.as_mut(), application.entry.icon()) {
                    assets.render(
                        frame,
                        format!("{label}:{}", application.id),
                        Source::Icon(icon.to_owned()),
                        image_area,
                    )
                } else {
                    false
                };
            if !rendered {
                let initials: String = application
                    .name
                    .chars()
                    .filter(|character| !character.is_whitespace())
                    .take(2)
                    .flat_map(char::to_uppercase)
                    .collect();
                let initials_area = Rect::new(
                    image_area.x,
                    image_area.y + image_area.height / 2,
                    image_area.width,
                    1,
                );
                frame.render_widget(
                    Paragraph::new(initials)
                        .centered()
                        .fg(if selected { ACCENT } else { CYAN })
                        .bold(),
                    initials_area,
                );
            }
            frame.render_widget(
                Paragraph::new(application.name.as_str())
                    .centered()
                    .fg(if selected { ACCENT } else { FOREGROUND })
                    .add_modifier(Modifier::BOLD),
                Rect::new(tile.x, tile.y + 5, tile.width, 1),
            );
        }
        app.hits.push(Hit {
            area: tile,
            target: Target::App(section, position),
        });
    }
}

fn render_results(frame: &mut Frame, app: &mut App, assets: &mut Option<Assets>, area: Rect) {
    if area.is_empty() {
        return;
    }
    if app.filtered.is_empty() {
        frame.render_widget(Paragraph::new(app.text.no_matches_back()).fg(MUTED), area);
        return;
    }
    let section = Section::Results.index();
    let row_height = area.height.min(3);
    let visible = usize::from((area.height / row_height).max(1));
    app.columns[section] = 1;
    app.rows[section] = visible;
    app.selected[section] = app.selected[section].min(app.filtered.len() - 1);
    let selected = app.selected[section];
    let offset = &mut app.offsets[section];
    if selected < *offset {
        *offset = selected;
    }
    if selected >= *offset + visible {
        *offset = selected + 1 - visible;
    }
    for (position, index) in app.filtered.iter().enumerate().skip(*offset).take(visible) {
        let row = Rect::new(
            area.x,
            area.y + (position - *offset) as u16 * row_height,
            area.width,
            row_height,
        );
        let application = &app.apps[*index];
        let selected = position == selected;
        let color = if selected { ACCENT } else { FOREGROUND };
        let icon_area = Rect::new(row.x + 3, row.y, 6, row.height);
        frame.render_widget(
            Paragraph::new(if selected { "▸" } else { " " }).fg(ACCENT),
            Rect::new(row.x, row.y + row.height / 2, 1, 1),
        );
        let rendered =
            if let (Some(assets), Some(icon)) = (assets.as_mut(), application.entry.icon()) {
                assets.render(
                    frame,
                    format!("result:{}", application.id),
                    Source::Icon(icon.into()),
                    icon_area,
                )
            } else {
                false
            };
        if !rendered {
            let initials: String = application
                .name
                .chars()
                .filter(|character| !character.is_whitespace())
                .take(2)
                .flat_map(char::to_uppercase)
                .collect();
            frame.render_widget(
                Paragraph::new(initials).centered().fg(CYAN),
                Rect::new(
                    icon_area.x,
                    icon_area.y + icon_area.height / 2,
                    icon_area.width,
                    1,
                ),
            );
        }
        frame.render_widget(
            Paragraph::new(application.name.as_str()).fg(color).bold(),
            Rect::new(
                row.x + 12,
                row.y + row.height / 2,
                row.width.saturating_sub(12),
                1,
            ),
        );
        app.hits.push(Hit {
            area: row,
            target: Target::App(Section::Results, position),
        });
    }
}

fn render_help(frame: &mut Frame, screen: Rect, text: crate::i18n::Translator) {
    let width = screen.width.saturating_sub(4).min(70);
    let height = screen.height.saturating_sub(2).min(17);
    let area = Rect::new(
        screen.x + (screen.width - width) / 2,
        screen.y + (screen.height - height) / 2,
        width,
        height,
    );
    frame.render_widget(Clear, area);
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .borders(Borders::ALL)
        .title(text.help_title())
        .border_style(Style::new().fg(ACCENT));
    let inner = block.inner(area).inner(Margin::new(1, 0));
    frame.render_widget(block, area);
    let text = text.help().join("\n");
    frame.render_widget(
        Paragraph::new(text)
            .fg(FOREGROUND)
            .wrap(Wrap { trim: false }),
        inner,
    );
}

#[cfg(test)]
mod tests {
    use ratatui::{Terminal, backend::TestBackend, layout::Rect};

    use super::{compact_logo_height, render_logo};

    #[test]
    fn keeps_a_configured_logo_visible_in_compact_terminals() {
        assert_eq!(compact_logo_height(24, 7, 0), 7);
        assert_eq!(compact_logo_height(20, 7, 0), 4);
        assert_eq!(compact_logo_height(40, 12, 0), 12);
        assert_eq!(compact_logo_height(24, 7, 1), 1);
        assert_eq!(compact_logo_height(40, 7, 1), 7);
    }

    #[test]
    fn logo_has_no_outer_frame() {
        let temporary = crate::test_support::TempDir::new();
        let app = crate::test_support::app(&[], temporary.path.join("history.toml"));
        let mut assets = None;
        let logo = Rect::new(3, 1, 14, 7);
        let mut terminal = Terminal::new(TestBackend::new(20, 10)).unwrap();
        terminal
            .draw(|frame| render_logo(frame, &app, &mut assets, logo))
            .unwrap();
        assert_eq!(terminal.backend().buffer()[(logo.x, logo.y)].symbol(), " ");
        assert_eq!(
            terminal.backend().buffer()[(logo.right() - 1, logo.y)].symbol(),
            " "
        );
    }
}
