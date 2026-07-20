import * as path from "node:path";
import { existsSync, mkdirSync, realpathSync, unlinkSync, writeFileSync } from "node:fs";
import { spawn, spawnSync } from "node:child_process";
import * as os from "node:os";
import { pathToFileURL } from "node:url";
import type { ExtensionAPI, ExtensionCommandContext } from "@earendil-works/pi-coding-agent";
import type { Model } from "@earendil-works/pi-ai";
import type { Component, TUI } from "@earendil-works/pi-tui";

type SessionListProgress = (loaded: number, total: number) => void;

type SessionInfo = {
	path: string;
	name?: string;
};

interface ModelSelectorConstructor {
	new (
		tui: TUI,
		currentModel: Model<any> | undefined,
		settingsManager: { setDefaultModelAndProvider(provider: string, modelId: string): void },
		modelRegistry: ExtensionCommandContext["modelRegistry"],
		scopedModels: readonly [],
		onSelect: (model: Model<any>) => void,
		onCancel: () => void,
		initialSearchInput?: string,
	): Component;
}

interface SessionSelectorConstructor {
	new (
		currentSessionsLoader: (onProgress?: SessionListProgress) => Promise<SessionInfo[]>,
		allSessionsLoader: (onProgress?: SessionListProgress) => Promise<SessionInfo[]>,
		onSelect: (sessionPath: string) => void,
		onCancel: () => void,
		onExit: () => void,
		requestRender: () => void,
		options: { showRenameHint: boolean },
		currentSessionFilePath?: string,
	): Component;
}

interface SessionManagerStatic {
	list(cwd: string, sessionDir: string | undefined, onProgress?: SessionListProgress): Promise<SessionInfo[]>;
	listAll(sessionDir?: string, onProgress?: SessionListProgress): Promise<SessionInfo[]>;
}

interface SettingsManagerStatic {
	create(cwd: string): { setDefaultModelAndProvider(provider: string, modelId: string): void };
}

function hostUrl(relativePath: string): string {
	const hostDistDir = path.dirname(realpathSync(process.argv[1]!));
	return new URL(relativePath, pathToFileURL(hostDistDir).href + "/").href;
}

function remoteTuiAvailable(): boolean {
	return process.env.PI_GROK_REMOTE_TUI === "1";
}

function unavailableCommand(name: string, reason: string) {
	return {
		description: `[experimental] Pi native /${name} (${reason})`,
		handler: async (_args: string, ctx: ExtensionCommandContext) => {
			ctx.ui.notify(`/${name} is not exposed by Pi RPC: ${reason}`, "warning");
		},
	};
}

/** First path token after a slash command; supports single/double quotes. */
function pathCommandArgument(args: string): string | undefined {
	const argsString = args.trimStart();
	if (!argsString) return undefined;

	const firstChar = argsString[0];
	if (firstChar === '"' || firstChar === "'") {
		const closingQuoteIndex = argsString.indexOf(firstChar, 1);
		if (closingQuoteIndex < 0) return undefined;
		return argsString.slice(1, closingQuoteIndex);
	}

	const firstWhitespaceIndex = argsString.search(/\s/);
	if (firstWhitespaceIndex < 0) return argsString;
	return argsString.slice(0, firstWhitespaceIndex);
}

function expandUserPath(outputPath: string, cwd: string): string {
	const expanded =
		outputPath.startsWith("~/") || outputPath === "~"
			? path.join(os.homedir(), outputPath.slice(1))
			: outputPath;
	return path.isAbsolute(expanded) ? expanded : path.resolve(cwd, expanded);
}

type ExportSessionToHtml = (
	sm: ExtensionCommandContext["sessionManager"],
	state?: unknown,
	options?: { outputPath?: string } | string,
) => Promise<string>;

type SessionHeader = {
	type: "session";
	version: number;
	id: string;
	timestamp: string;
	cwd: string;
};

