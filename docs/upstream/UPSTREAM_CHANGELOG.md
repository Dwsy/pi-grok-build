# Upstream Changelog

Changelog of upstream **Grok Build** (`xai-org/grok-build`) changes absorbed by
this fork (`Dwsy/grok-pi`). This is the **upstream update record**: it lists what
upstream changed and which features were affected, so each sync can be reviewed
before and after the merge.

> [!NOTE]
> Upstream commits are all titled `Synced from monorepo` and carry no useful
> message. Every feature description below is **derived from diff analysis**
> (changed files, added/removed code, new modules), not from commit messages.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Entries are **newest first**. This file is maintained by the
[`upstream-changelog`](../../.pi/skills/upstream-changelog/SKILL.md) skill.

## Entry schema

Each entry records:

| Field | Meaning |
|---|---|
| Upstream tip | Full upstream commit SHA being synced to |
| Range | `FROM..TO` git range (`merge-base..upstream-tip`) |
| SOURCE_REV | Monorepo revision from the `SOURCE_REV` file at the upstream tip |
| Date | Date the record was generated (YYYY-MM-DD) |
| Stats | Files changed, insertions(+), deletions(−) |
| Added / Changed / Fixed / Removed | Semantic feature bullets from diff analysis |
| Areas touched | Per-crate/area change statistics table |

---

## [3af4d5d] - 2026-07-22

- **Upstream tip:** `3af4d5d39897855bdcc74f23e690024a5dc05573`
- **Range:** `a881e67..3af4d5d`
- **SOURCE_REV:** `c5c4ce03436b4bb2cec43d3feaa27dee0109bf37`
- **Date:** 2026-07-22
- **Stats:** 556 files changed, +56609, −21892

### Added

- **Workflow engine** (`xai-workflow`): new crate with a durable workflow execution engine — `engine.rs` (1779 lines), journaling (`journal.rs`), metadata/validation (`meta.rs`, `validate.rs`), host interface (`host.rs`), and a `validate` example.
- **Execution risk scoring** (`xai-grok-workspace/permission/exec_risk.rs`): new 826-line module for scoring execution risk of commands.
- **Prompt queue combine** (`xai-prompt-queue/src/combine.rs`): new 247-line module for combining/merging queued prompts; new types in `types.rs`.
- **Permission auto-mode** (`xai-grok-workspace/permission/auto_mode.rs`): new 89-line module for automatic permission resolution.
- **Voice PCM processing** (`xai-grok-voice/src/pcm.rs`): new 63-line module for raw PCM audio handling.

### Changed

- **Permission system overhaul** (`xai-grok-workspace/permission/`): major expansion across `manager.rs` (+859), `policy.rs` (+440), `shell_access.rs` (+836), `bash_command_splitting.rs` (+718) — significantly richer command analysis, policy evaluation, and shell access control.
- **Workspace worktree** (`xai-grok-workspace/src/worktree/mod.rs`): +80 lines of worktree management changes.
- **Folder trust** (`xai-grok-workspace/src/folder_trust.rs`): updated trust model (+23).
- **Hub server** (`xai-grok-workspace/src/hub_server.rs`): updated server interface (+22).
- **Voice pipeline & probe** (`xai-grok-voice/`): pipeline rework (+72), probe changes (+43), lib updates.
- **Prompt queue core** (`xai-prompt-queue/src/lib.rs`): updated to integrate the new combine module.
- **Textarea** (`xai-ratatui-textarea/`): minor textarea adjustments.
- **Tool protocol & runtime** (`xai-tool-protocol/`, `xai-tool-runtime/`): identifier validation and error conversion test updates.
- **Computer Hub** (`xai-computer-hub-sdk/`, `xai-computer-hub-mcp-adapter/`): notification and bridge updates.
- **Hunk tracker** (`xai-hunk-tracker/src/actor/file_utils.rs`): file utility changes.

### Fixed

- _(none identified from diff analysis — this is a large monorepo sync dominated by new features and permission system expansion)_

### Removed

- _(none identified)_

### Areas touched

| Area | Files | + | − |
|---|---:|---:|---:|
| `xai-workflow` | 8 | 4254 | 0 |
| `xai-grok-workspace` | 14 | 3397 | 107 |
| `xai-prompt-queue` | 4 | 303 | 2 |
| `xai-grok-voice` | 4 | 184 | 3 |
| `xai-grok-pager` | ~180 | ~18000 | ~8000 |
| `xai-grok-shell` | ~60 | ~6000 | ~3000 |
| `xai-grok-tools` | ~80 | ~8000 | ~4000 |
| `xai-ratatui-textarea` | 2 | 6 | 2 |
| `xai-tool-protocol` | 1 | 9 | 0 |
| `xai-tool-runtime` | 2 | 20 | 2 |
| `xai-computer-hub-*` | 2 | 19 | 2 |
| `xai-hunk-tracker` | 1 | 23 | 0 |
| other / root | ~198 | ~16384 | ~6774 |
| **Total** | **556** | **+56609** | **−21892** |

> Note: per-area breakdown for the largest crates (`xai-grok-pager`, `xai-grok-shell`,
> `xai-grok-tools`) is approximate — the monorepo sync touches hundreds of files
> across these crates. The permission system and workflow engine are the highest-signal
> changes for Pi-Grok integration review.

### Integration risk notes

- **High risk:** `xai-grok-workspace/permission/` — the permission manager, policy, and shell access modules overlap with Pi-Grok's bash tool bridging and trust model. Review carefully during merge.
- **Medium risk:** `xai-grok-pager` — large surface area; check event loop, modal, picker, and app dispatch seams.
- **Low risk:** `xai-workflow` is a new isolated crate; `xai-prompt-queue/combine` is additive.
