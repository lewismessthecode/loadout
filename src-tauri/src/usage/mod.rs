pub mod claude;
pub mod codex;
pub mod cursor;
pub mod index;

use chrono::{DateTime, Utc};
use regex::Regex;
use std::sync::LazyLock;

#[derive(Debug, Clone, PartialEq)]
pub struct RawUse {
    pub skill_dir: String,
    pub skill_name: String,
    pub at: Option<DateTime<Utc>>,
}

static SKILL_PATH: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(/?[\w.~@+\-]+(?:/[\w.~@+\-]+)*)/SKILL\.md\b").unwrap()
});

pub fn use_from_dir(dir: &str, at: Option<DateTime<Utc>>) -> Option<RawUse> {
    let name = dir.rsplit('/').next().unwrap_or("");
    if name.is_empty() || matches!(name, "." | ".." | "~") {
        return None;
    }
    Some(RawUse { skill_dir: dir.to_string(), skill_name: name.to_string(), at })
}

pub fn uses_in_text(text: &str, at: Option<DateTime<Utc>>) -> Vec<RawUse> {
    SKILL_PATH
        .captures_iter(text)
        .filter_map(|c| use_from_dir(&c[1], at))
        .collect()
}

pub fn parse_rfc3339(s: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s).ok().map(|d| d.with_timezone(&Utc))
}
