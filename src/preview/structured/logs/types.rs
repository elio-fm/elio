#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum LogSource {
    Json,
    Access,
    General,
}

impl LogSource {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Json => "JSON lines",
            Self::Access => "Access log",
            Self::General => "Application log",
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct ParsedLogDocument {
    pub(super) source: LogSource,
    pub(super) entries: Vec<ParsedLogEntry>,
}

#[derive(Clone, Debug, Default)]
pub(super) struct ParsedLogEntry {
    pub(super) timestamp: Option<String>,
    pub(super) level: Option<String>,
    pub(super) message: String,
    pub(super) fields: Vec<(String, String)>,
    pub(super) continuations: Vec<String>,
}

#[derive(Clone, Debug)]
pub(super) struct RawLogEntry {
    pub(super) line: String,
    pub(super) continuations: Vec<String>,
}
