"use client";

import { useDict } from "@/i18n/provider";

const nativeCommands = [
  { cmd: "exit", desc: "Exit grok-pi" },
  { cmd: "help", desc: "Show available commands" },
  { cmd: "hotkeys", desc: "Keyboard shortcuts modal (aliases: shortcuts, keys; same as Ctrl+.)" },
  { cmd: "new", desc: "Start a new session" },
  { cmd: "compact", desc: "Compact context; optional custom instructions" },
  { cmd: "model", desc: "Open model selector (Pi catalog)" },
  { cmd: "effort", desc: "Set thinking effort level" },
  { cmd: "rename", desc: "Rename the current session" },
  { cmd: "resume", desc: "Session picker (Pi JSONL catalog)" },
  { cmd: "session-info", desc: "Session stats + context snapshot (alias: session)" },
  { cmd: "dashboard", desc: "Agent dashboard (also Ctrl+\\)" },
  { cmd: "copy", desc: "Copy last response to clipboard" },
  { cmd: "find", desc: "Search in scrollback" },
  { cmd: "transcript", desc: "Export transcript" },
  { cmd: "export", desc: "Export as Markdown (Grok transcript)" },
  { cmd: "expand", desc: "Expand collapsed tool output" },
  { cmd: "queue", desc: "Show steering / follow-up queue" },
  { cmd: "notify", desc: "View in-process notifications" },
  { cmd: "multiline", desc: "Toggle multiline input" },
  { cmd: "compact-mode", desc: "Toggle compact display" },
  { cmd: "vim-mode", desc: "Toggle Vim keybindings" },
  { cmd: "theme", desc: "Switch theme (supports pi:<name>)" },
  { cmd: "timestamps", desc: "Toggle message timestamps" },
  { cmd: "toggle-mouse-reporting", desc: "Toggle mouse support" },
];

const sessionCommands = [
  { cmd: "/jump", desc: "Turn picker with timeline previews; restore viewport" },
  { cmd: "/fork", desc: "Branch from a chosen user message (Pi fork + rebind)" },
  { cmd: "/clone", desc: "Duplicate current leaf into a new session file" },
  { cmd: "/reload", desc: "ctx.reload(); blocked while streaming or compacting; refreshes catalogs + Pi themes" },
  { cmd: "/review-session", desc: "Native code-review modal over session file edits" },
  { cmd: "/review-message", desc: "Turn-scoped review via jump-style overlay" },
  { cmd: "/export-html", desc: "Export session as HTML (or pass a .jsonl path)" },
  { cmd: "/pi-share", desc: "Private GitHub gist + pi.dev viewer" },
  { cmd: "/context", desc: "Live token breakdown modal (system/tools/AGENTS/skills)" },
  { cmd: "/recap", desc: "Session recap (alias /summarize; optional focus text). Auto when away ≥3 min" },
];

const modeCommands = [
  { cmd: "Ctrl+Shift+T", desc: "Toggle plan mode (write gate + exit_plan_mode approval)" },
  { cmd: "/view-plan", desc: "Open session .plan.md when plan mode is active" },
  { cmd: "/goal", desc: "Goal mode MVP (F2 pi_goal, restart required; default off)" },
  { cmd: "/workflow", desc: "Launch a Rhai workflow (F2 pi_workflows, restart; default off)" },
  { cmd: "/workflows", desc: "List available workflows (user + project .grok-pi/workflows)" },
  { cmd: "/create-workflow", desc: "Pager prompt to scaffold a workflow script" },
];

const resourceCommands = [
  { cmd: "/pi-config", desc: "Rust-native Pi resource manager (alias /pi-resources; also F2 → Pi resources)" },
  { cmd: "/login", desc: "Authenticate with Pi (Remote TUI path)" },
  { cmd: "/logout", desc: "Clear Pi credentials (Remote TUI path)" },
  { cmd: "/voice", desc: "Voice dictation via xAI STT (Ctrl+Space / F8)" },
];

