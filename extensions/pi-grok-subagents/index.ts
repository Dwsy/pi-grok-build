import { randomUUID } from "node:crypto";
import { Type } from "typebox";
import {
  createAgentSession,
  DefaultResourceLoader,
  getAgentDir,
  SessionManager,
  SettingsManager,
  type AgentSession,
  type AgentSessionEvent,
  type ExtensionAPI,
  type ExtensionContext,
} from "@earendil-works/pi-coding-agent";

const BRIDGE_TYPE = "pi-grok-subagent/v1";
const STATE_ENTRY_TYPE = "pi-grok-subagent-state/v1";
const PROGRESS_INTERVAL_MS = 2_000;
const MAX_BACKGROUND_CONCURRENCY = 4;

const CAPABILITY_TOOLS = {
  "read-only": ["read"],
  "read-write": ["read", "edit", "write"],
  execute: ["read", "bash"],
  all: ["read", "bash", "edit", "write"],
} as const;

type CapabilityMode = keyof typeof CAPABILITY_TOOLS;

const AGENT_PROFILES: Record<string, { capabilityMode: CapabilityMode; systemPrompt: string }> = {
  "general-purpose": {
    capabilityMode: "all",
    systemPrompt: "You are a focused coding subagent. Complete only the delegated task and return a concise evidence-based result.",
  },
  explore: {
    capabilityMode: "execute",
    systemPrompt: "You are a read-only exploration subagent. Inspect the codebase, run safe diagnostic commands, and report evidence without editing files.",
  },
  plan: {
    capabilityMode: "execute",
    systemPrompt: "You are a planning subagent. Inspect the codebase and return an implementation plan with risks and verification steps. Do not edit files.",
  },
};

type ChildUpdate =
  | { type: "assistant_delta"; text: string }
  | { type: "thinking_delta"; text: string }
  | { type: "user"; text: string }
  | { type: "tool_call"; toolCallId: string; toolName: string; args: unknown }
  | { type: "tool_update"; toolCallId: string; toolName: string; partialResult: unknown }
  | { type: "tool_result"; toolCallId: string; toolName: string; result: unknown; isError: boolean };

type BridgeKind = "spawned" | "progress" | "child_update" | "finished";

type BridgeEnvelope = {
  version: 1;
  sequence: number;
  replay: boolean;
  kind: BridgeKind;
  parentSessionId: string;
  subagentId: string;
  childSessionId: string;
  payload: Record<string, unknown>;
};

type PersistedRecord = {
  version: 1;
  id: string;
  childSessionId: string;
  childSessionFile: string;
  parentSessionId: string;
  parentToolCallId: string;
  prompt: string;
  description: string;
  type: string;
  capabilityMode: CapabilityMode;
  modelId: string;
  background: boolean;
  startedAt: number;
  status: "running" | "completed" | "failed" | "cancelled";
  turnCount: number;
  toolCallCount: number;
  tokensUsed: number;
};

type SubagentRecord = {
  id: string;
  childSessionId: string;
  childSessionFile: string;
  parentSessionId: string;
  parentToolCallId: string;
  prompt: string;
  description: string;
  type: string;
  capabilityMode: CapabilityMode;
  modelId: string;
  background: boolean;
  startedAt: number;
  session: AgentSession;
  turnCount: number;
  toolCallCount: number;
  toolsUsed: Set<string>;
  errorCount: number;
  tokensUsed: number;
  finished: boolean;
  cancelRequested: boolean;
  progressTimer: ReturnType<typeof setInterval>;
  removeAbortListener: () => void;
  unsubscribe: () => void;
};

function requireText(value: unknown, name: string): string {
  if (typeof value !== "string" || value.trim().length === 0) {
    throw new Error(`${name} is required`);
  }
  return value.trim();
}

function requireCapability(value: string | undefined): CapabilityMode {
  const capability = value ?? "all";
  if (!(capability in CAPABILITY_TOOLS)) {
    throw new Error(`unsupported capability_mode: ${capability}`);
  }
  return capability as CapabilityMode;
}

function resolveProfile(type: string, capabilityMode: string | undefined): {
  type: string;
  capabilityMode: CapabilityMode;
  systemPrompt: string;
} {
  const normalizedType = type.trim() || "general-purpose";
  const profile = AGENT_PROFILES[normalizedType] ?? AGENT_PROFILES["general-purpose"];
  return {
    type: normalizedType,
    capabilityMode: requireCapability(capabilityMode ?? profile.capabilityMode),
    systemPrompt: profile.systemPrompt,
  };
}

