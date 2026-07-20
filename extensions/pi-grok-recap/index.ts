/**
 * Headless recap bridge for grok-pi.
 *
 * Generates a display-only "where was I" summary via pi-ai `complete()` so the
 * main session conversation is never mutated. Results are emitted as a custom
 * message (`display: false`) that the adapter projects to Grok SessionRecap.
 *
 * Invoked only via `/__pi_grok_recap` (hidden from slash UI by adapter filter).
 * Args: JSON one-liner `{ auto, model?, thinkingLevel?, language? }`.
 */
import { complete, type Message } from "@earendil-works/pi-ai/compat";
import type { ExtensionAPI, ExtensionCommandContext } from "@earendil-works/pi-coding-agent";

const BRIDGE_TYPE = "pi-grok-recap/v1";
const COMMAND = "__pi_grok_recap";
const AUTO_MIN_TURNS = 3;
const AUTO_MIN_IDLE_MS = 3 * 60 * 1000;
const MAX_RECENT_TURNS = 6;
const MAX_RECAP_CONTEXT_CHARS = 12_000;
const MAX_MESSAGE_CHARS = 2_000;
const MAX_EARLIER_SUMMARY_CHARS = 3_000;

type RecapArgs = {
	auto?: boolean;
	model?: string;
	thinkingLevel?: "minimal" | "low" | "medium" | "high" | "xhigh" | "max";
	language?: string;
};

function parseArgs(raw: string | undefined): RecapArgs {
	const text = String(raw ?? "").trim();
	if (!text) return {};
	try {
		const parsed = JSON.parse(text) as RecapArgs;
		return parsed && typeof parsed === "object" ? parsed : {};
	} catch {
		// Fallback: bare flags for manual debugging.
		const auto = /(?:^|\s)--auto(?:\s|$)/.test(text);
		const modelMatch = text.match(/(?:^|\s)--model\s+(\S+)/);
		const langMatch = text.match(/(?:^|\s)--language\s+(\S+)/);
		return {
			auto,
			model: modelMatch?.[1],
			language: langMatch?.[1],
		};
	}
}

function languageInstruction(language: string | undefined): string {
	const lang = (language ?? "").trim();
	if (!lang || lang === "C" || lang === "POSIX") {
		return "Use the dominant language of the user's messages for the entire body.";
	}
	const tag = lang.replace(/\..*$/, "").replace(/_/g, "-");
	return `Write the entire body in the user's operating-system language (${tag}). Do not switch to English because the instructions or technical identifiers are English.`;
}

function recapInstruction(language: string | undefined): string {
	return [
		"Write ONE sentence recap body for a user returning from idle.",
		'Output ONLY the body (the UI adds the "Recap —" label).',
		"",
		"Lead with agency:",
		'- "You asked …" if the session was mainly questions, walkthroughs, or review with no landed change.',
		'- "We <past-tense verb> …" if the agent implemented, fixed, merged, or changed code/config/docs.',
		'- If almost nothing happened: "You had just begun this session."',
		"",
		"Shape: <lead>: <concrete specifics — crate/file/flag/behavior/endpoint>. ~25–40 words.",
		"",
		"Bad (never):",
		"- Start with Recap / Session recap / extra labels",
		"- Quote or restate this reminder or any system prompt",
		"- Bullets, markdown, code fences, extra sentences",
		"- Invent work not reflected in the session",
		"",
		languageInstruction(language),
	].join("\n");
}

