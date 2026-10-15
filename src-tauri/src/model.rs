use chrono::{DateTime, Utc};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum InstallKind {
    Dir,
    Symlink,
}

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SourceKind {
    Github,
    WellKnown,
    Plugin,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Install {
    pub agent: String,
    pub path: String,
    pub kind: InstallKind,
    pub plugin: bool,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    pub repo: String,
    pub author: String,
    pub url: String,
    pub kind: SourceKind,
}

#[derive(Serialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    pub total: u32,
    pub last_used: Option<DateTime<Utc>>,
    pub by_agent: BTreeMap<String, u32>,
    pub daily: BTreeMap<String, u32>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Skill {
    pub id: String,
    pub name: String,
    pub dir_name: String,
    pub description: String,
    pub canonical_dir: String,
    pub installs: Vec<Install>,
    pub source: Option<Source>,
    pub installed_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    pub tags: Vec<String>,
    pub auto_tags: Vec<String>,
    pub usage: Usage,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StoredEvent {
    pub agent: String,
    pub skill_dir: String,
    pub skill_name: String,
    pub at: DateTime<Utc>,
}
