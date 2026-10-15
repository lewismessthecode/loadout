use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UsageSource {
    Codex,
    Cursor,
    Claude,
    None,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
}

const fn skills(rel: &'static str) -> Root {
    Root { rel, kind: RootKind::Skills }
}

const fn plugins(rel: &'static str) -> Root {
    Root { rel, kind: RootKind::PluginCache }
}

pub const SHARED_ID: &str = "shared";

pub static AGENTS: &[AgentDef] = &[
    AgentDef {
        id: SHARED_ID,
        label: "Shared (.agents)",
        cli_name: None,
        roots: &[skills(".agents/skills")],
        usage: UsageSource::None,
    },
    AgentDef {
        id: "claude",
        label: "Claude Code",
        cli_name: Some("claude-code"),
        roots: &[skills(".claude/skills"), plugins(".claude/plugins/cache")],
        usage: UsageSource::Claude,
    },
    AgentDef {
        id: "codex",
        label: "Codex",
        cli_name: Some("codex"),
        roots: &[skills(".codex/skills")],
        usage: UsageSource::Codex,
    },
    AgentDef {
        id: "cursor",
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
        label: "Gemini CLI",
        cli_name: Some("gemini-cli"),
        roots: &[skills(".gemini/skills")],
        usage: UsageSource::None,
    },
    AgentDef {
        id: "opencode",
        label: "OpenCode",
        cli_name: Some("opencode"),
        roots: &[skills(".config/opencode/skills")],
        usage: UsageSource::None,
    },
    AgentDef {
        id: "windsurf",
        label: "Windsurf",
        cli_name: Some("windsurf"),
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
}
