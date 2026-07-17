use crate::{
    model::{
        PiCommand, PiHistoryItem, PiModel, PiSessionSwitch, PiSessionTree, PiState, PiToolContent,
        extract_delta, json_text, number, parse_commands, parse_messages, parse_models,
        parse_session_switch, parse_session_tree, parse_state, scan_local_sessions,
        scan_local_sessions_for_cwd, string,
    },
    pi_rpc::PiRpc,
    queue_bridge::{QueueLane, QueueMirror, queue_changed_params, string_list},
    todo_bridge::plan_update_for_tool,
};
use agent_client_protocol as acp;
use anyhow::{Result, anyhow};
use indexmap::IndexMap;
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    rc::Rc,
    time::Duration,
};
use tokio::sync::{mpsc, oneshot};
use xai_acp_lib::{AcpClientMessage, acp_send};

#[derive(Debug, Clone)]
pub struct PiBootstrap {
    state: PiState,
    models: Vec<PiModel>,
    commands: Vec<PiCommand>,
}

impl PiBootstrap {
    pub async fn load(rpc: &PiRpc) -> Result<Self> {
        let state = parse_state(&rpc.request(json!({ "type": "get_state" })).await?);
        let mut models = parse_models(
            &rpc.request(json!({ "type": "get_available_models" }))
                .await?,
        );
        if let Some(current) = state.model.clone()
            && !models
                .iter()
                .any(|model| model.provider == current.provider && model.id == current.id)
        {
            models.push(current);
        }
        let commands = parse_commands(&rpc.request(json!({ "type": "get_commands" })).await?);
        Ok(Self {
            state,
            models,
            commands,
        })
    }

    pub fn acp_models(&self) -> Option<acp::SessionModelState> {
        let (available, current) = build_model_catalog(
            &self.models,
            self.state.model.as_ref(),
            &self.state.thinking_level,
        );
        let current = current.or_else(|| available.first().map(|(id, _)| id.clone()))?;
        Some(acp::SessionModelState::new(
            current,
            available.into_values().collect(),
        ))
    }

    pub fn acp_commands(&self) -> Vec<acp::AvailableCommand> {
        command_catalog(&self.commands)
    }

    /// Pi session identifier used to seed the native Grok session surface.
    pub fn session_id(&self) -> &str {
        &self.state.session_id
    }

    /// Optional Pi session title used for Grok's terminal title and header.
    pub fn session_title(&self) -> Option<&str> {
        self.state.session_name.as_deref()
    }
}

struct ActivePrompt {
    id: u64,
    /// Pager-minted id (`_meta.promptId`); echoed on PromptResponse so non-running
    /// mid-turn completions are discarded instead of emitting phantom turns.
    client_prompt_id: Option<String>,
    completion: oneshot::Sender<PromptCompletion>,
    agent_started: bool,
    cancelled: bool,
}

struct PromptCompletion {
    reason: acp::StopReason,
    client_prompt_id: Option<String>,
}

#[derive(Debug, Clone, Copy, Default)]
struct StreamSeen {
    text: bool,
    thought: bool,
}

struct AdapterState {
    bootstrap: PiBootstrap,
    acp_session_id: String,
    model_map: HashMap<String, PiModel>,
    active_prompts: Vec<ActivePrompt>,
    next_prompt_id: u64,
    bash_running: bool,
    live_assistant: Option<StreamSeen>,
    session_dir: PathBuf,
    session_paths: HashMap<String, PathBuf>,
    /// Pi tool args keyed by toolCallId. End events may omit args; the pager
    /// still needs path/command when projecting native Read/Execute cards.
    tool_args: HashMap<String, Value>,
    /// Latest Pi context-window usage (tokens used). Stamped on ACP session
    /// updates as `_meta.totalTokens` so Grok's native context bar can render.
    last_context_tokens: Option<u64>,
    /// Pi steering / follow-up queue mirrored as Grok `x.ai/queue/changed`.
    queue_mirror: QueueMirror,
}

#[derive(Clone)]
pub struct PiAgent {
    rpc: PiRpc,
    client_tx: mpsc::UnboundedSender<AcpClientMessage>,
    state: Rc<RefCell<AdapterState>>,
}

impl PiAgent {
    pub fn new(
        rpc: PiRpc,
        client_tx: mpsc::UnboundedSender<AcpClientMessage>,
        bootstrap: PiBootstrap,
        session_dir: PathBuf,
    ) -> Self {
        let acp_session_id = bootstrap.state.session_id.clone();
        let model_map = bootstrap
            .models
            .iter()
            .cloned()
            .map(|model| (model_key(&model), model))
            .collect();
        Self {
            rpc,
            client_tx,
            state: Rc::new(RefCell::new(AdapterState {
                bootstrap,
                acp_session_id,
                model_map,
                active_prompts: Vec::new(),
                next_prompt_id: 1,
                bash_running: false,
                live_assistant: None,
                session_dir,
                session_paths: HashMap::new(),
                tool_args: HashMap::new(),
                last_context_tokens: None,
                queue_mirror: QueueMirror::default(),
            })),
        }
    }

    pub async fn run_events(self: Rc<Self>, mut events: mpsc::UnboundedReceiver<Value>) {
        while let Some(event) = events.recv().await {
            if let Err(error) = self.handle_event(event).await {
                tracing::warn!(%error, "failed to adapt Pi event into Grok ACP");
                self.send_ui_notification(&format!("Pi adapter: {error}"), Some("warning"))
                    .await;
            }
        }
        self.finish_prompts(acp::StopReason::Cancelled);
    }

    pub async fn refresh(&self) -> Result<PiBootstrap> {
        let bootstrap = PiBootstrap::load(&self.rpc).await?;
        self.replace_bootstrap(bootstrap.clone());
        Ok(bootstrap)
    }

    /// Publish Pi's local session catalog for Grok's existing native picker.
    ///
    /// Pi keeps ownership of the JSONL format and of switching; this read-only
    /// metadata projection only gives the pager a selectable catalog.
    pub async fn publish_session_catalog(&self, cwd: PathBuf, all: bool) {
        let session_dir = {
            let state = self.state.borrow();
            catalog_session_dir(&state.bootstrap.state, &state.session_dir)
        };
        let sessions = tokio::task::spawn_blocking(move || {
            if all {
                scan_local_sessions(&session_dir)
            } else {
                scan_local_sessions_for_cwd(&session_dir, &cwd)
            }
        })
        .await
        .unwrap_or_default();
        let paths: HashMap<_, _> = sessions
            .iter()
            .map(|session| (session.id.clone(), session.path.clone()))
            .collect();
        self.state.borrow_mut().session_paths.extend(paths);
        self.send_ext_notification(
            "pi/ui/session_catalog",
            json!({
                "scope": if all { "all" } else { "current" },
                "sessions": sessions.into_iter().map(|session| json!({
                    "id": session.id,
                    "summary": session.name.unwrap_or(session.first_message),
                    "cwd": session.cwd,
                    "createdAt": session.created_at,
                    "updatedAt": session.modified_at,
                    "messageCount": session.message_count,
                })).collect::<Vec<_>>(),
            }),
        )
        .await;
    }

    /// Request Pi to replace its active session. The adapter publishes the new
    /// session identity only after Pi accepts the switch and its replacement
    /// state can be loaded successfully.
    pub async fn switch_session(&self, session_path: &Path) -> Result<PiSessionSwitch> {
        let response = self
            .rpc
            .request(json!({
                "type": "switch_session",
                "sessionPath": session_path,
            }))
            .await?;
        let result = parse_session_switch(&response);
        if result.cancelled {
            return Ok(result);
        }
        let bootstrap = PiBootstrap::load(&self.rpc).await?;
        self.replace_bootstrap(bootstrap);
        Ok(result)
    }

    /// Read-only projection of Pi's current entry tree (`get_tree`).
    ///
    /// Parse + flatten + drop of the nested Value happen on a large-stack
    /// worker: long sessions produce trees deep enough to overflow the default
    /// Tokio worker stack even after serde_json recursion limits are disabled.
    async fn fetch_session_tree(&self) -> Result<PiSessionTree> {
        let data = self.rpc.request(json!({ "type": "get_tree" })).await?;
        let tree = tokio::task::spawn_blocking(move || {
            crate::pi_rpc::with_large_stack(move || {
                let tree = parse_session_tree(&data);
                drop(data);
                tree
            })
        })
        .await
        .map_err(|error| anyhow!("Pi get_tree worker failed: {error}"))?;
        Ok(tree)
    }

    /// Run a hidden bridge extension command (`/__pi_*`) and wait for the
    /// non-agent preflight probe to complete.
    async fn run_bridge_command(&self, command: &str, args: &str) -> Result<(), acp::Error> {
        let message = if args.trim().is_empty() {
            format!("/{command}")
        } else {
            format!("/{command} {args}")
        };
        let (completion_tx, completion_rx) = oneshot::channel();
        let prompt_id = {
            let mut state = self.state.borrow_mut();
            let prompt_id = state.next_prompt_id;
            state.next_prompt_id = state.next_prompt_id.wrapping_add(1).max(1);
            state.active_prompts.push(ActivePrompt {
                id: prompt_id,
                client_prompt_id: None,
                completion: completion_tx,
                agent_started: false,
                cancelled: false,
            });
            prompt_id
        };
        let request = json!({ "type": "prompt", "message": message });
        if let Err(error) = self.rpc.request(request).await {
            self.remove_prompt(prompt_id);
            return Err(acp_internal(error));
        }
        let probe = self.clone();
        tokio::task::spawn_local(async move {
            probe.probe_prompt_without_agent().await;
        });
        let _ = completion_rx.await;
        Ok(())
    }

    /// Navigate Pi's leaf via the injected `__pi_navigate_tree` extension
    /// command (official `ctx.navigateTree`).
    async fn navigate_session_tree(
        &self,
        entry_id: &str,
        summarize: bool,
        custom_instructions: Option<&str>,
    ) -> Result<Value, acp::Error> {
        let entry_id = entry_id.trim();
        if entry_id.is_empty() {
            return Err(acp::Error::invalid_params().data("tree entry id is empty"));
        }
        let mut args = entry_id.to_string();
        if summarize {
            args.push_str(" --summarize");
        }
        if let Some(instructions) = custom_instructions.map(str::trim).filter(|s| !s.is_empty()) {
            // Extension parses --instructions <rest-of-line>.
            args.push_str(" --instructions ");
            args.push_str(instructions);
        }
        self.run_bridge_command(NAVIGATE_TREE_COMMAND, &args).await?;

        // Leaf moved inside the same session file. Refresh adapter state only;
        // the pager issues session/load to clear scrollback and re-replay.
        let bootstrap = self.refresh().await.map_err(acp_internal)?;
        let tree = self.fetch_session_tree().await.map_err(acp_internal)?;
        Ok(json!({
            "sessionId": bootstrap.state.session_id,
            "leafId": tree.leaf_id,
            "cancelled": false,
        }))
    }

    async fn set_session_tree_label(
        &self,
        entry_id: &str,
        label: Option<&str>,
    ) -> Result<Value, acp::Error> {
        let entry_id = entry_id.trim();
        if entry_id.is_empty() {
            return Err(acp::Error::invalid_params().data("tree entry id is empty"));
        }
        let args = match label.map(str::trim).filter(|s| !s.is_empty()) {
            Some(text) => format!("{entry_id} {text}"),
            None => format!("{entry_id} --clear"),
        };
        self.run_bridge_command(LABEL_TREE_COMMAND, &args).await?;
        let tree = self.fetch_session_tree().await.map_err(acp_internal)?;
        Ok(json!({
            "leafId": tree.leaf_id,
            "entryId": entry_id,
            "label": label,
        }))
    }

    fn replace_bootstrap(&self, bootstrap: PiBootstrap) {
        let mut state = self.state.borrow_mut();
        state.acp_session_id = bootstrap.state.session_id.clone();
        state.model_map = bootstrap
            .models
            .iter()
            .cloned()
            .map(|model| (model_key(&model), model))
            .collect();
        state.bootstrap = bootstrap;
    }

    fn session_id(&self) -> acp::SessionId {
        acp::SessionId::new(self.state.borrow().acp_session_id.clone())
    }

    async fn send_update(&self, update: acp::SessionUpdate) {
        let mut notification = acp::SessionNotification::new(self.session_id(), update);
        if let Some(tokens) = self.state.borrow().last_context_tokens {
            let mut meta = acp::Meta::new();
            meta.insert("totalTokens".into(), json!(tokens));
            notification = notification.meta(Some(meta));
        }
        if let Err(error) = acp_send(notification, &self.client_tx).await {
            tracing::debug!(%error, "Grok pager closed while sending Pi session update");
        }
    }

    /// Pull Pi's current context-window estimate and push it to the pager bar.
    ///
    /// Grok's context bar needs `_meta.totalTokens` on session updates plus the
    /// model window (`totalContextTokens`). Pi owns the estimate via
    /// `get_session_stats.contextUsage`.
    async fn refresh_context_usage(&self) {
        let data = match self
            .rpc
            .request(json!({ "type": "get_session_stats" }))
            .await
        {
            Ok(data) => data,
            Err(error) => {
                tracing::debug!(%error, "failed to fetch Pi session stats for context bar");
                return;
            }
        };
        let Some(tokens) = context_tokens_from_stats(&data) else {
            return;
        };
        let changed = {
            let mut state = self.state.borrow_mut();
            if state.last_context_tokens == Some(tokens) {
                false
            } else {
                state.last_context_tokens = Some(tokens);
                true
            }
        };
        if changed {
            // Empty chunk is a no-op in the tracker but still carries
            // `_meta.totalTokens` for confirm_context_used.
            self.send_update(acp::SessionUpdate::AgentMessageChunk(text_chunk("")))
                .await;
        }
    }

    fn note_context_tokens(&self, tokens: u64) {
        if tokens == 0 {
            return;
        }
        self.state.borrow_mut().last_context_tokens = Some(tokens);
    }

    async fn send_ext_notification(&self, method: &str, params: Value) {
        let Ok(raw) = serde_json::value::to_raw_value(&params) else {
            return;
        };
        let notification = acp::ExtNotification::new(method, raw.into());
        if let Err(error) = acp_send(notification, &self.client_tx).await {
            tracing::debug!(%error, method, "Grok pager closed while sending Pi UI notification");
        }
    }

    async fn send_ui_notification(&self, message: &str, kind: Option<&str>) {
        self.send_ext_notification(
            "pi/ui/notify",
            json!({ "message": message, "notifyType": kind }),
        )
        .await;
    }

