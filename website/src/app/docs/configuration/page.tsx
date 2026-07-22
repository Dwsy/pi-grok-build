"use client";

import { useDict } from "@/i18n/provider";
import CodeBlock from "@/components/ui/CodeBlock";

const envVars = [
  { name: "PI_GROK_REMOTE_TUI", default: "1", desc: "Enable Pi ctx.ui.custom components rendered through Grok Pager" },
  { name: "PI_GROK_BASH", default: "1", desc: "Enable Grok-owned Bash integration with background tasks" },
  { name: "PI_GROK_NATIVE_COMMANDS", default: "0", desc: "Enable experimental /pi-* native commands" },
  { name: "GROK_PI_NO_AUTO_UPDATE", default: "unset", desc: "Disable background update checks" },
  { name: "GROK_PI_INSTALL_DIR", default: "~/.local/bin", desc: "Custom installation directory" },
];

const flags = [
  { flag: "--pi-cwd <path>", desc: "Run in a different project directory" },
  { flag: "--continue / -c", desc: "Continue the previous session" },
  { flag: "--no-extensions", desc: "Disable all bundled bridge extensions" },
  { flag: "--provider <name>", desc: "Set the model provider" },
  { flag: "--model <id>", desc: "Set the model" },
  { flag: "--thinking <level>", desc: "Set thinking effort level" },
  { flag: "--session <id>", desc: "Resume a specific session" },
  { flag: "--name <name>", desc: "Name the session" },
  { flag: "--system-prompt <text>", desc: "Override the system prompt" },
  { flag: "--append-system-prompt <text>", desc: "Append to the system prompt" },
  { flag: "--tools <list>", desc: "Restrict available tools" },
  { flag: "--exclude-tools <list>", desc: "Exclude specific tools" },
  { flag: "--approve", desc: "Auto-approve tool calls" },
  { flag: "--offline", desc: "Disable network access" },
];

export default function ConfigurationPage() {
  const dict = useDict();
  return (
    <div>
      <h1 className="text-3xl font-bold tracking-tight">{dict.docs.sidebar.configuration}</h1>
      <p className="mt-4 text-lg leading-relaxed text-text-secondary">
        grok-pi is configured through environment variables and CLI flags. Stable bridge extensions are enabled by default; experimental features are opt-in.
      </p>

      <h2 className="mt-10 text-xl font-semibold">Environment variables</h2>
      <div className="mt-4 overflow-x-auto rounded-md border border-border">
        <table className="w-full text-sm">
          <thead>
            <tr className="border-b border-border bg-surface/80">
              <th className="px-4 py-3 text-left font-semibold text-text-secondary">Variable</th>
              <th className="px-4 py-3 text-left font-semibold text-text-secondary">Default</th>
              <th className="px-4 py-3 text-left font-semibold text-text-secondary">Purpose</th>
            </tr>
          </thead>
          <tbody>
            {envVars.map((v, i) => (
              <tr key={v.name} className={`border-b border-border/50 ${i % 2 === 1 ? "bg-surface/20" : ""}`}>
                <td className="px-4 py-3 font-mono text-xs text-accent whitespace-nowrap">{v.name}</td>
                <td className="px-4 py-3 font-mono text-xs text-text-tertiary">{v.default}</td>
                <td className="px-4 py-3 text-text-secondary">{v.desc}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      <h2 className="mt-10 text-xl font-semibold">CLI flags</h2>
      <p className="mt-2 text-text-secondary">
        Pi startup options can be passed directly after <code className="font-mono text-accent text-sm">--</code>.
      </p>
      <div className="mt-4">
        <CodeBlock code="grok-pi -- --model openai/gpt-4o" label="Pass Pi flags" />
      </div>
      <div className="mt-4 overflow-x-auto rounded-md border border-border">
        <table className="w-full text-sm">
          <thead>
            <tr className="border-b border-border bg-surface/80">
              <th className="px-4 py-3 text-left font-semibold text-text-secondary">Flag</th>
              <th className="px-4 py-3 text-left font-semibold text-text-secondary">Description</th>
            </tr>
          </thead>
          <tbody>
            {flags.map((f, i) => (
              <tr key={f.flag} className={`border-b border-border/50 ${i % 2 === 1 ? "bg-surface/20" : ""}`}>
                <td className="px-4 py-3 font-mono text-xs text-accent whitespace-nowrap">{f.flag}</td>
                <td className="px-4 py-3 text-text-secondary">{f.desc}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      <h2 className="mt-10 text-xl font-semibold">Themes</h2>
      <p className="mt-2 text-text-secondary">
        grok-pi maps Pi theme JSON to Grok&apos;s native Theme system. Use <code className="font-mono text-accent text-sm">/theme pi:&lt;name&gt;</code> to apply a Pi theme.
      </p>
      <p className="mt-2 text-text-secondary">
        Two built-in experimental themes leave the main canvas transparent for terminal opacity/blur:
      </p>
      <ul className="mt-3 space-y-1.5 text-sm text-text-secondary">
        <li className="flex items-center gap-2">
          <span className="text-accent">•</span>
          <code className="font-mono text-xs">pi:transparent</code> — dark transparent
        </li>
        <li className="flex items-center gap-2">
          <span className="text-accent">•</span>
          <code className="font-mono text-xs">pi:transparent-light</code> — light transparent
        </li>
      </ul>
    </div>
  );
}
