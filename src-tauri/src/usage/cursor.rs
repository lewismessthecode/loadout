use super::{use_from_dir, uses_in_text, RawUse};
use chrono::{DateTime, FixedOffset, NaiveDateTime, Utc};
use regex::Regex;
use serde_json::Value;
use std::sync::LazyLock;

#[derive(Default)]
pub struct Ctx {
    pub last_ts: Option<DateTime<Utc>>,
}

static TIMESTAMP: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"<timestamp>\w+, (\w{3}) (\d{1,2}), (\d{4}), (\d{1,2}:\d{2} [AP]M) \(UTC([+-])(\d{1,2})(?::(\d{2}))?\)</timestamp>").unwrap()
});

static ATTACHED: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"Skill Name: [^\n]+\nPath: ([^\n]+)/SKILL\.md").unwrap()
});

pub fn prefilter(line: &str) -> bool {
    line.contains("SKILL.md") || line.contains("<timestamp>")
}

pub fn parse_timestamp(text: &str) -> Option<DateTime<Utc>> {
    let c = TIMESTAMP.captures_iter(text).last()?;
    let naive = NaiveDateTime::parse_from_str(
        &format!("{} {} {} {}", &c[1], &c[2], &c[3], &c[4]),
        "%b %d %Y %I:%M %p",
    )
    .ok()?;
    let hours: i32 = c[6].parse().ok()?;
    let mins: i32 = c.get(7).map(|m| m.as_str().parse().unwrap_or(0)).unwrap_or(0);
    let secs = hours * 3600 + mins * 60;
    let offset = if &c[5] == "-" { FixedOffset::west_opt(secs) } else { FixedOffset::east_opt(secs) }?;
    naive.and_local_timezone(offset).single().map(|d| d.with_timezone(&Utc))
}

pub fn parse_line(line: &str, ctx: &mut Ctx) -> Vec<RawUse> {
    let Ok(v) = serde_json::from_str::<Value>(line) else { return Vec::new() };
    let parts = v["message"]["content"].as_array().cloned().unwrap_or_default();
    match v["role"].as_str() {
        Some("user") => {
            let mut out = Vec::new();
            for p in &parts {
                let Some(text) = p["text"].as_str() else { continue };
                if let Some(ts) = parse_timestamp(text) {
                    ctx.last_ts = Some(ts);
                }
                for c in ATTACHED.captures_iter(text) {
                    out.extend(use_from_dir(c[1].trim(), ctx.last_ts));
                }
            }
            out
        }
        Some("assistant") => parts
            .iter()
            .filter(|p| p["type"] == "tool_use" && matches!(p["name"].as_str(), Some("Read" | "ReadFile")))
            .flat_map(|p| uses_in_text(p["input"]["path"].as_str().unwrap_or(""), ctx.last_ts))
            .collect(),
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamp_with_utc_offset() {
        let t = parse_timestamp("<timestamp>Thursday, Oct 15, 2026, 10:48 AM (UTC+8)</timestamp>").unwrap();
        assert_eq!(t.to_rfc3339(), "2026-10-15T02:48:00+00:00");
        let t = parse_timestamp("<timestamp>Friday, Sep 4, 2026, 2:52 AM (UTC-5)</timestamp>").unwrap();
        assert_eq!(t.to_rfc3339(), "2026-09-04T07:52:00+00:00");
    }

    #[test]
    fn read_tool_uses_latest_user_timestamp() {
        let mut ctx = Ctx::default();
        let user = r#"{"role":"user","message":{"content":[{"type":"text","text":"<timestamp>Friday, Sep 4, 2026, 10:31 AM (UTC+8)</timestamp>\n\n<user_query>go</user_query>"}]}}"#;
        assert!(parse_line(user, &mut ctx).is_empty());
        let read = r#"{"role":"assistant","message":{"content":[{"type":"tool_use","name":"Read","input":{"path":"/Users/lewisliu/.claude/plugins/cache/caveman/caveman/766dce6b1394/skills/cavecrew/SKILL.md"}}]}}"#;
        let u = parse_line(read, &mut ctx);
        assert_eq!(u.len(), 1);
        assert_eq!(u[0].skill_name, "cavecrew");
        assert_eq!(u[0].at.unwrap().to_rfc3339(), "2026-09-04T02:31:00+00:00");
    }

    #[test]
    fn manually_attached_skill_counts() {
        let mut ctx = Ctx::default();
        let user = r#"{"role":"user","message":{"content":[{"type":"text","text":"<timestamp>Friday, Sep 4, 2026, 10:31 AM (UTC+8)</timestamp>\n<manually_attached_skills>\nSkill Name: goal\nPath: /Users/lewisliu/.cursor/skills-cursor/goal/SKILL.md\nSKILL.md content:\n</manually_attached_skills>"}]}}"#;
        let u = parse_line(user, &mut ctx);
        assert_eq!(u.len(), 1);
        assert_eq!(u[0].skill_dir, "/Users/lewisliu/.cursor/skills-cursor/goal");
    }

    #[test]
    fn shell_commands_and_plain_text_are_ignored() {
        let mut ctx = Ctx::default();
        let shell = r#"{"role":"assistant","message":{"content":[{"type":"tool_use","name":"Shell","input":{"command":"head -c 1500 .agents/skills/tdd/SKILL.md"}}]}}"#;
        assert!(parse_line(shell, &mut ctx).is_empty());
        let text = r#"{"role":"assistant","message":{"content":[{"type":"text","text":"see ~/.agents/skills/tdd/SKILL.md"}]}}"#;
        assert!(parse_line(text, &mut ctx).is_empty());
    }
}
