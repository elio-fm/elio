use super::StaticImageOverlayRequest;
use crate::app::overlays::inline_image::RenderedImageDimensions;
use crate::app::{Entry, EntryKind, jobs};
use quick_xml::{Reader, events::Event};
use std::{fs, fs::File, io::Read, path::Path};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum StaticImageFormat {
    Png,
    Ico,
    Jpeg,
    Gif,
    Webp,
    Svg,
}

impl StaticImageFormat {
    fn detail_label(self) -> &'static str {
        match self {
            Self::Png => "PNG image",
            Self::Ico => "ICO image",
            Self::Jpeg => "JPEG image",
            Self::Gif => "GIF image",
            Self::Webp => "WebP image",
            Self::Svg => "SVG image",
        }
    }

    fn from_label(label: &'static str) -> Option<Self> {
        match label {
            "PNG image" => Some(Self::Png),
            "ICO image" | "Icon image" => Some(Self::Ico),
            "JPEG image" => Some(Self::Jpeg),
            "GIF image" => Some(Self::Gif),
            "WebP image" => Some(Self::Webp),
            "SVG image" => Some(Self::Svg),
            _ => None,
        }
    }
}

pub(super) fn static_image_detail_label(entry: &Entry) -> Option<&'static str> {
    static_image_format_for_entry(entry).map(StaticImageFormat::detail_label)
}

fn static_image_format_for_entry(entry: &Entry) -> Option<StaticImageFormat> {
    crate::file_info::inspect_entry_cached(entry)
        .specific_type_label
        .and_then(StaticImageFormat::from_label)
}

pub(super) fn static_image_format_for_overlay_request(
    request: &StaticImageOverlayRequest,
) -> Option<StaticImageFormat> {
    crate::file_info::inspect_path_cached(
        &request.path,
        EntryKind::File,
        request.size,
        request.modified,
    )
    .specific_type_label
    .and_then(StaticImageFormat::from_label)
    .or_else(|| sniff_static_image_format(&request.path))
}

pub(super) fn static_image_format_for_prepare_request(
    request: &jobs::ImagePrepareRequest,
) -> Option<StaticImageFormat> {
    crate::file_info::inspect_path_cached(
        &request.path,
        EntryKind::File,
        request.size,
        request.modified,
    )
    .specific_type_label
    .and_then(StaticImageFormat::from_label)
    .or_else(|| sniff_static_image_format(&request.path))
}

pub(super) fn static_image_format_for_path(path: &Path) -> Option<StaticImageFormat> {
    crate::file_info::inspect_path(path, EntryKind::File)
        .specific_type_label
        .and_then(StaticImageFormat::from_label)
        .or_else(|| sniff_static_image_format(path))
}

fn sniff_static_image_format(path: &Path) -> Option<StaticImageFormat> {
    let mut file = File::open(path).ok()?;
    let mut buffer = [0_u8; 512];
    let bytes_read = file.read(&mut buffer).ok()?;
    let prefix = &buffer[..bytes_read];
    if prefix.starts_with(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]) {
        return Some(StaticImageFormat::Png);
    }
    if prefix.starts_with(&[0xff, 0xd8, 0xff]) {
        return Some(StaticImageFormat::Jpeg);
    }
    if prefix.starts_with(b"GIF87a") || prefix.starts_with(b"GIF89a") {
        return Some(StaticImageFormat::Gif);
    }
    if prefix.len() >= 12 && &prefix[..4] == b"RIFF" && &prefix[8..12] == b"WEBP" {
        return Some(StaticImageFormat::Webp);
    }
    if prefix.len() >= 4 && prefix[..4] == [0x00, 0x00, 0x01, 0x00] {
        return Some(StaticImageFormat::Ico);
    }

    let text = std::str::from_utf8(prefix).ok()?;
    let trimmed = text.trim_start_matches(|ch: char| ch.is_ascii_whitespace() || ch == '\u{feff}');
    (trimmed.starts_with("<svg") || (trimmed.starts_with("<?xml") && trimmed.contains("<svg")))
        .then_some(StaticImageFormat::Svg)
}

pub(super) fn read_raster_dimensions(path: &Path) -> Option<RenderedImageDimensions> {
    let (mut width_px, mut height_px) = image::ImageReader::open(path)
        .ok()?
        .with_guessed_format()
        .ok()?
        .into_dimensions()
        .ok()?;
    if exif_orientation_swaps_dimensions(read_exif_orientation(path).unwrap_or(1)) {
        std::mem::swap(&mut width_px, &mut height_px);
    }
    (width_px > 0 && height_px > 0).then_some(RenderedImageDimensions {
        width_px,
        height_px,
    })
}