function exportBranchToJsonl(
	sessionManager: ExtensionCommandContext["sessionManager"],
	outputPath: string | undefined,
	currentSessionVersion: number,
): string {
	const filePath = expandUserPath(
		outputPath ?? `session-${new Date().toISOString().replace(/[:.]/g, "-")}.jsonl`,
		process.cwd(),
	);
	const dir = path.dirname(filePath);
	if (!existsSync(dir)) {
		mkdirSync(dir, { recursive: true });
	}

	const header: SessionHeader = {
		type: "session",
		version: currentSessionVersion,
		id: sessionManager.getSessionId(),
		timestamp: new Date().toISOString(),
		cwd: sessionManager.getCwd(),
	};

	const branchEntries = sessionManager.getBranch();
	const lines = [JSON.stringify(header)];
	let prevId: string | null = null;
	for (const entry of branchEntries) {
		const linear = { ...entry, parentId: prevId };
		lines.push(JSON.stringify(linear));
		prevId = entry.id;
	}

	writeFileSync(filePath, `${lines.join("\n")}\n`);
	return filePath;
}

async function runGhGistCreate(tmpFile: string): Promise<{ stdout: string; stderr: string; code: number | null }> {
	return await new Promise((resolve) => {
		const proc = spawn("gh", ["gist", "create", "--public=false", tmpFile]);
		let stdout = "";
		let stderr = "";
		proc.stdout?.on("data", (data) => {
			stdout += data.toString();
		});
		proc.stderr?.on("data", (data) => {
			stderr += data.toString();
		});
		proc.on("close", (code) => resolve({ stdout, stderr, code }));
		proc.on("error", (error) => resolve({ stdout, stderr: error.message, code: 1 }));
	});
}

