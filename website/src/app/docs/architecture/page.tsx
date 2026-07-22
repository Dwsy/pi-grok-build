"use client";

import { useDict } from "@/i18n/provider";
import Link from "next/link";

const layers = [
  {
    name: "Grok Pager",
    role: "Terminal lifecycle, input, rendering, dialogs, scrollback",
    color: "border-grok/30",
    bg: "bg-grok-dim",
    details: [
      "Owns the terminal — init, restore, alternate screen, minimal mode",
      "PromptWidget, slash completion, QuestionView, toasts, diffs",
      "Native SessionPicker, model selector, SessionTree, Tasks Pane",
      "F2 settings (external_only rows for Pi-only gates)",
      "Upstream workflow engine surfaces when Pi workflows are enabled",
    ],
  },
  {
    name: "pi-grok-adapter",
    role: "Headless JSONL RPC ↔ ACP bridge",
    color: "border-accent/30",
    bg: "bg-accent-glow",
    details: [
      "No terminal — no Ratatui, Crossterm, or raw-mode",
      "Tool / stream / queue / session catalog projection",
      "WorkflowHost, GoalHost, plan-mode tracker (adapter-owned state)",
      "x.ai/* ACP methods for bash background, subagent, workflow, recap",
      "Never invents a second TUI",
    ],
  },
  {
    name: "Pi Agent Core",
    role: "Agent loop, models, providers, tools, extensions, sessions",
    color: "border-success/30",
    bg: "bg-success/10",
    details: [
      "Always started in --mode rpc (system pi ≥ 0.80.10)",
      "Local JSONL sessions; trust, settings, package lifecycle",
      "Extension ecosystem + skills + prompts",
      "Sub-agent child AgentSession; compaction; model providers",
      "Source not modified for the bridge — inject via extension API",
    ],
  },
];

const seams = [
  { file: "UiProfile::External", desc: "Disables Grok.com product surfaces; keeps Pager renderer" },
  { file: "AcpConnection::external", desc: "Pager accepts an external ACP channel from the adapter" },
  { file: "run_external", desc: "Production terminal/event-loop without Grok Agent startup" },
  { file: "UI notification handlers", desc: "Status → toast / banner / title / editor" },
  { file: "QuestionView + Remote TUI", desc: "Native freeform + experimental ctx.ui.custom host" },
  { file: "Slash profile", desc: "Retained Grok commands + ACP catalog for Pi/dynamic" },
  { file: "Plan / queue / tasks", desc: "Native plan toggle, queue pane, background task cards" },
  { file: "Session tree / jump", desc: "Pi navigateTree + turn jump; not Grok destructive Rewind" },
  { file: "Voice dictation", desc: "Narrow Pager-owned /voice; does not own Pi models" },
];

const runtime = [
  {
    title: "Product isolation",
    body: "Default homes: ~/.grok-pi and <repo>/.grok-pi. No dual-scan of stock ~/.grok. Pi agent state remains under ~/.pi/agent (or --session-dir).",
  },
  {
    title: "Extension self-heal",
    body: "Bootstrap failure → binary-search --extension list → name culprit → relaunch without it. Escape: grok-pi -ne.",
  },
  {
    title: "Resource manager (Rust)",
    body: "/pi-config two-pane UI reads Pi settings/trust; admission policy at spawn. Package install/remove stays on the Pi CLI.",
  },
  {
    title: "Workflows & goal (opt-in)",
    body: "F2 pi_workflows / pi_goal default off; restart injects extensions. Scripts under .grok-pi/workflows.",
  },
];

export default function ArchitecturePage() {
  const dict = useDict();
  return (
    <div>
      <h1 className="text-3xl font-bold tracking-tight">
        {dict.docs.sidebar.architecture}
      </h1>
      <p className="mt-4 text-lg leading-relaxed text-text-secondary">
        Composition, not a fork. Three boundaries. Pi source is not modified for
        the bridge. The adapter stays headless. Grok Pager is the only TUI.
      </p>

      <h2 className="mt-10 text-xl font-semibold">The three layers</h2>
      <div className="mt-6 space-y-4">
        {layers.map((layer) => (
          <div
            key={layer.name}
            className={`rounded-md border ${layer.color} ${layer.bg} p-5`}
          >
            <h3 className="font-mono text-sm font-semibold text-text-primary">
              {layer.name}
            </h3>
            <p className="mt-1 text-xs text-text-tertiary">{layer.role}</p>
            <ul className="mt-4 space-y-1.5">
              {layer.details.map((d) => (
                <li
                  key={d}
                  className="flex items-start gap-2 text-sm text-text-secondary"
                >
                  <span className="mt-1 w-1 h-1 rounded-full bg-text-tertiary shrink-0" />
                  {d}
                </li>
              ))}
            </ul>
          </div>
        ))}
      </div>

      <h2 className="mt-10 text-xl font-semibold">Runtime design (0.0.8)</h2>
      <div className="mt-4 grid gap-3 sm:grid-cols-2">
        {runtime.map((r) => (
          <div
            key={r.title}
            className="rounded-md border border-border bg-surface p-4"
          >
            <h3 className="text-sm font-medium text-text-primary">{r.title}</h3>
            <p className="mt-1.5 text-xs leading-relaxed text-text-secondary">
              {r.body}
            </p>
          </div>
        ))}
      </div>
      <p className="mt-4 text-sm text-text-secondary">
        Field map:{" "}
        <Link href="/docs/features/" className="text-accent hover:underline">
          Feature matrix
        </Link>
        {" · "}
        <Link href="/docs/extensions/" className="text-accent hover:underline">
          Extensions
        </Link>
        {" · "}
        <a
          href="https://github.com/Dwsy/grok-pi/blob/main/NATIVE_GROK_TUI_ALIGNMENT.md"
          className="text-accent hover:underline"
          target="_blank"
          rel="noopener noreferrer"
        >
          NATIVE_GROK_TUI_ALIGNMENT.md
        </a>
      </p>

      <h2 className="mt-10 text-xl font-semibold">Integration seams</h2>
      <p className="mt-2 text-text-secondary">
        ACP does not cover every Pi UI/command. Narrow Pager seams (illustrative —
        verify against current source-identity baseline):
      </p>
      <div className="mt-4 space-y-2">
        {seams.map((seam) => (
          <div
            key={seam.file}
            className="flex flex-col sm:flex-row sm:items-start gap-1 sm:gap-3 rounded-md border border-border bg-surface px-4 py-3"
          >
            <code className="font-mono text-xs text-accent whitespace-nowrap sm:mt-0.5">
              {seam.file}
            </code>
            <span className="text-sm text-text-secondary">{seam.desc}</span>
          </div>
        ))}
      </div>

      <h2 className="mt-10 text-xl font-semibold">Invariants</h2>
      <ul className="mt-3 space-y-1.5 text-sm text-text-secondary">
        <li className="flex items-center gap-2">
          <span className="text-success">✓</span> Grok Pager is the only visible TUI
        </li>
        <li className="flex items-center gap-2">
          <span className="text-success">✓</span> Pi owns agent loop, sessions, models, extensions
        </li>
        <li className="flex items-center gap-2">
          <span className="text-success">✓</span> adapter has no Ratatui / Crossterm / raw-mode
        </li>
        <li className="flex items-center gap-2">
          <span className="text-success">✓</span> do not patch Pi source to extend RPC — extension API first
        </li>
        <li className="flex items-center gap-2">
          <span className="text-success">✓</span> project trust is Pi-owned; Grok does not re-adjudicate
        </li>
      </ul>
    </div>
  );
}
