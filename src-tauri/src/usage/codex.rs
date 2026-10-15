use super::{parse_rfc3339, use_from_dir, uses_in_text, RawUse};
use serde_json::Value;

pub fn prefilter(line: &str) -> bool {
    line.contains("SKILL.md")
}

pub fn parse_line(line: &str) -> Vec<RawUse> {
    let Ok(v) = serde_json::from_str::<Value>(line) else { return Vec::new() };
    if v["type"] != "response_item" {
        return Vec::new();
    }
    let at = v["timestamp"].as_str().and_then(parse_rfc3339);
    let p = &v["payload"];
    match p["type"].as_str() {
        Some("custom_tool_call") => uses_in_text(p["input"].as_str().unwrap_or(""), at),
        Some("function_call") => uses_in_text(p["arguments"].as_str().unwrap_or(""), at),
        Some("local_shell_call") => uses_in_text(&p["action"].to_string(), at),
        Some("message") if p["role"] == "user" => p["content"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|c| c["text"].as_str())
            .filter_map(|t| attached_skill(t, at))
            .collect(),
        _ => Vec::new(),
    }
}

fn attached_skill(text: &str, at: Option<chrono::DateTime<chrono::Utc>>) -> Option<RawUse> {
    let body = text.trim_start().strip_prefix("<skill>")?;
    let path = between(body, "<path>", "</path>")?;
    let dir = path.strip_suffix("/SKILL.md")?;
    use_from_dir(dir, at)
}

fn between<'a>(s: &'a str, open: &str, close: &str) -> Option<&'a str> {
    let start = s.find(open)? + open.len();
    let end = s[start..].find(close)? + start;
    Some(s[start..end].trim())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exec_reading_skill_counts() {
        let line = r#"{"timestamp": "2026-10-05T15:00:12.960Z", "ordinal": 293, "type": "response_item", "payload": {"type": "custom_tool_call", "id": "ctc_0c", "status": "completed", "call_id": "call_8V", "name": "exec", "input": "text(await tools.exec_command({cmd:\"cat /Users/lewisliu/.agents/skills/diagnosing-bugs/SKILL.md\",max_output_tokens:4500}));\n"}}"#;
        let u = parse_line(line);
        assert_eq!(u.len(), 1);
        assert_eq!(u[0].skill_name, "diagnosing-bugs");
        assert_eq!(u[0].skill_dir, "/Users/lewisliu/.agents/skills/diagnosing-bugs");
        assert_eq!(u[0].at.unwrap().to_rfc3339(), "2026-10-05T15:00:12.960+00:00");
    }

    #[test]
    fn skill_listing_in_system_prompt_is_not_usage() {
        let line = r##"{"timestamp":"2026-10-05T13:39:37.599Z","type":"response_item","payload":{"type":"message","role":"developer","content":[{"type":"input_text","text":"# Skills\nA skill is a set of local instructions to follow that is stored in a `SKILL.md` file.\n- imagegen: Create images (file: r2/imagegen/SKILL.md)\n- skill-creator: Guide (file: r2/skill-creator/SKILL.md)"}]}}"##;
        assert!(parse_line(line).is_empty());
    }

    #[test]
    fn searching_without_a_specific_dir_is_not_usage() {
        let line = r#"{"timestamp":"2026-10-05T13:00:00Z","type":"response_item","payload":{"type":"custom_tool_call","name":"exec","input":"text(await tools.exec_command({cmd:\"pwd; rg --files -g AGENTS.md -g SKILL.md\"}))"}}"#;
        assert!(parse_line(line).is_empty());
        let glob = r#"{"timestamp":"2026-10-05T13:00:00Z","type":"response_item","payload":{"type":"custom_tool_call","name":"exec","input":"cmd:\"ls ~/.agents/skills/*/SKILL.md\""}}"#;
        assert!(parse_line(glob).is_empty());
    }

    #[test]
    fn attached_skill_block_counts() {
        let line = r#"{"timestamp": "2026-06-24T07:41:29.367Z", "type": "response_item", "payload": {"type": "message", "role": "user", "content": [{"type": "input_text", "text": "<skill>\n<name>find-your-level</name>\n<path>/Users/lewisliu/Dev/x/.agents/skills/find-your-level/SKILL.md</path>\n---\nname: find-your-level\n---"}]}}"#;
        let u = parse_line(line);
        assert_eq!(u.len(), 1);
        assert_eq!(u[0].skill_name, "find-your-level");
    }

    #[test]
    fn tool_output_is_ignored() {
        let line = r#"{"timestamp":"2026-10-05T15:00:13Z","type":"response_item","payload":{"type":"custom_tool_call_output","output":"see /a/b/tdd/SKILL.md"}}"#;
        assert!(parse_line(line).is_empty());
    }
}
