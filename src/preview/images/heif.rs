use crate::{
    preview::external_commands::run_command_capture_stdout_cancellable,
    terminal_images::RenderedImageDimensions,
};
use serde::Deserialize;
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
    process::Command,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct HeifRenderInfo {
    pub(super) dimensions: RenderedImageDimensions,
    coded_dimensions: RenderedImageDimensions,
    transforms: Vec<HeifTransform>,
    tile_grid: Option<HeifTileGrid>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct HeifTileGrid {
    tiles: Vec<HeifTile>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct HeifTile {
    stream_index: u32,
    horizontal_offset: u32,
    vertical_offset: u32,
}

#[derive(Deserialize)]
struct FfprobeOutput {
    #[serde(default)]
    stream_groups: Vec<FfprobeStreamGroup>,
    #[serde(default)]
    streams: Vec<FfprobeStream>,
}

#[derive(Deserialize)]
struct FfprobeStreamGroup {
    #[serde(rename = "type")]
    group_type: Option<String>,
    disposition: Option<FfprobeDisposition>,
    components: Option<Vec<FfprobeComponent>>,
}

#[derive(Deserialize)]
struct FfprobeDisposition {
    default: Option<u8>,
}

#[derive(Deserialize)]
struct FfprobeComponent {
    width: Option<u32>,
    height: Option<u32>,
    subcomponents: Option<Vec<FfprobeTile>>,
}

#[derive(Deserialize)]
struct FfprobeTile {
    stream_index: Option<u32>,
    tile_horizontal_offset: Option<u32>,
    tile_vertical_offset: Option<u32>,
}

#[derive(Deserialize)]
struct FfprobeStream {
    width: Option<u32>,
    height: Option<u32>,
    disposition: Option<FfprobeDisposition>,
    tags: Option<FfprobeTags>,
    side_data_list: Option<Vec<FfprobeSideData>>,
}

#[derive(Deserialize)]
struct FfprobeTags {
    rotate: Option<String>,
}
#[derive(Deserialize)]
struct FfprobeSideData {
    rotation: Option<f64>,
}

pub(super) fn probe_render_info<F>(input_path: &Path, canceled: &F) -> Option<HeifRenderInfo>
where
    F: Fn() -> bool,
{
    if canceled() {
        return None;
    }
    let mut command = Command::new("ffprobe");
    command
        .args([
            "-v",
            "error",
            "-show_stream_groups",
            "-show_streams",
            "-of",
            "json",
        ])
        .arg(input_path);
    let output = run_command_capture_stdout_cancellable(command, "image-heif-ffprobe", canceled)?;
    if canceled() {
        return None;
    }
    let mut info = parse_render_info(&String::from_utf8_lossy(&output))?;
    // HEIF item transformations (irot) are not exposed by ffprobe for tiled
    // images. heif-info reports the container-level transform explicitly.
    if canceled() {
        return None;
    }
    info.transforms = match read_primary_item_transform(input_path) {
        Some(transforms) if !transforms.is_empty() => transforms,
        _ if info.tile_grid.is_none() => ffprobe_rotation(&String::from_utf8_lossy(&output))
            .map(|rotation| vec![HeifTransform::Rotate(rotation)])
            .unwrap_or_default(),
        _ => Vec::new(),
    };
    let rotation = info
        .transforms
        .iter()
        .fold(0u16, |rotation, transform| match transform {
            HeifTransform::Rotate(degrees) => (rotation + degrees) % 360,
            HeifTransform::Mirror(_) => rotation,
        });
    if matches!(rotation, 90 | 270) {
        std::mem::swap(
            &mut info.dimensions.width_px,
            &mut info.dimensions.height_px,
        );
    }
    Some(info)
}

fn ffprobe_rotation(raw: &str) -> Option<u16> {
    let output = serde_json::from_str::<FfprobeOutput>(raw).ok()?;
    let stream = output
        .streams
        .iter()
        .find(|s| s.disposition.as_ref().and_then(|d| d.default) == Some(1))?;
    stream
        .side_data_list
        .as_ref()
        .and_then(|items| items.iter().find_map(|data| data.rotation))
        .map(|rotation| ((360.0 - rotation).round() as i32).rem_euclid(360) as u16)
        .or_else(|| {
            stream
                .tags
                .as_ref()?
                .rotate
                .as_ref()?
                .parse::<i32>()
                .ok()
                .map(|v| v.rem_euclid(360) as u16)
        })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HeifTransform {
    Rotate(u16),
    Mirror(bool),
}

fn read_primary_item_transform(path: &Path) -> Option<Vec<HeifTransform>> {
    const MAX_META_BYTES: usize = 16 * 1024 * 1024;
    let mut file = File::open(path).ok()?;
    let file_len = file.metadata().ok()?.len();
    let mut offset = 0u64;
    let meta = loop {
        if offset.checked_add(8)? > file_len {
            return None;
        }
        file.seek(SeekFrom::Start(offset)).ok()?;
        let mut header = [0u8; 16];
        file.read_exact(&mut header[..8]).ok()?;
        let size32 = u32::from_be_bytes(header[..4].try_into().ok()?) as u64;
        let kind: [u8; 4] = header[4..8].try_into().ok()?;
        let header_len = if size32 == 1 {
            file.read_exact(&mut header[8..]).ok()?;
            16u64
        } else {
            8
        };
        let size = if size32 == 1 {
            u64::from_be_bytes(header[8..16].try_into().ok()?)
        } else if size32 == 0 {
            file_len - offset
        } else {
            size32
        };
        if size < header_len || offset.checked_add(size)? > file_len {
            return None;
        }
        if &kind == b"meta" {
            let payload_len = usize::try_from(size - header_len).ok()?;
            if payload_len > MAX_META_BYTES {
                return None;
            }
            let mut data = vec![0; payload_len];
            file.read_exact(&mut data).ok()?;
            break data;
        }
        offset = offset.checked_add(size)?;
    };
    parse_primary_item_transform_meta(&meta)
}

fn parse_primary_item_transform_meta(meta: &[u8]) -> Option<Vec<HeifTransform>> {
    let meta_boxes = boxes(meta, 4, meta.len())?;
    let primary = meta_boxes.iter().find(|b| &b.kind == b"pitm")?;
    let primary_id = item_id(&meta[primary.start..primary.end])?;
    let ipco = meta_boxes
        .iter()
        .find(|b| &b.kind == b"iprp")
        .and_then(|b| boxes(meta, b.start, b.end))?
        .into_iter()
        .find(|b| &b.kind == b"ipco")?;
    let properties = boxes(meta, ipco.start, ipco.end)?;
    let ipma = meta_boxes
        .iter()
        .find(|b| &b.kind == b"iprp")
        .and_then(|b| boxes(meta, b.start, b.end))?
        .into_iter()
        .find(|b| &b.kind == b"ipma")?;
    let associations = item_property_indices(&meta[ipma.start..ipma.end], primary_id)?;
    let mut transforms = Vec::new();
    for index in associations {
        let property = properties.get(index.checked_sub(1)?)?;
        match &property.kind {
            b"irot" => {
                let value = *meta.get(property.start)? & 3;
                transforms.push(HeifTransform::Rotate(((4 - value as u16) * 90) % 360));
            }
            b"imir" => transforms.push(HeifTransform::Mirror(*meta.get(property.start)? & 1 != 0)),
            _ => {}
        }
    }
    Some(transforms)
}

#[derive(Clone)]
struct BoxRef {
    kind: [u8; 4],
    start: usize,
    end: usize,
}

fn boxes(data: &[u8], mut offset: usize, end: usize) -> Option<Vec<BoxRef>> {
    let mut result = Vec::new();
    while offset < end {
        let header = data.get(offset..offset.checked_add(8)?)?;
        let size32 = u32::from_be_bytes(header[..4].try_into().ok()?) as u64;
        let kind = header[4..8].try_into().ok()?;
        let header_len = if size32 == 1 { 16 } else { 8 };
        let size = if size32 == 1 {
            u64::from_be_bytes(data.get(offset + 8..offset + 16)?.try_into().ok()?)
        } else if size32 == 0 {
            (end - offset) as u64
        } else {
            size32
        };
        let size = usize::try_from(size).ok()?;
        if size < header_len {
            return None;
        }
        let box_end = offset.checked_add(size)?;
        if box_end > end {
            return None;
        }
        result.push(BoxRef {
            kind,
            start: offset + header_len,
            end: box_end,
        });
        offset = box_end;
    }
    Some(result)
}

fn item_id(payload: &[u8]) -> Option<u32> {
    let version = *payload.first()?;
    if version == 0 {
        let offset = 4;
        Some(u16::from_be_bytes(payload.get(offset..offset + 2)?.try_into().ok()?) as u32)
    } else if version == 1 {
        let offset = 4;
        Some(u32::from_be_bytes(
            payload.get(offset..offset + 4)?.try_into().ok()?,
        ))
    } else {
        None
    }
}

fn item_property_indices(payload: &[u8], wanted: u32) -> Option<Vec<usize>> {
    let version = *payload.first()?;
    if version > 1 {
        return None;
    }
    let flags = u32::from_be_bytes([0, *payload.get(1)?, *payload.get(2)?, *payload.get(3)?]);
    let wide = flags & 1 != 0;
    let mut cursor = 4usize;
    let count = u32::from_be_bytes(payload.get(cursor..cursor + 4)?.try_into().ok()?) as usize;
    cursor += 4;
    for _ in 0..count {
        let id = if version < 1 {
            let v = u16::from_be_bytes(payload.get(cursor..cursor + 2)?.try_into().ok()?) as u32;
            cursor += 2;
            v
        } else {
            let v = u32::from_be_bytes(payload.get(cursor..cursor + 4)?.try_into().ok()?);
            cursor += 4;
            v
        };
        let n = *payload.get(cursor)? as usize;
        cursor += 1;
        let mut indices = Vec::new();
        for _ in 0..n {
            let value = if wide {
                let v = u16::from_be_bytes(payload.get(cursor..cursor + 2)?.try_into().ok()?);
                cursor += 2;
                v
            } else {
                let v = *payload.get(cursor)? as u16;
                cursor += 1;
                v
            };
            let index = (value & if wide { 0x7fff } else { 0x7f }) as usize;
            if index != 0 {
                indices.push(index);
            }
        }
        if id == wanted {
            return Some(indices);
        }
    }
    None
}

impl HeifRenderInfo {
    pub(super) fn transform_filter(&self) -> Option<String> {
        let mut filters = Vec::new();
        for transform in &self.transforms {
            match transform {
                HeifTransform::Rotate(90) => filters.push("transpose=clock"),
                HeifTransform::Rotate(180) => filters.push("hflip,vflip"),
                HeifTransform::Rotate(270) => filters.push("transpose=cclock"),
                HeifTransform::Rotate(_) => {}
                // HEIF imir mode 0 flips top-to-bottom; mode 1 flips left-to-right.
                HeifTransform::Mirror(false) => filters.push("vflip"),
                HeifTransform::Mirror(true) => filters.push("hflip"),
            }
        }
        (!filters.is_empty()).then(|| filters.join(","))
    }
    pub(super) fn tile_grid_filter(&self, scale: &str) -> Option<String> {
        let grid = self.tile_grid.as_ref()?;
        let inputs = grid
            .tiles
            .iter()
            .map(|tile| format!("[0:v:{}]", tile.stream_index))
            .collect::<String>();
        let layout = grid
            .tiles
            .iter()
            .map(|tile| format!("{}_{}", tile.horizontal_offset, tile.vertical_offset))
            .collect::<Vec<_>>()
            .join("|");
        let rotation = self
            .transform_filter()
            .map(|filter| format!(",{filter}"))
            .unwrap_or_default();
        Some(format!(
            "{inputs}xstack=inputs={}:layout={layout}:fill=black,crop={}:{}:0:0{rotation},{scale}[out]",
            grid.tiles.len(),
            self.coded_dimensions.width_px,
            self.coded_dimensions.height_px
        ))
    }
}

fn parse_render_info(raw: &str) -> Option<HeifRenderInfo> {
    let output = serde_json::from_str::<FfprobeOutput>(raw).ok()?;
    let grid = output.stream_groups.iter().find_map(|group| {
        (group.group_type.as_deref() == Some("Tile Grid")
            && group.disposition.as_ref().and_then(|value| value.default) == Some(1))
        .then_some(())?;
        let component = group.components.as_ref()?.first()?;
        let (width_px, height_px) = component.width.zip(component.height)?;
        let tiles = component
            .subcomponents
            .as_ref()?
            .iter()
            .map(|tile| {
                Some(HeifTile {
                    stream_index: tile.stream_index?,
                    horizontal_offset: tile.tile_horizontal_offset?,
                    vertical_offset: tile.tile_vertical_offset?,
                })
            })
            .collect::<Option<Vec<_>>>()?;
        (!tiles.is_empty()).then_some(HeifRenderInfo {
            dimensions: RenderedImageDimensions {
                width_px,
                height_px,
            },
            coded_dimensions: RenderedImageDimensions {
                width_px,
                height_px,
            },
            tile_grid: Some(HeifTileGrid { tiles }),
            transforms: Vec::new(),
        })
    });
    grid.or_else(|| {
        output.streams.iter().find_map(|stream| {
            (stream.disposition.as_ref().and_then(|value| value.default) == Some(1))
                .then(|| stream.width.zip(stream.height))
                .flatten()
                .map(|(width_px, height_px)| HeifRenderInfo {
                    dimensions: RenderedImageDimensions {
                        width_px,
                        height_px,
                    },
                    coded_dimensions: RenderedImageDimensions {
                        width_px,
                        height_px,
                    },
                    tile_grid: None,
                    transforms: Vec::new(),
                })
        })
    })
}

#[cfg(test)]
#[path = "tests/heif.rs"]
mod tests;
