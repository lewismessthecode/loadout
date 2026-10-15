use crate::agents::{AgentDef, RootKind};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Found {
    pub agent: &'static str,
    pub kind: RootKind,
    pub path: PathBuf,
    pub canonical: PathBuf,
    pub is_symlink: bool,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Meta {
    pub name: Option<String>,
    pub description: String,
    pub tags: Vec<String>,
}

const SKIP_DIRS: &[&str] = &["node_modules", ".git", "target", "dist", "__pycache__"];

pub fn scan(home: &Path, agents: &[AgentDef]) -> Vec<Found> {
    let mut out = Vec::new();
    for agent in agents {
        for root in agent.roots {
            let dir = home.join(root.rel);
            walk(&dir, 0, root.kind.max_depth(), agent.id, root.kind, &mut out);
        }
    }
    out
}

fn walk(
    dir: &Path,
    depth: usize,
    max_depth: usize,
    agent: &'static str,
    kind: RootKind,
    out: &mut Vec<Found>,
) {
    let Ok(read) = fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = read.flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let name = entry.file_name().to_string_lossy().to_string();
        if SKIP_DIRS.contains(&name.as_str()) {
            continue;
        }
        let path = entry.path();
        let Ok(meta) = fs::metadata(&path) else { continue };
        if !meta.is_dir() {
            continue;
        }
        if path.join("SKILL.md").is_file() {
            let is_symlink = fs::symlink_metadata(&path)
                .map(|m| m.file_type().is_symlink())
                .unwrap_or(false);
            let canonical = fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
            out.push(Found { agent, kind, path, canonical, is_symlink });
        } else if depth + 1 < max_depth && (!name.starts_with('.') || name == ".system") {
            walk(&path, depth + 1, max_depth, agent, kind, out);
        }
    }
}

pub fn read_meta(skill_dir: &Path) -> Meta {
    fs::read_to_string(skill_dir.join("SKILL.md"))
        .map(|t| parse_frontmatter(&t))
        .unwrap_or_default()
}

pub fn parse_frontmatter(text: &str) -> Meta {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut lines = text.lines();
    if lines.next().map(str::trim_end) != Some("---") {
        return Meta::default();
    }
    let mut block: Vec<&str> = Vec::new();
    for l in lines {
        if l.trim_end() == "---" {
            break;
        }
        block.push(l);
    }

    let mut meta = Meta::default();
    let mut i = 0;
    while i < block.len() {
        let line = block[i];
        i += 1;
        let Some((key, rest)) = split_key(line) else { continue };
        let mut parts: Vec<String> = Vec::new();
        let first = rest.trim();
        let is_block_indicator = matches!(first.chars().next(), Some('>') | Some('|'))
            && first.chars().skip(1).all(|c| c == '-' || c == '+' || c.is_ascii_digit());
        if !first.is_empty() && !is_block_indicator {
            parts.push(first.to_string());
        }
        while i < block.len() {
            let next = block[i];
            if next.trim().is_empty() {
                i += 1;
                continue;
            }
            if next.starts_with(' ') || next.starts_with('\t') {
                parts.push(next.trim().to_string());
                i += 1;
            } else {
                break;
            }
        }
        let value = unquote(&parts.join(" "));
        match key {
            "name" => meta.name = Some(value).filter(|v| !v.is_empty()),
            "description" => meta.description = value,
            "tags" => meta.tags = parse_inline_list(&value),
            _ => {}
        }
    }
    meta
}

fn split_key(line: &str) -> Option<(&str, &str)> {
    if line.starts_with(' ') || line.starts_with('\t') || line.starts_with('#') {
        return None;
    }
    let idx = line.find(':')?;
    let key = &line[..idx];
    if key.is_empty() || !key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
        return None;
    }
    Some((key, &line[idx + 1..]))
}

