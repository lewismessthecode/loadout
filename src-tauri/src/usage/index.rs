use super::{claude, codex, cursor, RawUse};
use crate::agents::UsageSource;
use crate::catalog::local_day;
use crate::model::StoredEvent;
use chrono::{DateTime, TimeZone, Utc};
use rusqlite::{params, Connection};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

pub struct Index {
    conn: Connection,
}

pub struct LogFile {
    pub agent: &'static str,
    pub source: UsageSource,
    pub path: PathBuf,
}

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS files (path TEXT PRIMARY KEY, mtime INTEGER NOT NULL, size INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS events (
  file TEXT NOT NULL, agent TEXT NOT NULL, skill_dir TEXT NOT NULL, skill_name TEXT NOT NULL,
  at INTEGER NOT NULL, day TEXT NOT NULL,
  UNIQUE (file, skill_name, day)
);
CREATE INDEX IF NOT EXISTS events_at ON events (at);
CREATE TABLE IF NOT EXISTS tags (skill_id TEXT NOT NULL, tag TEXT NOT NULL, PRIMARY KEY (skill_id, tag));
";

impl Index {
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        if let Some(dir) = path.parent() {
            let _ = fs::create_dir_all(dir);
        }
        Self::init(Connection::open(path)?)
    }

    pub fn open_in_memory() -> rusqlite::Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> rusqlite::Result<Self> {
        conn.execute_batch(SCHEMA)?;
        Ok(Index { conn })
    }

    pub fn refresh(&mut self, logs: &[LogFile]) -> rusqlite::Result<usize> {
        let known: HashMap<String, (i64, i64)> = {
            let mut stmt = self.conn.prepare("SELECT path, mtime, size FROM files")?;
            let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, (r.get(1)?, r.get(2)?))))?;
            rows.collect::<Result<_, _>>()?
        };
        let present: HashSet<String> = logs.iter().map(|l| l.path.to_string_lossy().to_string()).collect();

        let tx = self.conn.transaction()?;
        for gone in known.keys().filter(|p| !present.contains(*p)) {
            tx.execute("DELETE FROM events WHERE file = ?1", [gone])?;
            tx.execute("DELETE FROM files WHERE path = ?1", [gone])?;
        }

        let mut reparsed = 0;
        for log in logs {
            let key = log.path.to_string_lossy().to_string();
            let Ok(meta) = fs::metadata(&log.path) else { continue };
            let mtime_dt = meta.modified().ok().map(DateTime::<Utc>::from);
            let mtime = mtime_dt.map(|d| d.timestamp()).unwrap_or(0);
            let size = meta.len() as i64;
            if known.get(&key) == Some(&(mtime, size)) {
                continue;
            }
            let uses = parse_file(log);
            tx.execute("DELETE FROM events WHERE file = ?1", [&key])?;
            for u in uses {
                let at = u.at.or(mtime_dt).unwrap_or_else(|| Utc.timestamp_opt(0, 0).unwrap());
                tx.execute(
                    "INSERT OR IGNORE INTO events (file, agent, skill_dir, skill_name, at, day) VALUES (?1,?2,?3,?4,?5,?6)",
                    params![key, log.agent, u.skill_dir, u.skill_name, at.timestamp(), local_day(at)],
                )?;
            }
            tx.execute(
                "INSERT INTO files (path, mtime, size) VALUES (?1,?2,?3)
                 ON CONFLICT(path) DO UPDATE SET mtime = excluded.mtime, size = excluded.size",
                params![key, mtime, size],
            )?;
            reparsed += 1;
        }
        tx.commit()?;
        Ok(reparsed)
    }

    pub fn events(&self) -> rusqlite::Result<Vec<StoredEvent>> {
        let mut stmt = self.conn.prepare("SELECT agent, skill_dir, skill_name, at FROM events ORDER BY at")?;
        let rows = stmt.query_map([], |r| {
            Ok(StoredEvent {
                agent: r.get(0)?,
                skill_dir: r.get(1)?,
                skill_name: r.get(2)?,
                at: Utc.timestamp_opt(r.get::<_, i64>(3)?, 0).unwrap(),
            })
        })?;
        rows.collect()
    }

    pub fn all_tags(&self) -> rusqlite::Result<HashMap<String, Vec<String>>> {
        let mut stmt = self.conn.prepare("SELECT skill_id, tag FROM tags ORDER BY tag")?;
        let mut map: HashMap<String, Vec<String>> = HashMap::new();
        for row in stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))? {
            let (id, tag) = row?;
            map.entry(id).or_default().push(tag);
        }
        Ok(map)
    }

    pub fn set_tags(&mut self, skill_id: &str, tags: &[String]) -> rusqlite::Result<()> {
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM tags WHERE skill_id = ?1", [skill_id])?;
        for t in tags.iter().map(|t| t.trim()).filter(|t| !t.is_empty()) {
            tx.execute("INSERT OR IGNORE INTO tags (skill_id, tag) VALUES (?1, ?2)", params![skill_id, t])?;
        }
        tx.commit()
    }
}

