"use client";

import { useDict } from "@/i18n/provider";
import CodeBlock from "@/components/ui/CodeBlock";

export default function InstallationPage() {
  const dict = useDict();
  return (
    <div>
      <h1 className="text-3xl font-bold tracking-tight">{dict.docs.sidebar.installation}</h1>
      <p className="mt-4 text-lg leading-relaxed text-text-secondary">
        grok-pi ships as a single platform binary. No runtime dependencies beyond Pi itself.
      </p>

      <h2 className="mt-10 text-xl font-semibold">macOS / Linux</h2>
      <p className="mt-2 text-text-secondary">
        The install script detects your platform and architecture, downloads the correct binary, and places it in <code className="text-accent font-mono text-sm">~/.local/bin</code> by default.
      </p>
      <div className="mt-4">
        <CodeBlock
          code="curl -fsSL https://github.com/Dwsy/grok-pi/releases/latest/download/install.sh | sh"
          label="One-line install"
        />
      </div>
      <p className="mt-3 text-sm text-text-tertiary">
        Set <code className="font-mono text-accent">GROK_PI_INSTALL_DIR</code> to install to a custom directory.
      </p>

      <h2 className="mt-10 text-xl font-semibold">Windows</h2>
      <div className="mt-4">
        <CodeBlock
          code="irm https://github.com/Dwsy/grok-pi/releases/latest/download/install.ps1 | iex"
          label="PowerShell install"
        />
      </div>

      <h2 className="mt-10 text-xl font-semibold">Prerequisites</h2>
      <p className="mt-2 text-text-secondary">
        grok-pi requires Pi <strong className="text-text-primary">0.80.10 or newer</strong> as its agent core.
      </p>
      <div className="mt-4">
        <CodeBlock
          code="npm install --global @earendil-works/pi-coding-agent"
          label="Install Pi"
        />
      </div>

      <h2 className="mt-10 text-xl font-semibold">Build from source</h2>
      <p className="mt-2 text-text-secondary">
        Requirements: Rust <strong className="text-text-primary">1.92.0</strong>, Node.js <strong className="text-text-primary">22.19.0+</strong>, npm, and a system Pi installation.
      </p>
      <div className="mt-4 space-y-3">
        <CodeBlock code="./build.sh" label="Build" />
        <CodeBlock code="PI_BIN=pi ./run-local.sh /path/to/project" label="Run locally" />
      </div>

      <h2 className="mt-10 text-xl font-semibold">Verify installation</h2>
      <div className="mt-4">
        <CodeBlock code="grok-pi --version" label="Check version" />
      </div>
      <p className="mt-3 text-sm text-text-tertiary">
        You should see <code className="font-mono text-accent">grok-pi 0.0.6</code> or newer.
      </p>

      <h2 className="mt-10 text-xl font-semibold">Updating</h2>
      <p className="mt-2 text-text-secondary">
        grok-pi checks GitHub Releases for updates. Update manually or let it check in the background.
      </p>
      <div className="mt-4 space-y-3">
        <CodeBlock code="grok-pi update --check" label="Check for updates" />
        <CodeBlock code="grok-pi update" label="Install latest" />
      </div>
      <p className="mt-3 text-sm text-text-tertiary">
        Set <code className="font-mono text-accent">GROK_PI_NO_AUTO_UPDATE=1</code> to disable background update checks.
      </p>
    </div>
  );
}