fn unquote(s: &str) -> String {
    let s = s.trim();
    if s.len() >= 2 {
        let b = s.as_bytes();
        if (b[0] == b'"' && b[b.len() - 1] == b'"') || (b[0] == b'\'' && b[b.len() - 1] == b'\'') {
            let inner = &s[1..s.len() - 1];
            return if b[0] == b'"' { inner.replace("\\\"", "\"") } else { inner.replace("''", "'") };
        }
    }
    s.to_string()
}

fn parse_inline_list(v: &str) -> Vec<String> {
    let v = v.trim();
    let Some(inner) = v.strip_prefix('[').and_then(|x| x.strip_suffix(']')) else {
        return Vec::new();
    };
    inner
        .split(',')
        .map(|t| unquote(t.trim()))
        .filter(|t| !t.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::{AgentDef, Root, UsageSource};

    #[test]
    fn frontmatter_plain_and_quoted() {
        let m = parse_frontmatter("---\nname: tdd\ndescription: Test-driven development. Use when \"x\".\n---\n# body");
        assert_eq!(m.name.as_deref(), Some("tdd"));
        assert_eq!(m.description, "Test-driven development. Use when \"x\".");
        let m = parse_frontmatter("---\nname: \"a\"\ndescription: 'it''s fine'\n---\n");
        assert_eq!(m.name.as_deref(), Some("a"));
        assert_eq!(m.description, "it's fine");
    }

    #[test]
    fn frontmatter_folded_and_multiline() {
        let m = parse_frontmatter(
            "---\nname: find-your-level\nversion: 1.0.0\ndescription: >\n  Interactive quiz that maps\n  your knowledge.\ntags: [assessment, \"quiz\"]\n---\nbody",
        );
        assert_eq!(m.description, "Interactive quiz that maps your knowledge.");
        assert_eq!(m.tags, vec!["assessment", "quiz"]);
        let m = parse_frontmatter("---\nname: x\ndescription: first\n  second line\n---\n");
        assert_eq!(m.description, "first second line");
    }

    #[test]
    fn frontmatter_missing() {
        assert_eq!(parse_frontmatter("# no frontmatter"), Meta::default());
    }

    static ROOTS: [Root; 1] = [Root { rel: ".agents/skills", kind: RootKind::Skills }];
    static CURSOR: [Root; 1] = [Root { rel: ".cursor/skills", kind: RootKind::Skills }];

    #[test]
    fn scan_finds_dirs_and_symlinks() {
        let home = tempfile::tempdir().unwrap();
        let skill = home.path().join(".agents/skills/tdd");
        fs::create_dir_all(&skill).unwrap();
        fs::write(skill.join("SKILL.md"), "---\nname: tdd\n---\n").unwrap();
        let nested = home.path().join(".agents/skills/.system/imagegen");
        fs::create_dir_all(&nested).unwrap();
        fs::write(nested.join("SKILL.md"), "---\nname: imagegen\n---\n").unwrap();
        fs::create_dir_all(home.path().join(".agents/skills/not-a-skill")).unwrap();
        fs::create_dir_all(home.path().join(".cursor/skills")).unwrap();
        std::os::unix::fs::symlink(&skill, home.path().join(".cursor/skills/tdd")).unwrap();
        std::os::unix::fs::symlink("/nonexistent", home.path().join(".cursor/skills/broken")).unwrap();

        let agents = [
            AgentDef { id: "shared", label: "s", cli_name: None, roots: &ROOTS, usage: UsageSource::None },
            AgentDef { id: "cursor", label: "c", cli_name: Some("cursor"), roots: &CURSOR, usage: UsageSource::None },
        ];
        let found = scan(home.path(), &agents);
        assert_eq!(found.len(), 3);
        let link = found.iter().find(|f| f.agent == "cursor").unwrap();
        assert!(link.is_symlink);
        let real = found.iter().find(|f| f.agent == "shared" && f.path.ends_with("tdd")).unwrap();
        assert!(!real.is_symlink);
        assert_eq!(link.canonical, real.canonical);
        assert!(found.iter().any(|f| f.path.ends_with(".system/imagegen")));
    }
}
