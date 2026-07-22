"use client";

import { useDict } from "@/i18n/provider";
import CodeBlock from "@/components/ui/CodeBlock";

const envVars = [
  { name: "GROK_HOME", default: "~/.grok-pi", desc: "User state root (isolated from stock Grok ~/.grok)" },
  { name: "GROK_PROJECT_DIR", default: ".grok-pi", desc: "Project config/workflows/hooks dirname under the repo" },
  { name: "GROK_LEGACY_HOME", default: "~/.grok", desc: "Source tree for migrate-home / auto-migrate" },
  { name: "PI_GROK_REMOTE_TUI", default: "1", desc: "Host Pi ctx.ui.custom through Grok Pager (Remote TUI)" },
  { name: "PI_GROK_BASH", default: "1", desc: "Grok-owned Bash + Send to Background" },
  { name: "PI_GROK_NATIVE_COMMANDS", default: "0", desc: "Experimental /pi-* native selectors" },
  { name: "PI_GROK_EXCLUDE_TOOLS", default: "unset", desc: "Comma-separated built-in tools to exclude" },
  { name: "GROK_PI_NO_AUTO_UPDATE", default: "unset", desc: "Disable background GitHub update checks" },
  { name: "GROK_PI_INSTALL_DIR", default: "~/.local/bin", desc: "Custom install path for install.sh" },
  { name: "PI_BIN", default: "pi", desc: "Pi binary used by the host" },
  { name: "PI_CODING_AGENT_SESSION_DIR", default: "Pi default", desc: "Override Pi session root (same as Pi)" },
];

const flags = [
  { flag: "--pi-cwd <path>", desc: "Project directory for the Pi child" },
  { flag: "--pi-bin <path>", desc: "Pi executable path" },
  { flag: "--continue / -c", desc: "Continue the previous session (skips Welcome)" },
  { flag: "--session <id>", desc: "Resume session (partial UUID OK)" },
  { flag: "--session-dir <path>", desc: "Custom Pi session directory" },
  { flag: "--fork", desc: "Fork semantics at startup (Pi flag)" },
  { flag: "--no-session", desc: "Do not open/persist a session file" },
  { flag: "--name <name>", desc: "Name the session" },
  { flag: "--provider / --model", desc: "Provider and model selection" },
  { flag: "--thinking <level>", desc: "Thinking effort" },
  { flag: "--system-prompt / --append-system-prompt", desc: "Prompt overrides" },
  { flag: "--extension <path>", desc: "Extra Pi extension paths" },
  { flag: "--no-extensions / -ne", desc: "Disable injected bridge + user extensions" },
  { flag: "--no-skills / --no-context-files", desc: "Disable Pi skill / AGENTS discovery" },
  { flag: "--tools / --exclude-tools / --no-tools / -nt", desc: "Tool allow/deny lists" },
  { flag: "--no-builtin-tools / -nbt", desc: "Drop Pi built-in tools" },
  { flag: "--approve / --no-approve", desc: "Trust / approval gates" },
  { flag: "--offline", desc: "Disable network where Pi honors it" },
  { flag: "migrate-home", desc: "CLI subcommand: copy allowlisted files from legacy ~/.grok" },
  { flag: "update [--check]", desc: "Install or check GitHub Releases" },
];

const f2Gates = [
  { key: "[ui].pi_workflows", def: "off", note: "Rhai workflows (/workflow…); restart required" },
  { key: "[ui].pi_goal", def: "off", note: "Goal mode MVP (/goal); restart required" },
  { key: "[ui].pi_tree_file_rollback", def: "off", note: "SessionTree r/R file rollback; restart" },
  { key: "recap_mermaid", def: "off", note: "Render Mermaid in /recap bodies" },
  { key: "remote_tui_footer", def: "varies", note: "Remote TUI footer lab surface" },
];

