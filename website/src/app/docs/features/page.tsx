"use client";

import { useDict } from "@/i18n/provider";

type Status = "Native" | "Adapted" | "Native+Adapted" | "Boundary" | "Experimental";

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
    title: "Terminal & Display",
    rows: [
      { feature: "Terminal init/restore", status: "Native", notes: "Grok init_terminal / restore_terminal" },
      { feature: "Fullscreen / alternate screen", status: "Native", notes: "Grok screen mode, selected at startup" },
      { feature: "Minimal / scrollback-native", status: "Native", notes: "xai-grok-pager-minimal" },
      { feature: "Welcome / logo", status: "Native+Adapted", notes: "Defaults to Welcome; π block art injected" },
      { feature: "Welcome session prewarm", status: "Adapted", notes: "Background new_session; first keystroke attaches" },
      { feature: "Update check/install", status: "Adapted", notes: "GitHub-only releases JSON + install scripts" },
      { feature: "Agent Dashboard", status: "Native+Adapted", notes: "/dashboard · Ctrl+\\ · Pi session catalog" },
      { feature: "Prompt editing", status: "Native", notes: "PromptWidget" },
      { feature: "Theme / timestamps / mouse", status: "Native+Adapted", notes: "Pi theme JSON → Grok Theme via theme::pi" },
      { feature: "Voice dictation", status: "Native+Adapted", notes: "/voice · Ctrl+Space/F8 · xAI STT" },
      { feature: "Markdown / code blocks", status: "Native+Adapted", notes: "Pi text → ACP chunks → xai-grok-markdown" },
      { feature: "Tool cards", status: "Native+Adapted", notes: "read/bash/edit/write/grep/find/ls → native cards" },
      { feature: "Todo / plan list", status: "Native+Adapted", notes: "Pi todo → ACP Plan → native TodoPane" },
      { feature: "Plan mode", status: "Native+Adapted", notes: "Native toggle → adapter tracker → Pi tool gate" },
      { feature: "Diff rendering", status: "Native+Adapted", notes: "edit-like metadata → Grok tool/diff pipeline" },
      { feature: "Images", status: "Native+Adapted", notes: "Pi image blocks → ACP ImageContent" },
    ],
  },
  {
    title: "Agent & Streaming",
    rows: [
      { feature: "Prompt", status: "Adapted", notes: "ACP prompt → Pi prompt" },
      { feature: "Mid-turn send now", status: "Adapted", notes: "Grok sendNow → Pi steer" },
      { feature: "Follow-up queue", status: "Adapted", notes: "Default active-turn → Pi followUp" },
      { feature: "Abort", status: "Adapted", notes: "ACP cancel → Pi abort; abort_bash for Bash" },
      { feature: "Text stream", status: "Adapted", notes: "message_update → AgentMessageChunk" },
      { feature: "Thinking stream", status: "Adapted", notes: "message_update → AgentThoughtChunk" },
      { feature: "Tool lifecycle", status: "Adapted", notes: "ACP ToolCall / ToolCallUpdate" },
      { feature: "Bash background tasks", status: "Native+Adapted", notes: "Send to Background via x.ai/terminal/background" },
      { feature: "Sub-agents", status: "Native+Adapted", notes: "SubagentBlock, Tasks Pane, child AgentView" },
      { feature: "Compaction", status: "Native+Adapted", notes: "/compact → Pi compact; native progress blocks" },
      { feature: "Session recap", status: "Adapted", notes: "/recap + auto away; recap_model only" },
      { feature: "Context bar", status: "Adapted", notes: "Pi contextUsage → ACP totalTokens → top-right bar" },
      { feature: "Context click / /context", status: "Native+Adapted", notes: "Native ModalWindow with ContextInfoBlock chart" },
    ],
  },
  {
    title: "Model, Session & Commands",
    rows: [
      { feature: "Model catalog", status: "Adapted", notes: "get_available_models → native model selector" },
      { feature: "Thinking effort", status: "Adapted", notes: "Pi levels → Grok effort selector" },
      { feature: "New session", status: "Adapted", notes: "/new → Pi new_session" },
      { feature: "Rename", status: "Adapted", notes: "/rename → Pi set_session_name" },
      { feature: "Resume catalog", status: "Adapted", notes: "/resume reads Pi JSONL metadata" },
      { feature: "Session info", status: "Adapted", notes: "/session-info → Pi stats + context breakdown" },
      { feature: "Session tree", status: "Adapted", notes: "Native SessionTree modal with filter/search/tags" },
      { feature: "Session fork", status: "Adapted", notes: "ListOverlay → RPC fork → rebind + replay" },
      { feature: "Session clone", status: "Adapted", notes: "RPC clone → new session file → rebind" },
      { feature: "Resource reload", status: "Adapted", notes: "__pi_reload → ctx.reload(); blocks on streaming" },
      { feature: "HTML export / share", status: "Adapted", notes: "default-on /export-html + /pi-share" },
      { feature: "Pi Config manager", status: "Native+Adapted", notes: "F2 or /pi-config — two-pane resource manager" },
    ],
  },
];

export default function FeaturesPage() {
  const dict = useDict();
  return (
    <div>
      <h1 className="text-3xl font-bold tracking-tight">{dict.docs.sidebar.features}</h1>
      <p className="mt-4 text-lg leading-relaxed text-text-secondary">
        Field-level behavior and intentional boundaries. Status definitions:{" "}
        <strong className="text-grok">Native</strong> = Grok Pager component ·{" "}
        <strong className="text-accent-bright">Adapted</strong> = Pi semantics projected ·{" "}
        <strong className="text-success">Native+Adapted</strong> = both ·{" "}
        <strong className="text-text-tertiary">Boundary</strong> = deliberately not implemented.
      </p>

      {sections.map((section) => (
        <div key={section.title} className="mt-10">
          <h2 className="text-xl font-semibold mb-4">{section.title}</h2>
          <div className="overflow-x-auto rounded-md border border-border">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b border-border bg-surface/80">
                  <th className="px-4 py-3 text-left font-semibold text-text-secondary">Feature</th>
                  <th className="px-4 py-3 text-left font-semibold text-text-secondary">Status</th>
                  <th className="px-4 py-3 text-left font-semibold text-text-secondary">Notes</th>
                </tr>
              </thead>
              <tbody>
                {section.rows.map((row, i) => (
                  <tr key={row.feature} className={`border-b border-border/50 ${i % 2 === 1 ? "bg-surface/20" : ""}`}>
                    <td className="px-4 py-3 font-medium text-text-primary whitespace-nowrap">{row.feature}</td>
                    <td className="px-4 py-3">
                      <span className={`inline-block px-2 py-0.5 rounded-sm text-xs font-medium ${statusColors[row.status]}`}>
                        {row.status}
                      </span>
                    </td>
                    <td className="px-4 py-3 text-text-secondary text-xs">{row.notes}</td>
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
