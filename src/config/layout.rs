use serde::Deserialize;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct LayoutConfig {
    pub panes: Option<PaneWeights>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PaneWeights {
    pub places: u16,
    pub files: u16,
    pub preview: u16,
}

#[derive(Deserialize, Default)]
pub(super) struct LayoutConfigOverride {
    panes: Option<PaneWeightsOverride>,
}

#[derive(Deserialize, Default)]
struct PaneWeightsOverride {
    places: Option<u16>,
    files: Option<u16>,
    preview: Option<u16>,
}

impl LayoutConfig {
    pub(super) fn from_override(overrides: LayoutConfigOverride) -> anyhow::Result<Self> {
        let panes = overrides
            .panes
            .map(PaneWeights::from_override)
            .transpose()?;
        Ok(Self { panes })
    }
}

impl PaneWeights {
    fn from_override(overrides: PaneWeightsOverride) -> anyhow::Result<Self> {
        let places = overrides
            .places
            .ok_or_else(|| anyhow::anyhow!("layout.panes.places must be set"))?;
        let files = overrides
            .files
            .ok_or_else(|| anyhow::anyhow!("layout.panes.files must be set"))?;
        let preview = overrides
            .preview
            .ok_or_else(|| anyhow::anyhow!("layout.panes.preview must be set"))?;

        if files == 0 {
            anyhow::bail!("layout.panes.files must be greater than 0");
        }

        Ok(Self {
            places,
            files,
            preview,
        })
    }
}
