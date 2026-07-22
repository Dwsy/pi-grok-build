"use client";

import { useDict } from "@/i18n/provider";
import CodeBlock from "@/components/ui/CodeBlock";
import Link from "next/link";

export default function InstallationPage() {
  const dict = useDict();
  return (
    <div>
      <h1 className="text-3xl font-bold tracking-tight">
        {dict.docs.sidebar.installation}
      </h1>
      <p className="mt-4 text-lg leading-relaxed text-text-secondary">
        Single platform binary. Runtime dependency: Pi ≥{" "}
        <strong className="text-text-primary">0.80.10</strong>. Current release
        line: <strong className="text-text-primary">0.0.8</strong>.
      </p>

      <h2 className="mt-10 text-xl font-semibold">macOS / Linux</h2>
      <p className="mt-2 text-text-secondary">
        Detects platform, installs to{" "}
        <code className="text-accent font-mono text-sm">~/.local/bin</code>, and
        creates a <code className="font-mono text-accent text-sm">pi-grok</code>{" "}
        alias symlink.
      </p>
      <div className="mt-4">
        <CodeBlock
          code="curl -fsSL https://github.com/Dwsy/grok-pi/releases/latest/download/install.sh | sh"
          label="One-line install"
        />
      </div>
      <p className="mt-3 text-sm text-text-tertiary">
        Override install dir with{" "}
        <code className="font-mono text-accent">GROK_PI_INSTALL_DIR</code>.
      </p>

      <h2 className="mt-10 text-xl font-semibold">Windows</h2>
      <div className="mt-4">
        <CodeBlock
          code="irm https://github.com/Dwsy/grok-pi/releases/latest/download/install.ps1 | iex"
          label="PowerShell install"
        />
      </div>

      <h2 className="mt-10 text-xl font-semibold">Prerequisites</h2>
      <div className="mt-4">
        <CodeBlock
          code="npm install --global @earendil-works/pi-coding-agent"
          label="Install Pi"
        />
      </div>
      <p className="mt-3 text-sm text-text-secondary">
        Node.js ≥ 22.19.0 recommended for Pi package installs. grok-pi spawns{" "}
        <code className="font-mono text-xs text-accent">pi</code> (override with{" "}
        <code className="font-mono text-xs text-accent">--pi-bin</code> /{" "}
        <code className="font-mono text-xs text-accent">PI_BIN</code>).
      </p>

      <h2 className="mt-10 text-xl font-semibold">First run</h2>
      <div className="mt-4 space-y-3">
        <CodeBlock code="cd your-project && grok-pi" label="New session" />
        <CodeBlock code="grok-pi --continue" label="Continue last session" />
        <CodeBlock
          code="grok-pi --session 019f88c"
          label="Resume by partial UUID"
        />
      </div>
      <p className="mt-3 text-sm text-text-secondary">
        Coming from stock Grok or interactive Pi? See{" "}
        <Link href="/docs/migration/" className="text-accent hover:underline">
          Migration
        </Link>
        .
      </p>

      <h2 className="mt-10 text-xl font-semibold">Verify</h2>
      <div className="mt-4">
        <CodeBlock code="grok-pi --version" label="Check version" />
      </div>
      <p className="mt-3 text-sm text-text-tertiary">
        Expect <code className="font-mono text-accent">grok-pi 0.0.8</code> or
        newer.
      </p>

      <h2 className="mt-10 text-xl font-semibold">Updating</h2>
      <div className="mt-4 space-y-3">
        <CodeBlock code="grok-pi update --check" label="Check" />
        <CodeBlock code="grok-pi update" label="Install latest" />
      </div>
      <p className="mt-3 text-sm text-text-tertiary">
        Background checks use GitHub Releases (JSP proxy fallback for rate
        limits). Disable with{" "}
        <code className="font-mono text-accent">GROK_PI_NO_AUTO_UPDATE=1</code>.
      </p>

      <h2 className="mt-10 text-xl font-semibold">When an extension crashes startup</h2>
      <p className="mt-2 text-text-secondary">
        grok-pi <strong className="text-text-primary">self-heals</strong>: binary-search
        the extension list, print the culprit, relaunch without it. Manual escape:
      </p>
      <div className="mt-4">
        <CodeBlock code="grok-pi -ne" label="No extensions" />
      </div>

      <h2 className="mt-10 text-xl font-semibold">Build from source</h2>
      <p className="mt-2 text-text-secondary">
        Rust <strong className="text-text-primary">1.92.0</strong> (see{" "}
        <code className="font-mono text-xs">rust-toolchain.toml</code>), Node.js{" "}
        <strong className="text-text-primary">22.19.0+</strong>, npm, system Pi.
      </p>
      <div className="mt-4 space-y-3">
        <CodeBlock code="./build.sh" label="Build" />
        <CodeBlock
          code="PI_BIN=pi ./run-local.sh /path/to/project"
          label="Run locally"
        />
      </div>
    </div>
  );
}
