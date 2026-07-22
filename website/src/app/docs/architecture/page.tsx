"use client";

import { useDict } from "@/i18n/provider";

const layers = [
  {
    name: "Grok Pager",
    role: "Terminal lifecycle, input, rendering, dialogs, scrollback",
    color: "border-grok/30",
    bg: "bg-grok-dim",
    details: [
      "Owns the terminal — init, restore, alternate screen, minimal mode",
      "PromptWidget handles all input, multiline, and Vim mode",
      "xai-grok-markdown renders streaming text and reasoning",
      "Native tool cards, diffs, QuestionView overlays, toasts",
      "Session picker, model selector, slash completion dropdown",
    ],
  },
  {
    name: "pi-grok-adapter",
    role: "Headless JSONL RPC ↔ ACP bridge",
    color: "border-accent/30",
    bg: "bg-accent-glow",
    details: [
      "Converts Pi JSONL RPC messages to ACP protocol",
      "No terminal ownership — no Ratatui, no Crossterm, no raw-mode",
      "Maps Pi tool events to ACP ToolCall / ToolCallUpdate",
      "Injects compatibility extensions via Pi's official extension API",
      "Handles session catalog, queue, context breakdown bridging",
    ],
  },
  {
    name: "Pi Agent Core",
    role: "Agent loop, models, providers, tools, extensions, sessions",
    color: "border-success/30",
    bg: "bg-success/10",
    details: [
      "Runs the agent loop — prompt → think → tool → respond",
      "Manages model providers, retries, and compaction",
      "Owns sessions as local JSONL files",
      "Extension ecosystem: skills, prompts, custom tools",
      "Sub-agent orchestration with foreground/background execution",
    ],
  },
];

const seams = [
  { file: "UiProfile::External", desc: "Disables Grok.com product capabilities without changing the renderer" },
  { file: "AcpConnection::external", desc: "Lets the Pager accept an external ACP channel" },
  { file: "run_external", desc: "Reuses the production terminal/event-loop, skipping Grok Agent startup" },
  { file: "UI notification handlers", desc: "Maps fire-and-forget status to native toast/banner/title/editor" },
  { file: "QuestionView hints", desc: "Reuses native freeform editor with Pi timeout revocation" },
  { file: "Slash profile", desc: "Selects existing Grok commands meaningful for Pi" },
  { file: "/compact <instructions>", desc: "Passes optional text to Pi customInstructions" },
  { file: "Screen-mode boundary", desc: "Retains native renderer, exposes startup option only" },
  { file: "Voice dictation", desc: "Opts into existing Pager /voice flow with xAI STT" },
];

export default function ArchitecturePage() {
  const dict = useDict();
  return (
    <div>
      <h1 className="text-3xl font-bold tracking-tight">{dict.docs.sidebar.architecture}</h1>
      <p className="mt-4 text-lg leading-relaxed text-text-secondary">
        grok-pi is not a fork. It&apos;s a composition of three boundaries with zero hacks. Pi source is not modified. Grok renderer files are not modified. The adapter is a headless bridge.
      </p>

      <h2 className="mt-10 text-xl font-semibold">The three layers</h2>
      <div className="mt-6 space-y-4">
        {layers.map((layer) => (
          <div key={layer.name} className={`rounded-md border ${layer.color} ${layer.bg} p-5`}>
            <h3 className="font-mono text-sm font-semibold text-text-primary">{layer.name}</h3>
            <p className="mt-1 text-xs text-text-tertiary">{layer.role}</p>
            <ul className="mt-4 space-y-1.5">
              {layer.details.map((d) => (
                <li key={d} className="flex items-start gap-2 text-sm text-text-secondary">
                  <span className="mt-1 w-1 h-1 rounded-full bg-text-tertiary shrink-0" />
                  {d}
                </li>
              ))}
            </ul>
          </div>
        ))}
      </div>

      <h2 className="mt-10 text-xl font-semibold">Integration seams</h2>
      <p className="mt-2 text-text-secondary">
        The ACP standard doesn&apos;t cover all of Pi&apos;s UI/command semantics. These 9 narrow seams are the only changes to Grok Pager files:
      </p>
      <div className="mt-4 space-y-2">
        {seams.map((seam) => (
          <div key={seam.file} className="flex items-start gap-3 rounded-md border border-border bg-surface px-4 py-3">
            <code className="font-mono text-xs text-accent whitespace-nowrap mt-0.5">{seam.file}</code>
            <span className="text-sm text-text-secondary">{seam.desc}</span>
          </div>
        ))}
      </div>

      <h2 className="mt-10 text-xl font-semibold">Verification</h2>
      <p className="mt-2 text-text-secondary">
        A SHA-256 baseline against the uploaded Grok source confirms:
      </p>
      <ul className="mt-3 space-y-1.5 text-sm text-text-secondary">
        <li className="flex items-center gap-2"><span className="text-success">✓</span> 283 renderer/input/Markdown files remain byte-for-byte identical</li>
        <li className="flex items-center gap-2"><span className="text-success">✓</span> 2698 non-seam Grok files remain byte-for-byte identical</li>
        <li className="flex items-center gap-2"><span className="text-success">✓</span> 17 allowed-to-change files live only in workspace manifest, ACP connection, App state, and slash profile seams</li>
        <li className="flex items-center gap-2"><span className="text-success">✓</span> pi-grok-adapter contains no Ratatui, Crossterm, Terminal, Frame, Widget, draw, event::read, or raw-mode calls</li>
      </ul>
    </div>
  );
}
