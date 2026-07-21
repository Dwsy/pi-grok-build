/**
 * Present the Remote TUI host as `tui` to third-party Pi extensions.
 *
 * Pi itself stays in JSONL RPC mode. This adjusts only the extension context
 * at Pi's UI-binding boundary, after grok-pi has installed its Remote TUI
 * `custom()` host. It intentionally has no effect unless the host opts in.
 */

import { dirname } from "node:path";
import { pathToFileURL } from "node:url";
import { realpathSync } from "node:fs";
import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";

type ExtensionRunnerLike = {
  setUIContext: (uiContext: unknown, mode?: string) => void;
};

type ExtensionRunnerConstructor = {
  prototype: ExtensionRunnerLike & { __piGrokTuiModeFacade?: boolean };
};

function hostUrl(relativePath: string): string {
  const hostDistDir = dirname(realpathSync(process.argv[1]!));
  return new URL(relativePath, pathToFileURL(`${hostDistDir}/`)).href;
}

async function installTuiModeFacade(): Promise<void> {
  const module = (await import(hostUrl("core/extensions/runner.js"))) as {
    ExtensionRunner?: ExtensionRunnerConstructor;
  };
  const prototype = module.ExtensionRunner?.prototype;
  if (!prototype) {
    throw new Error("Pi ExtensionRunner is unavailable for grok-pi Remote TUI compatibility");
  }
  if (prototype.__piGrokTuiModeFacade) return;

  const original = prototype.setUIContext;
  prototype.setUIContext = function setUIContext(this: ExtensionRunnerLike, uiContext: unknown, mode = "print"): void {
    original.call(this, uiContext, mode === "rpc" ? "tui" : mode);
  };
  prototype.__piGrokTuiModeFacade = true;
}

export default async function (_pi: ExtensionAPI): Promise<void> {
  if (process.env.PI_GROK_EXTENSION_TUI_COMPAT !== "1") return;
  await installTuiModeFacade();
}
