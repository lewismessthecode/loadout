use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LockEntry {
    pub source: String,
    #[serde(default)]
    pub source_type: String,
    #[serde(default)]
    pub source_url: String,
    #[serde(default)]
    pub skill_path: Option<String>,
    #[serde(default)]
    pub plugin_name: Option<String>,
    pub installed_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Default, Deserialize)]
struct LockFile {
    #[serde(default)]
    skills: HashMap<String, LockEntry>,
}

pub type Lock = HashMap<String, LockEntry>;

pub fn parse(text: &str) -> Lock {
    serde_json::from_str::<LockFile>(text)
        .map(|f| f.skills)
        .unwrap_or_default()
}

pub fn read(path: &Path) -> Lock {
    std::fs::read_to_string(path)
        .map(|t| parse(&t))
        .unwrap_or_default()
}

pub fn remove_entries(path: &Path, names: &[String]) -> std::io::Result<usize> {
    let text = std::fs::read_to_string(path)?;
    let mut value: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    let mut removed = 0;
    if let Some(skills) = value.get_mut("skills").and_then(|s| s.as_object_mut()) {
        for n in names {
            if skills.remove(n).is_some() {
                removed += 1;
            }
        }
    }
    if removed > 0 {
        let out = serde_json::to_string_pretty(&value)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(path, out + "\n")?;
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{
      "version": 3,
      "skills": {
        "frontend-design": {
          "source": "anthropics/skills",
          "sourceType": "github",
          "sourceUrl": "https://github.com/anthropics/skills.git",
          "skillPath": "skills/frontend-design/SKILL.md",
          "skillFolderHash": "928950704df8a8b885c03de5da626331e6f29cf8",
          "pluginName": "example-skills",
          "installedAt": "2026-01-23T07:41:56.785Z",
          "updatedAt": "2026-06-03T08:23:57.672Z"
        },
        "lark-base": { "source": "open.feishu.cn", "sourceType": "well-known", "sourceUrl": "https://open.feishu.cn", "installedAt": "2026-03-01T00:00:00.000Z", "updatedAt": "2026-03-01T00:00:00.000Z" }
      },
      "dismissed": {}
    }"#;

    #[test]
    fn parses_real_shape() {
        let lock = parse(SAMPLE);
        assert_eq!(lock.len(), 2);
        let e = &lock["frontend-design"];
        assert_eq!(e.source, "anthropics/skills");
        assert_eq!(e.plugin_name.as_deref(), Some("example-skills"));
        assert_eq!(
            e.installed_at.unwrap().to_rfc3339(),
            "2026-01-23T07:41:56.785+00:00"
        );
        assert_eq!(lock["lark-base"].source_type, "well-known");
    }

    #[test]
    fn garbage_is_empty() {
        assert!(parse("not json").is_empty());
    }

    #[test]
    fn remove_entries_keeps_other_fields() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("lock.json");
        std::fs::write(&p, SAMPLE).unwrap();
        let n = remove_entries(&p, &["lark-base".into(), "nope".into()]).unwrap();
        assert_eq!(n, 1);
        let lock = read(&p);
        assert_eq!(lock.len(), 1);
        let raw: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
        assert_eq!(raw["version"], 3);
        assert!(raw["dismissed"].is_object());
    }
}

pub fn global_path(home: &Path) -> std::path::PathBuf {
    if dirs::home_dir().as_deref() == Some(home) {
        if let Some(p) = std::env::var_os("XDG_STATE_HOME").filter(|v| !v.is_empty()) {
            return std::path::PathBuf::from(p).join("skills/.skill-lock.json");
        }
    }
    home.join(".agents/.skill-lock.json")
}
