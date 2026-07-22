"use client";

import Link from "next/link";
import { useDict } from "@/i18n/provider";
import CodeBlock from "@/components/ui/CodeBlock";

const cards = [
  {
    title: "Installation",
    desc: "One-line install for macOS, Linux, and Windows.",
    href: "/docs/installation/",
  },
  {
    title: "Configuration",
    desc: "Env vars, F2 gates, isolation homes, and CLI flags.",
    href: "/docs/configuration/",
  },
  {
    title: "Architecture",
    desc: "Three boundaries, zero hacks. How the bridge works.",
    href: "/docs/architecture/",
  },
  {
    title: "Commands",
    desc: "Native slash, Pi session ops, workflows, plan, and more.",
    href: "/docs/commands/",
  },
  {
    title: "Features",
    desc: "Resource manager, self-heal, bash, sub-agents, recap.",
    href: "/docs/features/",
  },
  {
    title: "Extensions",
    desc: "Bundled bridges + recommended juicesharp todo/ask.",
    href: "/docs/extensions/",
  },
  {
    title: "Migration",
    desc: "From stock Grok Build or from interactive Pi.",
    href: "/docs/migration/",
  },
] as const;

export default function DocsHome() {
  const dict = useDict();
  return (
    <div>
      <h1 className="text-3xl font-bold tracking-tight">{dict.docs.title}</h1>
      <p className="mt-4 text-lg leading-relaxed text-text-secondary max-w-2xl">
        Everything you need to install, configure, and master grok-pi — Pi&apos;s
        agent core inside Grok Build&apos;s native terminal UI. Docs track{" "}
        <strong className="text-text-primary">v0.0.8</strong> (workflows, isolation,
        self-heal, export).
      </p>

      <div className="mt-10 grid gap-4 sm:grid-cols-2">
        {cards.map((card) => (
          <Link
            key={card.href}
            href={card.href}
            className="group rounded-md border border-border bg-surface p-5 transition-colors duration-150 hover:border-border-accent"
          >
            <h3 className="font-medium text-text-primary text-sm">
              {card.title}
            </h3>
            <p className="mt-2 text-sm text-text-secondary leading-relaxed">
              {card.desc}
            </p>
          </Link>
        ))}
      </div>

      <div className="mt-12">
        <h2 className="text-xl font-semibold mb-4">Quick start</h2>
        <div className="space-y-3">
          <CodeBlock
            code="curl -fsSL https://github.com/Dwsy/grok-pi/releases/latest/download/install.sh | sh"
            label="Install grok-pi"
          />
          <CodeBlock
            code="npm install --global @earendil-works/pi-coding-agent"
            label="Ensure Pi ≥ 0.80.10"
          />
          <CodeBlock code="cd your-project && grok-pi" label="Run" />
        </div>
      </div>
    </div>
  );
}
