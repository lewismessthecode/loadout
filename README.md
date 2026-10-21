<p align="center">
  <img src="design/icon-1024.png" width="112" alt="Loadout icon">
</p>

<h1 align="center">Loadout</h1>

<p align="center">
  A local manager for the agent skills on your Mac.<br>
  See every skill, where it came from, and how often you actually use it.
</p>

https://github.com/user-attachments/assets/d26bf419-6909-4e2b-a12b-e4c5bcb73985

Skills pile up. Claude Code, Codex, Cursor, OpenCode and the shared `~/.agents` folder each keep their own copy, plugins add more, and after a few months nobody remembers which ones are used. Loadout scans all of those folders, merges them into one list, and reads your own session logs to show when each skill was last used.

Your skill list, tags, and usage history stay on your machine. Network requests start only when you install skills, check for updates, or apply an update. Update checks read GitHub metadata. Installation and update preparation use the skills CLI.

## Install

1. Download `Loadout_0.2.1_universal.dmg` from the [latest release](https://github.com/lewismessthecode/loadout/releases/latest).
2. Open the DMG and drag Loadout to Applications.
3. Open Loadout.

The build is universal, so it runs natively on Apple silicon and Intel. It needs macOS 12 or later.

The app is signed ad hoc and is not notarized, because that needs a paid Apple Developer account. A copy downloaded through a browser carries a quarantine flag, and macOS may refuse its first launch. Clear the flag once:

```sh
xattr -dr com.apple.quarantine /Applications/Loadout.app
```

You can also open System Settings, go to Privacy & Security, and press Open Anyway after the first refusal. A copy you build yourself never gets the flag and opens straight away. Loadout never asks for Full Disk Access, Automation, Accessibility or Screen Recording. It only reads files under your home folder that your own user can already read.

## What it does

- **One list for your agents.** Scans `~/.agents/skills`, the skill folders of Claude Code, Codex, Cursor, Gemini and OpenCode, and supported plugin caches. Symlinks are followed, so a skill linked into three agents shows up once with three installs. Broken links are skipped.
- **Detected agents.** Agent configuration directories determine which agents appear first in the sidebar and installation chooser. Other supported installation targets can be expanded when needed. Select at least one destination before installing. Existing Windsurf folders remain readable, but Windsurf is no longer an installation target.
- **Live list.** Adding or removing a `SKILL.md` inside a watched skill folder updates the open window. Loadout waits 500 ms after the last change, then rescans the list and the sidebar. Usage counts and the heatmap update when you press refresh, or when the window gains focus. Refresh also rereads agent detection and configured scan roots.
- **Source and author.** Reads the `npx skills` lockfile at `~/.agents/.skill-lock.json` to show the repo each skill came from. Plugin skills get their source from the plugin cache path.
- **Usage evidence.** Sort by recorded uses or last observed use. No recent record means Loadout has not observed a use in the available logs. It does not establish that a skill is unused. Usage parsing currently covers Claude Code, Codex and Cursor.
- **Tags.** Add your own tags. They are stored locally and survive rescans.
- **Install.** Press Install, enter `owner/repo`, pick the agents, and Loadout runs `npx skills add` for you with live output.
- **Skill updates.** Check for updates explicitly, review the results, and apply selected updates. Loadout preserves the existing shared installation and its links, keeps the previous files in a backup, and reports the result for each skill.
- **Delete.** Removes the skill folder and its lockfile entry. Files go to the Trash, so you can put them back.
- **Light, dark and system appearance.** The theme is applied before first paint, so there is no flash.
- **English and Chinese.** The UI follows your system language and can be switched in the sidebar footer. Skill names, descriptions and paths are your own data and are never translated.
- **Keyboard.** Up and down arrows move through the list, `Cmd+F` focuses search, and `Esc` closes the detail panel.

## How usage is counted

Loadout does not instrument anything. It reads the session logs your agents already write and looks for evidence that a skill was loaded.

| Agent | What counts as a use |
| --- | --- |
| Claude Code | A `Skill` tool call, or a read of a `SKILL.md` |
| Codex | A tool call whose input mentions a `SKILL.md`, or a `<skill>` attachment |
| Cursor | A read of a `SKILL.md`, or a skill you attached by hand |

One skill counts at most once per session file per day, so a long session that reads the same skill ten times is one use. Counts are indexed incrementally in `~/Library/Application Support/Loadout/loadout.db`. Only the skill name, the agent and the day are stored. Prompts and replies are never copied. Delete the file and the next scan rebuilds it.

Counts are a lower bound. An agent that never reads a skill file, or logs in a format Loadout does not know, shows zero.

## Skill updates

Automatic checks support GitHub skills with a recorded folder hash and skill path in a version 3 skills lockfile. The skill must be an unambiguous shared installation under `~/.agents/skills`. Plugin caches, external checkouts, independent copies, unsupported sources and incomplete installation records show an explanation instead of an update action. Plugin skills remain managed by their coding agent's plugin system.

A check compares the recorded upstream revision with GitHub. A current result describes that recorded revision. It does not prove that local files are unmodified. Before applying an update, Loadout preserves the previous directory, including local additions, and shows the backup location in the result. Existing agent links keep their destinations.

Loadout prepares update files with a pinned skills CLI in a temporary home. It applies the reviewed revision to the existing installation and updates its lock entry. It does not run the upstream `skills check` or `skills update` command against your home directory. Checks never remove skills that disappeared upstream.

## Build from source

You need Node 22.20 or later, pnpm, and a Rust toolchain. Installing and preparing skill updates also requires Node and Git.

```sh
pnpm install
pnpm tauri dev
```

To build the installable app and DMG for both architectures:

```sh
rustup target add aarch64-apple-darwin x86_64-apple-darwin
pnpm release
```

That runs `tauri build` for a universal binary, then `scripts/make-dmg.sh` wraps the app in a DMG with `hdiutil`. The output is under `src-tauri/target/universal-apple-darwin/release/bundle/`, in `macos/` and `dmg/`. The DMG is built without Finder scripting on purpose, so the build asks for no Automation permission.

Tests:

```sh
pnpm test                       # frontend, vitest
cargo test --manifest-path src-tauri/Cargo.toml
```

Some Rust tests are ignored by default because they touch your real home folder. To check the scanner against your own machine, run `cargo test smoke_against_real_home -- --ignored --nocapture`.

## The icon

`design/icon.svg` is the source. `pnpm icon` renders it to `design/icon-1024.png` with resvg and regenerates everything under `src-tauri/icons`.

## Translations

Messages live in `src/lib/messages/en.ts` and `zh.ts`. `en.ts` defines the keys and `zh.ts` is typed against it, so a missing Chinese string fails `tsc`. A test also checks that both languages use the same `{placeholders}`. To add a language, add a catalog with the same shape and register it in `src/lib/i18n.tsx`.

## Stack

Tauri 2 and Rust for the scanner, the skill-folder watch, the log parsers and the SQLite index. React 19, Vite, Tailwind 4 and TanStack Query for the UI.

## License

MIT
