"use client";

import { useDict } from "@/i18n/provider";
import CodeBlock from "@/components/ui/CodeBlock";

const grokSteps = [
  {
    step: "1",
    title: "Install grok-pi",
    desc: "One command. The installer detects your platform and installs the binary to ~/.local/bin.",
    code: "curl -fsSL https://github.com/Dwsy/grok-pi/releases/latest/download/install.sh | sh",
  },
  {
    step: "2",
    title: "Ensure Pi ≥ 0.80.10",
    desc: "grok-pi drives Pi as its agent core. Install or update Pi via npm.",
    code: "npm install --global @earendil-works/pi-coding-agent",
  },
  {
    step: "3",
    title: "Run in your project",
    desc: "cd into your project and go. Grok Pager keybindings and themes still apply; agent power comes from Pi.",
    code: "cd your-project && grok-pi",
  },
];

const piSteps = [
  {
    step: "1",
    title: "Install grok-pi (keep Pi)",
    desc: "You already have Pi. Add the host binary only — sessions, models, and ~/.pi/agent stay where they are.",
    code: "curl -fsSL https://github.com/Dwsy/grok-pi/releases/latest/download/install.sh | sh",
  },
  {
    step: "2",
    title: "Run grok-pi instead of pi",
    desc: "Same project cwd. Same Pi session store. Different TUI (Grok Pager) in front of the same agent core.",
    code: "cd your-project && grok-pi",
  },
  {
    step: "3",
    title: "Resume existing sessions",
    desc: "Partial UUID works, same as Pi --session. Or use /resume inside the TUI.",
    code: "grok-pi --session 019f88c\ngrok-pi --continue",
  },
];

const piKeep = [
  {
    title: "Sessions stay on Pi",
    desc: "JSONL under ~/.pi/agent/sessions (or --session-dir / PI_CODING_AGENT_SESSION_DIR). No import step. /resume lists the same catalog.",
  },
  {
    title: "Models, tools, extensions",
    desc: "Pi providers, tools, skills, prompts, and extensions keep loading from Pi paths. grok-pi injects only bridge extensions for the Pager surface.",
  },
  {
    title: "Settings still apply",
    desc: "Pi settings.json and auth remain authoritative for the agent. UI chrome (F2, themes display) lives under ~/.grok-pi — isolated from stock Grok.",
  },
  {
    title: "Quit → resume command",
    desc: "On exit you get: To resume this session: grok-pi --session <uuid> — same idea as interactive pi, host binary name swapped.",
  },
];

const piDiffs = [
  {
    title: "TUI is Grok Pager",
    desc: "No Pi interactive TUI. Slash completion, tool cards, diffs, and modals are native Grok surfaces mapped to Pi RPC.",
  },
  {
    title: "Some Pi-only pickers change",
    desc: "/model and /resume use Grok SessionPicker / model UI, not Pi’s TUI components. Behavior is equivalent; layout may differ.",
  },
  {
    title: "Product state dirs",
    desc: "UI prefs/workflows default to ~/.grok-pi and <repo>/.grok-pi so they never collide with stock Grok ~/.grok. Pi agent state is unchanged.",
  },
  {
    title: "Always RPC mode",
    desc: "grok-pi always starts Pi with --mode rpc. Do not expect interactive-only Pi widgets that require an in-process TUI factory.",
  },
];

const advantages = [
  {
    title: "Keep your Grok muscle memory",
    desc: "Same Pager, same slash commands, same Ctrl+key shortcuts. grok-pi adds capability, not complexity.",
  },
  {
    title: "Unlock any model",
    desc: "/model opens Pi’s full catalog — GPT-4o, Claude, Gemini, local LLMs, custom endpoints. Switch mid-session.",
  },
  {
    title: "Own your sessions",
    desc: "Local JSONL files. Fork, clone, tag, recap. No cloud dependency.",
  },
  {
    title: "Full extension ecosystem",
    desc: "Pi extensions, skills, and prompts appear as native Grok slash commands.",
  },
  {
    title: "Sub-agents & parallel work",
    desc: "Pi sub-agents project into native SubagentBlock, Tasks Pane, and child AgentView.",
  },
  {
    title: "Context visibility",
    desc: "Click the context bar or run /context for a live breakdown of tokens.",
  },
];