    /// Build Grok-native `x.ai/session/info` from Pi session stats.
    ///
    /// Mirrors the intent of the pi-context extension (`getContextUsage` +
    /// message-length estimate) but returns the ACP envelope that the pager's
    /// `ContextInfoBlock` already knows how to render — no second UI.
    async fn handle_session_info(&self) -> Result<acp::ExtResponse, acp::Error> {
        let stats = self
            .rpc
            .request(json!({ "type": "get_session_stats" }))
            .await
            .map_err(acp_internal)?;
        // Best-effort breakdown. Prefer live messages; fall back to branch entries
        // (session file shape) so empty/failed get_messages still yields a bar.
        let messages = match self.rpc.request(json!({ "type": "get_messages" })).await {
            Ok(value) if value.get("messages").and_then(Value::as_array).is_some_and(|m| !m.is_empty())
                || value.as_array().is_some_and(|m| !m.is_empty()) =>
            {
                Some(value)
            }
            _ => self
                .rpc
                .request(json!({ "type": "get_entries" }))
                .await
                .ok()
                .map(entries_to_messages_value),
        };
        let (session_id, model, cached_tokens) = {
            let state = self.state.borrow();
            (
                state.acp_session_id.clone(),
                state.bootstrap.state.model.clone(),
                state.last_context_tokens,
            )
        };
        let cwd = std::env::current_dir()
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_default();
        let response = build_session_info_response(
            &stats,
            messages.as_ref(),
            &session_id,
            &cwd,
            model.as_ref(),
            cached_tokens,
        );
        if let Some(used) = response
            .get("context")
            .and_then(|context| context.get("used"))
            .and_then(Value::as_u64)
            .filter(|&tokens| tokens > 0)
        {
            self.note_context_tokens(used);
        }
        ext_response(response).map_err(acp_internal)
    }

    async fn send_status(&self, key: &str, text: Option<&str>) {
        self.send_ext_notification(
            "pi/ui/status",
            json!({ "statusKey": key, "statusText": text }),
        )
        .await;
    }

    /// Publish Pi's authoritative queue as Grok's native shared-queue surface.
    async fn publish_queue_snapshot(&self) {
        let (session_id, snapshot) = {
            let state = self.state.borrow();
            (
                state.acp_session_id.clone(),
                state.queue_mirror.snapshot(),
            )
        };
        let steering_text = (snapshot.steering_count > 0)
            .then(|| format!("{} steering", snapshot.steering_count));
        let follow_up_text = (snapshot.follow_up_count > 0)
            .then(|| format!("{} follow-up", snapshot.follow_up_count));
        self.send_status("steering", steering_text.as_deref()).await;
        self.send_status("follow-up", follow_up_text.as_deref())
            .await;
        self.send_ext_notification(
            "x.ai/queue/changed",
            queue_changed_params(&session_id, &snapshot),
        )
        .await;
    }

    async fn apply_pi_queue_update(&self, event: &Value) {
        let steering = string_list(event.get("steering"));
        let follow_up = string_list(event.get("followUp"));
        {
            let mut state = self.state.borrow_mut();
            state
                .queue_mirror
                .apply_queue_update(&steering, &follow_up);
        }
        self.publish_queue_snapshot().await;
    }

    async fn rebroadcast_queue_mirror(&self) {
        self.publish_queue_snapshot().await;
    }

    async fn send_title(&self, title: Option<&str>) {
        let title = title
            .filter(|title| !title.trim().is_empty())
            .unwrap_or("Pi");
        self.send_ext_notification("pi/ui/title", json!({ "title": title }))
            .await;
    }

    async fn send_commands(&self, commands: &[PiCommand]) {
        self.send_update(acp::SessionUpdate::AvailableCommandsUpdate(
            acp::AvailableCommandsUpdate::new(command_catalog(commands)),
        ))
        .await;
    }

    async fn send_models(&self, bootstrap: &PiBootstrap) {
        let Some(models) = bootstrap.acp_models() else {
            return;
        };
        match serde_json::to_value(models) {
            Ok(value) => {
                self.send_ext_notification("x.ai/models/update", value)
                    .await;
            }
            Err(error) => tracing::warn!(%error, "failed to serialize Pi model state"),
        }
    }

    async fn publish_bootstrap(&self, bootstrap: &PiBootstrap) {
        self.send_commands(&bootstrap.commands).await;
        self.send_models(bootstrap).await;
        self.send_title(bootstrap.state.session_name.as_deref())
            .await;
    }

    async fn replay_history(&self) -> Result<()> {
        let data = self.rpc.request(json!({ "type": "get_messages" })).await?;
        for item in parse_messages(&data) {
            self.replay_history_item(item).await;
        }
        Ok(())
    }

