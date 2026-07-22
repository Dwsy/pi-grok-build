"use client";

import { useDict } from "@/i18n/provider";
import Link from "next/link";

type Status =
  | "Native"
  | "Adapted"
  | "Native+Adapted"
  | "Boundary"
  | "Experimental";

const statusColors: Record<Status, string> = {
  Native: "text-grok bg-grok-dim",
  Adapted: "text-accent-bright bg-accent-glow",
  "Native+Adapted": "text-success bg-success/10",
  Boundary: "text-text-tertiary bg-surface",
  Experimental: "text-warning bg-warning/10",
};

interface FeatureRow {
  feature: string;
  status: Status;
  notes: string;
}

const sections: { title: string; rows: FeatureRow[] }[] = [
  {
    title: "Terminal & display",
    rows: [
      { feature: "Terminal init/restore", status: "Native", notes: "Grok init_terminal / restore_terminal" },
      { feature: "Welcome / logo", status: "Native+Adapted", notes: "Welcome default; π block art; --continue skips Welcome" },
      { feature: "Update check/install", status: "Adapted", notes: "GitHub Releases + JSP proxy fallback; grok-pi update" },
      { feature: "Theme / timestamps / mouse", status: "Native+Adapted", notes: "Pi theme JSON → Grok Theme; pi:transparent*" },
      { feature: "Voice dictation", status: "Native+Adapted", notes: "/voice · Ctrl+Space/F8 · xAI STT" },
      { feature: "Markdown / tool cards / diffs", status: "Native+Adapted", notes: "ACP chunks → native Pager surfaces" },
      { feature: "Todo / plan list", status: "Native+Adapted", notes: "rpiv-todo → ACP Plan → TodoPane" },
      { feature: "Plan mode", status: "Native+Adapted", notes: "Ctrl+Shift+T · tool gate · exit_plan_mode" },
      { feature: "Goal mode", status: "Adapted", notes: "F2 pi_goal default off; /goal + agent_settled follow-up" },
    ],
  },
  {
    title: "Agent & streaming",
    rows: [
      { feature: "Prompt / steer / follow-up", status: "Adapted", notes: "ACP prompt; sendNow→steer; default mid-turn→followUp" },
      { feature: "Abort + queue clear", status: "Adapted", notes: "clear_queue then abort; queue mirror via x.ai/queue/changed" },
      { feature: "Bash + Send to Background", status: "Native+Adapted", notes: "pi-grok-bash; x.ai/terminal/background; task kill" },
      { feature: "Sub-agents", status: "Native+Adapted", notes: "pi-grok-subagents → SubagentBlock / Tasks Pane" },
      { feature: "Rhai workflows", status: "Native+Adapted", notes: "F2 pi_workflows; /workflow /workflows /create-workflow" },
      { feature: "Compaction", status: "Native+Adapted", notes: "/compact → Pi compact; native progress blocks" },
      { feature: "Session recap", status: "Adapted", notes: "/recap · auto away ≥3 min · optional Mermaid F2" },
      { feature: "Context bar + /context", status: "Native+Adapted", notes: "Live breakdown modal; not written to history" },
    ],
  },
  {
    title: "Model, session & commands",
    rows: [
      { feature: "Model catalog", status: "Adapted", notes: "get_available_models → native picker" },
      { feature: "Resume / session picker", status: "Adapted", notes: "Pi JSONL catalog; Ctrl+F full-text search" },
      { feature: "Session tree", status: "Adapted", notes: "navigateTree; non-destructive (≠ Grok Rewind)" },
      { feature: "Fork / clone", status: "Adapted", notes: "/fork /clone · rebind + session/load" },
      { feature: "Jump / review", status: "Native+Adapted", notes: "/jump turns; /review-session · /review-message" },
      { feature: "Reload", status: "Adapted", notes: "/reload blocks streaming+compacting; theme rediscover" },
      { feature: "HTML export / share", status: "Adapted", notes: "/export-html · /pi-share (default-on)" },
      { feature: "Pi resource manager", status: "Native+Adapted", notes: "Rust /pi-config two-pane; not install/remove" },
    ],
  },
  {
    title: "Reliability & isolation",
    rows: [
      { feature: "Product home isolation", status: "Adapted", notes: "~/.grok-pi + <repo>/.grok-pi; migrate-home" },
      { feature: "Extension self-heal", status: "Adapted", notes: "Bootstrap bisect on crashing --extension; -ne escape" },
      { feature: "Resource admission policy", status: "Adapted", notes: "Allow/block lists + heuristics at spawn" },
      { feature: "Remote TUI", status: "Experimental", notes: "PI_GROK_REMOTE_TUI=1; ctx.mode facade tui; no Pi fork" },
    ],
  },
  {
    title: "Boundaries (deliberate)",
    rows: [
      { feature: "Grok cloud history / usage", status: "Boundary", notes: "Pi owns local sessions" },
      { feature: "Adapter TUI / second renderer", status: "Boundary", notes: "adapter stays headless" },
      { feature: "Pi source RPC patches", status: "Boundary", notes: "prefer official extension API" },
      { feature: "rpiv-ask pure JSONL", status: "Boundary", notes: "needs Remote TUI custom host" },
      { feature: "Grok destructive Rewind", status: "Boundary", notes: "use SessionTree navigation instead" },
    ],
  },
];