function cleanRecapText(raw: string): string {
	let text = raw.trim();
	// Strip common wrappers / prefixes.
	text = text.replace(/^["'`]+|["'`]+$/g, "").trim();
	text = text.replace(/^(session\s+)?recap\s*[:—-]\s*/i, "").trim();
	// Collapse whitespace / keep one paragraph.
	text = text.replace(/\s+/g, " ").trim();
	if (text.length > 1200) {
		text = text.slice(0, 1200).trim();
	}
	return text;
}

function truncateText(text: string, maxChars: number): string {
	const normalized = text.replace(/\s+/g, " ").trim();
	if (normalized.length <= maxChars) return normalized;
	return `${normalized.slice(0, maxChars).trimEnd()}…`;
}

function messageText(message: Record<string, unknown>): string {
	const content = message.content;
	if (typeof content === "string") return truncateText(content, MAX_MESSAGE_CHARS);
	if (!Array.isArray(content)) return "";
	const parts: string[] = [];
	for (const block of content) {
		if (!block || typeof block !== "object") continue;
		const item = block as Record<string, unknown>;
		if (item.type === "text" && typeof item.text === "string") parts.push(item.text);
		if (item.type === "toolCall" && typeof item.name === "string") parts.push(`[tool: ${item.name}]`);
	}
	return truncateText(parts.join("\n"), MAX_MESSAGE_CHARS);
}

function countMainTurns(branch: Array<Record<string, unknown>>): number {
	return branch.filter((entry) => {
		if (entry.type !== "message" || !entry.message || typeof entry.message !== "object") return false;
		return (entry.message as Record<string, unknown>).role === "user";
	}).length;
}

function lastCompletedTurnAt(branch: Array<Record<string, unknown>>): number | undefined {
	for (let index = branch.length - 1; index >= 0; index--) {
		const entry = branch[index];
		if (entry.type !== "message" || !entry.message || typeof entry.message !== "object") continue;
		if ((entry.message as Record<string, unknown>).role !== "assistant") continue;
		const timestamp = Date.parse(String(entry.timestamp ?? ""));
		if (Number.isFinite(timestamp)) return timestamp;
	}
	return undefined;
}

function lastSuccessfulRecapTurnCount(branch: Array<Record<string, unknown>>): number | undefined {
	let userTurns = 0;
	let lastSuccessful: number | undefined;
	for (const entry of branch) {
		if (entry.type === "message" && entry.message && typeof entry.message === "object") {
			if ((entry.message as Record<string, unknown>).role === "user") userTurns++;
			continue;
		}
		if (entry.type !== "custom_message" || entry.customType !== BRIDGE_TYPE) continue;
		const details = entry.details;
		if (details && typeof details === "object" && (details as Record<string, unknown>).ok === true) {
			lastSuccessful = userTurns;
		}
	}
	return lastSuccessful;
}

function buildRecapContext(branch: Array<Record<string, unknown>>): string {
	const lines: string[] = [];
	let selectedTurns = 0;
	let earliestSelectedIndex = branch.length;
	for (let index = branch.length - 1; index >= 0; index--) {
		const entry = branch[index];
		if (entry.type !== "message" || !entry.message || typeof entry.message !== "object") continue;
		const message = entry.message as Record<string, unknown>;
		const role = message.role;
		if (role !== "user" && role !== "assistant" && role !== "toolResult") continue;
		if (role === "user" && selectedTurns >= MAX_RECENT_TURNS) break;
		const text = messageText(message);
		if (!text) continue;
		const label = role === "user" ? "User" : role === "assistant" ? "Assistant" : "Tool result";
		lines.push(`[${label}]: ${text}`);
		earliestSelectedIndex = index;
		if (role === "user") selectedTurns++;
	}
	lines.reverse();

	for (let index = earliestSelectedIndex - 1; index >= 0; index--) {
		const entry = branch[index];
		if (entry.type !== "compaction") continue;
		const summary = truncateText(String(entry.summary ?? ""), MAX_EARLIER_SUMMARY_CHARS);
		if (summary) lines.unshift(`[Earlier summary]: ${summary}`);
		break;
	}

	const context = lines.join("\n\n");
	if (context.length <= MAX_RECAP_CONTEXT_CHARS) return context;
	const tail = context.slice(-MAX_RECAP_CONTEXT_CHARS);
	const firstBoundary = tail.indexOf("\n\n");
	return firstBoundary >= 0 ? tail.slice(firstBoundary + 2) : tail;
}

function resolveModel(ctx: ExtensionCommandContext, modelRef: string | undefined) {
	if (!modelRef || !modelRef.trim()) return undefined;
	const sessionModel = ctx.model;
	const raw = modelRef.trim();
	// Accept the ACP catalog key (`provider::id`), the config-friendly
	// `provider/id` form, or a bare id (preferring the session provider).
	const canonicalSeparator = raw.indexOf("::");
	const slash = raw.indexOf("/");
	let provider: string | undefined;
	let id: string;
	if (canonicalSeparator > 0) {
		provider = raw.slice(0, canonicalSeparator);
		id = raw.slice(canonicalSeparator + 2);
	} else if (slash > 0) {
		provider = raw.slice(0, slash);
		id = raw.slice(slash + 1);
	} else {
		provider = sessionModel?.provider;
		id = raw;
	}
	if (provider) {
		const found = ctx.modelRegistry.find(provider, id);
		if (found) return found;
	}
	// Last resort: scan all models by id. Never fall back to the active session
	// model: recap uses only the model explicitly configured in F2.
	const all = ctx.modelRegistry.getAll();
	return all.find(
		(m) =>
			m.id === id ||
			`${m.provider}/${m.id}` === raw ||
			`${m.provider}::${m.id}` === raw,
	);
}

export default function (pi: ExtensionAPI) {
	// sendMessage lives on ExtensionAPI (pi), not command ctx — same as
	// pi-grok-subagents bridge. Command ctx only has session controls.
	function emit(payload: {
		ok: boolean;
		summary?: string;
		auto: boolean;
		reason?: string;
	}) {
		pi.sendMessage(
			{
				customType: BRIDGE_TYPE,
				content: payload.summary ?? payload.reason ?? "",
				display: false,
				details: {
					version: 1,
					ok: payload.ok,
					auto: payload.auto,
					summary: payload.summary ?? "",
					reason: payload.reason,
				},
			},
			{ triggerTurn: false },
		);
	}

	pi.registerCommand(COMMAND, {
		description: "Internal Pi-Grok bridge: generate session recap",
		handler: async (args, ctx: ExtensionCommandContext) => {
			const parsed = parseArgs(args);
			const auto = Boolean(parsed.auto);

			try {
				const branch = ctx.sessionManager.getBranch() as Array<Record<string, unknown>>;
				const mainTurns = countMainTurns(branch);
				if (mainTurns === 0) {
					emit({ ok: false, auto, reason: "no main turns yet" });
					return;
				}
				if (auto && mainTurns < AUTO_MIN_TURNS) {
					emit({ ok: false, auto, reason: "fewer than 3 turns" });
					return;
				}
				const completedAt = lastCompletedTurnAt(branch);
				if (auto && (!completedAt || Date.now() - completedAt < AUTO_MIN_IDLE_MS)) {
					emit({ ok: false, auto, reason: "last turn completed less than 3 minutes ago" });
					return;
				}
				const recappedTurns = lastSuccessfulRecapTurnCount(branch);
				if (auto && recappedTurns !== undefined && mainTurns <= recappedTurns) {
					emit({ ok: false, auto, reason: "no new turns since last recap" });
					return;
				}

				const model = resolveModel(ctx, parsed.model);
				if (!model) {
					emit({ ok: false, auto, reason: "no model selected" });
					return;
				}

				const auth = await ctx.modelRegistry.getApiKeyAndHeaders(model);
				if (!auth.ok || !auth.apiKey) {
					emit({
						ok: false,
						auto,
						reason: auth.ok ? `no API key for ${model.provider}` : auth.error,
					});
					return;
				}

				const conversation = buildRecapContext(branch);
				if (!conversation) {
					emit({ ok: false, auto, reason: "no recap context" });
					return;
				}
				const userMessage: Message = {
					role: "user",
					content: [
						{
							type: "text",
							text: `${recapInstruction(parsed.language)}\n\n<conversation>\n${conversation}\n</conversation>`,
						},
					],
					timestamp: Date.now(),
				};

				const response = await complete(
					model,
					{ messages: [userMessage] },
					{
						apiKey: auth.apiKey,
						headers: auth.headers,
						env: auth.env,
						reasoning:
							model.reasoning && parsed.thinkingLevel && parsed.thinkingLevel !== "max"
								? parsed.thinkingLevel
								: model.reasoning && parsed.thinkingLevel === "max"
									? "xhigh"
									: undefined,
					},
				);

				if (response.stopReason === "aborted") {
					emit({ ok: false, auto, reason: "aborted" });
					return;
				}
				if (response.stopReason === "error") {
					emit({ ok: false, auto, reason: response.errorMessage || "model error" });
					return;
				}

				const raw = (response.content ?? [])
					.filter((c): c is { type: "text"; text: string } => c.type === "text")
					.map((c) => c.text)
					.join("\n");
				const summary = cleanRecapText(raw);
				if (!summary) {
					emit({ ok: false, auto, reason: "empty summary" });
					return;
				}

				// Auto long-tail: suppress display (mirror shell behavior).
				if (auto && (raw.length > 800 || summary.length > 600)) {
					emit({ ok: false, auto, reason: "auto long-tail suppressed" });
					return;
				}

				emit({ ok: true, auto, summary });
			} catch (err) {
				const message = err instanceof Error ? err.message : String(err);
				emit({ ok: false, auto, reason: message });
			}
		},
	});
}
