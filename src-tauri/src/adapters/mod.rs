pub mod cron;
pub mod launchd;
pub mod systemd;
pub mod windows;

use crate::model::{ParseWarning, ScheduledJob};

#[derive(Debug, Default)]
pub struct AdapterResult {
    pub jobs: Vec<ScheduledJob>,
    pub warnings: Vec<ParseWarning>,
}

pub(crate) fn warning(code: &str, message: impl Into<String>, source: &str) -> ParseWarning {
    ParseWarning {
        code: code.into(),
        message: message.into(),
        source_reference: source.into(),
    }
}
