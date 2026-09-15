use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver, Sender},
    thread,
};

use anyhow::{Context, Result};
use image::{DynamicImage, ImageReader, imageops::FilterType};
use ratatui::{
    Frame,
    layout::{Rect, Size},
};
use ratatui_image::{Image, Resize, picker::Picker, protocol::Protocol};

use crate::{
    config,
    graphics::{LegacyImage, Placements},
};

enum Prepared {
    Standard(Protocol),
    Legacy(LegacyImage),
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum Source {
    File(PathBuf),
    Icon(String),
}

struct Request {
    key: String,
    source: Source,
    size: Size,
}

struct Response {
    key: String,
    size: Size,
    protocol: Option<Prepared>,
    error: Option<String>,
}

struct Cached {
    size: Size,
    protocol: Option<Prepared>,
}

pub struct Assets {
    sender: Sender<Request>,
    receiver: Receiver<Response>,
    cache: HashMap<String, Cached>,
    pub warning: Option<String>,
    pub placements: Placements,
}

impl Assets {
    pub fn new(picker: Picker, theme: Option<String>) -> Self {
        Self::with_legacy(picker, theme, false)
    }

    pub fn with_legacy(picker: Picker, theme: Option<String>, legacy: bool) -> Self {
        let (sender, requests) = mpsc::channel::<Request>();
        let (responses, receiver) = mpsc::channel();
        thread::spawn(move || {
            let resolver = IconResolver::new(theme.as_deref());
            let mut decoded: HashMap<Source, Option<DynamicImage>> = HashMap::new();
            for request in requests {
                let image = decoded.entry(request.source.clone()).or_insert_with(|| {
                    let path = match &request.source {
                        Source::File(path) => Some(path.clone()),
                        Source::Icon(name) => resolver.resolve(name),
                    }?;
                    load_image(&path).ok()
                });
                let protocol = image.as_ref().and_then(|image| {
                    if legacy {
                        LegacyImage::new(image, request.size, picker.font_size())
                            .ok()
                            .map(Prepared::Legacy)
                    } else {
                        picker
                            .new_protocol(
                                image.clone(),
                                request.size,
                                Resize::Scale(Some(FilterType::Lanczos3)),
                            )
                            .ok()
                            .map(Prepared::Standard)
                    }
                });
                let error = if protocol.is_none() && matches!(request.source, Source::File(_)) {
                    match &request.source {
                        Source::File(_) => Some("No se pudo cargar el logo configurado".into()),
                        Source::Icon(_) => None,
                    }
                } else {
                    None
                };
                if responses
                    .send(Response {
                        key: request.key,
                        size: request.size,
                        protocol,
                        error,
                    })
                    .is_err()
                {
                    break;
                }
                if decoded.len() > 128 {
                    decoded.clear();
                }
            }
        });
        Self {
            sender,
            receiver,
            cache: HashMap::new(),
            warning: None,
            placements: Placements::default(),
        }
    }

    pub fn poll(&mut self) -> bool {
        let mut changed = false;
        while let Ok(response) = self.receiver.try_recv() {
            if let Some(cached) = self.cache.get_mut(&response.key)
                && cached.size == response.size
            {
                cached.protocol = response.protocol;
                if response.error.is_some() {
                    self.warning = response.error;
                }
                changed = true;
            }
        }
        changed
    }

