use super::{parse_rfc3339, use_from_dir, uses_in_text, RawUse};
use serde_json::Value;

pub fn prefilter(line: &str) -> bool {
    line.contains("\"Skill\"") || line.contains("SKILL.md")
}

pub fn parse_line(line: &str) -> Vec<RawUse> {
    let Ok(v) = serde_json::from_str::<Value>(line) else { return Vec::new() };
    if v["type"] != "assistant" {
        return Vec::new();
    }
    let at = v["timestamp"].as_str().and_then(parse_rfc3339);
    v["message"]["content"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|c| c["type"] == "tool_use")
        .flat_map(|c| match c["name"].as_str() {
            Some("Skill") => c["input"]["skill"]
                .as_str()
                .or_else(|| c["input"]["name"].as_str())
                .map(|s| s.rsplit(':').next().unwrap_or(s))
                .and_then(|name| use_from_dir(name, at))
                .into_iter()
                .collect(),
            Some("Read") => uses_in_text(c["input"]["file_path"].as_str().unwrap_or(""), at),
            _ => Vec::new(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skill_tool_call_counts_and_strips_plugin_prefix() {
        let line = r#"{"type":"assistant","timestamp":"2026-09-01T02:03:04.000Z","message":{"role":"assistant","content":[{"type":"text","text":"ok"},{"type":"tool_use","id":"t1","name":"Skill","input":{"skill":"caveman:caveman-review","args":"x"}}]}}"#;
        let u = parse_line(line);
        assert_eq!(u.len(), 1);
        assert_eq!(u[0].skill_name, "caveman-review");
        assert_eq!(u[0].skill_dir, "caveman-review");
    }

    #[test]
    fn reading_a_skill_file_counts() {
        let line = r#"{"type":"assistant","timestamp":"2026-09-01T02:03:04.000Z","message":{"content":[{"type":"tool_use","name":"Read","input":{"file_path":"/Users/me/.claude/skills/tdd/SKILL.md"}}]}}"#;
        let u = parse_line(line);
        assert_eq!(u[0].skill_dir, "/Users/me/.claude/skills/tdd");
    }

    #[test]
    fn user_text_mentioning_skill_is_ignored() {
        let line = r#"{"type":"user","timestamp":"2026-09-01T02:03:04.000Z","message":{"role":"user","content":"please read ~/.claude/skills/tdd/SKILL.md"}}"#;
        assert!(parse_line(line).is_empty());
    }
}
