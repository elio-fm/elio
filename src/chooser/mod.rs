mod output;
mod selection;

pub(crate) use output::write_selected_paths;
pub(crate) use selection::{ChooserExit, ChooserState};

#[cfg(test)]
mod tests;