    async fn replay_history_item(&self, item: PiHistoryItem) {
        let update = match item {
            PiHistoryItem::UserText(text) => acp::SessionUpdate::UserMessageChunk(text_chunk(text)),
            PiHistoryItem::UserImage { data, mime_type } => {
                acp::SessionUpdate::UserMessageChunk(content_chunk(acp::ContentBlock::Image(
                    acp::ImageContent::new(data, mime_type),
                )))
            }
            PiHistoryItem::AgentText(text) => {
                acp::SessionUpdate::AgentMessageChunk(text_chunk(text))
            }
            PiHistoryItem::AgentThought(text) => {
                acp::SessionUpdate::AgentThoughtChunk(text_chunk(text))
            }
            PiHistoryItem::ToolStart {
                id,
                name,
                arguments,
            } => {
                let arguments = normalize_tool_raw_input(&name, arguments);
                if let Some(args) = arguments.clone() {
                    self.state.borrow_mut().tool_args.insert(id.clone(), args);
                }
                acp::SessionUpdate::ToolCall(
                    acp::ToolCall::new(acp::ToolCallId::new(id), name.clone())
                        .kind(tool_kind(&name))
                        .status(acp::ToolCallStatus::InProgress)
                        .content(edit_diff_content(&name, arguments.as_ref()).unwrap_or_default())
                        .locations(Vec::new())
                        .raw_input(arguments),
                )
            }
            PiHistoryItem::ToolEnd {
                id,
                name,
                content,
                raw_output,
                is_error,
            } => {
                let mut raw = raw_output.unwrap_or(Value::Null);
                // History often stores `details` as raw_output and the body in
                // separate content blocks. Fold text into the payload so bash/read
                // projection still sees stdout / file text.
                if pi_result_text(&raw).is_empty() {
                    let text = content
                        .iter()
                        .filter_map(|item| match item {
                            PiToolContent::Text(text) => Some(text.as_str()),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    if !text.is_empty() {
                        raw = json!({ "content": [{ "type": "text", "text": text }] });
                    }
                }
                let args = self.state.borrow_mut().tool_args.remove(&id);
                let normalized = normalize_tool_raw_output(&name, args.as_ref(), &raw, is_error);
                let mut fields = acp::ToolCallUpdateFields::new()
                    .title(Some(name.clone()))
                    .status(Some(if is_error {
                        acp::ToolCallStatus::Failed
                    } else {
                        acp::ToolCallStatus::Completed
                    }))
                    .raw_output(Some(normalized));
                if tool_kind(&name) != acp::ToolKind::Edit {
                    fields = fields.content(Some(history_tool_content(content)));
                }
                // Project todo-plugin snapshots onto the native TodoPane before
                // the tool card update so resume restores badge state.
                if let Some(plan) = plan_update_for_tool(&name, &raw, is_error) {
                    self.send_update(acp::SessionUpdate::Plan(plan)).await;
                }
                acp::SessionUpdate::ToolCallUpdate(acp::ToolCallUpdate::new(
                    acp::ToolCallId::new(id),
                    fields,
                ))
            }
        };
        self.send_update(update).await;
    }

    async fn handle_event(&self, event: Value) -> Result<()> {
        let event_type = event
            .get("type")
            .or_else(|| event.get("event"))
            .and_then(Value::as_str)
            .unwrap_or_default();
        match event_type {
            "agent_start" => {
                for active in &mut self.state.borrow_mut().active_prompts {
                    active.agent_started = true;
                }
            }
            "agent_settled" => {
                self.refresh_context_usage().await;
                {
                    let mut state = self.state.borrow_mut();
                    state.queue_mirror.clear_running();
                }
                // Idle barrier: drop any stale runningPromptId so the pager can
                // drain local rows without waiting on a ghost running id.
                self.rebroadcast_queue_mirror().await;
                self.finish_prompts(acp::StopReason::EndTurn);
            }
            // `agent_end` is not the Pi idle barrier. Retry, compaction and
            // extension handlers can continue after it; `agent_settled` owns
            // ACP prompt completion.
            "agent_end" | "turn_start" => {}
            "turn_end" => self.refresh_context_usage().await,
            "message_start" => self.handle_message_start(&event),
            "message_update" => self.handle_message_update(&event).await,
            "message_end" => self.handle_message_end(&event).await,
            "tool_execution_start" => self.handle_tool_start(&event).await,
            "tool_execution_update" => self.handle_tool_update(&event).await,
            "tool_execution_end" => self.handle_tool_end(&event).await,
            "extension_ui_request" => self.handle_extension_ui(event).await?,
            "extension_error" => {
                let message = event
                    .get("error")
                    .map(json_text)
                    .filter(|text| !text.is_empty())
                    .or_else(|| string(&event, &["message"]).map(ToOwned::to_owned))
                    .unwrap_or_else(|| "Pi extension error".to_string());
                self.send_ui_notification(&message, Some("error")).await;
            }
            "compaction_start" | "auto_compaction_start" => {
                self.send_status("compaction", Some("Compacting context…"))
                    .await;
            }
            "compaction_end" | "auto_compaction_end" => {
                self.send_status("compaction", None).await;
                if let Some(error) = string(&event, &["errorMessage", "error"])
                    && !error.is_empty()
                {
                    self.send_ui_notification(error, Some("error")).await;
                }
            }
            "auto_retry_start" => {
                let attempt = event.get("attempt").and_then(Value::as_u64).unwrap_or(0);
                let maximum = event
                    .get("maxAttempts")
                    .and_then(Value::as_u64)
                    .unwrap_or(0);
                let delay_ms = event.get("delayMs").and_then(Value::as_u64).unwrap_or(0);
                let error =
                    string(&event, &["errorMessage", "message", "reason"]).unwrap_or_default();
                let mut text = if maximum > 0 {
                    format!("Retrying {attempt}/{maximum}")
                } else {
                    "Retrying".to_string()
                };
                if delay_ms > 0 {
                    text.push_str(&format!(" in {:.1}s", delay_ms as f64 / 1000.0));
                }
                if !error.is_empty() {
                    text.push_str(": ");
                    text.push_str(error);
                }
                self.send_status("retry", Some(&text)).await;
            }
            "auto_retry_end" => {
                self.send_status("retry", None).await;
                if event.get("success").and_then(Value::as_bool) == Some(false)
                    && let Some(error) = string(&event, &["finalError", "errorMessage"])
                {
                    self.send_ui_notification(error, Some("error")).await;
                }
            }
            "queue_update" => {
                // Pi emits full text arrays; mirror them into the native queue
                // pane so optimistic server rows confirm and dequeue.
                self.apply_pi_queue_update(&event).await;
            }
            "thinking_level_changed" | "session_info_changed" => match self.refresh().await {
                Ok(bootstrap) => self.publish_bootstrap(&bootstrap).await,
                Err(error) => {
                    tracing::warn!(%error, "failed to refresh Pi state after state change");
                }
            },
            "adapter_diagnostic" => {
                if let Some(message) = string(&event, &["message"]) {
                    self.send_ui_notification(message, Some("warning")).await;
                }
            }
            "adapter_process_exit" => {
                let message = string(&event, &["message"]).unwrap_or("Pi RPC process exited");
                self.send_ui_notification(message, Some("error")).await;
                self.finish_prompts(acp::StopReason::Cancelled);
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_message_start(&self, event: &Value) {
        if message_role(event) == Some("assistant") {
            self.state.borrow_mut().live_assistant = Some(StreamSeen::default());
        }
    }

    async fn handle_message_update(&self, event: &Value) {
        let (text, thought) = extract_delta(event);
        {
            let mut state = self.state.borrow_mut();
            let seen = state.live_assistant.get_or_insert_with(StreamSeen::default);
            seen.text |= !text.is_empty();
            seen.thought |= !thought.is_empty();
        }
        if !thought.is_empty() {
            self.send_update(acp::SessionUpdate::AgentThoughtChunk(text_chunk(thought)))
                .await;
        }
        if !text.is_empty() {
            self.send_update(acp::SessionUpdate::AgentMessageChunk(text_chunk(text)))
                .await;
        }
    }

    async fn handle_message_end(&self, event: &Value) {
        if message_role(event) != Some("assistant") {
            return;
        }
        let seen = self
            .state
            .borrow_mut()
            .live_assistant
            .take()
            .unwrap_or_default();
        let Some(message) = event.get("message") else {
            return;
        };
        // Prefer the assistant message's own usage for a low-latency bar update;
        // agent_settled still revalidates via get_session_stats.
        if let Some(tokens) = message
            .get("usage")
            .and_then(context_tokens_from_usage)
        {
            self.note_context_tokens(tokens);
        }
        let terminal_error = string(message, &["errorMessage", "error_message"])
            .filter(|error| !error.is_empty())
            .map(ToOwned::to_owned);
        for item in parse_messages(&json!({ "messages": [message] })) {
            match item {
                PiHistoryItem::AgentThought(text) if !seen.thought => {
                    self.send_update(acp::SessionUpdate::AgentThoughtChunk(text_chunk(text)))
                        .await;
                }
                PiHistoryItem::AgentText(text) if !seen.text => {
                    self.send_update(acp::SessionUpdate::AgentMessageChunk(text_chunk(text)))
                        .await;
                }
                _ => {}
            }
        }
        if seen.text
            && let Some(error) = terminal_error
        {
            self.send_ui_notification(&error, Some("error")).await;
        }
    }

    fn finish_prompts(&self, requested_reason: acp::StopReason) {
        let active_prompts = std::mem::take(&mut self.state.borrow_mut().active_prompts);
        for active in active_prompts {
            let reason = if active.cancelled {
                acp::StopReason::Cancelled
            } else {
                requested_reason.clone()
            };
            let _ = active.completion.send(PromptCompletion {
                reason,
                client_prompt_id: active.client_prompt_id,
            });
        }
    }

    fn remove_prompt(&self, id: u64) {
        let mut state = self.state.borrow_mut();
        if let Some(index) = state
            .active_prompts
            .iter()
            .position(|active| active.id == id)
        {
            state.active_prompts.remove(index);
        }
    }

    fn allocate_operation_id(&self) -> u64 {
        let mut state = self.state.borrow_mut();
        let id = state.next_prompt_id;
        state.next_prompt_id = state.next_prompt_id.wrapping_add(1).max(1);
        id
    }

    async fn probe_prompt_without_agent(&self) {
        // Pi acknowledges prompt preflight before its asynchronous event stream.
        // A short grace period lets a normal agent_start arrive. Extension
        // commands that complete without an agent run otherwise have no
        // agent_settled event, so get_state is the authoritative fallback.
        tokio::time::sleep(Duration::from_millis(40)).await;
        let should_probe = self
            .state
            .borrow()
            .active_prompts
            .iter()
            .any(|active| !active.agent_started);
        if !should_probe {
            return;
        }
        let Ok(value) = self.rpc.request(json!({ "type": "get_state" })).await else {
            return;
        };
        let pi_state = parse_state(&value);
        let should_finish = self
            .state
            .borrow()
            .active_prompts
            .iter()
            .any(|active| !active.agent_started)
            && !pi_state.is_streaming;
        if should_finish {
            self.finish_prompts(acp::StopReason::EndTurn);
        }
    }

    async fn execute_bash(
        &self,
        command: String,
        meta: Option<&acp::Meta>,
    ) -> Result<acp::PromptResponse, acp::Error> {
        let serial = self.allocate_operation_id();
        {
            let mut state = self.state.borrow_mut();
            if state.bash_running {
                return Err(
                    acp::Error::invalid_params().data("Pi already has a Bash command running")
                );
            }
            state.bash_running = true;
        }

        let call_id = meta
            .and_then(|meta| meta.get("promptId"))
            .and_then(Value::as_str)
            .filter(|id| !id.trim().is_empty())
            .map(|id| format!("pi-bash:{id}"))
            .unwrap_or_else(|| format!("pi-bash:{serial}"));
        let title = format!("$ {command}");
        self.send_update(acp::SessionUpdate::ToolCall(
            acp::ToolCall::new(acp::ToolCallId::new(call_id.clone()), title.clone())
                .kind(acp::ToolKind::Execute)
                .status(acp::ToolCallStatus::InProgress)
                .content(Vec::new())
                .locations(Vec::new())
                .raw_input(Some(json!({ "command": command.clone() }))),
        ))
        .await;

        let result = self
            .rpc
            .request(json!({ "type": "bash", "command": command }))
            .await;
        self.state.borrow_mut().bash_running = false;

        match result {
            Ok(result) => {
                let cancelled = result
                    .get("cancelled")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                let exit_code = result.get("exitCode").and_then(Value::as_i64);
                let failed = cancelled || exit_code.is_some_and(|code| code != 0);
                let output = format_bash_result(&result);
                let raw_output = bash_tool_output(
                    &command,
                    &result,
                    failed && !cancelled,
                );
                self.send_update(acp::SessionUpdate::ToolCallUpdate(
                    acp::ToolCallUpdate::new(
                        acp::ToolCallId::new(call_id),
                        acp::ToolCallUpdateFields::new()
                            .title(Some(title))
                            .status(Some(if failed {
                                acp::ToolCallStatus::Failed
                            } else {
                                acp::ToolCallStatus::Completed
                            }))
                            .content(Some(vec![acp::ToolCallContent::from(
                                acp::ContentBlock::Text(acp::TextContent::new(output)),
                            )]))
                            .raw_output(Some(raw_output)),
                    ),
                ))
                .await;
                Ok(acp::PromptResponse::new(if cancelled {
                    acp::StopReason::Cancelled
                } else {
                    acp::StopReason::EndTurn
                }))
            }
            Err(error) => {
                self.send_update(acp::SessionUpdate::ToolCallUpdate(
                    acp::ToolCallUpdate::new(
                        acp::ToolCallId::new(call_id),
                        acp::ToolCallUpdateFields::new()
                            .title(Some(title))
                            .status(Some(acp::ToolCallStatus::Failed))
                            .content(Some(vec![acp::ToolCallContent::from(
                                acp::ContentBlock::Text(acp::TextContent::new(error.to_string())),
                            )])),
                    ),
                ))
                .await;
                Err(acp_internal(error))
            }
        }
    }

    async fn handle_tool_start(&self, event: &Value) {
        let id = string(event, &["toolCallId", "id"]).unwrap_or("pi-tool");
        let name = string(event, &["toolName", "name"]).unwrap_or("Tool");
        let args = normalize_tool_raw_input(
            name,
            event.get("args").or_else(|| event.get("input")).cloned(),
        );
        if let Some(args) = args.clone() {
            self.state
                .borrow_mut()
                .tool_args
                .insert(id.to_string(), args);
        }
        let content = edit_diff_content(name, args.as_ref()).unwrap_or_default();
        self.send_update(acp::SessionUpdate::ToolCall(
            acp::ToolCall::new(acp::ToolCallId::new(id.to_string()), name.to_string())
                .kind(tool_kind(name))
                .status(acp::ToolCallStatus::InProgress)
                .content(content)
                .locations(Vec::new())
                .raw_input(args),
        ))
        .await;
    }

    async fn handle_tool_update(&self, event: &Value) {
        let id = string(event, &["toolCallId", "id"]).unwrap_or("pi-tool");
        let output = event
            .get("partialResult")
            .or_else(|| event.get("result"))
            .cloned()
            .unwrap_or(Value::Null);
        let name = string(event, &["toolName", "name"]).unwrap_or_default();
        let args = normalize_tool_raw_input(
            name,
            event
                .get("args")
                .or_else(|| event.get("input"))
                .cloned()
                .or_else(|| self.state.borrow().tool_args.get(id).cloned()),
        );
        if let Some(args) = args.clone() {
            self.state
                .borrow_mut()
                .tool_args
                .insert(id.to_string(), args);
        }
        let raw_output = normalize_tool_raw_output(name, args.as_ref(), &output, false);
        let mut fields = acp::ToolCallUpdateFields::new()
            .status(Some(acp::ToolCallStatus::InProgress))
            .raw_output(Some(raw_output));
        if tool_kind(name) != acp::ToolKind::Edit {
            fields = fields.content(Some(tool_content(&output)));
        }
        self.send_update(acp::SessionUpdate::ToolCallUpdate(
            acp::ToolCallUpdate::new(acp::ToolCallId::new(id.to_string()), fields),
        ))
        .await;
    }

    async fn handle_tool_end(&self, event: &Value) {
        let id = string(event, &["toolCallId", "id"]).unwrap_or("pi-tool");
        let output = event.get("result").cloned().unwrap_or(Value::Null);
        let is_error = event.get("isError").and_then(Value::as_bool) == Some(true);
        let status = if is_error {
            acp::ToolCallStatus::Failed
        } else {
            acp::ToolCallStatus::Completed
        };
        let name = string(event, &["toolName", "name"]).unwrap_or_default();
        let args = normalize_tool_raw_input(
            name,
            event
                .get("args")
                .or_else(|| event.get("input"))
                .cloned()
                .or_else(|| self.state.borrow_mut().tool_args.remove(id)),
        );
        let raw_output = normalize_tool_raw_output(name, args.as_ref(), &output, is_error);
        let mut fields = acp::ToolCallUpdateFields::new()
            .status(Some(status))
            .raw_output(Some(raw_output));
        if tool_kind(name) != acp::ToolKind::Edit {
            fields = fields.content(Some(tool_content(&output)));
        }
        // Live path: rpiv-todo (and future TodoSource plugins) publish a full
        // task snapshot under tool result details → native TodoPane via Plan.
        if let Some(plan) = plan_update_for_tool(name, &output, is_error) {
            self.send_update(acp::SessionUpdate::Plan(plan)).await;
        }
        self.send_update(acp::SessionUpdate::ToolCallUpdate(
            acp::ToolCallUpdate::new(acp::ToolCallId::new(id.to_string()), fields),
        ))
        .await;
    }

    async fn handle_extension_ui(&self, event: Value) -> Result<()> {
        let method = string(&event, &["method"])
            .unwrap_or_default()
            .to_ascii_lowercase();
        match method.as_str() {
            "notify" => {
                let message = string(&event, &["message"]).unwrap_or_default();
                let kind = string(&event, &["notifyType", "kind"]);
                self.send_ui_notification(message, kind).await;
            }
            "setstatus" => {
                let key = string(&event, &["statusKey", "key"]).unwrap_or("extension");
                let text = string(&event, &["statusText", "text"]);
                self.send_status(key, text.filter(|text| !text.is_empty()))
                    .await;
            }
            "setwidget" => {
                // Grok owns the sticky surface and ordering; the adapter only
                // forwards Pi's semantic widget payload.
                self.send_ext_notification("pi/ui/widget", event).await;
            }
            "settitle" => {
                if let Some(title) = string(&event, &["title"]) {
                    self.send_title(Some(title)).await;
                }
            }
            "set_editor_text" | "seteditortext" => {
                if let Some(text) = string(&event, &["text"]) {
                    self.send_ext_notification("pi/ui/editor_text", json!({ "text": text }))
                        .await;
                }
            }
            "select" | "confirm" | "input" | "editor" => {
                let agent = self.clone();
                tokio::task::spawn_local(async move {
                    if let Err(error) = agent.ask_extension_question(event.clone()).await {
                        tracing::warn!(%error, "Pi extension question failed");
                        agent.respond_extension_cancelled(&event);
                        agent
                            .send_ui_notification(
                                &format!("Pi extension dialog failed: {error}"),
                                Some("error"),
                            )
                            .await;
                    }
                });
            }
            _ => self.respond_extension_cancelled(&event),
        }
        Ok(())
    }

    fn respond_extension_cancelled(&self, event: &Value) {
        if let Some(id) = event.get("id") {
            let _ = self.rpc.notify(json!({
                "type": "extension_ui_response",
                "id": id,
                "cancelled": true,
            }));
        }
    }

    async fn ask_extension_question(&self, event: Value) -> Result<()> {
        let id = event
            .get("id")
            .cloned()
            .ok_or_else(|| anyhow!("Pi extension UI request has no id"))?;
        let method = string(&event, &["method"])
            .unwrap_or_default()
            .to_ascii_lowercase();
        let title = string(&event, &["title", "message"]).unwrap_or("Pi extension");
        let mut options = Vec::new();
        if method == "select" {
            for option in event
                .get("options")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
            {
                options.push(json!({
                    "label": option,
                    "description": "",
                    "preview": null,
                    "id": null,
                }));
            }
        } else if method == "confirm" {
            options.push(json!({ "label": "Yes", "description": "", "preview": null, "id": null }));
            options.push(json!({ "label": "No", "description": "", "preview": null, "id": null }));
        }
        let mut question = if method == "confirm" {
            string(&event, &["message"]).unwrap_or(title).to_string()
        } else {
            title.to_string()
        };
        if method == "input"
            && let Some(placeholder) = string(&event, &["placeholder"])
            && !placeholder.is_empty()
        {
            question.push_str("\n\n");
            question.push_str(placeholder);
        }
        let initial_text = if method == "editor" {
            string(&event, &["prefill"]).unwrap_or_default()
        } else {
            ""
        };
        let tool_call_id = extension_tool_call_id(&id);
        let params = json!({
            "sessionId": self.session_id().0.to_string(),
            "toolCallId": tool_call_id.clone(),
            "questions": [{
                "question": question,
                "options": options,
                "multiSelect": false,
                "id": "pi-question",
            }],
            "mode": "default",
            "initialText": initial_text,
            "noFreeform": method == "select" || method == "confirm",
        });
        let raw = serde_json::value::to_raw_value(&params)?;
        let request = acp::ExtRequest::new("x.ai/ask_user_question", raw.into());
        let response = match extension_dialog_timeout(&event) {
            Some(duration) => {
                match tokio::time::timeout(duration, acp_send(request, &self.client_tx)).await {
                    Ok(response) => response.map_err(|error| anyhow!(error.to_string()))?,
                    Err(_) => {
                        // Pi resolves its own dialog promise on the same timeout but
                        // does not emit a cancellation event. Explicitly retract the
                        // native Grok QuestionView so it cannot remain as a zombie
                        // overlay after the extension has resumed.
                        self.send_ext_notification(
                            "pi/ui/cancel_interaction",
                            json!({ "toolCallId": tool_call_id }),
                        )
                        .await;
                        self.respond_extension_cancelled(&event);
                        return Ok(());
                    }
                }
            }
            None => acp_send(request, &self.client_tx)
                .await
                .map_err(|error| anyhow!(error.to_string()))?,
        };
        let outer: Value = serde_json::from_str(response.0.get())?;
        let result = outer.get("result").unwrap_or(&outer);
        if result.get("outcome").and_then(Value::as_str) == Some("cancelled") {
            self.rpc.notify(json!({
                "type": "extension_ui_response",
                "id": id,
                "cancelled": true,
            }))?;
            return Ok(());
        }
        let answer = extension_answer(&method, result).unwrap_or_default();
        let response = match method.as_str() {
            "confirm" => json!({
                "type": "extension_ui_response",
                "id": id,
                "confirmed": answer.eq_ignore_ascii_case("yes"),
            }),
            _ => json!({
                "type": "extension_ui_response",
                "id": id,
                "value": answer,
            }),
        };
        self.rpc.notify(response)?;
        Ok(())
    }
}

#[async_trait::async_trait(?Send)]
impl acp::Agent for PiAgent {
    async fn initialize(
        &self,
        _arguments: acp::InitializeRequest,
    ) -> Result<acp::InitializeResponse, acp::Error> {
        Ok(acp::InitializeResponse::new(acp::ProtocolVersion::V1)
            .agent_capabilities(
                acp::AgentCapabilities::new()
                    .load_session(true)
                    .prompt_capabilities(
                        acp::PromptCapabilities::new()
                            .image(true)
                            .embedded_context(true),
                    ),
            )
            .agent_info(acp::Implementation::new("pi", env!("CARGO_PKG_VERSION")).title("Pi")))
    }

    async fn authenticate(
        &self,
        _arguments: acp::AuthenticateRequest,
    ) -> Result<acp::AuthenticateResponse, acp::Error> {
        Ok(acp::AuthenticateResponse::new())
    }

    async fn new_session(
        &self,
        _arguments: acp::NewSessionRequest,
    ) -> Result<acp::NewSessionResponse, acp::Error> {
        let result = self
            .rpc
            .request(json!({ "type": "new_session" }))
            .await
            .map_err(acp_internal)?;
        let cancelled = result
            .get("cancelled")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let bootstrap = if cancelled {
            self.state.borrow().bootstrap.clone()
        } else {
            self.refresh().await.map_err(acp_internal)?
        };
        if !cancelled {
            self.state.borrow_mut().acp_session_id = bootstrap.state.session_id.clone();
        }
        self.publish_bootstrap(&bootstrap).await;
        let mut response = acp::NewSessionResponse::new(bootstrap.state.session_id.clone());
        if let Some(models) = bootstrap.acp_models() {
            response = response.models(Some(models));
        }
        Ok(response)
    }

    async fn load_session(
        &self,
        arguments: acp::LoadSessionRequest,
    ) -> Result<acp::LoadSessionResponse, acp::Error> {
        let requested = arguments.session_id.0.to_string();
        let active = self.state.borrow().bootstrap.state.session_id.clone();
        if requested != active {
            let session_path = self
                .state
                .borrow()
                .session_paths
                .get(&requested)
                .cloned()
                .ok_or_else(|| {
                    acp::Error::invalid_params().data(format!(
                        "Pi session {requested} is not in the local catalog"
                    ))
                })?;
            let result = self
                .switch_session(&session_path)
                .await
                .map_err(acp_internal)?;
            if result.cancelled {
                return Err(acp::Error::invalid_params().data("Pi session switch cancelled"));
            }
        }
        let bootstrap = self.state.borrow().bootstrap.clone();
        if requested != bootstrap.state.session_id {
            return Err(acp::Error::invalid_params().data(format!(
                "Pi switched to {}, not requested session {requested}",
                bootstrap.state.session_id
            )));
        }
        self.state.borrow_mut().acp_session_id = requested;
        self.replay_history().await.map_err(acp_internal)?;
        self.publish_bootstrap(&bootstrap).await;
        self.refresh_context_usage().await;
        let mut response = acp::LoadSessionResponse::new();
        if let Some(models) = bootstrap.acp_models() {
            response = response.models(Some(models));
        }
        Ok(response)
    }

    async fn set_session_mode(
        &self,
        _arguments: acp::SetSessionModeRequest,
    ) -> Result<acp::SetSessionModeResponse, acp::Error> {
        // Pi thinking is exposed through Grok's native model/effort surface,
        // not ACP session modes. No modes are advertised during initialize.
        Ok(acp::SetSessionModeResponse::new())
    }

    async fn prompt(
        &self,
        arguments: acp::PromptRequest,
    ) -> Result<acp::PromptResponse, acp::Error> {
        if let Some(command) = direct_bash_command(&arguments.prompt) {
            return self.execute_bash(command, arguments.meta.as_ref()).await;
        }

        let (message, images) = prompt_to_pi(&arguments.prompt);
        if message.trim().is_empty() && images.is_empty() {
            return Err(acp::Error::invalid_params().data("Pi prompt is empty"));
        }

        let client_prompt_id = arguments
            .meta
            .as_ref()
            .and_then(|meta| meta.get("promptId"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .map(str::to_string);

        let (completion_tx, completion_rx) = oneshot::channel();
        let (prompt_id, streaming_behavior) = {
            let mut state = self.state.borrow_mut();
            let already_active = !state.active_prompts.is_empty();
            let prompt_id = state.next_prompt_id;
            state.next_prompt_id = state.next_prompt_id.wrapping_add(1).max(1);
            let streaming_behavior =
                prompt_streaming_behavior(already_active, arguments.meta.as_ref());
            // Prefer the pager's client promptId so optimistic queue echoes
            // confirm by id (not only by text content).
            if let Some(lane) = streaming_behavior.and_then(queue_lane_for_behavior)
                && let Some(client_id) = client_prompt_id.as_deref()
            {
                state.queue_mirror.reserve(
                    client_id.to_string(),
                    message.clone(),
                    lane,
                );
            }
            state.active_prompts.push(ActivePrompt {
                id: prompt_id,
                client_prompt_id: client_prompt_id.clone(),
                completion: completion_tx,
                agent_started: false,
                cancelled: false,
            });
            (prompt_id, streaming_behavior)
        };
        let mut request = json!({ "type": "prompt", "message": message });
        if !images.is_empty() {
            request["images"] = Value::Array(images);
        }
        if let Some(streaming_behavior) = streaming_behavior {
            request["streamingBehavior"] = Value::String(streaming_behavior.to_string());
        }
        if let Err(error) = self.rpc.request(request).await {
            self.remove_prompt(prompt_id);
            return Err(acp_internal(error));
        }
        let probe = self.clone();
        tokio::task::spawn_local(async move {
            probe.probe_prompt_without_agent().await;
        });
        // Wait for agent_settled (or cancel). Mid-turn followUp/steer prompts
        // share this barrier with the primary turn; PromptResponse MUST carry
        // promptId so the pager discards non-current completions instead of
        // painting phantom "Worked for 0.0s" markers for each queued item.
        let completion = completion_rx.await.unwrap_or(PromptCompletion {
            reason: acp::StopReason::Cancelled,
            client_prompt_id: client_prompt_id.clone(),
        });
        Ok(prompt_response(
            completion.reason,
            completion
                .client_prompt_id
                .as_deref()
                .or(client_prompt_id.as_deref()),
        ))
    }

    async fn cancel(&self, _arguments: acp::CancelNotification) -> Result<(), acp::Error> {
        let command = {
            let mut state = self.state.borrow_mut();
            for active in &mut state.active_prompts {
                active.cancelled = true;
            }
            if state.bash_running {
                "abort_bash"
            } else {
                "abort"
            }
        };
        if let Err(error) = self.rpc.request(json!({ "type": command })).await {
            self.finish_prompts(acp::StopReason::Cancelled);
            return Err(acp_internal(error));
        }
        let probe = self.clone();
        tokio::task::spawn_local(async move {
            probe.probe_prompt_without_agent().await;
        });
        Ok(())
    }

    async fn set_session_model(
        &self,
        arguments: acp::SetSessionModelRequest,
    ) -> Result<acp::SetSessionModelResponse, acp::Error> {
        let model_id = arguments.model_id.0.to_string();
        let model = self
            .state
            .borrow()
            .model_map
            .get(&model_id)
            .cloned()
            .ok_or_else(|| {
                acp::Error::invalid_params().data(format!("unknown Pi model: {model_id}"))
            })?;
        let requested_effort = arguments
            .meta
            .as_ref()
            .and_then(|meta| meta.get("reasoningEffort"))
            .and_then(Value::as_str);
        let pi_effort = requested_effort
            .map(|effort| {
                model.pi_level_for_acp_effort(effort).ok_or_else(|| {
                    acp::Error::invalid_params().data(format!(
                        "Pi model {} does not support reasoning effort {effort}",
                        model.label
                    ))
                })
            })
            .transpose()?;
        let model_is_current = self
            .state
            .borrow()
            .bootstrap
            .state
            .model
            .as_ref()
            .is_some_and(|current| current.provider == model.provider && current.id == model.id);
        if !model_is_current {
            self.rpc
                .request(json!({
                    "type": "set_model",
                    "provider": model.provider,
                    "modelId": model.id,
                }))
                .await
                .map_err(acp_internal)?;
        }
        if let Some(level) = pi_effort {
            self.rpc
                .request(json!({
                    "type": "set_thinking_level",
                    "level": level,
                }))
                .await
                .map_err(acp_internal)?;
        }
        let bootstrap = self.refresh().await.map_err(acp_internal)?;
        self.publish_bootstrap(&bootstrap).await;
        Ok(acp::SetSessionModelResponse::new())
    }

    async fn ext_method(&self, arguments: acp::ExtRequest) -> Result<acp::ExtResponse, acp::Error> {
        match arguments.method.as_ref() {
            "x.ai/interject" => self.handle_steer_message(arguments.params.get()).await,
            "x.ai/compact_conversation" => {
                let params: Value =
                    serde_json::from_str(arguments.params.get()).unwrap_or_default();
                let mut request = json!({ "type": "compact" });
                if let Some(instructions) =
                    string(&params, &["customInstructions", "instructions", "context"])
                    && !instructions.trim().is_empty()
                {
                    request["customInstructions"] = Value::String(instructions.to_string());
                }
                let data = self.rpc.request(request).await.map_err(acp_internal)?;
                ext_response(data).map_err(acp_internal)
            }
            "pi/session/list" => {
                let params: Value =
                    serde_json::from_str(arguments.params.get()).unwrap_or_default();
                let cwd = string(&params, &["cwd"])
                    .filter(|cwd| !cwd.trim().is_empty())
                    .map(PathBuf::from)
                    .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
                let all = string(&params, &["scope"]) == Some("all");
                self.publish_session_catalog(cwd, all).await;
                ext_response(json!({})).map_err(acp_internal)
            }
            // Pi session entry tree (read-only projection of get_tree).
            "pi/session/tree" => {
                let tree = self.fetch_session_tree().await.map_err(acp_internal)?;
                ext_response(json!({
                    "leafId": tree.leaf_id,
                    "nodes": tree.rows.iter().map(|row| json!({
                        "id": row.id,
                        "parentId": row.parent_id,
                        "depth": row.depth,
                        "isLeaf": row.is_leaf,
                        "isCurrent": row.is_current,
                        "onActivePath": row.on_active_path,
                        "role": row.role,
                        "preview": row.preview,
                        "detail": row.detail,
                        "label": row.label,
                        "labelTimestamp": row.label_timestamp,
                        "entryType": row.entry_type,
                        "timestamp": row.timestamp,
                        "childIds": row.child_ids,
                        "hasText": row.has_text,
                    })).collect::<Vec<_>>(),
                }))
                .map_err(acp_internal)
            }
            // Navigate leaf via injected extension command → ctx.navigateTree.
            "pi/session/navigate_tree" => {
                let params: Value =
                    serde_json::from_str(arguments.params.get()).unwrap_or_default();
                let entry_id = string(&params, &["entryId", "id", "targetId"])
                    .map(str::trim)
                    .filter(|id| !id.is_empty())
                    .ok_or_else(|| acp::Error::invalid_params().data("entryId is required"))?;
                let summarize = params
                    .get("summarize")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                let custom_instructions =
                    string(&params, &["customInstructions", "instructions"]);
                let data = self
                    .navigate_session_tree(entry_id, summarize, custom_instructions)
                    .await?;
                ext_response(data).map_err(acp_internal)
            }
            // Set/clear entry label via injected extension → ctx.setLabel.
            "pi/session/tree_label" => {
                let params: Value =
                    serde_json::from_str(arguments.params.get()).unwrap_or_default();
                let entry_id = string(&params, &["entryId", "id", "targetId"])
                    .map(str::trim)
                    .filter(|id| !id.is_empty())
                    .ok_or_else(|| acp::Error::invalid_params().data("entryId is required"))?;
                let clear = params
                    .get("clear")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                let label = if clear {
                    None
                } else {
                    string(&params, &["label", "text"])
                };
                let data = self.set_session_tree_label(entry_id, label).await?;
                ext_response(data).map_err(acp_internal)
            }
            "x.ai/session/rename" => {
                let params: Value =
                    serde_json::from_str(arguments.params.get()).map_err(acp_internal)?;
                let title = string(&params, &["title", "name"])
                    .map(str::trim)
                    .filter(|title| !title.is_empty())
                    .ok_or_else(|| acp::Error::invalid_params().data("session title is empty"))?;
                self.rpc
                    .request(json!({ "type": "set_session_name", "name": title }))
                    .await
                    .map_err(acp_internal)?;
                if let Ok(bootstrap) = self.refresh().await {
                    self.publish_bootstrap(&bootstrap).await;
                } else {
                    self.send_title(Some(title)).await;
                }
                ext_response(json!({})).map_err(acp_internal)
            }
            // Grok `/context` and context-bar click fetch this; map Pi
            // get_session_stats (+ message estimate) into native ContextInfo.
            "x.ai/session/info" => self.handle_session_info().await,
            method => Err(acp::Error::new(
                acp::ErrorCode::MethodNotFound.into(),
                format!("Method not found: {method}"),
            )),
        }
    }

    async fn ext_notification(&self, arguments: acp::ExtNotification) -> Result<(), acp::Error> {
        match arguments.method.as_ref() {
            "x.ai/queue/remove" | "x.ai/queue/clear" | "x.ai/queue/edit" | "x.ai/queue/reorder" => {
                // Pi RPC does not expose per-item queue mutation / clearQueue.
                // Rebroadcast the authoritative Pi mirror so the pager cannot
                // keep a client-only ghost removal, and surface the boundary.
                if matches!(
                    arguments.method.as_ref(),
                    "x.ai/queue/remove" | "x.ai/queue/clear" | "x.ai/queue/edit"
                ) {
                    self.send_ui_notification(
                        "Pi queue is read-mirrored; remove/edit/clear are not supported over RPC",
                        Some("warning"),
                    )
                    .await;
                }
                self.rebroadcast_queue_mirror().await;
                Ok(())
            }
            "x.ai/queue/interject" => {
                // Promote by text via steer (same lane as x.ai/interject).
                // Pi cannot drop a single follow-up row, so a follow-up item may
                // still deliver later — documented adapter boundary.
                let params: Value =
                    serde_json::from_str(arguments.params.get()).unwrap_or_default();
                let id = string(&params, &["id"]).unwrap_or_default();
                let text = {
                    let state = self.state.borrow();
                    state.queue_mirror.text_for_id(id).map(str::to_string)
                };
                let text = text.or_else(|| {
                    string(&params, &["newText", "text"]).map(str::to_string)
                });
                if let Some(text) = text.filter(|text| !text.trim().is_empty()) {
                    let _ = self
                        .rpc
                        .request(json!({
                            "type": "prompt",
                            "message": text,
                            "streamingBehavior": "steer",
                        }))
                        .await;
                }
                self.rebroadcast_queue_mirror().await;
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

impl PiAgent {
    async fn handle_steer_message(&self, params_raw: &str) -> Result<acp::ExtResponse, acp::Error> {
        let params: Value = serde_json::from_str(params_raw).map_err(acp_internal)?;
        let blocks = params
            .get("content")
            .cloned()
            .and_then(|value| serde_json::from_value::<Vec<acp::ContentBlock>>(value).ok());
        let (message, images) = if let Some(blocks) = blocks.as_deref() {
            prompt_to_pi(blocks)
        } else {
            (
                string(&params, &["text"]).unwrap_or_default().to_string(),
                Vec::new(),
            )
        };
        if message.trim().is_empty() && images.is_empty() {
            return Err(acp::Error::invalid_params().data("Pi interjection is empty"));
        }
        if let Some(client_id) = string(&params, &["interjectionId", "promptId"])
            .map(str::trim)
            .filter(|id| !id.is_empty())
        {
            self.state.borrow_mut().queue_mirror.reserve(
                client_id.to_string(),
                message.clone(),
                QueueLane::Steering,
            );
        }
        let mut request = json!({
            "type": "prompt",
            "message": message,
            "streamingBehavior": "steer",
        });
        if !images.is_empty() {
            request["images"] = Value::Array(images);
        }
        let data = self.rpc.request(request).await.map_err(acp_internal)?;
        ext_response(data).map_err(acp_internal)
    }
}

fn build_model_catalog(
    models: &[PiModel],
    current: Option<&PiModel>,
    thinking_level: &str,
) -> (IndexMap<acp::ModelId, acp::ModelInfo>, Option<acp::ModelId>) {
    let mut available = IndexMap::new();
    for model in models {
        let id = acp::ModelId::new(model_key(model));
        let mut meta = serde_json::Map::new();
        if let Some(tokens) = model.context_window {
            meta.insert("totalContextTokens".into(), json!(tokens));
        }
        meta.insert("acceptsImages".into(), json!(model.accepts_images));
        let reasoning_efforts = model_reasoning_efforts(model);
        if !reasoning_efforts.is_empty() {
            meta.insert("supportsReasoningEffort".into(), json!(true));
            meta.insert(
                "reasoningEffort".into(),
                json!(pi_effort_to_acp(thinking_level)),
            );
            meta.insert("reasoningEfforts".into(), Value::Array(reasoning_efforts));
        }
        let info = acp::ModelInfo::new(id.clone(), model.label.clone()).meta(Some(meta));
        available.insert(id, info);
    }
    let current = current.map(|model| acp::ModelId::new(model_key(model)));
    (available, current)
}

/// Internal bridge commands injected by grok-pi; never advertised to slash UI.
const NAVIGATE_TREE_COMMAND: &str = "__pi_navigate_tree";
const LABEL_TREE_COMMAND: &str = "__pi_tree_label";

fn is_bridge_command(name: &str) -> bool {
    name.eq_ignore_ascii_case(NAVIGATE_TREE_COMMAND)
        || name.eq_ignore_ascii_case(LABEL_TREE_COMMAND)
}

fn command_catalog(commands: &[PiCommand]) -> Vec<acp::AvailableCommand> {
    // The adapter reports Pi's command catalog verbatim (normalized and
    // deduplicated). Grok's native CommandRegistry owns collision policy with
    // pager-local commands such as /help, /model, and /compact.
    let mut seen = HashSet::new();
    commands
        .iter()
        .filter_map(|command| {
            let name = command.name.trim().trim_start_matches('/');
            if name.is_empty()
                || is_bridge_command(name)
                || !seen.insert(name.to_ascii_lowercase())
            {
                return None;
            }
            let description = if command.description.trim().is_empty() {
                if command.source.trim().is_empty() {
                    "Pi command".to_string()
                } else {
                    format!("Pi {} command", command.source)
                }
            } else {
                command.description.clone()
            };
            Some(acp::AvailableCommand::new(name.to_string(), description))
        })
        .collect()
}

fn model_key(model: &PiModel) -> String {
    if model.provider.is_empty() {
        model.id.clone()
    } else {
        format!("{}::{}", model.provider, model.id)
    }
}

fn catalog_session_dir(state: &PiState, configured_dir: &Path) -> PathBuf {
    state
        .session_file
        .as_deref()
        .map(Path::new)
        .and_then(Path::parent)
        .filter(|directory| !directory.starts_with(configured_dir))
        .map(Path::to_path_buf)
        .unwrap_or_else(|| configured_dir.to_path_buf())
}

fn content_chunk(content: acp::ContentBlock) -> acp::ContentChunk {
    acp::ContentChunk::new(content)
}

fn text_chunk(text: impl Into<String>) -> acp::ContentChunk {
    content_chunk(acp::ContentBlock::Text(acp::TextContent::new(text)))
}

/// Extract context-window used tokens from Pi `get_session_stats` data.
fn context_tokens_from_stats(data: &Value) -> Option<u64> {
    let usage = data.get("contextUsage")?;
    // After compaction Pi may report null until the next assistant response.
    usage
        .get("tokens")
        .and_then(Value::as_u64)
        .or_else(|| {
            usage
                .get("tokens")
                .and_then(Value::as_f64)
                .map(|value| value.round() as u64)
        })
        .filter(|&tokens| tokens > 0)
}

fn context_window_from_stats(data: &Value) -> Option<u64> {
    data.get("contextUsage").and_then(|usage| {
        usage
            .get("contextWindow")
            .or_else(|| usage.get("context_window"))
            .and_then(|value| value.as_u64().or_else(|| value.as_f64().map(|n| n.round() as u64)))
            .filter(|&tokens| tokens > 0)
    })
}

fn usage_pct_from_stats(data: &Value, used: u64, total: u64) -> u8 {
    if let Some(percent) = data
        .get("contextUsage")
        .and_then(|usage| usage.get("percent"))
        .and_then(Value::as_f64)
    {
        return percent.round().clamp(0.0, 100.0) as u8;
    }
    if total == 0 {
        0
    } else {
        ((used as f64 / total as f64) * 100.0)
            .round()
            .clamp(0.0, 100.0) as u8
    }
}

/// Approximate token count used by the pi-context extension (`ceil(len/4)`).
fn estimate_tokens_text(text: &str) -> u64 {
    if text.is_empty() {
        0
    } else {
        ((text.len() as f64) / 4.0).ceil() as u64
    }
}

fn estimate_tokens_value(value: &Value) -> u64 {
    match value {
        Value::Null => 0,
        Value::String(text) => estimate_tokens_text(text),
        Value::Array(items) => items.iter().map(estimate_tokens_value).sum(),
        Value::Object(map) => map.values().map(estimate_tokens_value).sum(),
        other => estimate_tokens_text(&other.to_string()),
    }
}

/// Project `get_entries` payload into a `{ messages: [...] }` shape that
/// [`estimate_message_tokens`] already understands.
fn entries_to_messages_value(entries: Value) -> Value {
    let items = entries
        .get("entries")
        .and_then(Value::as_array)
        .cloned()
        .or_else(|| entries.as_array().cloned())
        .unwrap_or_default();
    let messages: Vec<Value> = items
        .into_iter()
        .filter_map(|entry| {
            let kind = string(&entry, &["type", "kind"])
                .unwrap_or_default()
                .to_ascii_lowercase();
            if kind == "message" || kind.is_empty() {
                Some(entry.get("message").cloned().unwrap_or(entry))
            } else if kind.contains("compaction") || kind.contains("branch") {
                // Keep summary text so compaction/branch tokens contribute.
                Some(entry)
            } else {
                None
            }
        })
        .collect();
    json!({ "messages": messages })
}

/// Raw (unscaled) message-window estimate, mirroring pi-context categories.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct MessageTokenEstimate {
    /// User + assistant text (+ thought) content.
    messages: u64,
    /// Tool calls + tool results (shown as Reasoning/overhead when unallocated).
    tool_payload: u64,
    compaction_count: u64,
}

fn estimate_message_tokens(messages: &Value) -> MessageTokenEstimate {
    let mut estimate = MessageTokenEstimate::default();
    let Some(items) = messages
        .get("messages")
        .and_then(Value::as_array)
        .or_else(|| messages.as_array())
    else {
        return estimate;
    };
    for item in items {
        let message = item.get("message").unwrap_or(item);
        let role = string(message, &["role", "type"])
            .unwrap_or_default()
            .to_ascii_lowercase();
        match role.as_str() {
            "user" => {
                estimate.messages += estimate_tokens_value(message.get("content").unwrap_or(message));
            }
            "assistant" => {
                if let Some(content) = message.get("content").and_then(Value::as_array) {
                    for part in content {
                        match content_kind_local(part).as_str() {
                            "text" => {
                                if let Some(text) = string(part, &["text"]) {
                                    estimate.messages += estimate_tokens_text(text);
                                }
                            }
                            "thinking" | "reasoning" => {
                                if let Some(text) =
                                    string(part, &["thinking", "reasoning", "text"])
                                {
                                    estimate.messages += estimate_tokens_text(text);
                                }
                            }
                            "toolcall" | "tool_call" | "tool" => {
                                estimate.tool_payload += estimate_tokens_value(part);
                            }
                            _ => {
                                estimate.messages += estimate_tokens_value(part);
                            }
                        }
                    }
                } else if let Some(text) = message.get("content").and_then(Value::as_str) {
                    estimate.messages += estimate_tokens_text(text);
                }
            }
            "toolresult" | "tool_result" => {
                estimate.tool_payload += estimate_tokens_value(message.get("content").unwrap_or(&Value::Null));
            }
            "bashexecution" | "bash_execution" => {
                estimate.tool_payload += estimate_tokens_text(
                    string(message, &["command"]).unwrap_or_default(),
                );
                estimate.tool_payload += estimate_tokens_text(
                    string(message, &["output"]).unwrap_or_default(),
                );
            }
            "branchsummary"
            | "branch_summary"
            | "compactionsummary"
            | "compaction_summary" => {
                estimate.messages += estimate_tokens_text(
                    string(message, &["summary", "text"]).unwrap_or_default(),
                );
                if role.contains("compaction") {
                    estimate.compaction_count += 1;
                }
            }
            _ => {}
        }
    }
    estimate
}

fn content_kind_local(value: &Value) -> String {
    string(value, &["type", "kind"])
        .unwrap_or_default()
        .to_ascii_lowercase()
        .replace(['-', '_'], "")
}

/// Scale raw char-based estimates so they sum to the authoritative `used`
/// total from Pi (`contextUsage.tokens`), same ratio trick as pi-context.
fn scale_token_parts(parts: &[u64], used: u64) -> Vec<u64> {
    let raw_total: u64 = parts.iter().sum();
    if raw_total == 0 || used == 0 {
        return parts.iter().map(|_| 0).collect();
    }
    let ratio = used as f64 / raw_total as f64;
    let mut scaled: Vec<u64> = parts
        .iter()
        .map(|&part| ((part as f64) * ratio).round() as u64)
        .collect();
    // Keep the bar total coherent: adjust the largest bucket if rounding drifts.
    let scaled_sum: u64 = scaled.iter().sum();
    if scaled_sum != used
        && let Some((idx, _)) = scaled
            .iter()
            .enumerate()
            .max_by_key(|(_, value)| *value)
    {
        if scaled_sum > used {
            scaled[idx] = scaled[idx].saturating_sub(scaled_sum - used);
        } else {
            scaled[idx] = scaled[idx].saturating_add(used - scaled_sum);
        }
    }
    scaled
}

/// Project Pi stats (+ optional message estimate) into Grok `SessionInfoResponse` JSON.
///
/// Shape matches `xai_grok_shell::session::SessionInfoResponse` so the pager can
/// deserialize into `ContextInfo` and push `RenderBlock::context_info`.
fn build_session_info_response(
    stats: &Value,
    messages: Option<&Value>,
    session_id: &str,
    cwd: &str,
    model: Option<&PiModel>,
    cached_tokens: Option<u64>,
) -> Value {
    let used = context_tokens_from_stats(stats)
        .or(cached_tokens)
        .unwrap_or(0);
    let total = context_window_from_stats(stats)
        .or_else(|| model.and_then(|m| m.context_window))
        .unwrap_or(0);
    let estimate = messages
        .map(estimate_message_tokens)
        .unwrap_or_default();
    // When message estimation fails (RPC error / empty transcript), put the
    // whole window into Messages so the bar is not 100% "Reasoning/overhead".
    let raw_parts = [estimate.messages, estimate.tool_payload];
    let raw_total: u64 = raw_parts.iter().sum();
    let message_tokens = if used == 0 {
        0
    } else if raw_total == 0 {
        used
    } else {
        scale_token_parts(&raw_parts, used)
            .first()
            .copied()
            .unwrap_or(used)
    };
    // tool_payload becomes Reasoning/overhead via used - system - messages.
    // System prompt / tool definitions are not exposed over Pi RPC, so they stay 0
    // (pi-context can read them in-process; we cannot without expanding RPC).
    let free_tokens = total.saturating_sub(used);
    let usage_pct = usage_pct_from_stats(stats, used, total);
    let turns = number(stats, &["userMessages", "user_messages"]).unwrap_or(0);
    let tool_call_count = number(stats, &["toolCalls", "tool_calls"]).unwrap_or(0);
    let message_count = number(stats, &["totalMessages", "total_messages"]).unwrap_or(0);
    let model_id = model.map(|m| m.id.clone());
    let model_label = model.map(|m| {
        if m.label.is_empty() {
            m.id.clone()
        } else {
            m.label.clone()
        }
    });

    // Build without explicit JSON nulls so Option fields that lack
    // `#[serde(default)]` never see `null` on the wire.
    let mut response = json!({
        "sessionId": session_id,
        "cwd": cwd,
        "agentName": "pi",
        "showModelFingerprint": false,
        "turns": turns,
        "turnIndex": turns.saturating_sub(1),
        "context": {
            "used": used,
            "total": total,
            "systemPromptTokens": 0,
            "toolDefinitionsCount": 0,
            "toolDefinitionsTokens": 0,
            "compactionCount": estimate.compaction_count,
            "turnCount": turns,
            "toolCallCount": tool_call_count,
            "messageCount": message_count,
            "messageTokens": message_tokens,
            "freeTokens": free_tokens,
            "usagePct": usage_pct,
            "autoCompactThresholdPercent": 85_u8,
        }
    });
    if let Some(obj) = response.as_object_mut() {
        if let Some(id) = model_id {
            obj.insert("model".into(), Value::String(id));
        }
        if let Some(label) = model_label {
            obj.insert("modelDisplayName".into(), Value::String(label));
        }
    }
    response
}

/// Approximate context tokens from a Pi assistant `usage` object.
///
/// Mirrors Pi's `calculateContextTokens`: prefer `totalTokens`, else sum
/// input/output/cache components.
fn context_tokens_from_usage(usage: &Value) -> Option<u64> {
    if let Some(total) = usage
        .get("totalTokens")
        .or_else(|| usage.get("total_tokens"))
        .and_then(Value::as_u64)
        .filter(|&tokens| tokens > 0)
    {
        return Some(total);
    }
    let field = |names: &[&str]| -> u64 {
        names
            .iter()
            .find_map(|name| usage.get(*name).and_then(Value::as_u64))
            .unwrap_or(0)
    };
    let total = field(&["input"])
        + field(&["output"])
        + field(&["cacheRead", "cache_read"])
        + field(&["cacheWrite", "cache_write"]);
    (total > 0).then_some(total)
}

fn history_tool_content(content: Vec<PiToolContent>) -> Vec<acp::ToolCallContent> {
    content
        .into_iter()
        .map(|item| match item {
            PiToolContent::Text(text) => {
                acp::ToolCallContent::from(acp::ContentBlock::Text(acp::TextContent::new(text)))
            }
            PiToolContent::Image { data, mime_type } => acp::ToolCallContent::from(
                acp::ContentBlock::Image(acp::ImageContent::new(data, mime_type)),
            ),
        })
        .collect()
}

fn tool_content(value: &Value) -> Vec<acp::ToolCallContent> {
    let source = value.get("content").unwrap_or(value);
    let mut output = Vec::new();
    match source {
        Value::Array(items) => {
            for item in items {
                let kind = string(item, &["type", "kind"])
                    .unwrap_or_default()
                    .to_ascii_lowercase();
                if kind == "image"
                    && let (Some(data), Some(mime_type)) = (
                        string(item, &["data"]),
                        string(item, &["mimeType", "mime_type"]),
                    )
                {
                    output.push(acp::ToolCallContent::from(acp::ContentBlock::Image(
                        acp::ImageContent::new(data, mime_type),
                    )));
                } else {
                    let text = string(item, &["text", "content", "message", "output"])
                        .map(ToOwned::to_owned)
                        .unwrap_or_else(|| json_text(item));
                    if !text.is_empty() {
                        output.push(acp::ToolCallContent::from(acp::ContentBlock::Text(
                            acp::TextContent::new(text),
                        )));
                    }
                }
            }
        }
        _ => {
            let text = json_text(source);
            if !text.is_empty() {
                output.push(acp::ToolCallContent::from(acp::ContentBlock::Text(
                    acp::TextContent::new(text),
                )));
            }
        }
    }
    output
}

/// Convert Pi's edit/write input contract into ACP's native diff payload.
///
/// The Pager's Edit card and viewer intentionally render only `Diff` content;
/// ordinary text results do not provide the old/new source needed for a hunk.
fn edit_diff_content(tool_name: &str, args: Option<&Value>) -> Option<Vec<acp::ToolCallContent>> {
    if tool_kind(tool_name) != acp::ToolKind::Edit {
        return None;
    }
    let args = args?;
    let path = string(args, &["path", "filePath", "file_path", "target_file"])?;
    if let Some(edits) = args.get("edits").and_then(Value::as_array) {
        let diffs = edits
            .iter()
            .filter_map(|edit| {
                let old_text = string(edit, &["oldText", "old_text"])?;
                let new_text = string(edit, &["newText", "new_text"])?;
                Some(acp::ToolCallContent::Diff(
                    acp::Diff::new(path, new_text.to_owned()).old_text(Some(old_text.to_owned())),
                ))
            })
            .collect::<Vec<_>>();
        return (!diffs.is_empty()).then_some(diffs);
    }
    let new_text = string(args, &["newText", "new_text", "content"])?;
    let old_text = string(args, &["oldText", "old_text"]).map(ToOwned::to_owned);
    Some(vec![acp::ToolCallContent::Diff(
        acp::Diff::new(path, new_text.to_owned()).old_text(old_text),
    )])
}

fn tool_kind(name: &str) -> acp::ToolKind {
    let name = name.to_ascii_lowercase();
    // Exact Pi builtin names first (avoid substring false-positives).
    match name.as_str() {
        "read" => return acp::ToolKind::Read,
        "bash" => return acp::ToolKind::Execute,
        "edit" | "write" => return acp::ToolKind::Edit,
        "grep" | "find" => return acp::ToolKind::Search,
        // ListDir is detected in the pager via `raw_input.target_directory`
        // (there is no ACP ListDir kind). Keep Other so that branch can match.
        "ls" => return acp::ToolKind::Other,
        _ => {}
    }
    if name.contains("read") {
        acp::ToolKind::Read
    } else if name.contains("write") || name.contains("edit") || name.contains("patch") {
        acp::ToolKind::Edit
    } else if name.contains("delete") || name.contains("remove") {
        acp::ToolKind::Delete
    } else if name.contains("move") || name.contains("rename") {
        acp::ToolKind::Move
    } else if name.contains("search") || name.contains("grep") || name.contains("find") {
        acp::ToolKind::Search
    } else if name.contains("bash") || name.contains("shell") || name.contains("exec") {
        acp::ToolKind::Execute
    } else if name.contains("fetch") || name.contains("web") {
        acp::ToolKind::Fetch
    } else {
        acp::ToolKind::Other
    }
}

fn is_find_tool(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    name == "find" || name == "glob" || name.ends_with("_find") || name.contains("glob_file")
}

fn is_ls_tool(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    name == "ls" || name == "list_dir" || name == "listdir" || name == "list_directory"
}

/// Rewrite Pi tool args into shapes the native Grok cards already understand.
///
/// - `write` → `variant: "Write"` so Edit cards show "Creating "
/// - `ls` → `target_directory` (ListDir card key)
/// - `grep` → `-i` alias for `ignoreCase`
/// - `find` → `output_mode: files_with_matches`
fn normalize_tool_raw_input(name: &str, args: Option<Value>) -> Option<Value> {
    let mut args = args?;
    let Some(obj) = args.as_object_mut() else {
        return Some(args);
    };
    let lower = name.to_ascii_lowercase();

    if lower == "write" || lower.ends_with("_write") {
        obj.entry("variant".to_string())
            .or_insert_with(|| json!("Write"));
    }

    if is_ls_tool(name) {
        if let Some(path) = obj.get("path").cloned() {
            obj.entry("target_directory".to_string()).or_insert(path);
        } else {
            obj.entry("target_directory".to_string())
                .or_insert_with(|| json!("."));
        }
    }

    if is_find_tool(name) {
        obj.entry("output_mode".to_string())
            .or_insert_with(|| json!("files_with_matches"));
        // Prefer `pattern` as the search term; copy glob-like patterns into
        // `glob_pattern` for extractors that look for it.
        if let Some(pattern) = obj.get("pattern").cloned() {
            obj.entry("glob_pattern".to_string()).or_insert(pattern);
        }
    }

    if lower == "grep" || lower.contains("grep") {
        if let Some(ignore_case) = obj.get("ignoreCase").cloned() {
            obj.entry("-i".to_string()).or_insert(ignore_case);
        }
    }

    Some(args)
}

/// Project Pi tool results into the typed `raw_output` shapes native Grok cards
/// deserialize (`ToolOutput::ReadFile` / `Bash` / `GrepSearch` / `ListDir`).
///
/// Without this conversion the Read card has no path/line metadata, the
/// Execute card/viewer has command only, and Search/ListDir cards show empty
/// structured results — Pi's payload is text `content`, not Grok's tagged
/// tool output enum.
fn normalize_tool_raw_output(
    name: &str,
    args: Option<&Value>,
    result: &Value,
    is_error: bool,
) -> Value {
    if is_ls_tool(name) {
        return ls_tool_output(args, result, is_error);
    }
    match tool_kind(name) {
        acp::ToolKind::Read => read_tool_output(args, result, is_error),
        acp::ToolKind::Execute => {
            let command = args
                .and_then(|value| string(value, &["command", "cmd"]))
                .unwrap_or_default()
                .to_string();
            bash_tool_output(&command, result, is_error)
        }
        acp::ToolKind::Search => {
            if is_find_tool(name) {
                find_tool_output(result, is_error)
            } else {
                grep_tool_output(result, is_error)
            }
        }
        _ => result.clone(),
    }
}

fn pi_result_text(value: &Value) -> String {
    if let Some(text) = value.get("output").and_then(Value::as_str) {
        return text.to_string();
    }
    let source = value.get("content").unwrap_or(value);
    match source {
        Value::Array(items) => items
            .iter()
            .filter_map(|item| {
                string(item, &["text", "content", "message", "output"])
                    .map(str::to_owned)
                    .or_else(|| item.as_str().map(str::to_owned))
            })
            .collect::<Vec<_>>()
            .join("\n"),
        Value::String(text) => text.clone(),
        _ => {
            let text = json_text(source);
            if text == "null" { String::new() } else { text }
        }
    }
}

fn read_tool_output(args: Option<&Value>, result: &Value, is_error: bool) -> Value {
    let text = pi_result_text(result);
    if is_error {
        let message = if text.trim().is_empty() {
            "Read failed".to_string()
        } else {
            text
        };
        return json!({
            "type": "ReadFile",
            "FileReadError": message,
        });
    }

    let path = args
        .and_then(|value| string(value, &["path", "filePath", "file_path", "target_file"]))
        .unwrap_or_default()
        .to_string();
    let offset = args
        .and_then(|value| value.get("offset"))
        .and_then(Value::as_u64)
        .map(|n| n as usize);
    let limit = args
        .and_then(|value| value.get("limit"))
        .and_then(Value::as_u64)
        .map(|n| n as usize);

    // Strip Pi continuation footers for line counting; keep full text for content.
    let body = text
        .split("\n\n[")
        .next()
        .unwrap_or(text.as_str())
        .trim_end_matches('\n');
    let content_lines = if body.is_empty() {
        0
    } else {
        body.lines().count()
    };
    let total_from_footer = text
        .rsplit_once(" of ")
        .and_then(|(_, rest)| {
            rest.split(|c: char| !c.is_ascii_digit())
                .find(|part| !part.is_empty())
                .and_then(|digits| digits.parse::<usize>().ok())
        });
    let start_index = offset.unwrap_or(1).saturating_sub(1);
    let total_lines = total_from_footer
        .unwrap_or(start_index.saturating_add(content_lines))
        .max(content_lines);

    // Pager Read cards treat FileContent.offset as a 0-based skip count
    // (`start = offset + 1`). Pi's offset is 1-indexed. When Pi omits a window,
    // still publish a 0-based full-file range so the header can show line counts.
    let (stored_offset, stored_limit) = match (offset, limit) {
        (None, None) if content_lines > 0 => (Some(0usize), Some(content_lines)),
        (offset, limit) => (offset.map(|value| value.saturating_sub(1)), limit),
    };

    json!({
        "type": "ReadFile",
        "FileContent": {
            "content": text,
            "absolute_path": path,
            "offset": stored_offset,
            "limit": stored_limit,
            "raw_output": body,
            "total_lines": total_lines,
        }
    })
}

/// Project Pi `grep` text (`path:line: content`) into `ToolOutput::GrepSearch`.
fn grep_tool_output(result: &Value, is_error: bool) -> Value {
    let text = pi_result_text(result);
    let trimmed = text.trim();
    if is_error {
        return json!({
            "type": "GrepSearch",
            "stdout": text.as_bytes().to_vec(),
            "stderr": text.as_bytes().to_vec(),
            "exit_code": 2,
            "match_count": 0,
            "file_matches": [],
        });
    }
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("No matches found") {
        return json!({
            "type": "GrepSearch",
            "stdout": text.as_bytes().to_vec(),
            "stderr": Vec::<u8>::new(),
            "exit_code": 1,
            "match_count": 0,
            "file_matches": [],
        });
    }

    let (file_matches, match_count) = parse_pi_grep_matches(&text);
    json!({
        "type": "GrepSearch",
        "stdout": text.as_bytes().to_vec(),
        "stderr": Vec::<u8>::new(),
        "exit_code": 0,
        "match_count": match_count,
        "file_matches": file_matches,
    })
}

/// Project Pi `find` path list into `ToolOutput::GrepSearch` (files_with_matches).
fn find_tool_output(result: &Value, is_error: bool) -> Value {
    let text = pi_result_text(result);
    let trimmed = text.trim();
    if is_error {
        return json!({
            "type": "GrepSearch",
            "stdout": text.as_bytes().to_vec(),
            "stderr": text.as_bytes().to_vec(),
            "exit_code": 2,
            "match_count": 0,
            "file_matches": [],
        });
    }
    if trimmed.is_empty()
        || trimmed.eq_ignore_ascii_case("No files found matching pattern")
        || trimmed.eq_ignore_ascii_case("No files found")
    {
        return json!({
            "type": "GrepSearch",
            "stdout": text.as_bytes().to_vec(),
            "stderr": Vec::<u8>::new(),
            "exit_code": 0,
            "match_count": 0,
            "file_matches": [],
        });
    }

    // One path per line. Store as stdout so the pager can also recover
    // file_paths via parse_file_paths_from_stdout when file_matches is empty.
    let paths: Vec<&str> = trimmed
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    let match_count = paths.len();
    json!({
        "type": "GrepSearch",
        "stdout": text.as_bytes().to_vec(),
        "stderr": Vec::<u8>::new(),
        "exit_code": 0,
        "match_count": match_count,
        "file_matches": [],
    })
}

/// Project Pi `ls` listing into `ToolOutput::ListDir`.
fn ls_tool_output(args: Option<&Value>, result: &Value, is_error: bool) -> Value {
    let text = pi_result_text(result);
    let path = args
        .and_then(|value| string(value, &["path", "target_directory", "targetDirectory"]))
        .unwrap_or(".")
        .to_string();

    if is_error {
        let message = if text.trim().is_empty() {
            "List directory failed".to_string()
        } else {
            text
        };
        // Prefer NotFound when Pi's message looks like a missing path.
        if message.to_ascii_lowercase().contains("not found")
            || message.to_ascii_lowercase().contains("no such file")
        {
            return json!({ "type": "ListDir", "NotFound": message });
        }
        if message.to_ascii_lowercase().contains("not a directory") {
            return json!({ "type": "ListDir", "NotADirectory": message });
        }
        if message.to_ascii_lowercase().contains("permission") {
            return json!({ "type": "ListDir", "PermissionDenied": message });
        }
        return json!({ "type": "ListDir", "Error": message });
    }

    json!({
        "type": "ListDir",
        "Content": {
            "content": text,
            "absolute_root_path": path,
        }
    })
}

/// Parse Pi grep output lines:
/// - match: `path:line: content`
/// - context: `path-line- content` (ignored for match_count)
fn parse_pi_grep_matches(text: &str) -> (Vec<Value>, usize) {
    // path -> ordered line matches
    let mut order: Vec<String> = Vec::new();
    let mut by_path: IndexMap<String, Vec<Value>> = IndexMap::new();
    let mut match_count = 0usize;

    for line in text.lines() {
        let line = line.trim_end();
        if line.is_empty() {
            continue;
        }
        // Prefer match lines `path:N: rest` over context `path-N- rest`.
        if let Some((path, line_number, content)) = split_pi_grep_match_line(line) {
            match_count += 1;
            if !by_path.contains_key(path) {
                order.push(path.to_string());
            }
            by_path.entry(path.to_string()).or_default().push(json!({
                "line_number": line_number,
                "content": content,
            }));
        }
    }

    let file_matches = order
        .into_iter()
        .filter_map(|path| {
            let matches = by_path.swap_remove(&path)?;
            Some(json!({
                "path": path,
                "matches": matches,
            }))
        })
        .collect();
    (file_matches, match_count)
}

fn split_pi_grep_match_line(line: &str) -> Option<(&str, usize, &str)> {
    // Format: `relative/path:12: content` — path may contain colons on Windows
    // (`C:\...`), so scan from the right for `:digits:`.
    let bytes = line.as_bytes();
    let mut i = bytes.len();
    // Find last ": <content>" separator after a line number.
    while i > 0 {
        // find `:` that starts content
        if let Some(colon_content) = line[..i].rfind(':') {
            let after = &line[colon_content + 1..];
            // content may start with space
            let before = &line[..colon_content];
            if let Some(colon_line) = before.rfind(':') {
                let line_str = &before[colon_line + 1..];
                if let Ok(line_number) = line_str.parse::<usize>() {
                    let path = &before[..colon_line];
                    if !path.is_empty() && line_number > 0 {
                        let content = after.strip_prefix(' ').unwrap_or(after);
                        return Some((path, line_number, content));
                    }
                }
            }
            i = colon_content;
        } else {
            break;
        }
    }
    None
}

fn bash_tool_output(command: &str, result: &Value, is_error: bool) -> Value {
    let text = if result.get("output").and_then(Value::as_str).is_some()
        && result.get("content").is_none()
    {
        // Direct Pi `bash` RPC response: { output, exitCode, ... }.
        result
            .get("output")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    } else {
        pi_result_text(result)
    };

    let exit_code = result
        .get("exitCode")
        .and_then(Value::as_i64)
        .or_else(|| {
            result
                .get("details")
                .and_then(|details| details.get("exitCode"))
                .and_then(Value::as_i64)
        })
        .unwrap_or(if is_error { 1 } else { 0 });

    let truncated = result
        .get("truncated")
        .and_then(Value::as_bool)
        .or_else(|| {
            result
                .pointer("/details/truncation/truncated")
                .and_then(Value::as_bool)
        })
        .unwrap_or(false);

    let output_file = result
        .get("fullOutputPath")
        .and_then(Value::as_str)
        .or_else(|| {
            result
                .pointer("/details/fullOutputPath")
                .and_then(Value::as_str)
        })
        .unwrap_or("")
        .to_string();

    let bytes = text.as_bytes().to_vec();
    let total_bytes = bytes.len();
    json!({
        "type": "Bash",
        "output": bytes,
        "output_for_prompt": text,
        "exit_code": exit_code,
        "command": command,
        "truncated": truncated,
        "signal": null,
        "timed_out": false,
        "description": null,
        "current_dir": "",
        "output_file": output_file,
        "total_bytes": total_bytes,
        "was_bare_echo": false,
    })
}

fn direct_bash_command(blocks: &[acp::ContentBlock]) -> Option<String> {
    blocks.iter().find_map(|block| {
        let acp::ContentBlock::Text(text) = block else {
            return None;
        };
        text.meta
            .as_ref()
            .and_then(|meta| meta.get("bash_command"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|command| !command.is_empty())
            .map(ToOwned::to_owned)
    })
}

/// Stamp client `promptId` on PromptResponse so the pager can discard queued
/// mid-turn RPC completions that never became `current_prompt_id`.
fn prompt_response(
    reason: acp::StopReason,
    client_prompt_id: Option<&str>,
) -> acp::PromptResponse {
    let mut response = acp::PromptResponse::new(reason);
    if let Some(prompt_id) = client_prompt_id.filter(|id| !id.is_empty()) {
        let mut meta = acp::Meta::new();
        meta.insert("promptId".into(), Value::String(prompt_id.to_string()));
        response = response.meta(Some(meta));
    }
    response
}

fn prompt_streaming_behavior(
    already_active: bool,
    meta: Option<&acp::Meta>,
) -> Option<&'static str> {
    if !already_active {
        return None;
    }
    // Cancel-and-send / send-now is an interrupt (steer).
    if meta
        .and_then(|meta| meta.get("sendNow"))
        .and_then(Value::as_bool)
        == Some(true)
    {
        return Some("steer");
    }
    // Explicit followUp meta wins; otherwise mid-turn prompts queue as follow-up
    // (FEATURE_MATRIX: default active-turn prompt → Pi follow_up).
    if meta
        .and_then(|meta| meta.get("followUp"))
        .and_then(Value::as_bool)
        == Some(false)
    {
        return Some("steer");
    }
    Some("followUp")
}

fn queue_lane_for_behavior(behavior: &str) -> Option<QueueLane> {
    match behavior {
        "steer" => Some(QueueLane::Steering),
        "followUp" => Some(QueueLane::FollowUp),
        _ => None,
    }
}

fn format_bash_result(result: &Value) -> String {
    let mut text = result
        .get("output")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let mut notes = Vec::new();
    if result.get("cancelled").and_then(Value::as_bool) == Some(true) {
        notes.push("Command cancelled".to_string());
    } else if let Some(exit_code) = result.get("exitCode").and_then(Value::as_i64) {
        notes.push(format!("Exit code: {exit_code}"));
    }
    if result.get("truncated").and_then(Value::as_bool) == Some(true) {
        let suffix = result
            .get("fullOutputPath")
            .and_then(Value::as_str)
            .map(|path| format!("Output truncated; full output: {path}"))
            .unwrap_or_else(|| "Output truncated".to_string());
        notes.push(suffix);
    }
    if !notes.is_empty() {
        if !text.is_empty() && !text.ends_with('\n') {
            text.push('\n');
        }
        text.push_str(&notes.join("\n"));
    }
    if text.is_empty() {
        "Command completed with no output".to_string()
    } else {
        text
    }
}

fn prompt_to_pi(blocks: &[acp::ContentBlock]) -> (String, Vec<Value>) {
    let mut parts = Vec::new();
    let mut images = Vec::new();
    for block in blocks {
        match block {
            acp::ContentBlock::Text(text) => parts.push(text.text.clone()),
            acp::ContentBlock::Image(image) => images.push(json!({
                "type": "image",
                "data": image.data,
                "mimeType": image.mime_type,
            })),
            acp::ContentBlock::ResourceLink(link) => {
                parts.push(format!("[resource] {}", link.uri));
            }
            acp::ContentBlock::Resource(resource) => {
                parts.push(json_text(
                    &serde_json::to_value(resource).unwrap_or(Value::Null),
                ));
            }
            _ => {}
        }
    }
    (parts.join("\n\n"), images)
}

fn message_role(event: &Value) -> Option<&str> {
    event
        .get("message")
        .and_then(|message| string(message, &["role", "type"]))
}

fn model_reasoning_efforts(model: &PiModel) -> Vec<Value> {
    let mut efforts = Vec::new();
    for level in &model.thinking_levels {
        let entry = match level.as_str() {
            "off" => Some(json!({ "id": "off", "value": "none", "label": "Off" })),
            "minimal" => Some(json!({ "id": "minimal", "value": "minimal", "label": "Minimal" })),
            "low" => Some(json!({ "id": "low", "value": "low", "label": "Low" })),
            "medium" => Some(json!({ "id": "medium", "value": "medium", "label": "Medium" })),
            "high" => Some(json!({ "id": "high", "value": "high", "label": "High" })),
            "xhigh" | "max" => {
                if efforts.iter().any(|value: &Value| {
                    value.get("value").and_then(Value::as_str) == Some("xhigh")
                }) {
                    None
                } else {
                    Some(json!({ "id": "xhigh", "value": "xhigh", "label": "Extra high" }))
                }
            }
            _ => None,
        };
        if let Some(entry) = entry {
            efforts.push(entry);
        }
    }
    efforts
}

fn pi_effort_to_acp(level: &str) -> &str {
    match level.to_ascii_lowercase().as_str() {
        "off" | "none" => "none",
        "minimal" => "minimal",
        "low" => "low",
        "high" => "high",
        "xhigh" | "max" => "xhigh",
        _ => "medium",
    }
}

fn extension_tool_call_id(id: &Value) -> String {
    let id = id
        .as_str()
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| id.to_string());
    format!("pi-extension-ui:{id}")
}

fn extension_dialog_timeout(event: &Value) -> Option<Duration> {
    event
        .get("timeout")
        .and_then(Value::as_u64)
        .filter(|milliseconds| *milliseconds > 0)
        .map(Duration::from_millis)
}

fn selected_answer(value: &Value) -> Option<String> {
    let answers = value.get("answers").and_then(Value::as_object)?;
    for answer in answers.values() {
        if let Some(text) = answer.as_str() {
            return Some(text.to_string());
        }
        if let Some(text) = answer
            .as_array()
            .and_then(|items| items.first())
            .and_then(Value::as_str)
        {
            return Some(text.to_string());
        }
    }
    None
}

fn annotated_answer(value: &Value) -> Option<String> {
    let annotations = value.get("annotations").and_then(Value::as_object)?;
    for annotation in annotations.values() {
        if let Some(notes) = annotation.get("notes").and_then(Value::as_str) {
            return Some(notes.to_string());
        }
    }
    None
}

/// Translate Grok QuestionView's response into the value Pi expects.
///
/// Freeform rows are represented by the native question component as the
/// selected option `Other`, with the actual editor text under
/// `annotations.<question>.notes`. Pi input/editor must therefore prefer notes;
/// select/confirm must prefer the selected option.
fn extension_answer(method: &str, value: &Value) -> Option<String> {
    let direct = || {
        value
            .get("value")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
    };
    match method {
        "input" | "editor" => annotated_answer(value)
            .or_else(|| selected_answer(value))
            .or_else(direct),
        _ => selected_answer(value)
            .or_else(|| annotated_answer(value))
            .or_else(direct),
    }
}

fn ext_response(value: Value) -> Result<acp::ExtResponse> {
    let raw = serde_json::value::to_raw_value(&json!({ "result": value }))?;
    Ok(acp::ExtResponse::new(raw.into()))
}

fn acp_internal(error: impl std::fmt::Display) -> acp::Error {
    acp::Error::internal_error().data(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_file_discovers_a_settings_configured_session_directory() {
        let fallback = Path::new("/home/user/.pi/agent/sessions");
        let state = PiState {
            session_file: Some("/data/pi-sessions/current.jsonl".to_string()),
            ..PiState::default()
        };
        assert_eq!(
            catalog_session_dir(&state, fallback),
            PathBuf::from("/data/pi-sessions")
        );

        let default_state = PiState {
            session_file: Some("/home/user/.pi/agent/sessions/project/current.jsonl".to_string()),
            ..PiState::default()
        };
        assert_eq!(catalog_session_dir(&default_state, fallback), fallback);
    }

    #[test]
    fn command_catalog_is_pi_owned_and_deduplicated() {
        let commands = vec![
            PiCommand {
                name: "/review".into(),
                description: "Review changes".into(),
                source: "extension".into(),
            },
            PiCommand {
                name: "REVIEW".into(),
                description: "Duplicate".into(),
                source: "prompt".into(),
            },
            PiCommand {
                name: "brief".into(),
                description: String::new(),
                source: "skill".into(),
            },
            PiCommand {
                name: NAVIGATE_TREE_COMMAND.into(),
                description: "internal".into(),
                source: "extension".into(),
            },
            PiCommand {
                name: LABEL_TREE_COMMAND.into(),
                description: "internal".into(),
                source: "extension".into(),
            },
        ];
        let serialized = serde_json::to_value(command_catalog(&commands)).unwrap();
        let text = serialized.to_string();
        assert_eq!(text.matches("review").count(), 1);
        assert!(text.contains("Review changes"));
        assert!(text.contains("brief"));
        assert!(text.contains("Pi skill command"));
        assert!(!text.contains(NAVIGATE_TREE_COMMAND));
        assert!(!text.contains(LABEL_TREE_COMMAND));
    }

    #[test]
    fn grok_direct_bash_meta_maps_to_pi_bash() {
        let mut meta = acp::Meta::new();
        meta.insert("bash_command".into(), json!("git status"));
        let blocks = vec![acp::ContentBlock::Text(
            acp::TextContent::new("!git status").meta(Some(meta)),
        )];
        assert_eq!(direct_bash_command(&blocks).as_deref(), Some("git status"));
    }

    #[test]
    fn prompt_response_echoes_client_prompt_id() {
        let response = prompt_response(acp::StopReason::EndTurn, Some("client-uuid"));
        assert_eq!(response.stop_reason, acp::StopReason::EndTurn);
        assert_eq!(
            response
                .meta
                .as_ref()
                .and_then(|meta| meta.get("promptId"))
                .and_then(Value::as_str),
            Some("client-uuid")
        );
        let bare = prompt_response(acp::StopReason::Cancelled, None);
        assert!(bare.meta.is_none());
    }

    #[test]
    fn pi_tui_queue_modes_map_to_pi_streaming_behavior() {
        assert_eq!(prompt_streaming_behavior(false, None), None);
        // Mid-turn default is follow-up (wait for turn), not steer.
        assert_eq!(prompt_streaming_behavior(true, None), Some("followUp"));

        let mut meta = acp::Meta::new();
        meta.insert("followUp".into(), Value::Bool(true));
        assert_eq!(
            prompt_streaming_behavior(true, Some(&meta)),
            Some("followUp")
        );

        let mut send_now = acp::Meta::new();
        send_now.insert("sendNow".into(), Value::Bool(true));
        assert_eq!(
            prompt_streaming_behavior(true, Some(&send_now)),
            Some("steer")
        );

        let mut force_steer = acp::Meta::new();
        force_steer.insert("followUp".into(), Value::Bool(false));
        assert_eq!(
            prompt_streaming_behavior(true, Some(&force_steer)),
            Some("steer")
        );
    }

    #[test]
    fn pi_read_maps_to_native_read_card() {
        assert_eq!(tool_kind("read"), acp::ToolKind::Read);
        assert_eq!(tool_kind("use_skill"), acp::ToolKind::Other);
    }

    #[test]
    fn bash_result_is_presented_in_native_tool_card_text() {
        let text = format_bash_result(&json!({
            "output": "ok",
            "exitCode": 0,
            "cancelled": false,
            "truncated": true,
            "fullOutputPath": "/tmp/pi-bash.log",
        }));
        assert!(text.contains("ok"));
        assert!(text.contains("Exit code: 0"));
        assert!(text.contains("/tmp/pi-bash.log"));
    }

    #[test]
    fn pi_read_result_projects_native_readfile_raw_output() {
        let raw = normalize_tool_raw_output(
            "read",
            Some(&json!({ "path": "src/lib.rs", "offset": 10, "limit": 20 })),
            &json!({
                "content": [{ "type": "text", "text": "fn main() {}\n// end\n\n[Showing lines 10-11 of 42. Use offset=12 to continue.]" }],
            }),
            false,
        );
        assert_eq!(raw.get("type").and_then(Value::as_str), Some("ReadFile"));
        let file = raw.get("FileContent").expect("FileContent variant");
        assert_eq!(
            file.get("absolute_path").and_then(Value::as_str),
            Some("src/lib.rs")
        );
        assert_eq!(file.get("offset").and_then(Value::as_u64), Some(9));
        assert_eq!(file.get("limit").and_then(Value::as_u64), Some(20));
        assert_eq!(file.get("total_lines").and_then(Value::as_u64), Some(42));
        assert!(
            file.get("raw_output")
                .and_then(Value::as_str)
                .is_some_and(|text| text.contains("fn main()"))
        );
    }

    #[test]
    fn context_tokens_prefer_session_stats_and_usage_total() {
        assert_eq!(
            context_tokens_from_stats(&json!({
                "contextUsage": { "tokens": 60_000, "contextWindow": 200_000, "percent": 30.0 }
            })),
            Some(60_000)
        );
        assert_eq!(
            context_tokens_from_stats(&json!({
                "contextUsage": { "tokens": null, "contextWindow": 200_000, "percent": null }
            })),
            None
        );
        assert_eq!(
            context_tokens_from_usage(&json!({
                "totalTokens": 12_345,
                "input": 1,
                "output": 2,
            })),
            Some(12_345)
        );
        assert_eq!(
            context_tokens_from_usage(&json!({
                "input": 100,
                "output": 50,
                "cacheRead": 20,
                "cacheWrite": 5,
            })),
            Some(175)
        );
    }

    #[test]
    fn session_info_maps_pi_stats_into_native_context_shape() {
        let model = PiModel {
            provider: "openai".into(),
            id: "gpt-test".into(),
            label: "GPT Test".into(),
            context_window: Some(200_000),
            reasoning: false,
            accepts_images: false,
            thinking_levels: vec!["off".into()],
        };
        let stats = json!({
            "sessionId": "sess-1",
            "userMessages": 3,
            "assistantMessages": 2,
            "toolCalls": 4,
            "toolResults": 4,
            "totalMessages": 9,
            "contextUsage": { "tokens": 10_000, "contextWindow": 200_000, "percent": 5.0 }
        });
        let messages = json!({
            "messages": [
                { "role": "user", "content": "hello world" },
                {
                    "role": "assistant",
                    "content": [
                        { "type": "text", "text": "thinking out loud" },
                        {
                            "type": "toolCall",
                            "id": "t1",
                            "name": "read",
                            "arguments": { "path": "a.rs" }
                        }
                    ]
                },
                {
                    "role": "toolResult",
                    "toolCallId": "t1",
                    "content": [{ "type": "text", "text": "fn main() {}" }]
                }
            ]
        });
        let response = build_session_info_response(
            &stats,
            Some(&messages),
            "sess-1",
            "/repo",
            Some(&model),
            None,
        );
        assert_eq!(response["sessionId"], json!("sess-1"));
        assert_eq!(response["cwd"], json!("/repo"));
        assert_eq!(response["model"], json!("gpt-test"));
        assert_eq!(response["modelDisplayName"], json!("GPT Test"));
        assert_eq!(response["agentName"], json!("pi"));
        assert_eq!(response["turns"], json!(3));
        assert_eq!(response["context"]["used"], json!(10_000));
        assert_eq!(response["context"]["total"], json!(200_000));
        assert_eq!(response["context"]["freeTokens"], json!(190_000));
        assert_eq!(response["context"]["usagePct"], json!(5));
        assert_eq!(response["context"]["toolCallCount"], json!(4));
        assert_eq!(response["context"]["messageCount"], json!(9));
        assert_eq!(response["context"]["turnCount"], json!(3));
        // Message bucket must be non-zero when text is present and scales into used.
        assert!(response["context"]["messageTokens"].as_u64().unwrap_or(0) > 0);
        let message_tokens = response["context"]["messageTokens"].as_u64().unwrap();
        assert!(message_tokens <= 10_000);
        // system stays 0 without RPC access; overhead fills the rest of used.
        assert_eq!(response["context"]["systemPromptTokens"], json!(0));
    }

    #[test]
    fn session_info_falls_back_to_cached_tokens_when_stats_null() {
        let stats = json!({
            "userMessages": 1,
            "toolCalls": 0,
            "totalMessages": 1,
            "contextUsage": { "tokens": null, "contextWindow": 100_000, "percent": null }
        });
        let response = build_session_info_response(
            &stats,
            None,
            "sess-2",
            "/tmp",
            None,
            Some(42_000),
        );
        assert_eq!(response["context"]["used"], json!(42_000));
        assert_eq!(response["context"]["total"], json!(100_000));
        assert_eq!(response["context"]["freeTokens"], json!(58_000));
        // No message estimate → put the full used window into Messages.
        assert_eq!(response["context"]["messageTokens"], json!(42_000));
        assert!(response.get("model").is_none());
        assert!(response.get("resolvedModelId").is_none());
    }

    #[test]
    fn entries_payload_projects_message_roles() {
        let entries = json!({
            "entries": [
                { "type": "message", "message": { "role": "user", "content": "hi there" } },
                {
                    "type": "message",
                    "message": {
                        "role": "assistant",
                        "content": [{ "type": "text", "text": "hello" }]
                    }
                }
            ]
        });
        let messages = entries_to_messages_value(entries);
        let estimate = estimate_message_tokens(&messages);
        assert!(estimate.messages > 0);
    }

    #[test]
    fn scale_token_parts_preserves_used_total() {
        assert_eq!(scale_token_parts(&[75, 25], 1_000), vec![750, 250]);
        let scaled = scale_token_parts(&[1, 1, 1], 10);
        assert_eq!(scaled.iter().sum::<u64>(), 10);
    }

    #[test]
    fn pi_bash_result_projects_native_bash_raw_output() {
        let raw = normalize_tool_raw_output(
            "bash",
            Some(&json!({ "command": "ls -la" })),
            &json!({
                "content": [{ "type": "text", "text": "total 48\nREADME.md\n" }],
                "details": { "fullOutputPath": null },
            }),
            false,
        );
        assert_eq!(raw.get("type").and_then(Value::as_str), Some("Bash"));
        assert_eq!(raw.get("command").and_then(Value::as_str), Some("ls -la"));
        assert_eq!(raw.get("exit_code").and_then(Value::as_i64), Some(0));
        let output = raw
            .get("output_for_prompt")
            .and_then(Value::as_str)
            .unwrap_or_default();
        assert!(output.contains("README.md"));

        let direct = bash_tool_output(
            "echo hi",
            &json!({
                "output": "hi\n",
                "exitCode": 0,
                "truncated": false,
            }),
            false,
        );
        assert_eq!(direct.get("type").and_then(Value::as_str), Some("Bash"));
        assert_eq!(
            direct.get("output_for_prompt").and_then(Value::as_str),
            Some("hi\n")
        );
    }

    #[test]
    fn pi_edit_and_write_inputs_produce_native_diff_content() {
        let edit = edit_diff_content(
            "edit",
            Some(&json!({
                "path": "README.md",
                "oldText": "before\n",
                "newText": "after\n",
            })),
        )
        .expect("edit input must become a diff");
        let acp::ToolCallContent::Diff(diff) = &edit[0] else {
            panic!("edit input must produce ACP Diff content");
        };
        assert_eq!(diff.path.to_string_lossy(), "README.md");
        assert_eq!(diff.old_text.as_deref(), Some("before\n"));
        assert_eq!(diff.new_text, "after\n");

        let current_edit = edit_diff_content(
            "edit",
            Some(&json!({
                "path": "README.md",
                "edits": [
                    { "oldText": "before\n", "newText": "after\n" },
                    { "oldText": "first\n", "newText": "second\n" },
                ],
            })),
        )
        .expect("current edit input must become diffs");
        assert_eq!(current_edit.len(), 2);

        let write = edit_diff_content(
            "write",
            Some(&json!({ "path": "README.md", "content": "new file\n" })),
        )
        .expect("write input must become a diff");
        let acp::ToolCallContent::Diff(diff) = &write[0] else {
            panic!("write input must produce ACP Diff content");
        };
        assert_eq!(diff.old_text, None);
        assert_eq!(diff.new_text, "new file\n");
    }

    #[test]
    fn pi_builtin_tool_kinds() {
        assert_eq!(tool_kind("read"), acp::ToolKind::Read);
        assert_eq!(tool_kind("bash"), acp::ToolKind::Execute);
        assert_eq!(tool_kind("edit"), acp::ToolKind::Edit);
        assert_eq!(tool_kind("write"), acp::ToolKind::Edit);
        assert_eq!(tool_kind("grep"), acp::ToolKind::Search);
        assert_eq!(tool_kind("find"), acp::ToolKind::Search);
        assert_eq!(tool_kind("ls"), acp::ToolKind::Other);
    }

    #[test]
    fn pi_write_raw_input_gets_write_variant() {
        let args = normalize_tool_raw_input(
            "write",
            Some(json!({ "path": "a.rs", "content": "x" })),
        )
        .unwrap();
        assert_eq!(args.get("variant").and_then(Value::as_str), Some("Write"));
    }

    #[test]
    fn pi_ls_raw_input_gets_target_directory() {
        let args =
            normalize_tool_raw_input("ls", Some(json!({ "path": "src" }))).unwrap();
        assert_eq!(
            args.get("target_directory").and_then(Value::as_str),
            Some("src")
        );
    }

    #[test]
    fn pi_grep_result_projects_native_grepsearch() {
        let raw = normalize_tool_raw_output(
            "grep",
            Some(&json!({ "pattern": "fn main", "path": "." })),
            &json!({
                "content": [{
                    "type": "text",
                    "text": "src/main.rs:10: fn main() {\nsrc/lib.rs:3: fn main_helper() {\n"
                }],
            }),
            false,
        );
        assert_eq!(raw.get("type").and_then(Value::as_str), Some("GrepSearch"));
        assert_eq!(raw.get("match_count").and_then(Value::as_u64), Some(2));
        let files = raw.get("file_matches").and_then(Value::as_array).unwrap();
        assert_eq!(files.len(), 2);
        assert_eq!(
            files[0].get("path").and_then(Value::as_str),
            Some("src/main.rs")
        );
        assert_eq!(
            files[0]
                .get("matches")
                .and_then(Value::as_array)
                .unwrap()[0]
                .get("line_number")
                .and_then(Value::as_u64),
            Some(10)
        );
    }

    #[test]
    fn pi_find_result_projects_files_with_matches() {
        let raw = normalize_tool_raw_output(
            "find",
            Some(&json!({ "pattern": "*.rs" })),
            &json!({
                "content": [{ "type": "text", "text": "src/a.rs\nsrc/b.rs\n" }],
            }),
            false,
        );
        assert_eq!(raw.get("type").and_then(Value::as_str), Some("GrepSearch"));
        assert_eq!(raw.get("match_count").and_then(Value::as_u64), Some(2));
    }

    #[test]
    fn pi_ls_result_projects_native_listdir() {
        let raw = normalize_tool_raw_output(
            "ls",
            Some(&json!({ "path": "src" })),
            &json!({
                "content": [{ "type": "text", "text": "main.rs\nlib.rs\n" }],
            }),
            false,
        );
        assert_eq!(raw.get("type").and_then(Value::as_str), Some("ListDir"));
        let content = raw.get("Content").expect("ListDir Content");
        assert_eq!(
            content.get("absolute_root_path").and_then(Value::as_str),
            Some("src")
        );
        assert!(
            content
                .get("content")
                .and_then(Value::as_str)
                .is_some_and(|t| t.contains("main.rs"))
        );
    }

    #[test]
    fn pi_grep_match_line_parser() {
        let (path, line, content) =
            split_pi_grep_match_line("crates/foo/bar.rs:42: let x = 1;").unwrap();
        assert_eq!(path, "crates/foo/bar.rs");
        assert_eq!(line, 42);
        assert_eq!(content, "let x = 1;");
        assert!(split_pi_grep_match_line("crates/foo/bar.rs-42- context").is_none());
    }

    #[test]
    fn pi_input_and_editor_prefer_native_freeform_annotations() {
        let result = json!({
            "answers": { "pi-question": ["Other"] },
            "annotations": { "pi-question": { "notes": "typed in Grok PromptWidget" } },
            "value": "fallback",
        });
        assert_eq!(
            extension_answer("input", &result).as_deref(),
            Some("typed in Grok PromptWidget")
        );
        assert_eq!(
            extension_answer("editor", &result).as_deref(),
            Some("typed in Grok PromptWidget")
        );
    }

    #[test]
    fn pi_select_and_confirm_prefer_native_selected_option() {
        let result = json!({
            "answers": { "pi-question": ["Yes"] },
            "annotations": { "pi-question": { "notes": "ignored freeform" } },
            "value": "fallback",
        });
        assert_eq!(extension_answer("select", &result).as_deref(), Some("Yes"));
        assert_eq!(extension_answer("confirm", &result).as_deref(), Some("Yes"));
    }

    #[test]
    fn pi_extension_timeout_is_milliseconds_and_zero_means_no_timeout() {
        assert_eq!(
            extension_dialog_timeout(&json!({ "timeout": 2500 })),
            Some(Duration::from_millis(2500))
        );
        assert_eq!(extension_dialog_timeout(&json!({ "timeout": 0 })), None);
        assert_eq!(extension_dialog_timeout(&json!({})), None);
    }

    #[test]
    fn extension_tool_call_ids_are_stable_and_namespaced() {
        assert_eq!(
            extension_tool_call_id(&json!("dialog-7")),
            "pi-extension-ui:dialog-7"
        );
        assert_eq!(extension_tool_call_id(&json!(17)), "pi-extension-ui:17");
    }
}