function textFromContent(content: unknown): string {
  if (typeof content === "string") return content;
  if (!Array.isArray(content)) return "";
  return content
    .filter((block): block is { type: string; text?: unknown } => typeof block === "object" && block !== null)
    .filter((block) => block.type === "text" && typeof block.text === "string")
    .map((block) => block.text as string)
    .join("");
}

function lastAssistantText(session: AgentSession): string {
  for (let index = session.messages.length - 1; index >= 0; index -= 1) {
    const message = session.messages[index];
    if (message.role !== "assistant") continue;
    return message.content
      .filter((block) => block.type === "text")
      .map((block) => block.text)
      .join("")
      .trim();
  }
  return "";
}

function extractUsage(message: unknown): number {
  if (typeof message !== "object" || message === null) return 0;
  const usage = (message as { usage?: unknown }).usage;
  if (typeof usage !== "object" || usage === null) return 0;
  const input = (usage as { input?: unknown }).input;
  const output = (usage as { output?: unknown }).output;
  return (typeof input === "number" ? input : 0) + (typeof output === "number" ? output : 0);
}

function childUpdate(event: AgentSessionEvent): ChildUpdate | undefined {
  if (event.type === "message_update") {
    if (event.assistantMessageEvent.type === "text_delta") {
      return { type: "assistant_delta", text: event.assistantMessageEvent.delta };
    }
    if (event.assistantMessageEvent.type === "thinking_delta") {
      return { type: "thinking_delta", text: event.assistantMessageEvent.delta };
    }
  }
  if (event.type === "tool_execution_start") {
    return {
      type: "tool_call",
      toolCallId: event.toolCallId,
      toolName: event.toolName,
      args: event.args,
    };
  }
  if (event.type === "tool_execution_update") {
    return {
      type: "tool_update",
      toolCallId: event.toolCallId,
      toolName: event.toolName,
      partialResult: event.partialResult,
    };
  }
  if (event.type === "tool_execution_end") {
    return {
      type: "tool_result",
      toolCallId: event.toolCallId,
      toolName: event.toolName,
      result: event.result,
      isError: event.isError,
    };
  }
  return undefined;
}