pub(super) fn read_exif_orientation(path: &Path) -> Option<u16> {
    let mut file = File::open(path).ok()?;
    let mut soi = [0_u8; 2];
    file.read_exact(&mut soi).ok()?;
    if soi != [0xff, 0xd8] {
        return None;
    }

    loop {
        let mut prefix = [0_u8; 1];
        file.read_exact(&mut prefix).ok()?;
        while prefix[0] != 0xff {
            file.read_exact(&mut prefix).ok()?;
        }

        let mut marker = [0_u8; 1];
        file.read_exact(&mut marker).ok()?;
        while marker[0] == 0xff {
            file.read_exact(&mut marker).ok()?;
        }

        match marker[0] {
            0xd8 | 0x01 => continue,
            0xd9 | 0xda => return None,
            _ => {
                let mut length = [0_u8; 2];
                file.read_exact(&mut length).ok()?;
                let payload_len = usize::from(u16::from_be_bytes(length)).checked_sub(2)?;
                let mut payload = vec![0_u8; payload_len];
                file.read_exact(&mut payload).ok()?;
                if marker[0] == 0xe1 && payload.starts_with(b"Exif\0\0") {
                    return parse_exif_orientation(&payload[6..]);
                }
            }
        }
    }
}

fn parse_exif_orientation(tiff: &[u8]) -> Option<u16> {
    if tiff.len() < 8 {
        return None;
    }
    let little_endian = match &tiff[..2] {
        b"II" => true,
        b"MM" => false,
        _ => return None,
    };
    let read_u16 = |offset: usize| -> Option<u16> {
        let bytes: [u8; 2] = tiff.get(offset..offset + 2)?.try_into().ok()?;
        Some(if little_endian {
            u16::from_le_bytes(bytes)
        } else {
            u16::from_be_bytes(bytes)
        })
    };
    let read_u32 = |offset: usize| -> Option<u32> {
        let bytes: [u8; 4] = tiff.get(offset..offset + 4)?.try_into().ok()?;
        Some(if little_endian {
            u32::from_le_bytes(bytes)
        } else {
            u32::from_be_bytes(bytes)
        })
    };

    if read_u16(2)? != 42 {
        return None;
    }
    let ifd_offset = read_u32(4)? as usize;
    let entry_count = usize::from(read_u16(ifd_offset)?);
    let mut entry_offset = ifd_offset + 2;
    for _ in 0..entry_count {
        let tag = read_u16(entry_offset)?;
        let field_type = read_u16(entry_offset + 2)?;
        let count = read_u32(entry_offset + 4)?;
        if tag == 0x0112 && field_type == 3 && count >= 1 {
            return read_u16(entry_offset + 8);
        }
        entry_offset += 12;
    }
    None
}

fn exif_orientation_swaps_dimensions(orientation: u16) -> bool {
    matches!(orientation, 5..=8)
}

pub(super) fn read_svg_dimensions(path: &Path) -> Option<RenderedImageDimensions> {
    let bytes = fs::read(path).ok()?;
    let mut reader = Reader::from_reader(bytes.as_slice());
    reader.config_mut().trim_text(true);

    let mut buffer = Vec::new();
    loop {
        match reader.read_event_into(&mut buffer).ok()? {
            Event::Start(tag) | Event::Empty(tag) if tag.name().as_ref() == b"svg" => {
                let mut width = None;
                let mut height = None;
                let mut view_box = None;
                for attribute in tag.attributes().flatten() {
                    let key = attribute.key.as_ref();
                    let value = attribute.decode_and_unescape_value(reader.decoder()).ok()?;
                    match key {
                        b"width" => width = parse_svg_length_px(&value),
                        b"height" => height = parse_svg_length_px(&value),
                        b"viewBox" => view_box = parse_svg_view_box(&value),
                        _ => {}
                    }
                }

                return match (width, height, view_box) {
                    (Some(width_px), Some(height_px), _) if width_px > 0 && height_px > 0 => {
                        Some(RenderedImageDimensions {
                            width_px,
                            height_px,
                        })
                    }
                    (_, _, Some((width_px, height_px))) if width_px > 0.0 && height_px > 0.0 => {
                        Some(RenderedImageDimensions {
                            width_px: width_px.round() as u32,
                            height_px: height_px.round() as u32,
                        })
                    }
                    _ => None,
                };
            }
            Event::Eof => return None,
            _ => {}
        }
        buffer.clear();
    }
}

fn parse_svg_length_px(value: &str) -> Option<u32> {
    let trimmed = value.trim();
    let numeric = trimmed
        .strip_suffix("px")
        .unwrap_or(trimmed)
        .trim()
        .parse::<f32>()
        .ok()?;
    (numeric > 0.0).then_some(numeric.round() as u32)
}

fn parse_svg_view_box(value: &str) -> Option<(f32, f32)> {
    let mut parts = value
        .split(|character: char| character.is_ascii_whitespace() || character == ',')
        .filter(|part| !part.is_empty());
    let _min_x = parts.next()?.parse::<f32>().ok()?;
    let _min_y = parts.next()?.parse::<f32>().ok()?;
    let width = parts.next()?.parse::<f32>().ok()?;
    let height = parts.next()?.parse::<f32>().ok()?;
    Some((width, height))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_path(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("elio-static-image-format-{label}-{unique}"))
    }

    #[test]
    fn static_image_format_sniffs_collision_suffixed_jpeg_path() {
        let root = temp_path("jpeg-collision-suffix");
        fs::create_dir_all(&root).expect("failed to create temp root");
        let path = root.join("photo.jpeg.2");
        fs::write(&path, [0xff, 0xd8, 0xff, 0xdb]).expect("failed to write jpeg signature");

        assert_eq!(
            static_image_format_for_path(&path),
            Some(StaticImageFormat::Jpeg)
        );

        fs::remove_dir_all(root).expect("failed to remove temp root");
    }
}