export default function ConfigurationPage() {
  const dict = useDict();
  return (
    <div>
      <h1 className="text-3xl font-bold tracking-tight">
        {dict.docs.sidebar.configuration}
      </h1>
      <p className="mt-4 text-lg leading-relaxed text-text-secondary">
        Env vars, CLI flags, F2 feature gates, and product-isolated state trees.
        Stable bridges default on; experimental surfaces are opt-in.
      </p>

      <h2 className="mt-10 text-xl font-semibold">Product state isolation</h2>
      <p className="mt-2 text-text-secondary">
        grok-pi does <strong className="text-text-primary">not</strong> share
        stock Grok&apos;s config roots by default:
      </p>
      <div className="mt-4 overflow-x-auto rounded-md border border-border">
        <table className="w-full text-sm">
          <thead>
            <tr className="border-b border-border bg-surface/80">
              <th className="px-4 py-3 text-left font-semibold text-text-secondary">Layer</th>
              <th className="px-4 py-3 text-left font-semibold text-text-secondary">stock Grok</th>
              <th className="px-4 py-3 text-left font-semibold text-text-secondary">grok-pi default</th>
            </tr>
          </thead>
          <tbody className="text-text-secondary">
            <tr className="border-b border-border/50">
              <td className="px-4 py-3">User home</td>
              <td className="px-4 py-3 font-mono text-xs">~/.grok</td>
              <td className="px-4 py-3 font-mono text-xs text-accent">~/.grok-pi</td>
            </tr>
            <tr>
              <td className="px-4 py-3">Project tree</td>
              <td className="px-4 py-3 font-mono text-xs">&lt;repo&gt;/.grok</td>
              <td className="px-4 py-3 font-mono text-xs text-accent">&lt;repo&gt;/.grok-pi</td>
            </tr>
          </tbody>
        </table>
      </div>
      <p className="mt-3 text-sm text-text-secondary">
        Pi agent state stays under{" "}
        <code className="font-mono text-xs text-accent">~/.pi/agent</code> (or{" "}
        <code className="font-mono text-xs text-accent">--session-dir</code>). No
        dual-scan of stock trees. Migrate UI prefs with:
      </p>
      <div className="mt-3 space-y-3">
        <CodeBlock
          code={`grok-pi migrate-home --status
grok-pi migrate-home --dry-run
grok-pi migrate-home          # copy allowlisted files
grok-pi migrate-home --include-auth   # optional`}
          label="migrate-home"
        />
      </div>
      <p className="mt-3 text-xs text-text-tertiary">
        Empty target + legacy data → one-shot auto-migrate with{" "}
        <code className="font-mono">.migrated-from-legacy</code> marker. Workflows
        are not auto-copied; place Rhai scripts under{" "}
        <code className="font-mono">~/.grok-pi/workflows</code> or{" "}
        <code className="font-mono">&lt;repo&gt;/.grok-pi/workflows</code>.
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
              <tr
                key={v.name}
                className={`border-b border-border/50 ${i % 2 === 1 ? "bg-surface/20" : ""}`}
              >
                <td className="px-4 py-3 font-mono text-xs text-accent whitespace-nowrap">
                  {v.name}
                </td>
                <td className="px-4 py-3 font-mono text-xs text-text-tertiary">
                  {v.default}
                </td>
                <td className="px-4 py-3 text-text-secondary">{v.desc}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      <h2 className="mt-10 text-xl font-semibold">CLI flags & subcommands</h2>
      <p className="mt-2 text-text-secondary">
        First-class Pi flags are forwarded by the host. Extra args after{" "}
        <code className="font-mono text-accent text-sm">--</code> still pass through.
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
              <tr
                key={f.flag}
                className={`border-b border-border/50 ${i % 2 === 1 ? "bg-surface/20" : ""}`}
              >
                <td className="px-4 py-3 font-mono text-xs text-accent whitespace-nowrap">
                  {f.flag}
                </td>
                <td className="px-4 py-3 text-text-secondary">{f.desc}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      <h2 className="mt-10 text-xl font-semibold">F2 feature gates</h2>
      <p className="mt-2 text-sm text-text-secondary">
        Open F2 (settings). Pi-only rows are{" "}
        <code className="font-mono text-xs text-accent">external_only</code> — hidden
        unless you are on the grok-pi / external profile.
      </p>
      <div className="mt-4 overflow-x-auto rounded-md border border-border">
        <table className="w-full text-sm">
          <thead>
            <tr className="border-b border-border bg-surface/80">
              <th className="px-4 py-3 text-left font-semibold text-text-secondary">Setting</th>
              <th className="px-4 py-3 text-left font-semibold text-text-secondary">Default</th>
              <th className="px-4 py-3 text-left font-semibold text-text-secondary">Notes</th>
            </tr>
          </thead>
          <tbody>
            {f2Gates.map((g, i) => (
              <tr
                key={g.key}
                className={`border-b border-border/50 ${i % 2 === 1 ? "bg-surface/20" : ""}`}
              >
                <td className="px-4 py-3 font-mono text-xs text-accent whitespace-nowrap">
                  {g.key}
                </td>
                <td className="px-4 py-3 font-mono text-xs text-text-tertiary">
                  {g.def}
                </td>
                <td className="px-4 py-3 text-text-secondary">{g.note}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      <h2 className="mt-10 text-xl font-semibold">Pi resource manager</h2>
      <p className="mt-2 text-text-secondary">
        <code className="font-mono text-accent text-sm">/pi-config</code> (alias{" "}
        <code className="font-mono text-accent text-sm">/pi-resources</code>) or F2 →
        Pi resources opens a <strong className="text-text-primary">Rust-native</strong>{" "}
        two-pane manager: extensions, skills, prompts, themes across global and
        trusted-project scopes. Reads Pi{" "}
        <code className="font-mono text-xs">settings.json</code> /{" "}
        <code className="font-mono text-xs">trust.json</code>. Does{" "}
        <strong className="text-text-primary">not</strong> run{" "}
        <code className="font-mono text-xs">pi install/remove/update</code> — use the
        Pi CLI for package lifecycle, then refresh or{" "}
        <code className="font-mono text-xs">/reload</code>.
      </p>
      <ul className="mt-3 space-y-1.5 text-sm text-text-secondary list-disc pl-5">
        <li>Filters: All / Enabled / Disabled; policy view (<code className="font-mono text-xs">a</code>); refresh (<code className="font-mono text-xs">r</code>); Tab = scope</li>
        <li>Source identities: GitHub / npm / local paths (not generic tags)</li>
        <li>Admission policy can block noisy sources (e.g. custom header/footer, pi-tool-display)</li>
      </ul>

      <h2 className="mt-10 text-xl font-semibold">Extension self-heal</h2>
      <p className="mt-2 text-text-secondary">
        If a Pi extension crashes RPC bootstrap, grok-pi runs a{" "}
        <strong className="text-text-primary">VS Code-style binary search</strong>{" "}
        over <code className="font-mono text-xs">--extension</code> paths, names the
        culprit, and relaunches without it so you are not stuck. Escape hatch:{" "}
        <code className="font-mono text-accent text-sm">grok-pi -ne</code>.
      </p>

      <h2 className="mt-10 text-xl font-semibold">Themes</h2>
      <p className="mt-2 text-text-secondary">
        <code className="font-mono text-accent text-sm">/theme pi:&lt;name&gt;</code>{" "}
        maps Pi theme JSON into Grok Theme. Built-ins for terminal opacity:
      </p>
      <ul className="mt-3 space-y-1.5 text-sm text-text-secondary">
        <li>
          <code className="font-mono text-xs text-accent">pi:transparent</code> — dark
        </li>
        <li>
          <code className="font-mono text-xs text-accent">pi:transparent-light</code> — light
        </li>
      </ul>
    </div>
  );
}
