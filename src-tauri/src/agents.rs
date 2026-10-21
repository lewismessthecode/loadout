use serde::Serialize;
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UsageSource {
    Codex,
    Cursor,
    Claude,
    None,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RootKind {
    Skills,
    PluginCache,
}

impl RootKind {
    pub fn max_depth(self) -> usize {
        match self {
            RootKind::Skills => 2,
            RootKind::PluginCache => 8,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Root {
    pub rel: &'static str,
    pub kind: RootKind,
}

#[derive(Clone, Copy, Debug)]
pub struct AgentDef {
    pub id: &'static str,
    pub label: &'static str,
    pub cli_name: Option<&'static str>,
    pub roots: &'static [Root],
    pub usage: UsageSource,
    pub detection_root: &'static str,
}

const fn skills(rel: &'static str) -> Root {
    Root {
        rel,
        kind: RootKind::Skills,
    }
}

const fn plugins(rel: &'static str) -> Root {
    Root {
        rel,
        kind: RootKind::PluginCache,
    }
}

pub const SHARED_ID: &str = "shared";

pub static AGENTS: &[AgentDef] = &[
    AgentDef {
        id: SHARED_ID,
        detection_root: ".agents",
        label: "Shared (.agents)",
        cli_name: None,
        roots: &[skills(".agents/skills")],
        usage: UsageSource::None,
    },
    AgentDef {
        id: "claude",
        detection_root: ".claude",
        label: "Claude Code",
        cli_name: Some("claude-code"),
        roots: &[skills(".claude/skills"), plugins(".claude/plugins/cache")],
        usage: UsageSource::Claude,
    },
    AgentDef {
        id: "codex",
        detection_root: ".codex",
        label: "Codex",
        cli_name: Some("codex"),
        roots: &[skills(".codex/skills"), plugins(".codex/plugins/cache")],
        usage: UsageSource::Codex,
    },
    AgentDef {
        id: "cursor",
        detection_root: ".cursor",
        label: "Cursor",
        cli_name: Some("cursor"),
        roots: &[
            skills(".cursor/skills"),
            skills(".cursor/skills-cursor"),
            plugins(".cursor/plugins/cache"),
        ],
        usage: UsageSource::Cursor,
    },
    AgentDef {
        id: "gemini",
        detection_root: ".gemini",
        label: "Gemini CLI",
        cli_name: Some("gemini-cli"),
        roots: &[skills(".gemini/skills")],
        usage: UsageSource::None,
    },
    AgentDef {
        id: "opencode",
        detection_root: ".config/opencode",
        label: "OpenCode",
        cli_name: Some("opencode"),
        roots: &[skills(".config/opencode/skills")],
        usage: UsageSource::None,
    },
    AgentDef {
        id: "windsurf",
        detection_root: ".codeium/windsurf",
        label: "Windsurf",
        cli_name: None,
        roots: &[skills(".codeium/windsurf/skills")],
        usage: UsageSource::None,
    },
];

pub fn find(id: &str) -> Option<&'static AgentDef> {
    AGENTS.iter().find(|a| a.id == id)
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AgentInfo {
    pub id: &'static str,
    pub label: &'static str,
    pub detected: bool,
    pub installable: bool,
    pub usage_supported: bool,
    pub roots: Vec<ScanRoot>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ScanRoot {
    pub path: String,
    pub available: bool,
    pub kind: RootKind,
}

pub fn detected(home: &Path, agent: &AgentDef) -> bool {
    home.join(agent.detection_root).is_dir()
}
