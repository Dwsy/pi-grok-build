"use client";

import { useDict } from "@/i18n/provider";
import CodeBlock from "@/components/ui/CodeBlock";

const steps = [
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
    desc: "cd into your project and go. Your Grok keybindings, themes, and habits carry over.",
    code: "cd your-project && grok-pi",
  },
];

const advantages = [
  {
    title: "Keep your Grok muscle memory",
    desc: "Same Pager, same slash commands, same Ctrl+key shortcuts. grok-pi adds capability, not complexity. Every keybinding you've memorized still works.",
  },
  {
    title: "Unlock any model",
    desc: "Stop being locked to a single provider. /model opens Pi's full catalog — GPT-4o, Claude, Gemini, local LLMs, custom endpoints. Switch mid-session.",
  },
  {
    title: "Own your sessions",
    desc: "Local JSONL files. Fork, clone, tag, recap. No cloud dependency. Your work stays on your machine, under your control.",
  },
  {
    title: "Full extension ecosystem",
    desc: "Pi's extensions, skills, and prompts appear as native Grok slash commands. Install once, use everywhere. Community ecosystem included.",
  },
  {
    title: "Sub-agents & parallel work",
    desc: "Pi sub-agents project into native SubagentBlock, Tasks Pane, and child AgentView. Run foreground or background — your choice.",
  },
  {
    title: "Context visibility",
    desc: "Click the context bar or run /context for a live breakdown: system, tools, AGENTS.md, skills, messages. Know exactly where your tokens go.",
  },
];

const faqs = [
  {
    q: "Do I lose any Grok Build features?",
    a: "grok-pi retains all Grok Pager rendering, input, and navigation features. Grok product-specific features (cloud history, usage tracking, plugins) are replaced by Pi equivalents — local sessions, extension ecosystem, and full model access.",
  },
  {
    q: "Can I go back to stock Grok Build?",
    a: "Yes. grok-pi is a separate binary. Your stock grok installation is untouched. Run grok for the original experience, grok-pi for the bridged one.",
  },
  {
    q: "Does grok-pi modify Pi or Grok source?",
    a: "No. Pi source is not modified. 283 Grok renderer files remain byte-for-byte identical. The adapter is a headless JSONL RPC ↔ ACP bridge with 9 narrow seams.",
  },
  {
    q: "What about my existing Pi sessions?",
    a: "They work as-is. grok-pi reads Pi's JSONL session files directly. /resume shows your full session catalog with metadata, and the session tree supports fork, clone, and navigation.",
  },
];

export default function MigrationPage() {
  const dict = useDict();
  return (
    <div>
      <h1 className="text-3xl font-bold tracking-tight">{dict.docs.sidebar.migration}</h1>
      <p className="mt-4 text-lg leading-relaxed text-text-secondary">
        If you can run grok, you can run grok-pi. Same terminal. Same keybindings. Same muscle memory. Just more power underneath.
      </p>

      <h2 className="mt-10 text-xl font-semibold">Migration steps</h2>
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

      <h2 className="mt-12 text-xl font-semibold">Why switch?</h2>
      <div className="mt-6 grid gap-4 sm:grid-cols-2">
        {advantages.map((a) => (
          <div key={a.title} className="rounded-md border border-border bg-surface p-5">
            <h3 className="font-medium text-text-primary text-sm">{a.title}</h3>
            <p className="mt-1.5 text-xs leading-relaxed text-text-secondary">{a.desc}</p>
          </div>
        ))}
      </div>

      <h2 className="mt-12 text-xl font-semibold">FAQ</h2>
      <div className="mt-6 space-y-4">
        {faqs.map((faq) => (
          <div key={faq.q} className="rounded-md border border-border bg-surface p-5">
            <h3 className="font-medium text-text-primary text-sm">{faq.q}</h3>
            <p className="mt-2 text-sm leading-relaxed text-text-secondary">{faq.a}</p>
          </div>
        ))}
      </div>
    </div>
  );
}