const faqs = [
  {
    q: "Do I lose any Grok Build features?",
    a: "grok-pi retains Grok Pager rendering, input, and navigation. Grok product-only surfaces (cloud history, usage, plugins) are replaced by Pi equivalents — local sessions, extensions, full model access.",
  },
  {
    q: "Can I keep using interactive pi?",
    a: "Yes. grok-pi is a separate binary. Run pi for Pi’s TUI, grok-pi for Grok Pager + Pi core. Sessions are shared when they use the same session dir.",
  },
  {
    q: "Can I go back to stock Grok Build?",
    a: "Yes. stock grok is untouched. Run grok for the original product; grok-pi for the bridged host.",
  },
  {
    q: "Does grok-pi modify Pi or Grok source?",
    a: "No. Pi source is not modified for the bridge. Grok renderer identity is guarded; the adapter is a headless JSONL RPC ↔ ACP bridge.",
  },
  {
    q: "What about my existing Pi sessions?",
    a: "They work as-is. grok-pi reads Pi JSONL directly. /resume shows the catalog; --session <uuid> reopens a specific one.",
  },
];

function StepList({
  steps,
}: {
  steps: { step: string; title: string; desc: string; code: string }[];
}) {
  return (
    <div className="mt-6 space-y-6">
      {steps.map((s) => (
        <div key={s.step} className="flex gap-4">
          <div className="shrink-0 w-7 h-7 rounded-md border border-border bg-surface flex items-center justify-center font-mono text-xs font-semibold text-accent">
            {s.step}
          </div>
          <div className="flex-1">
            <h3 className="font-semibold text-text-primary">{s.title}</h3>
            <p className="mt-1 text-sm text-text-secondary">{s.desc}</p>
            <div className="mt-3">
              <CodeBlock code={s.code} />
            </div>
          </div>
        </div>
      ))}
    </div>
  );
}

function CardGrid({
  items,
}: {
  items: { title: string; desc: string }[];
}) {
  return (
    <div className="mt-6 grid gap-4 sm:grid-cols-2">
      {items.map((a) => (
        <div
          key={a.title}
          className="rounded-md border border-border bg-surface p-5"
        >
          <h3 className="font-medium text-text-primary text-sm">{a.title}</h3>
          <p className="mt-1.5 text-xs leading-relaxed text-text-secondary">
            {a.desc}
          </p>
        </div>
      ))}
    </div>
  );
}

export default function MigrationPage() {
  const dict = useDict();
  return (
    <div>
      <h1 className="text-3xl font-bold tracking-tight">
        {dict.docs.sidebar.migration}
      </h1>
      <p className="mt-4 text-lg leading-relaxed text-text-secondary">
        Two entry paths, one host: keep Grok muscle memory or keep Pi sessions —
        grok-pi sits between Grok Pager and Pi agent core.
      </p>

      <h2 id="from-grok" className="mt-10 text-xl font-semibold">
        From stock Grok Build
      </h2>
      <p className="mt-2 text-sm text-text-secondary">
        Same terminal, same shortcuts — agent runtime becomes Pi.
      </p>
      <StepList steps={grokSteps} />

      <h2 id="from-pi" className="mt-12 text-xl font-semibold">
        From interactive Pi
      </h2>
      <p className="mt-2 text-sm text-text-secondary">
        Keep Pi sessions, models, and extensions. Swap the front-end for Grok
        Pager.
      </p>
      <StepList steps={piSteps} />

      <h3 className="mt-10 text-base font-semibold text-text-primary">
        What you keep
      </h3>
      <CardGrid items={piKeep} />

      <h3 className="mt-10 text-base font-semibold text-text-primary">
        What changes
      </h3>
      <CardGrid items={piDiffs} />

      <h2 id="migrate-home" className="mt-12 text-xl font-semibold">
        UI home: stock Grok → ~/.grok-pi
      </h2>
      <p className="mt-2 text-sm text-text-secondary">
        From 0.0.8, user chrome defaults to{" "}
        <code className="font-mono text-xs text-accent">~/.grok-pi</code> so it never
        collides with stock Grok. Pi sessions stay under{" "}
        <code className="font-mono text-xs text-accent">~/.pi/agent</code>. Optional
        one-shot copy of allowlisted files:
      </p>
      <div className="mt-4 space-y-3">
        <CodeBlock
          code={`grok-pi migrate-home --status
grok-pi migrate-home --dry-run
grok-pi migrate-home`}
          label="migrate-home"
        />
      </div>
      <p className="mt-3 text-xs text-text-tertiary">
        Empty target + legacy data may auto-migrate once. Workflows are not copied;
        put Rhai scripts in ~/.grok-pi/workflows or &lt;repo&gt;/.grok-pi/workflows
        after enabling F2 Pi workflows.
      </p>

      <h2 className="mt-12 text-xl font-semibold">Why switch?</h2>
      <CardGrid items={advantages} />

      <h2 className="mt-12 text-xl font-semibold">FAQ</h2>
      <div className="mt-6 space-y-4">
        {faqs.map((faq) => (
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
