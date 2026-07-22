"use client";

import { useDict } from "@/i18n/provider";
import CodeBlock from "@/components/ui/CodeBlock";

const bundledHighlights = [
  {
    id: "bash",
    name: "pi-grok-bash",
    badge: "Default on",
    title: "Enhanced Bash + Send to Background",
    lead: "Owns every Bash child process so the Pager can promote a live foreground command into Grok’s native background-task UI without re-running it.",
    points: [
      "Foreground bash reuses Pi createBashToolDefinition output/render semantics.",
      "Pager “Send to Background” transfers the same subprocess via x.ai/terminal/background (toolCallId control file).",
      "Native task cards: kill via x.ai/task/kill; agent tools get_task_output / wait_tasks / kill_task stay available.",
      "Supports is_background + description for model-started background shells.",
    ],
    gate: "PI_GROK_BASH=1 (default). Disable with PI_GROK_BASH=0 or --no-extensions.",
  },
  {
    id: "subagents",
    name: "pi-grok-subagents",
    badge: "Default on",
    title: "Sub-agents (parallel work)",
    lead: "Spawns a real Pi child AgentSession and projects lifecycle into native SubagentBlock, Tasks Pane, and child AgentView.",
    points: [
      "Profiles: general-purpose (all tools), explore / plan (safer tool sets).",
      "Capability modes: read-only, read-write, execute, all.",
      "Foreground or background (max concurrency 4 for background).",
      "Versioned bridge pi-grok-subagent/v1 → adapter → x.ai/subagent/* surfaces; cancel is first-class.",
      "Child sessions are persisted; parent resume can rebuild lifecycle from index entries.",
    ],
    gate: "Injected with other bridge extensions. Disabled under --no-extensions / -ne.",
  },
];

const bundledCatalog = [
  { name: "pi-grok-bash", role: "Bash ownership + background promote", default: "On" },
  { name: "pi-grok-subagents", role: "Child AgentSession + native task UI", default: "On" },
  { name: "pi-grok-context", role: "System/tools/AGENTS/skills breakdown for /context", default: "On" },
  { name: "pi-grok-recap", role: "Display-only session recap (no history mutation)", default: "On" },
  { name: "pi-grok-auth", role: "/login /logout via Remote TUI surfaces", default: "On" },
  { name: "pi-grok-export", role: "/export-html and /pi-share (gist)", default: "On" },
  { name: "pi-grok-remote-tui", role: "ctx.ui.custom host + frame projection", default: "On*" },
  { name: "pi-grok-rpc-compat", role: "Present mode=tui to third-party extensions", default: "With Remote TUI" },
  { name: "pi-grok-plan-mode", role: "Plan gate + exit_plan_mode approval", default: "On" },
  { name: "pi-grok-goal", role: "/goal + update_goal (F2, restart)", default: "Off (F2)" },
  { name: "pi-grok-rollback", role: "Tree file rollback snapshots", default: "Off (F2)" },
  { name: "pi-grok-tools", role: "F2 built-in tool allow/deny preference", default: "On" },
  { name: "pi-grok-workflows", role: "Pi spawn backend for Rhai workflows", default: "Off (F2)" },
  { name: "pi-grok-native-commands", role: "Experimental /pi-* selectors", default: "Off (env)" },
];

const advancedSurfaces = [
  {
    title: "Recap (/recap)",
    body: "pi-grok-recap is display-only — does not rewrite session history. Auto when away ≥3 min (and ≥3 turns). Optional Mermaid via F2 recap_mermaid. Configure recap_model in F2 (never silently falls back to the live session model).",
  },
  {
    title: "Rhai workflows",
    body: "F2 → Pi workflows (default off, restart). Scripts under ~/.grok-pi/workflows and <repo>/.grok-pi/workflows. Slash: /workflow, /workflows, /create-workflow. Host uses upstream xai-workflow with a Pi spawn backend; __pi_workflow_* bridge cmds are hidden from the catalog.",
  },
  {
    title: "Self-heal on bad extensions",
    body: "If any injected or discovered extension kills RPC bootstrap, the host binary-searches --extension paths, prints the culprit, and relaunches without it. Manual: grok-pi -ne.",
  },
];

const recommended = [
  {
    pkg: "npm:@juicesharp/rpiv-todo",
    title: "Todo list → native TodoPane",
    status: "Recommended · Adapted",
    body: "Agent updates a structured task list via the todo tool. grok-pi projects details.tasks into ACP Plan → Grok TodoPane / plan badge. The raw todo tool card is suppressed in scrollback so you see one native list, not a duplicate card.",
    install: "pi install npm:@juicesharp/rpiv-todo",
    notes: [
      "Unidirectional: Pi tool details → ACP Plan → Pager UI.",
      "Works in RPC mode without Remote TUI.",
      "Use for multi-step agent work you want visible as a checklist.",
    ],
  },
  {
    pkg: "npm:@juicesharp/rpiv-ask-user-question",
    title: "Structured questions → QuestionView",
    status: "Recommended · Remote TUI path",
    body: "Lets the agent ask multi-option / multi-select questions mid-turn. Interactive Pi can open a custom factory UI; under grok-pi the stable path is Remote TUI (default on), which runs the factory in-process and projects onto native Grok QuestionView-style surfaces when the bridge can host it.",
    install: "pi install npm:@juicesharp/rpiv-ask-user-question",
    notes: [
      "Keep PI_GROK_REMOTE_TUI=1 (default) so third-party custom UI can run.",
      "Pure JSONL RPC without the custom host cannot serialize factory components — that is why Remote TUI exists.",
      "If a questionnaire declines, check Remote TUI is enabled and the extension is loaded (not blocked by policy).",
    ],
  },
];

