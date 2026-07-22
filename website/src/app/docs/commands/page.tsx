"use client";

import { useDict } from "@/i18n/provider";

const nativeCommands = [
  { cmd: "exit", desc: "Exit grok-pi" },
  { cmd: "help", desc: "Show available commands" },
  { cmd: "hotkeys", desc: "Show keyboard shortcuts (aliases: shortcuts, keys)" },
  { cmd: "new", desc: "Start a new session" },
  { cmd: "compact", desc: "Compact context with optional custom instructions" },
  { cmd: "model", desc: "Open model selector" },
  { cmd: "effort", desc: "Set thinking effort level" },
  { cmd: "rename", desc: "Rename the current session" },
  { cmd: "resume", desc: "Open session picker to resume a previous session" },
  { cmd: "session-info", desc: "Show session stats and context breakdown (alias: session)" },
  { cmd: "dashboard", desc: "Open agent dashboard (also Ctrl+\\)" },
  { cmd: "copy", desc: "Copy last response to clipboard" },
  { cmd: "find", desc: "Search in scrollback" },
  { cmd: "transcript", desc: "Export transcript" },
  { cmd: "export", desc: "Export as Markdown" },
  { cmd: "expand", desc: "Expand collapsed tool output" },
  { cmd: "queue", desc: "Show steering/follow-up queue" },
  { cmd: "notify", desc: "View in-process notifications" },
  { cmd: "multiline", desc: "Toggle multiline input mode" },
  { cmd: "compact-mode", desc: "Toggle compact display mode" },
  { cmd: "vim-mode", desc: "Toggle Vim keybindings" },
  { cmd: "theme", desc: "Switch theme (supports pi:<name>)" },
  { cmd: "timestamps", desc: "Toggle message timestamps" },
  { cmd: "toggle-mouse-reporting", desc: "Toggle mouse support" },
];

const piCommands = [
  { cmd: "/pi-config", desc: "Open native resource manager for extensions, skills, prompts, themes (alias: /pi-resources, F2)" },
  { cmd: "/export-html", desc: "Export session as HTML (or .jsonl)" },
  { cmd: "/pi-share", desc: "Share session via private GitHub gist + pi.dev viewer" },
  { cmd: "/context", desc: "Open context breakdown modal with live chart" },
  { cmd: "/recap", desc: "Generate session recap (auto-generated when away ≥3 min)" },
  { cmd: "/voice", desc: "Voice dictation via xAI STT (also Ctrl+Space / F8)" },
  { cmd: "/login", desc: "Authenticate with Pi (Remote TUI)" },
  { cmd: "/logout", desc: "Clear Pi credentials (Remote TUI)" },
];

const excludedCommands = [
  "history", "login", "logout", "usage", "plugins", "mcp",
  "memory", "workspace", "share", "voice", "debug",
];

export default function CommandsPage() {
  const dict = useDict();
  return (
    <div>
      <h1 className="text-3xl font-bold tracking-tight">{dict.docs.sidebar.commands}</h1>
      <p className="mt-4 text-lg leading-relaxed text-text-secondary">
        grok-pi retains Grok&apos;s native slash commands and adds Pi-powered commands through the ACP command catalog. Extension, prompt, and skill commands from Pi appear dynamically.
      </p>

      <h2 className="mt-10 text-xl font-semibold">Retained Grok native commands</h2>
      <div className="mt-4 overflow-x-auto rounded-md border border-border">
        <table className="w-full text-sm">
          <thead>
            <tr className="border-b border-border bg-surface/80">
              <th className="px-4 py-3 text-left font-semibold text-text-secondary">Command</th>
              <th className="px-4 py-3 text-left font-semibold text-text-secondary">Description</th>
            </tr>
          </thead>
          <tbody>
            {nativeCommands.map((c, i) => (
              <tr key={c.cmd} className={`border-b border-border/50 ${i % 2 === 1 ? "bg-surface/20" : ""}`}>
                <td className="px-4 py-2.5 font-mono text-xs text-accent whitespace-nowrap">/{c.cmd}</td>
                <td className="px-4 py-2.5 text-text-secondary">{c.desc}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      <h2 className="mt-10 text-xl font-semibold">Pi-powered commands</h2>
      <p className="mt-2 text-text-secondary">
        These commands bridge Pi capabilities into the native Grok experience.
      </p>
      <div className="mt-4 overflow-x-auto rounded-md border border-border">
        <table className="w-full text-sm">
          <thead>
            <tr className="border-b border-border bg-surface/80">
              <th className="px-4 py-3 text-left font-semibold text-text-secondary">Command</th>
              <th className="px-4 py-3 text-left font-semibold text-text-secondary">Description</th>
            </tr>
          </thead>
          <tbody>
            {piCommands.map((c, i) => (
              <tr key={c.cmd} className={`border-b border-border/50 ${i % 2 === 1 ? "bg-surface/20" : ""}`}>
                <td className="px-4 py-2.5 font-mono text-xs text-accent whitespace-nowrap">{c.cmd}</td>
                <td className="px-4 py-2.5 text-text-secondary">{c.desc}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      <h2 className="mt-10 text-xl font-semibold">Dynamic Pi commands</h2>
      <p className="mt-2 text-text-secondary">
        Extension, prompt, and skill commands returned by Pi are not hard-coded in Rust. They enter the Grok native slash suggestion/dropdown through the ACP command catalog. Name conflicts are de-duplicated by the Grok registry.
      </p>

      <h2 className="mt-10 text-xl font-semibold">Deliberately excluded</h2>
      <p className="mt-2 text-text-secondary">
        These Grok product or local session-store commands are not exposed because they depend on Grok&apos;s cloud backend or conflict with Pi&apos;s ownership model:
      </p>
      <div className="mt-3 flex flex-wrap gap-2">
        {excludedCommands.map((cmd) => (
          <code key={cmd} className="px-2.5 py-1 rounded-md bg-surface border border-border text-xs font-mono text-text-tertiary line-through">
            /{cmd}
          </code>
        ))}
      </div>
    </div>
  );
}