export default function piGrokSubagents(pi: ExtensionAPI): void {
  if (process.env.PI_GROK_SUBAGENTS !== "1") return;

  const records = new Map<string, SubagentRecord>();
  const queuedBackground: Array<{ record: SubagentRecord; prompt: string }> = [];
  let runningBackground = 0;
  let nextSequence = 1;

  function emit(
    record: Pick<SubagentRecord, "id" | "childSessionId" | "parentSessionId">,
    kind: BridgeKind,
    payload: Record<string, unknown>,
    replay = false,
  ): void {
    const envelope: BridgeEnvelope = {
      version: 1,
      sequence: nextSequence,
      replay,
      kind,
      parentSessionId: record.parentSessionId,
      subagentId: record.id,
      childSessionId: record.childSessionId,
      payload,
    };
    nextSequence += 1;
    if (replay) {
      // Replay runs during session_start. Keep the existing message shape for
      // the adapter, but do not append replay records to the active session.
      pi.sendMessage(
        {
          customType: BRIDGE_TYPE,
          content: "",
          display: false,
          details: envelope,
        },
        { triggerTurn: false },
      );
      return;
    }
    // Live bridge traffic is session/TUI state, not an LLM message. Using
    // sendMessage() here would steer the parent while it is streaming, even
    // with triggerTurn:false, causing child deltas to create phantom turns.
    pi.appendEntry(BRIDGE_TYPE, envelope);
  }

  function persistedRecord(entry: unknown): PersistedRecord | undefined {
    if (typeof entry !== "object" || entry === null) return undefined;
    const candidate = entry as { type?: unknown; customType?: unknown; data?: unknown };
    if (candidate.type !== "custom" || candidate.customType !== STATE_ENTRY_TYPE) return undefined;
    if (typeof candidate.data !== "object" || candidate.data === null) return undefined;
    const value = candidate.data as Partial<PersistedRecord>;
    if (
      value.version !== 1 ||
      typeof value.id !== "string" ||
      typeof value.childSessionId !== "string" ||
      typeof value.childSessionFile !== "string" ||
      typeof value.parentSessionId !== "string" ||
      typeof value.parentToolCallId !== "string" ||
      typeof value.prompt !== "string" ||
      typeof value.description !== "string" ||
      typeof value.type !== "string" ||
      typeof value.capabilityMode !== "string" ||
      typeof value.modelId !== "string" ||
      typeof value.background !== "boolean" ||
      typeof value.startedAt !== "number" ||
      typeof value.status !== "string" ||
      typeof value.turnCount !== "number" ||
      typeof value.toolCallCount !== "number" ||
      typeof value.tokensUsed !== "number"
    ) {
      return undefined;
    }
    return value as PersistedRecord;
  }

  function replayChildTranscript(snapshot: PersistedRecord): void {
    let entries: readonly unknown[];
    try {
      entries = SessionManager.open(snapshot.childSessionFile).getBranch();
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      emit(snapshot, "finished", {
        status: "failed",
        durationMs: 0,
        turns: snapshot.turnCount,
        toolCalls: snapshot.toolCallCount,
        tokensUsed: snapshot.tokensUsed,
        error: `child transcript is unavailable: ${message}`,
      }, true);
      return;
    }

    for (const entry of entries) {
      const message = (entry as { type?: unknown; message?: unknown }).message;
      if (typeof message !== "object" || message === null) continue;
      const childMessage = message as {
        role?: unknown;
        content?: unknown;
        toolCallId?: unknown;
        toolName?: unknown;
        isError?: unknown;
      };
      if (childMessage.role === "user") {
        emit(snapshot, "child_update", {
          update: { type: "user", text: textFromContent(childMessage.content) },
        }, true);
        continue;
      }
      if (childMessage.role === "assistant" && Array.isArray(childMessage.content)) {
        for (const block of childMessage.content) {
          if (typeof block !== "object" || block === null) continue;
          const value = block as { type?: unknown; text?: unknown; thinking?: unknown; id?: unknown; name?: unknown; arguments?: unknown };
          if (value.type === "text" && typeof value.text === "string") {
            emit(snapshot, "child_update", { update: { type: "assistant_delta", text: value.text } }, true);
          } else if (value.type === "thinking" && typeof value.thinking === "string") {
            emit(snapshot, "child_update", { update: { type: "thinking_delta", text: value.thinking } }, true);
          } else if (value.type === "toolCall" && typeof value.id === "string" && typeof value.name === "string") {
            emit(snapshot, "child_update", {
              update: { type: "tool_call", toolCallId: value.id, toolName: value.name, args: value.arguments ?? {} },
            }, true);
          }
        }
        continue;
      }
      if (
        childMessage.role === "toolResult" &&
        typeof childMessage.toolCallId === "string" &&
        typeof childMessage.toolName === "string"
      ) {
        emit(snapshot, "child_update", {
          update: {
            type: "tool_result",
            toolCallId: childMessage.toolCallId,
            toolName: childMessage.toolName,
            result: { content: childMessage.content },
            isError: childMessage.isError === true,
          },
        }, true);
      }
    }
  }

  function replayPersistedRecords(ctx: ExtensionContext): void {
    const latest = new Map<string, PersistedRecord>();
    for (const entry of ctx.sessionManager.getEntries()) {
      const snapshot = persistedRecord(entry);
      if (snapshot) latest.set(snapshot.id, snapshot);
    }
    const parentSessionId = ctx.sessionManager.getSessionId();
    for (const snapshot of [...latest.values()].sort((left, right) => left.startedAt - right.startedAt)) {
      if (snapshot.parentSessionId !== parentSessionId) continue;
      emit(snapshot, "spawned", {
        parentToolCallId: snapshot.parentToolCallId,
        description: snapshot.description,
        subagentType: snapshot.type,
        background: snapshot.background,
        capabilityMode: snapshot.capabilityMode,
        model: snapshot.modelId,
        prompt: snapshot.prompt,
      }, true);
      replayChildTranscript(snapshot);
      const status = snapshot.status === "running" ? "cancelled" : snapshot.status;
      emit(snapshot, "finished", {
        status,
        durationMs: Math.max(0, Date.now() - snapshot.startedAt),
        turns: snapshot.turnCount,
        toolCalls: snapshot.toolCallCount,
        tokensUsed: snapshot.tokensUsed,
        error: snapshot.status === "running" ? "Pi host restarted before child completion" : undefined,
      }, true);
    }
  }

  function persist(record: SubagentRecord, status: PersistedRecord["status"]): void {
    const snapshot: PersistedRecord = {
      version: 1,
      id: record.id,
      childSessionId: record.childSessionId,
      childSessionFile: record.childSessionFile,
      parentSessionId: record.parentSessionId,
      parentToolCallId: record.parentToolCallId,
      prompt: record.prompt,
      description: record.description,
      type: record.type,
      capabilityMode: record.capabilityMode,
      modelId: record.modelId,
      background: record.background,
      startedAt: record.startedAt,
      status,
      turnCount: record.turnCount,
      toolCallCount: record.toolCallCount,
      tokensUsed: record.tokensUsed,
    };
    pi.appendEntry(STATE_ENTRY_TYPE, snapshot);
  }

  function emitProgress(record: SubagentRecord): void {
    if (record.finished) return;
    emit(record, "progress", {
      durationMs: Date.now() - record.startedAt,
      turnCount: record.turnCount,
      toolCallCount: record.toolCallCount,
      toolsUsed: [...record.toolsUsed],
      errorCount: record.errorCount,
      tokensUsed: record.tokensUsed,
    });
  }

  function finish(record: SubagentRecord, status: "completed" | "failed" | "cancelled", error?: string): void {
    if (record.finished) return;
    record.finished = true;
    clearInterval(record.progressTimer);
    record.removeAbortListener();
    record.unsubscribe();
    persist(record, status);
    emit(record, "finished", {
      status,
      durationMs: Date.now() - record.startedAt,
      turns: record.turnCount,
      toolCalls: record.toolCallCount,
      tokensUsed: record.tokensUsed,
      error,
      output: lastAssistantText(record.session),
    });
  }

  async function createRecord(
    toolCallId: string,
    params: {
      prompt: string;
      description: string;
      subagent_type: string;
      background?: boolean;
      capability_mode?: string;
    },
    signal: AbortSignal | undefined,
    ctx: ExtensionContext,
  ): Promise<SubagentRecord> {
    const prompt = requireText(params.prompt, "prompt");
    const description = requireText(params.description, "description");
    const profile = resolveProfile(params.subagent_type || "general-purpose", params.capability_mode);
    const model = ctx.model;
    if (!model) throw new Error("no Pi model is selected");

    const agentDir = getAgentDir();
    const settingsManager = SettingsManager.create(ctx.cwd, agentDir);
    const resourceLoader = new DefaultResourceLoader({
      cwd: ctx.cwd,
      agentDir,
      noExtensions: true,
      noSkills: true,
      noPromptTemplates: true,
      noThemes: true,
      noContextFiles: true,
      systemPromptOverride: () => profile.systemPrompt,
      appendSystemPromptOverride: () => [],
    });
    await resourceLoader.reload();

    const { session } = await createAgentSession({
      cwd: ctx.cwd,
      agentDir,
      sessionManager: SessionManager.create(ctx.cwd),
      settingsManager,
      modelRegistry: ctx.modelRegistry,
      model,
      tools: [...CAPABILITY_TOOLS[profile.capabilityMode]],
      resourceLoader,
    });
    await session.bindExtensions({});

    const childSessionFile = session.sessionFile;
    if (!childSessionFile) throw new Error("child session persistence is unavailable");
    const parentSessionId = ctx.sessionManager.getSessionId();
    const id = randomUUID();
    const record = {
      id,
      childSessionId: session.sessionId,
      childSessionFile,
      parentSessionId,
      parentToolCallId: toolCallId,
      prompt,
      description,
      type: profile.type,
      capabilityMode: profile.capabilityMode,
      modelId: model.id,
      background: params.background === true,
      startedAt: Date.now(),
      session,
      turnCount: 0,
      toolCallCount: 0,
      toolsUsed: new Set<string>(),
      errorCount: 0,
      tokensUsed: 0,
      finished: false,
      cancelRequested: false,
    } as Omit<SubagentRecord, "progressTimer" | "removeAbortListener" | "unsubscribe">;

    const unsubscribe = session.subscribe((event) => {
      if (event.type === "turn_end") record.turnCount += 1;
      if (event.type === "tool_execution_start") {
        record.toolCallCount += 1;
        record.toolsUsed.add(event.toolName);
      }
      if (event.type === "tool_execution_end" && event.isError) record.errorCount += 1;
      if (event.type === "message_end" && event.message.role === "assistant") {
        record.tokensUsed += extractUsage(event.message);
      }
      const update = childUpdate(event);
      if (update) emit(record, "child_update", { update });
    });

    const onAbort = () => {
      record.cancelRequested = true;
      session.abort();
    };
    signal?.addEventListener("abort", onAbort, { once: true });
    const removeAbortListener = () => signal?.removeEventListener("abort", onAbort);
    const progressTimer = setInterval(() => emitProgress(record as SubagentRecord), PROGRESS_INTERVAL_MS);
    const completeRecord: SubagentRecord = { ...record, progressTimer, removeAbortListener, unsubscribe };

    records.set(id, completeRecord);
    persist(completeRecord, "running");
    emit(completeRecord, "spawned", {
      parentToolCallId: toolCallId,
      description,
      subagentType: completeRecord.type,
      background: completeRecord.background,
      capabilityMode: profile.capabilityMode,
      model: model.id,
      prompt,
    });
    emit(completeRecord, "child_update", { update: { type: "user", text: prompt } });
    return completeRecord;
  }

  async function run(record: SubagentRecord, prompt: string): Promise<string> {
    try {
      await record.session.prompt(prompt);
      const output = lastAssistantText(record.session);
      finish(record, "completed");
      return output;
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      finish(record, record.cancelRequested ? "cancelled" : "failed", message);
      throw error;
    }
  }

  pi.on("session_start", (_event, ctx) => {
    replayPersistedRecords(ctx);
  });

  function scheduleBackground(record: SubagentRecord, prompt: string): void {
    if (runningBackground >= MAX_BACKGROUND_CONCURRENCY) {
      queuedBackground.push({ record, prompt });
      return;
    }
    runningBackground += 1;
    void run(record, prompt)
      .catch(() => undefined)
      .finally(() => {
        runningBackground -= 1;
        const next = queuedBackground.shift();
        if (next) scheduleBackground(next.record, next.prompt);
      });
  }

  pi.registerTool({
    name: "spawn_subagent",
    label: "Spawn Subagent",
    description: "Launch an autonomous Pi child session shown in Grok's native subagent UI.",
    parameters: Type.Object({
      prompt: Type.String({ description: "Self-contained task for the child agent." }),
      description: Type.String({ description: "Short 3-5 word task label." }),
      subagent_type: Type.Optional(Type.String({ description: "Agent type label for the native Grok view." })),
      background: Type.Optional(Type.Boolean({ description: "Run asynchronously and return the child ID immediately." })),
      capability_mode: Type.Optional(
        Type.String({ description: "One of read-only, read-write, execute, or all." }),
      ),
    }),
    async execute(toolCallId, params, signal, _onUpdate, ctx) {
      const record = await createRecord(toolCallId, params, signal, ctx);
      if (record.background) {
        scheduleBackground(record, params.prompt);
        return {
          content: [{ type: "text", text: `Started subagent ${record.id}.` }],
          details: { subagentId: record.id, childSessionId: record.childSessionId, background: true },
        };
      }
      const output = await run(record, params.prompt);
      return {
        content: [{ type: "text", text: output || "Subagent completed without text output." }],
        details: { subagentId: record.id, childSessionId: record.childSessionId, background: false },
      };
    },
  });

  pi.registerTool({
    name: "get_command_or_subagent_output",
    label: "Get Subagent Output",
    description: "Return the latest result from a Pi-Grok subagent.",
    parameters: Type.Object({ subagent_id: Type.String() }),
    async execute(_toolCallId, params) {
      const record = records.get(requireText(params.subagent_id, "subagent_id"));
      if (!record) throw new Error(`unknown subagent: ${params.subagent_id}`);
      return {
        content: [{ type: "text", text: lastAssistantText(record.session) || "Subagent is still running." }],
        details: { subagentId: record.id, finished: record.finished },
      };
    },
  });

  pi.registerCommand("__pi_grok_subagent_cancel", {
    description: "Internal Pi-Grok bridge command: cancel a subagent",
    handler: async (args) => {
      const id = requireText(args, "subagent id");
      const record = records.get(id);
      if (!record) throw new Error(`unknown subagent: ${id}`);
      if (!record.finished) {
        record.cancelRequested = true;
        record.session.abort();
      }
    },
  });

  pi.on("session_shutdown", () => {
    for (const record of records.values()) {
      if (!record.finished) {
        record.cancelRequested = true;
        record.session.abort();
      }
    }
  });
}
