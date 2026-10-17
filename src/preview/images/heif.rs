use crate::{
    preview::external_commands::run_command_capture_stdout_cancellable,
    terminal_images::RenderedImageDimensions,
};
use serde::Deserialize;
use std::{path::Path, process::Command};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct HeifRenderInfo {
    pub(super) dimensions: RenderedImageDimensions,
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
    parse_render_info(&String::from_utf8_lossy(&output))
}

impl HeifRenderInfo {
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
        Some(format!(
            "{inputs}xstack=inputs={}:layout={layout}:fill=black,crop={}:{}:0:0,{scale}[out]",
            grid.tiles.len(),
            self.dimensions.width_px,
            self.dimensions.height_px
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
            tile_grid: Some(HeifTileGrid { tiles }),
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
                    tile_grid: None,
                })
        })
    })
}

#[cfg(test)]
#[path = "tests/heif.rs"]
mod tests;