export default function FeaturesPage() {
  const dict = useDict();
  return (
    <div>
      <h1 className="text-3xl font-bold tracking-tight">
        {dict.docs.sidebar.features}
      </h1>
      <p className="mt-4 text-lg leading-relaxed text-text-secondary">
        Field-level map for{" "}
        <strong className="text-text-primary">v0.0.8</strong>. Full SSOT:{" "}
        <a
          href="https://github.com/Dwsy/grok-pi/blob/main/FEATURE_MATRIX.md"
          className="text-accent hover:underline"
          target="_blank"
          rel="noopener noreferrer"
        >
          FEATURE_MATRIX.md
        </a>
        . Status:{" "}
        <strong className="text-grok">Native</strong> ·{" "}
        <strong className="text-accent-bright">Adapted</strong> ·{" "}
        <strong className="text-success">Native+Adapted</strong> ·{" "}
        <strong className="text-text-tertiary">Boundary</strong> ·{" "}
        <strong className="text-warning">Experimental</strong>.
      </p>
      <p className="mt-3 text-sm text-text-secondary">
        Deep dives:{" "}
        <Link href="/docs/extensions/" className="text-accent hover:underline">
          Extensions
        </Link>
        {" · "}
        <Link href="/docs/commands/" className="text-accent hover:underline">
          Commands
        </Link>
        {" · "}
        <Link href="/docs/configuration/" className="text-accent hover:underline">
          Configuration
        </Link>
      </p>

      {sections.map((section) => (
        <div key={section.title} className="mt-10">
          <h2 className="text-xl font-semibold mb-4">{section.title}</h2>
          <div className="overflow-x-auto rounded-md border border-border">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b border-border bg-surface/80">
                  <th className="px-4 py-3 text-left font-semibold text-text-secondary">
                    Feature
                  </th>
                  <th className="px-4 py-3 text-left font-semibold text-text-secondary">
                    Status
                  </th>
                  <th className="px-4 py-3 text-left font-semibold text-text-secondary">
                    Notes
                  </th>
                </tr>
              </thead>
              <tbody>
                {section.rows.map((row, i) => (
                  <tr
                    key={row.feature}
                    className={`border-b border-border/50 ${i % 2 === 1 ? "bg-surface/20" : ""}`}
                  >
                    <td className="px-4 py-3 font-medium text-text-primary whitespace-nowrap">
                      {row.feature}
                    </td>
                    <td className="px-4 py-3">
                      <span
                        className={`inline-block px-2 py-0.5 rounded-sm text-xs font-medium ${statusColors[row.status]}`}
                      >
                        {row.status}
                      </span>
                    </td>
                    <td className="px-4 py-3 text-text-secondary text-xs">
                      {row.notes}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      ))}
    </div>
  );
}
