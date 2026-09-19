use std::{
    collections::HashMap,
    io::{self, Cursor, Write},
    sync::{
        Arc,
        atomic::{AtomicU32, Ordering},
    },
};

use anyhow::Result;
use image::{DynamicImage, ImageFormat, imageops::FilterType};
use ratatui::layout::{Rect, Size};
use ratatui_image::{FontSize, Resize};

static NEXT_IMAGE: AtomicU32 = AtomicU32::new(1);

#[derive(Clone)]
pub struct LegacyImage {
    pub id: u32,
    pub size: Size,
    transfer: Arc<str>,
}

impl LegacyImage {
    pub fn new(
        image: &DynamicImage,
        size: Size,
        font: FontSize,
        background: [u8; 4],
    ) -> Result<Self> {
        let resize = Resize::Scale(Some(FilterType::Lanczos3));
        let size = resize.size_for(image, font, size);
        let image = resize.resize(image, font, size, Some(image::Rgba(background)));
        let mut png = Cursor::new(Vec::new());
        image.write_to(&mut png, ImageFormat::Png)?;
        let encoded = base64(png.get_ref());
        let id = (std::process::id() & 0xffff) << 16
            | (NEXT_IMAGE.fetch_add(1, Ordering::Relaxed) & 0xffff);
        let mut transfer = String::new();
        let chunks = encoded.as_bytes().chunks(4096);
        let count = chunks.len();
        for (index, chunk) in chunks.enumerate() {
            let more = usize::from(index + 1 < count);
            if index == 0 {
                transfer.push_str(&format!("\x1b_Ga=t,f=100,t=d,i={id},q=2,m={more};"));
            } else {
                transfer.push_str(&format!("\x1b_Gm={more};"));
            }
            transfer.push_str(std::str::from_utf8(chunk)?);
            transfer.push_str("\x1b\\");
        }
        Ok(Self {
            id,
            size,
            transfer: transfer.into(),
        })
    }
}

#[derive(Default)]
pub struct Placements {
    visible: HashMap<u32, Rect>,
    pending: Vec<(LegacyImage, Rect)>,
    viewport: Option<Rect>,
    invalidated: bool,
}

impl Placements {
    pub fn begin(&mut self) {
        self.pending.clear();
    }

    pub fn push(&mut self, image: LegacyImage, area: Rect) {
        self.pending.push((image, area));
    }

    pub fn set_viewport(&mut self, area: Rect) {
        if self.viewport.replace(area) != Some(area) {
            self.invalidated = true;
        }
    }

    pub fn flush(&mut self, writer: &mut impl Write) -> io::Result<()> {
        let next: HashMap<u32, Rect> = self
            .pending
            .iter()
            .map(|(image, area)| (image.id, *area))
            .collect();
        for (id, area) in &self.visible {
            if self.invalidated || next.get(id) != Some(area) {
                write!(writer, "\x1b_Ga=d,d=I,i={id},q=2\x1b\\")?;
            }
        }
        for (image, area) in &self.pending {
            if !self.invalidated && self.visible.get(&image.id) == Some(area) {
                continue;
            }
            write!(
                writer,
                "\x1b7\x1b[{};{}H{}\x1b_Ga=p,i={},c={},r={},C=1,q=2\x1b\\\x1b8",
                area.y + 1,
                area.x + 1,
                image.transfer,
                image.id,
                area.width,
                area.height
            )?;
        }
        writer.flush()?;
        self.visible = next;
        self.invalidated = false;
        Ok(())
    }
}

impl Drop for Placements {
    fn drop(&mut self) {
        self.pending.clear();
        let _ = self.flush(&mut io::stdout());
    }
}

fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut encoded = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let first = chunk[0];
        let second = chunk.get(1).copied().unwrap_or_default();
        let third = chunk.get(2).copied().unwrap_or_default();
        encoded.push(ALPHABET[usize::from(first >> 2)] as char);
        encoded.push(ALPHABET[usize::from((first & 3) << 4 | second >> 4)] as char);
        encoded.push(if chunk.len() > 1 {
            ALPHABET[usize::from((second & 15) << 2 | third >> 6)] as char
        } else {
            '='
        });
        encoded.push(if chunk.len() > 2 {
            ALPHABET[usize::from(third & 63)] as char
        } else {
            '='
        });
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_base64_padding_and_binary_data() {
        for (input, output) in [
            (&b""[..], ""),
            (&b"f"[..], "Zg=="),
            (&b"fo"[..], "Zm8="),
            (&b"foo"[..], "Zm9v"),
            (&[255, 254, 253][..], "//79"),
        ] {
            assert_eq!(base64(input), output);
        }
    }

    #[test]
    fn legacy_graphics_uses_proportional_cells_for_centering() {
        let image = LegacyImage::new(
            &DynamicImage::new_rgb8(512, 512),
            Size::new(10, 3),
            FontSize::new(10, 20),
            [24, 24, 37, 255],
        )
        .unwrap();
        assert_eq!(image.size, Size::new(6, 3));
    }

    #[test]
    fn legacy_graphics_restores_unchanged_placements_after_resize() {
        let image = LegacyImage::new(
            &DynamicImage::new_rgb8(64, 64),
            Size::new(6, 3),
            FontSize::new(10, 20),
            [24, 24, 37, 255],
        )
        .unwrap();
        let mut placements = Placements::default();
        let area = Rect::new(3, 4, 6, 3);
        placements.set_viewport(Rect::new(0, 0, 100, 40));
        placements.push(image.clone(), area);
        placements.flush(&mut Vec::new()).unwrap();

        placements.begin();
        placements.set_viewport(Rect::new(0, 0, 100, 41));
        placements.push(image.clone(), area);
        let mut resized = Vec::new();
        placements.flush(&mut resized).unwrap();
        let resized = String::from_utf8(resized).unwrap();
        assert!(resized.starts_with(&format!("\x1b_Ga=d,d=I,i={},q=2\x1b\\", image.id)));
        assert!(resized.contains("a=t,f=100"));
        assert!(resized.contains(&format!("a=p,i={}", image.id)));

        placements.begin();
        placements.set_viewport(Rect::new(0, 0, 100, 41));
        placements.push(image, area);
        let mut repeated = Vec::new();
        placements.flush(&mut repeated).unwrap();
        assert!(repeated.is_empty());
        placements.begin();
        placements.flush(&mut repeated).unwrap();
    }

    #[test]
    fn legacy_graphics_reuses_images_and_removes_stale_placements() {
        let image = LegacyImage::new(
            &DynamicImage::new_rgb8(64, 64),
            Size::new(6, 3),
            FontSize::new(10, 20),
            [24, 24, 37, 255],
        )
        .unwrap();
        let mut placements = Placements::default();
        let area = Rect::new(3, 4, 6, 3);
        placements.push(image.clone(), area);
        let mut output = Vec::new();
        placements.flush(&mut output).unwrap();
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("a=t,f=100"));
        assert!(output.contains("a=p,"));
        assert!(!output.contains("U=1"));
        assert!(output.contains("\x1b[5;4H"));
        let mut repeated = Vec::new();
        placements.flush(&mut repeated).unwrap();
        assert!(repeated.is_empty());
        placements.begin();
        placements.flush(&mut repeated).unwrap();
        assert!(
            String::from_utf8(repeated)
                .unwrap()
                .contains(&format!("d=I,i={}", image.id))
        );
    }
}
