use std::path::Path;

pub(crate) fn read_macro_event_calendar_text(path: &Path) -> Option<String> {
    std::fs::read_to_string(path).ok()
}