fn parse_file(log: &LogFile) -> Vec<RawUse> {
    let Ok(file) = fs::File::open(&log.path) else { return Vec::new() };
    let mut out = Vec::new();
    let mut ctx = cursor::Ctx::default();
    for line in BufReader::new(file).split(b'\n').flatten() {
        let Ok(line) = String::from_utf8(line) else { continue };
        match log.source {
            UsageSource::Codex if codex::prefilter(&line) => out.extend(codex::parse_line(&line)),
            UsageSource::Cursor if cursor::prefilter(&line) => out.extend(cursor::parse_line(&line, &mut ctx)),
            UsageSource::Claude if claude::prefilter(&line) => out.extend(claude::parse_line(&line)),
            _ => {}
        }
    }
    out
}

pub fn discover(home: &Path) -> Vec<LogFile> {
    let mut out = Vec::new();
    let mut push = |agent: &'static str, source: UsageSource, dir: PathBuf, max_depth: usize| {
        let mut files = Vec::new();
        collect_jsonl(&dir, max_depth, &mut files);
        out.extend(files.into_iter().map(|path| LogFile { agent, source, path }));
    };
    push("codex", UsageSource::Codex, home.join(".codex/sessions"), 5);
    push("codex", UsageSource::Codex, home.join(".codex/archived_sessions"), 2);
    push("claude", UsageSource::Claude, home.join(".claude/projects"), 4);
    if let Ok(projects) = fs::read_dir(home.join(".cursor/projects")) {
        for p in projects.flatten() {
            push("cursor", UsageSource::Cursor, p.path().join("agent-transcripts"), 5);
        }
    }
    out
}

fn collect_jsonl(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    let Ok(read) = fs::read_dir(dir) else { return };
    for entry in read.flatten() {
        let Ok(ft) = entry.file_type() else { continue };
        let path = entry.path();
        if ft.is_dir() {
            if depth > 0 {
                collect_jsonl(&path, depth - 1, out);
            }
        } else if ft.is_file() && path.extension().map(|e| e == "jsonl").unwrap_or(false) {
            out.push(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &Path, rel: &str, lines: &[&str]) -> PathBuf {
        let p = dir.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(&p, lines.join("\n") + "\n").unwrap();
        p
    }

    const CODEX_USE: &str = r#"{"timestamp":"2026-10-05T15:00:12.960Z","type":"response_item","payload":{"type":"custom_tool_call","input":"cmd:\"cat /Users/me/.agents/skills/tdd/SKILL.md\""}}"#;

    #[test]
    fn refresh_is_incremental_and_dedupes_per_session_day() {
        let home = tempfile::tempdir().unwrap();
        let p = write(home.path(), ".codex/sessions/2026/10/05/a.jsonl", &[CODEX_USE, CODEX_USE, "garbage"]);
        write(
            home.path(),
            ".cursor/projects/proj/agent-transcripts/s1/s1.jsonl",
            &[
                r#"{"role":"user","message":{"content":[{"type":"text","text":"<timestamp>Friday, Sep 4, 2026, 10:31 AM (UTC+8)</timestamp>"}]}}"#,
                r#"{"role":"assistant","message":{"content":[{"type":"tool_use","name":"Read","input":{"path":"/a/.cursor/skills/frontend-design/SKILL.md"}}]}}"#,
            ],
        );

        let mut idx = Index::open_in_memory().unwrap();
        assert_eq!(idx.refresh(&discover(home.path())).unwrap(), 2);
        let events = idx.events().unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events.iter().filter(|e| e.skill_name == "tdd").count(), 1);
        assert_eq!(events[0].skill_name, "frontend-design");
        assert_eq!(events[0].agent, "cursor");

        assert_eq!(idx.refresh(&discover(home.path())).unwrap(), 0);

        let later = CODEX_USE.replace("2026-10-05", "2026-10-06");
        let mut f = fs::OpenOptions::new().append(true).open(&p).unwrap();
        std::io::Write::write_all(&mut f, format!("{later}\n").as_bytes()).unwrap();
        assert_eq!(idx.refresh(&discover(home.path())).unwrap(), 1);
        assert_eq!(idx.events().unwrap().iter().filter(|e| e.skill_name == "tdd").count(), 2);

        fs::remove_file(&p).unwrap();
        idx.refresh(&discover(home.path())).unwrap();
        assert_eq!(idx.events().unwrap().len(), 1);
    }

    #[test]
    fn tags_roundtrip() {
        let mut idx = Index::open_in_memory().unwrap();
        idx.set_tags("abc", &["b".into(), " a ".into(), "".into(), "b".into()]).unwrap();
        assert_eq!(idx.all_tags().unwrap()["abc"], vec!["a", "b"]);
        idx.set_tags("abc", &[]).unwrap();
        assert!(idx.all_tags().unwrap().is_empty());
    }
}