export default async function piGrokNativeCommands(pi: ExtensionAPI) {
	if (process.env.PI_GROK !== "1") {
		return;
	}

	const [{ ModelSelectorComponent }, { SessionSelectorComponent, SessionManager, SettingsManager }] = await Promise.all([
		import(hostUrl("modes/interactive/components/model-selector.js")) as Promise<{
			ModelSelectorComponent: ModelSelectorConstructor;
		}>,
		Promise.all([
			import(hostUrl("modes/interactive/components/session-selector.js")),
			import(hostUrl("core/session-manager.js")),
			import(hostUrl("core/settings-manager.js")),
		]).then(([selector, manager, settings]) => ({
			SessionSelectorComponent: selector.SessionSelectorComponent as SessionSelectorConstructor,
			SessionManager: manager.SessionManager as SessionManagerStatic,
			SettingsManager: settings.SettingsManager as SettingsManagerStatic,
		})),
	]);

	pi.registerCommand("pi-model", {
		description: "[experimental] Open Pi's native model selector",
		handler: async (args: string, ctx: ExtensionCommandContext) => {
			if (!remoteTuiAvailable()) {
				ctx.ui.notify("/pi-model requires PI_GROK_REMOTE_TUI=1", "warning");
				return;
			}

			await ctx.ui.custom<void>((tui, _theme, _keybindings, done) => {
				const selector = new ModelSelectorComponent(
					tui,
					ctx.model,
					SettingsManager.create(ctx.cwd),
					ctx.modelRegistry,
					[],
					(model) => {
						done(undefined);
						void pi.setModel(model);
					},
					() => done(undefined),
					args.trim() || undefined,
				);
				return selector;
			});
		},
	});

	pi.registerCommand("pi-resume", {
		description: "[experimental] Open Pi's native session selector",
		handler: async (_args: string, ctx: ExtensionCommandContext) => {
			if (!remoteTuiAvailable()) {
				ctx.ui.notify("/pi-resume requires PI_GROK_REMOTE_TUI=1", "warning");
				return;
			}

			const switchSession = ctx.switchSession.bind(ctx);
			const sessionManager = ctx.sessionManager;
			await ctx.ui.custom<void>((_tui, _theme, _keybindings, done) => {
				const currentSessionsLoader = (onProgress?: (loaded: number, total: number) => void) =>
					SessionManager.list(sessionManager.getCwd(), sessionManager.getSessionDir(), onProgress);
				const allSessionsLoader = (onProgress?: (loaded: number, total: number) => void) =>
					SessionManager.listAll(sessionManager.getSessionDir(), onProgress);
				const selector = new SessionSelectorComponent(
					currentSessionsLoader,
					allSessionsLoader,
					(sessionPath) => {
						done(undefined);
						void switchSession(sessionPath);
					},
					() => done(undefined),
					() => done(undefined),
					() => {},
					{ showRenameHint: false },
					sessionManager.getSessionFile(),
				);
				return selector;
			});
		},
	});

	pi.registerCommand("pi-reload", {
		description: "Reload Pi resources using Pi's native reload lifecycle",
		handler: async (_args: string, ctx: ExtensionCommandContext) => {
			await ctx.reload();
		},
	});

	// pi-login / pi-logout live in the default-on pi-grok-auth extension.

	const [{ exportSessionToHtml }, { getShareViewerUrl }, { CURRENT_SESSION_VERSION }] = await Promise.all([
		import(hostUrl("core/export-html/index.js")) as Promise<{ exportSessionToHtml: ExportSessionToHtml }>,
		import(hostUrl("config.js")) as Promise<{ getShareViewerUrl: (gistId: string) => string }>,
		import(hostUrl("core/session-manager.js")) as Promise<{ CURRENT_SESSION_VERSION: number }>,
	]);

	// Hand the mic to Pi's own export/share paths (host dist), not Grok transcript export.
	pi.registerCommand("pi-export", {
		description: "[experimental] Export the current Pi session as HTML or JSONL",
		handler: async (args: string, ctx: ExtensionCommandContext) => {
			const outputPath = pathCommandArgument(args);
			try {
				if (outputPath?.endsWith(".jsonl")) {
					const filePath = exportBranchToJsonl(ctx.sessionManager, outputPath, CURRENT_SESSION_VERSION);
					ctx.ui.notify(`Session exported to: ${filePath}`, "info");
					return;
				}

				const resolvedOutput = outputPath ? expandUserPath(outputPath, ctx.cwd) : undefined;
				const filePath = await exportSessionToHtml(ctx.sessionManager, undefined, resolvedOutput);
				ctx.ui.notify(`Session exported to: ${filePath}`, "info");
			} catch (error: unknown) {
				ctx.ui.notify(
					`Failed to export session: ${error instanceof Error ? error.message : String(error)}`,
					"error",
				);
			}
		},
	});

	pi.registerCommand("pi-import", unavailableCommand("import", "Pi RPC has no import-session operation"));

	pi.registerCommand("pi-share", {
		description: "[experimental] Share the current Pi session via private GitHub gist",
		handler: async (_args: string, ctx: ExtensionCommandContext) => {
			try {
				const authResult = spawnSync("gh", ["auth", "status"], { encoding: "utf-8" });
				if (authResult.error) {
					ctx.ui.notify("GitHub CLI (gh) is not installed. Install it from https://cli.github.com/", "error");
					return;
				}
				if (authResult.status !== 0) {
					ctx.ui.notify("GitHub CLI is not logged in. Run 'gh auth login' first.", "error");
					return;
				}
			} catch {
				ctx.ui.notify("GitHub CLI (gh) is not installed. Install it from https://cli.github.com/", "error");
				return;
			}

			const tmpFile = path.join(os.tmpdir(), `pi-grok-share-${process.pid}-${Date.now()}.html`);
			try {
				await exportSessionToHtml(ctx.sessionManager, undefined, tmpFile);
			} catch (error: unknown) {
				ctx.ui.notify(
					`Failed to export session: ${error instanceof Error ? error.message : String(error)}`,
					"error",
				);
				return;
			}

			ctx.ui.notify("Creating private gist...", "info");
			try {
				const result = await runGhGistCreate(tmpFile);
				if (result.code !== 0) {
					const errorMsg = result.stderr?.trim() || "Unknown error";
					ctx.ui.notify(`Failed to create gist: ${errorMsg}`, "error");
					return;
				}

				const gistUrl = result.stdout?.trim();
				const gistId = gistUrl?.split("/").pop();
				if (!gistId) {
					ctx.ui.notify("Failed to parse gist ID from gh output", "error");
					return;
				}

				const previewUrl = getShareViewerUrl(gistId);
				ctx.ui.notify(`Share URL: ${previewUrl}\nGist: ${gistUrl}`, "info");
			} catch (error: unknown) {
				ctx.ui.notify(
					`Failed to create gist: ${error instanceof Error ? error.message : String(error)}`,
					"error",
				);
			} finally {
				try {
					unlinkSync(tmpFile);
				} catch {
					// Ignore cleanup errors
				}
			}
		},
	});
}