    pub fn render(&mut self, frame: &mut Frame, key: String, source: Source, area: Rect) -> bool {
        if area.is_empty() {
            return false;
        }
        let key = format!("{key}:{source:?}");
        let size = Size::new(area.width, area.height);
        if self
            .cache
            .get(&key)
            .is_none_or(|cached| cached.size != size)
        {
            if self.cache.len() >= 256 {
                self.cache.clear();
            }
            self.cache.insert(
                key.clone(),
                Cached {
                    size,
                    protocol: None,
                },
            );
            let _ = self.sender.send(Request {
                key: key.clone(),
                source,
                size,
            });
        }
        if let Some(protocol) = self
            .cache
            .get(&key)
            .and_then(|cached| cached.protocol.as_ref())
        {
            let size = match protocol {
                Prepared::Standard(protocol) => protocol.size(),
                Prepared::Legacy(image) => image.size,
            };
            let centered = Rect::new(
                area.x + area.width.saturating_sub(size.width) / 2,
                area.y + area.height.saturating_sub(size.height) / 2,
                size.width.min(area.width),
                size.height.min(area.height),
            );
            match protocol {
                Prepared::Standard(protocol) => frame.render_widget(Image::new(protocol), centered),
                Prepared::Legacy(image) => self.placements.push(image.clone(), centered),
            }
            true
        } else {
            false
        }
    }
}

fn load_image(path: &Path) -> Result<DynamicImage> {
    let mut reader = ImageReader::open(path)?.with_guessed_format()?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(4096);
    limits.max_image_height = Some(4096);
    limits.max_alloc = Some(64 * 1024 * 1024);
    reader.limits(limits);
    let image = reader
        .decode()
        .with_context(|| format!("Imagen inválida: {}", path.display()))?;
    Ok(if image.width() > 1024 || image.height() > 1024 {
        image.resize(1024, 1024, FilterType::Lanczos3)
    } else {
        image
    })
}

struct IconResolver {
    directories: Vec<PathBuf>,
}

impl IconResolver {
    fn new(theme: Option<&str>) -> Self {
        let data_dirs = config::data_dirs().unwrap_or_default();
        let mut roots = vec![config::home().unwrap_or_default().join(".icons")];
        roots.extend(data_dirs.iter().map(|path| path.join("icons")));
        let mut directories = roots.clone();
        let mut themes = Vec::new();
        if let Some(theme) = theme {
            themes.push(theme.to_owned());
        }
        themes.extend([
            "hicolor".to_owned(),
            "breeze".to_owned(),
            "Adwaita".to_owned(),
            "oxygen".to_owned(),
        ]);
        let mut visited = HashSet::new();
        for theme in themes {
            add_theme(&theme, &roots, &mut visited, &mut directories);
        }
        directories.extend(data_dirs.iter().map(|path| path.join("pixmaps")));
        Self { directories }
    }

    fn resolve(&self, name: &str) -> Option<PathBuf> {
        let path = Path::new(name);
        let raster_extensions = ["png", "webp", "ico", "jpg", "jpeg", "gif"];
        let extension = path
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or_default();
        if path.is_absolute() {
            if raster_extensions.contains(&extension) && path.is_file() {
                return Some(path.to_owned());
            }
            for extension in raster_extensions {
                let candidate = path.with_extension(extension);
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
            return None;
        }
        if path.components().count() != 1 {
            return None;
        }
        let stem = if raster_extensions.contains(&extension)
            || ["svg", "svgz", "xpm"].contains(&extension)
        {
            path.file_stem()?.to_str()?
        } else {
            name
        };
        for directory in &self.directories {
            if raster_extensions.contains(&extension) && directory.join(name).is_file() {
                return Some(directory.join(name));
            }
            for extension in raster_extensions {
                let candidate = directory.join(format!("{stem}.{extension}"));
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
        None
    }
}

fn add_theme(
    theme: &str,
    roots: &[PathBuf],
    visited: &mut HashSet<String>,
    directories: &mut Vec<PathBuf>,
) {
    if Path::new(theme).components().count() != 1 || !visited.insert(theme.to_owned()) {
        return;
    }
    let mut inherits = Vec::new();
    for root in roots {
        let base = root.join(theme);
        let Ok(index) = fs::read_to_string(base.join("index.theme")) else {
            continue;
        };
        let mut header = false;
        let mut paths = Vec::new();
        for line in index.lines() {
            let line = line.trim();
            if line.starts_with('[') {
                header = line == "[Icon Theme]";
            }
            if !header {
                continue;
            }
            if let Some(value) = line
                .strip_prefix("Directories=")
                .or_else(|| line.strip_prefix("ScaledDirectories="))
            {
                paths.extend(
                    value
                        .split(',')
                        .map(str::trim)
                        .filter(|path| !path.is_empty())
                        .map(str::to_owned),
                );
            }
            if let Some(value) = line.strip_prefix("Inherits=") {
                inherits.extend(
                    value
                        .split(',')
                        .map(str::trim)
                        .filter(|name| !name.is_empty())
                        .map(str::to_owned),
                );
            }
        }
        paths.sort_by_key(|path| {
            let size = path
                .split('/')
                .next()
                .and_then(|size| size.split('x').next()?.parse::<u32>().ok())
                .unwrap_or(0);
            size.abs_diff(256)
        });
        directories.push(base.clone());
        directories.extend(paths.into_iter().map(|path| base.join(path)));
    }
    for inherited in inherits {
        add_theme(&inherited, roots, visited, directories);
    }
}

#[cfg(test)]
mod quality_tests {
    use super::*;

    #[test]
    fn loading_keeps_high_resolution_sources_for_native_graphics() {
        let temporary = crate::test_support::TempDir::new();
        let path = temporary.path.join("logo.png");
        DynamicImage::new_rgb8(512, 512).save(&path).unwrap();
        let image = load_image(&path).unwrap();
        assert_eq!((image.width(), image.height()), (512, 512));
    }
}