const treeNotes = [
  "Double-Esc (empty prompt) or /rewind opens SessionTree (Pi navigateTree — non-destructive branches).",
  "SessionTree: Enter navigate, filter/search/tags; with F2 pi_tree_file_rollback: r = preview, R = execute file rollback.",
];

const excludedCommands = [
  "history", "usage", "plugins", "mcp", "memory", "workspace", "share", "debug",
  "minimal", "fullscreen",
];

function Table({
  rows,
}: {
  rows: { cmd: string; desc: string }[];
}) {
  return (
    <div className="mt-4 overflow-x-auto rounded-md border border-border">
      <table className="w-full text-sm">
        <thead>
          <tr className="border-b border-border bg-surface/80">
            <th className="px-4 py-3 text-left font-semibold text-text-secondary">
              Command
            </th>
            <th className="px-4 py-3 text-left font-semibold text-text-secondary">
              Description
            </th>
          </tr>
        </thead>
        <tbody>
          {rows.map((c, i) => (
            <tr
              key={c.cmd}
              className={`border-b border-border/50 ${i % 2 === 1 ? "bg-surface/20" : ""}`}
            >
              <td className="px-4 py-2.5 font-mono text-xs text-accent whitespace-nowrap">
                {c.cmd.startsWith("/") || c.cmd.startsWith("Ctrl")
                  ? c.cmd
                  : `/${c.cmd}`}
              </td>
              <td className="px-4 py-2.5 text-text-secondary">{c.desc}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

export default function CommandsPage() {
  const dict = useDict();
  return (
    <div>
      <h1 className="text-3xl font-bold tracking-tight">
        {dict.docs.sidebar.commands}
      </h1>
      <p className="mt-4 text-lg leading-relaxed text-text-secondary">
        grok-pi keeps Grok Pager slash surfaces and maps Pi capabilities through
        the ACP command catalog. Extension / prompt / skill commands from Pi
        appear dynamically.
      </p>

      <h2 className="mt-10 text-xl font-semibold">Retained Grok native</h2>
      <Table rows={nativeCommands} />

      <h2 className="mt-10 text-xl font-semibold">Session & tree</h2>
      <Table rows={sessionCommands} />
      <ul className="mt-4 space-y-1.5 text-sm text-text-secondary list-disc pl-5">
        {treeNotes.map((n) => (
          <li key={n}>{n}</li>
        ))}
      </ul>

      <h2 className="mt-10 text-xl font-semibold">Plan, goal, workflows</h2>
      <p className="mt-2 text-sm text-text-secondary">
        Goal and workflows are F2 opt-in (default off) and require a full quit +
        restart so the host can inject extensions at process start.
      </p>
      <Table rows={modeCommands} />

      <h2 className="mt-10 text-xl font-semibold">Resources, auth, voice</h2>
      <Table rows={resourceCommands} />

      <h2 className="mt-10 text-xl font-semibold">Dynamic Pi commands</h2>
      <p className="mt-2 text-text-secondary">
        Extension, prompt, and skill commands returned by Pi are not hard-coded
        in Rust. They enter the Grok slash dropdown via ACP; name conflicts are
        de-duplicated by the Grok registry. Bridge-only names like{" "}
        <code className="font-mono text-xs text-accent">__pi_workflow_*</code>{" "}
        are filtered from the catalog.
      </p>

      <h2 className="mt-10 text-xl font-semibold">Deliberately excluded</h2>
      <p className="mt-2 text-text-secondary">
        Grok product / cloud-store commands and startup-only renderer modes are
        not exposed (Pi owns sessions; switch fullscreen/minimal at launch):
      </p>
      <div className="mt-3 flex flex-wrap gap-2">
        {excludedCommands.map((cmd) => (
          <code
            key={cmd}
            className="px-2.5 py-1 rounded-md bg-surface border border-border text-xs font-mono text-text-tertiary line-through"
          >
            /{cmd}
          </code>
        ))}
      </div>
      <p className="mt-4 text-sm text-text-tertiary">
        Bare <code className="font-mono text-accent">/login</code> /{" "}
        <code className="font-mono text-accent">/logout</code> here are Pi auth
        (Remote TUI), not Grok cloud login.
      </p>
    </div>
  );
}