export default function ExtensionsPage() {
  const dict = useDict();
  return (
    <div>
      <h1 className="text-3xl font-bold tracking-tight">
        {dict.docs.sidebar.extensions}
      </h1>
      <p className="mt-4 text-lg leading-relaxed text-text-secondary max-w-2xl">
        grok-pi injects thin bridge extensions into Pi so native Grok surfaces can
        own Bash, sub-agents, context, recap, and more — without forking Pi. On
        top of that, install community packages the same way you would for
        interactive Pi.
      </p>

      <h2 id="recommended" className="mt-10 text-xl font-semibold">
        Recommended community extensions
      </h2>
      <p className="mt-2 text-sm text-text-secondary">
        These are not bundled. Install once into your Pi agent home; both{" "}
        <code className="font-mono text-accent text-xs">pi</code> and{" "}
        <code className="font-mono text-accent text-xs">grok-pi</code> load them.
      </p>

      <div className="mt-6 space-y-8">
        {recommended.map((r) => (
          <section
            key={r.pkg}
            className="rounded-md border border-border bg-surface p-5"
          >
            <div className="flex flex-wrap items-center gap-2">
              <h3 className="font-semibold text-text-primary">{r.title}</h3>
              <span className="text-[10px] uppercase tracking-wide px-2 py-0.5 rounded border border-border text-text-tertiary">
                {r.status}
              </span>
            </div>
            <p className="mt-1 font-mono text-xs text-accent">{r.pkg}</p>
            <p className="mt-3 text-sm leading-relaxed text-text-secondary">
              {r.body}
            </p>
            <div className="mt-4">
              <CodeBlock code={r.install} label="Install" />
            </div>
            <ul className="mt-4 space-y-1.5 text-sm text-text-secondary list-disc pl-5">
              {r.notes.map((n) => (
                <li key={n}>{n}</li>
              ))}
            </ul>
          </section>
        ))}
      </div>

      <div className="mt-6">
        <CodeBlock
          code={`# Recommended pair for grok-pi workflows
pi install npm:@juicesharp/rpiv-todo
pi install npm:@juicesharp/rpiv-ask-user-question

# Verify in resource manager
grok-pi
# then F2 or /pi-config → Extensions`}
          label="Quick install"
        />
      </div>

      <h2 id="bash" className="mt-12 text-xl font-semibold">
        Bundled: enhanced Bash
      </h2>
      <p className="mt-2 text-sm text-text-secondary">
        {bundledHighlights[0].lead}
      </p>
      <ul className="mt-4 space-y-1.5 text-sm text-text-secondary list-disc pl-5">
        {bundledHighlights[0].points.map((p) => (
          <li key={p}>{p}</li>
        ))}
      </ul>
      <p className="mt-3 text-xs text-text-tertiary font-mono">
        {bundledHighlights[0].gate}
      </p>
      <div className="mt-4 rounded-md border border-border bg-surface p-4 text-sm text-text-secondary">
        <p className="font-medium text-text-primary text-sm mb-2">
          What you see in the TUI
        </p>
        <ol className="list-decimal pl-5 space-y-1">
          <li>Agent runs a long bash (build, test, install…).</li>
          <li>
            Use Pager&apos;s native <strong>Send to Background</strong> on the
            tool card — process keeps running; card becomes a task row.
          </li>
          <li>
            Kill, wait, or poll from the task UI or via agent tools (
            <code className="font-mono text-xs text-accent">
              get_task_output
            </code>
            ,{" "}
            <code className="font-mono text-xs text-accent">wait_tasks</code>,{" "}
            <code className="font-mono text-xs text-accent">kill_task</code>).
          </li>
        </ol>
      </div>

      <h2 id="subagents" className="mt-12 text-xl font-semibold">
        Bundled: sub-agents
      </h2>
      <p className="mt-2 text-sm text-text-secondary">
        {bundledHighlights[1].lead}
      </p>
      <ul className="mt-4 space-y-1.5 text-sm text-text-secondary list-disc pl-5">
        {bundledHighlights[1].points.map((p) => (
          <li key={p}>{p}</li>
        ))}
      </ul>
      <p className="mt-3 text-xs text-text-tertiary font-mono">
        {bundledHighlights[1].gate}
      </p>
      <div className="mt-4 overflow-x-auto">
        <table className="w-full text-sm border-collapse">
          <thead>
            <tr className="border-b border-border text-left text-text-tertiary">
              <th className="py-2 pr-4 font-medium">Profile</th>
              <th className="py-2 pr-4 font-medium">Tools</th>
              <th className="py-2 font-medium">Use when</th>
            </tr>
          </thead>
          <tbody className="text-text-secondary">
            <tr className="border-b border-border/60">
              <td className="py-2 pr-4 font-mono text-xs text-accent">
                general-purpose
              </td>
              <td className="py-2 pr-4">read, bash, edit, write</td>
              <td className="py-2">Delegated implementation slices</td>
            </tr>
            <tr className="border-b border-border/60">
              <td className="py-2 pr-4 font-mono text-xs text-accent">
                explore
              </td>
              <td className="py-2 pr-4">read, bash</td>
              <td className="py-2">Codebase investigation, diagnostics</td>
            </tr>
            <tr>
              <td className="py-2 pr-4 font-mono text-xs text-accent">plan</td>
              <td className="py-2 pr-4">read, bash</td>
              <td className="py-2">Plans with risks + verification only</td>
            </tr>
          </tbody>
        </table>
      </div>

      <h2 id="catalog" className="mt-12 text-xl font-semibold">
        Full bridge catalog
      </h2>
      <p className="mt-2 text-sm text-text-secondary">
        Source lives under{" "}
        <code className="font-mono text-xs text-accent">extensions/</code> in the
        repo. Host injects them at spawn; they are not Pi core patches.
      </p>
      <div className="mt-4 overflow-x-auto">
        <table className="w-full text-sm border-collapse">
          <thead>
            <tr className="border-b border-border text-left text-text-tertiary">
              <th className="py-2 pr-3 font-medium">Extension</th>
              <th className="py-2 pr-3 font-medium">Role</th>
              <th className="py-2 font-medium">Default</th>
            </tr>
          </thead>
          <tbody className="text-text-secondary">
            {bundledCatalog.map((row) => (
              <tr key={row.name} className="border-b border-border/60">
                <td className="py-2 pr-3 font-mono text-xs text-accent whitespace-nowrap">
                  {row.name}
                </td>
                <td className="py-2 pr-3">{row.role}</td>
                <td className="py-2 whitespace-nowrap">{row.default}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <p className="mt-3 text-xs text-text-tertiary">
        * Remote TUI default-on; set{" "}
        <code className="font-mono text-accent">PI_GROK_REMOTE_TUI=0</code> to
        disable. F2-gated features need a full process restart after toggle.
      </p>

      <h2 id="advanced" className="mt-12 text-xl font-semibold">
        Recap, workflows, self-heal
      </h2>
      <div className="mt-6 space-y-4">
        {advancedSurfaces.map((s) => (
          <div
            key={s.title}
            className="rounded-md border border-border bg-surface p-5"
          >
            <h3 className="font-medium text-text-primary text-sm">{s.title}</h3>
            <p className="mt-2 text-sm leading-relaxed text-text-secondary">
              {s.body}
            </p>
          </div>
        ))}
      </div>

      <h2 id="control" className="mt-12 text-xl font-semibold">
        Enable / disable
      </h2>
      <div className="mt-4 space-y-3">
        <CodeBlock
          code={`# Disable all injected bridge extensions
grok-pi -ne
# or
grok-pi --no-extensions

# Bash only
PI_GROK_BASH=0 grok-pi

# Remote TUI (custom UI host for packages like rpiv-ask)
PI_GROK_REMOTE_TUI=0 grok-pi   # off
PI_GROK_REMOTE_TUI=1 grok-pi   # on (default)`}
          label="Gates"
        />
      </div>
      <p className="mt-3 text-sm text-text-secondary">
        User / project extensions still load through Pi discovery (
        <code className="font-mono text-xs text-accent">~/.pi/agent</code>,
        trusted project trees) unless blocked by{" "}
        <code className="font-mono text-xs text-accent">/pi-config</code> policy.
      </p>

      <h2 id="faq" className="mt-12 text-xl font-semibold">
        FAQ
      </h2>
      <div className="mt-6 space-y-4">
        {[
          {
            q: "Do I need to install pi-grok-bash myself?",
            a: "No. The composition binary injects it at runtime. You only install community packages (todo / ask).",
          },
          {
            q: "Why is todo recommended if the host already has plan mode?",
            a: "Plan mode is a write gate + approval flow. rpiv-todo is a living checklist projected into TodoPane — complementary, not a replacement.",
          },
          {
            q: "Will ask-user-question work without Remote TUI?",
            a: "Not reliably. The package depends on in-process custom UI. Keep Remote TUI on (default) for questionnaires.",
          },
          {
            q: "Can I still use arbitrary Pi extensions?",
            a: "Yes. Install with pi install … or drop packages under Pi extension paths; manage visibility in F2 / /pi-config.",
          },
        ].map((faq) => (
          <div
            key={faq.q}
            className="rounded-md border border-border bg-surface p-5"
          >
            <h3 className="font-medium text-text-primary text-sm">{faq.q}</h3>
            <p className="mt-2 text-sm leading-relaxed text-text-secondary">
              {faq.a}
            </p>
          </div>
        ))}
      </div>
    </div>
  );
}
