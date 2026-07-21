//! grok-pi resource admission policy.
//!
//! Evaluates the [`PiResourceCatalog`] discovered by [`crate::pi_resource_config`]
//! and produces a [`PiLaunchPlan`] containing only the resources that should be
//! passed to the Pi RPC child process.  Resources that conflict with the Grok
//! Pager renderer (tool renderers, TUI header/footer overrides, raw terminal
//! hooks) are blocked by default; users can override via an explicit allowlist.
//!
//! The policy does **not** re-implement Pi's resource discovery or loading.
//! It only decides *which already-discovered resource paths* are forwarded to
//! Pi via `--extension` / `--skill` / `--prompt-template` / `--theme` flags
//! after the corresponding `--no-*` discovery flags.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::pi_resource_config::{
    PiResource, PiResourceCatalog, PiResourceType,
};

// ── Default blocklist ────────────────────────────────────────────────────────

/// Package sources that are unconditionally blocked unless the user
/// explicitly allows them.  These are known to conflict with the Grok Pager
/// renderer or RPC transport.
pub(crate) const DEFAULT_BLOCKED_SOURCES: &[&str] = &[
    // Re-registers all seven built-in tools with Pi-TUI renderers and
    // monkey-patches `pi.registerTool`, breaking the RPC tool pipeline.
    "npm:pi-tool-display",
    // Pi-TUI-only footer/header overlays; no value under Pager.
    "npm:pi-custom-header",
    "npm:pi-custom-footer",
];

/// File-name fragments that indicate a Pi-TUI renderer extension.
/// Matched case-insensitively against the resource file name.
const RENDERER_NAME_HINTS: &[&str] = &[
    "tool-display",
    "custom-header",
    "custom-footer",
];

// ── Policy configuration ─────────────────────────────────────────────────────

/// User-facing policy knobs, persisted under `[pi.resources]` in
/// `~/.grok/config.toml`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ResourcePolicy {
    /// Package sources or path fragments the user explicitly allows even if
    /// they would otherwise be blocked.  Takes precedence over the default
    /// blocklist and renderer heuristics.
    pub allow: Vec<String>,
    /// Additional package sources or path fragments the user wants blocked
    /// beyond the built-in defaults.
    pub block: Vec<String>,
    /// When `true`, skip the renderer-name heuristic and only apply the
    /// explicit source lists.  Useful for power users who want full control.
    pub disable_heuristics: bool,
}

impl ResourcePolicy {
    /// Load the policy from `~/.grok/config.toml` (`[pi.resources]`).
    /// Returns `Default` when the file or table is missing.
    pub fn load_from_config() -> Self {
        let path = xai_grok_tools::util::grok_home::grok_home().join("config.toml");
        Self::load_from_path(&path)
    }

    /// Path-injectable variant for testing.
    pub fn load_from_path(path: &Path) -> Self {
        let Ok(content) = std::fs::read_to_string(path) else {
            return Self::default();
        };
        let Ok(doc) = toml::from_str::<toml::Value>(&content) else {
            return Self::default();
        };
        doc.get("pi")
            .and_then(|pi| pi.get("resources"))
            .and_then(|res| res.clone().try_into().ok())
            .unwrap_or_default()
    }

    /// Persist the policy to `~/.grok/config.toml` under `[pi.resources]`,
    /// preserving all other tables and keys.
    pub fn save_to_config(&self) -> std::io::Result<()> {
        let path = xai_grok_tools::util::grok_home::grok_home().join("config.toml");
        self.save_to_path(&path)
    }

    /// Path-injectable variant for testing.
    pub fn save_to_path(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = std::fs::read_to_string(path).unwrap_or_default();
        let mut doc: toml_edit::DocumentMut = content
            .parse()
            .unwrap_or_else(|_| toml_edit::DocumentMut::new());

        let table = doc
            .entry("pi")
            .or_insert(toml_edit::Item::Table(toml_edit::Table::new()))
            .as_table_mut()
            .expect("[pi] must be a table");
        let resources = table
            .entry("resources")
            .or_insert(toml_edit::Item::Table(toml_edit::Table::new()))
            .as_table_mut()
            .expect("[pi.resources] must be a table");

        resources["allow"] = toml_edit::value(
            self.allow
                .iter()
                .map(|s| toml_edit::Value::from(s.as_str()))
                .collect::<toml_edit::Array>(),
        );
        resources["block"] = toml_edit::value(
            self.block
                .iter()
                .map(|s| toml_edit::Value::from(s.as_str()))
                .collect::<toml_edit::Array>(),
        );
        resources["disable_heuristics"] = toml_edit::value(self.disable_heuristics);

        std::fs::write(path, doc.to_string())
    }
}

impl ResourcePolicy {
    /// Evaluate the catalog and return the launch plan.
    pub fn evaluate(&self, catalog: &PiResourceCatalog) -> PiLaunchPlan {
        let mut plan = PiLaunchPlan::default();

        for resource in &catalog.resources {
            let decision = self.decide(resource);
            match decision {
                Decision::Allow => plan.add(resource),
                Decision::Block { reason } => {
                    plan.blocked.push(BlockedResource {
                        path: resource.path.clone(),
                        source: resource.source.clone(),
                        resource_type: resource.resource_type,
                        reason,
                    });
                }
            }
        }

        plan
    }

    fn decide(&self, resource: &PiResource) -> Decision {
        // 1. Pi owns its own enable/disable settings. A resource disabled in
        //    Pi settings is never loaded, regardless of host policy.
        if !resource.enabled {
            return Decision::Block {
                reason: "disabled in Pi settings".to_owned(),
            };
        }

        // 2. User explicit allow overrides the *host* blocklist and
        //    heuristics only — it cannot re-enable Pi-disabled resources.
        if self.matches_any(&resource.source, &resource.path, &self.allow) {
            return Decision::Allow;
        }

        // 3. User explicit block.
        if self.matches_any(&resource.source, &resource.path, &self.block) {
            return Decision::Block {
                reason: "blocked by user policy".to_owned(),
            };
        }

        // 4. Built-in default blocklist (source match).
        for blocked in DEFAULT_BLOCKED_SOURCES {
            if resource.source == *blocked {
                return Decision::Block {
                    reason: format!(
                        "default-blocked: {} conflicts with Grok Pager renderer",
                        blocked
                    ),
                };
            }
        }

        // 5. Renderer name heuristic (unless disabled).
        if !self.disable_heuristics {
            let file_name = resource
                .path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default()
                .to_lowercase();
            let parent_name = resource
                .path
                .parent()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                .unwrap_or_default()
                .to_lowercase();
            for hint in RENDERER_NAME_HINTS {
                if file_name.contains(hint) || parent_name.contains(hint) {
                    return Decision::Block {
                        reason: format!(
                            "heuristic: '{}' looks like a Pi-TUI renderer (matches '{}')",
                            resource.display_name(),
                            hint
                        ),
                    };
                }
            }
        }

        Decision::Allow
    }

    /// Check whether an explicit CLI path (e.g. from `grok-pi -e <path>`)
    /// should be blocked by the host policy.  Returns `Some(reason)` if the
    /// path is blocked, `None` if it is allowed.
    ///
    /// This is used to filter user-supplied `--extension` paths that were
    /// already written into `pi_args` before the catalog-based evaluation.
    pub fn check_explicit_path(&self, path: &str) -> Option<String> {
        let path_buf = Path::new(path);

        // User explicit allow always wins for explicit paths.
        if self.matches_any(path, path_buf, &self.allow) {
            return None;
        }

        // User explicit block.
        if self.matches_any(path, path_buf, &self.block) {
            return Some("blocked by user policy".to_owned());
        }

        // Default blocklist: match against known conflicting package names.
        for blocked in DEFAULT_BLOCKED_SOURCES {
            let pkg_name = blocked
                .strip_prefix("npm:")
                .unwrap_or(blocked);
            if path.contains(pkg_name) {
                return Some(format!(
                    "default-blocked: {} conflicts with Grok Pager renderer",
                    blocked
                ));
            }
        }

        // Renderer name heuristic.
        if !self.disable_heuristics {
            let file_name = path_buf
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default()
                .to_lowercase();
            let parent_name = path_buf
                .parent()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                .unwrap_or_default()
                .to_lowercase();
            for hint in RENDERER_NAME_HINTS {
                if file_name.contains(hint) || parent_name.contains(hint) {
                    return Some(format!(
                        "heuristic: path looks like a Pi-TUI renderer (matches '{}')",
                        hint
                    ));
                }
            }
        }

        None
    }

    fn matches_any(&self, source: &str, path: &Path, patterns: &[String]) -> bool {
        let path_str = path.to_string_lossy();
        patterns.iter().any(|pattern| {
            source == pattern.as_str()
                || path_str.contains(pattern.as_str())
                || path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|name| name == pattern.as_str())
        })
    }
}

#[derive(Debug)]
enum Decision {
    Allow,
    Block { reason: String },
}

// ── Launch plan ──────────────────────────────────────────────────────────────

/// The concrete set of CLI arguments to pass to the Pi RPC child process.
#[derive(Debug, Clone, Default)]
pub struct PiLaunchPlan {
    /// Approved extension paths.
    pub extensions: Vec<PathBuf>,
    /// Approved skill paths.
    pub skills: Vec<PathBuf>,
    /// Approved prompt template paths.
    pub prompts: Vec<PathBuf>,
    /// Approved theme paths.
    pub themes: Vec<PathBuf>,
    /// Resources that were blocked, with reasons (for diagnostics / UI).
    pub blocked: Vec<BlockedResource>,
}

impl PiLaunchPlan {
    fn add(&mut self, resource: &PiResource) {
        match resource.resource_type {
            PiResourceType::Extensions => self.extensions.push(resource.path.clone()),
            PiResourceType::Skills => self.skills.push(resource.path.clone()),
            PiResourceType::Prompts => self.prompts.push(resource.path.clone()),
            PiResourceType::Themes => self.themes.push(resource.path.clone()),
        }
    }

    /// Append `--no-*` discovery flags and explicit resource paths to `args`.
    ///
    /// Bridge extensions injected by grok-pi (subagent, bash, recap, etc.)
    /// are **not** part of this plan; the caller appends them separately
    /// after this method so they always load regardless of policy.
    pub fn append_args(&self, args: &mut Vec<String>) {
        // Disable Pi's own discovery; we supply everything explicitly.
        args.push("--no-extensions".to_string());
        args.push("--no-skills".to_string());
        args.push("--no-prompt-templates".to_string());
        args.push("--no-themes".to_string());

        for path in &self.extensions {
            args.extend([
                "--extension".to_string(),
                path.to_string_lossy().into_owned(),
            ]);
        }
        for path in &self.skills {
            args.extend([
                "--skill".to_string(),
                path.to_string_lossy().into_owned(),
            ]);
        }
        for path in &self.prompts {
            args.extend([
                "--prompt-template".to_string(),
                path.to_string_lossy().into_owned(),
            ]);
        }
        for path in &self.themes {
            args.extend([
                "--theme".to_string(),
                path.to_string_lossy().into_owned(),
            ]);
        }
    }

    /// Human-readable summary of blocked resources for startup diagnostics.
    pub fn blocked_summary(&self) -> Option<String> {
        if self.blocked.is_empty() {
            return None;
        }
        let mut lines = Vec::new();
        for blocked in &self.blocked {
            lines.push(format!(
                "  • {} ({}) — {}",
                blocked.path.display(),
                blocked.source,
                blocked.reason,
            ));
        }
        Some(format!(
            "grok-pi resource policy blocked {} resource(s):\n{}",
            self.blocked.len(),
            lines.join("\n")
        ))
    }
}

/// A resource that was prevented from loading.
#[derive(Debug, Clone)]
pub struct BlockedResource {
    pub path: PathBuf,
    pub source: String,
    pub resource_type: PiResourceType,
    pub reason: String,
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pi_resource_config::{
        PiProjectOverride, PiResourceOrigin, PiResourceScope,
    };

    fn make_resource(
        path: &str,
        source: &str,
        resource_type: PiResourceType,
        enabled: bool,
    ) -> PiResource {
        PiResource {
            path: PathBuf::from(path),
            resource_type,
            scope: PiResourceScope::User,
            origin: PiResourceOrigin::Package,
            source: source.to_owned(),
            base_dir: PathBuf::from("/home/user/.pi/agent"),
            enabled,
            inherited_enabled: enabled,
            project_override: PiProjectOverride::Inherit,
        }
    }

    fn make_catalog(resources: Vec<PiResource>) -> PiResourceCatalog {
        PiResourceCatalog {
            resources,
            project_trusted: false,
            agent_dir: PathBuf::from("/home/user/.pi/agent"),
            cwd: PathBuf::from("/home/user/project"),
        }
    }

    #[test]
    fn pi_tool_display_blocked_by_default() {
        let catalog = make_catalog(vec![
            make_resource(
                "/home/user/.pi/agent/npm/node_modules/pi-tool-display/index.ts",
                "npm:pi-tool-display",
                PiResourceType::Extensions,
                true,
            ),
            make_resource(
                "/home/user/.pi/agent/extensions/my-tool/index.ts",
                "local",
                PiResourceType::Extensions,
                true,
            ),
        ]);
        let policy = ResourcePolicy::default();
        let plan = policy.evaluate(&catalog);

        assert_eq!(plan.extensions.len(), 1);
        assert!(plan.extensions[0].ends_with("my-tool/index.ts"));
        assert_eq!(plan.blocked.len(), 1);
        assert!(plan.blocked[0].reason.contains("pi-tool-display"));
    }

    #[test]
    fn user_allow_overrides_default_block() {
        let catalog = make_catalog(vec![make_resource(
            "/home/user/.pi/agent/npm/node_modules/pi-tool-display/index.ts",
            "npm:pi-tool-display",
            PiResourceType::Extensions,
            true,
        )]);
        let policy = ResourcePolicy {
            allow: vec!["npm:pi-tool-display".to_owned()],
            ..Default::default()
        };
        let plan = policy.evaluate(&catalog);

        assert_eq!(plan.extensions.len(), 1);
        assert!(plan.blocked.is_empty());
    }

    #[test]
    fn user_block_adds_to_defaults() {
        let catalog = make_catalog(vec![make_resource(
            "/home/user/.pi/agent/extensions/noisy-ext/index.ts",
            "local",
            PiResourceType::Extensions,
            true,
        )]);
        let policy = ResourcePolicy {
            block: vec!["noisy-ext".to_owned()],
            ..Default::default()
        };
        let plan = policy.evaluate(&catalog);

        assert!(plan.extensions.is_empty());
        assert_eq!(plan.blocked.len(), 1);
        assert!(plan.blocked[0].reason.contains("user policy"));
    }

    #[test]
    fn renderer_heuristic_blocks_by_name() {
        let catalog = make_catalog(vec![make_resource(
            "/home/user/.pi/agent/extensions/some-tool-display/index.ts",
            "npm:some-tool-display",
            PiResourceType::Extensions,
            true,
        )]);
        let policy = ResourcePolicy::default();
        let plan = policy.evaluate(&catalog);

        assert!(plan.extensions.is_empty());
        assert!(plan.blocked[0].reason.contains("heuristic"));
    }

    #[test]
    fn disabled_heuristics_allows_name_match() {
        let catalog = make_catalog(vec![make_resource(
            "/home/user/.pi/agent/extensions/some-tool-display/index.ts",
            "npm:some-tool-display",
            PiResourceType::Extensions,
            true,
        )]);
        let policy = ResourcePolicy {
            disable_heuristics: true,
            ..Default::default()
        };
        let plan = policy.evaluate(&catalog);

        assert_eq!(plan.extensions.len(), 1);
    }

    #[test]
    fn disabled_resource_is_blocked() {
        let catalog = make_catalog(vec![make_resource(
            "/home/user/.pi/agent/extensions/good-ext/index.ts",
            "local",
            PiResourceType::Extensions,
            false, // disabled in Pi settings
        )]);
        let policy = ResourcePolicy::default();
        let plan = policy.evaluate(&catalog);

        assert!(plan.extensions.is_empty());
        assert!(plan.blocked[0].reason.contains("disabled in Pi settings"));
    }

    #[test]
    fn allow_does_not_override_pi_disabled() {
        // A resource disabled in Pi settings must stay blocked even if the
        // host allow-list matches it. Pi owns its own enable/disable state.
        let catalog = make_catalog(vec![make_resource(
            "/home/user/.pi/agent/extensions/some-ext/index.ts",
            "npm:some-ext",
            PiResourceType::Extensions,
            false, // disabled in Pi settings
        )]);
        let policy = ResourcePolicy {
            allow: vec!["npm:some-ext".to_owned()],
            ..Default::default()
        };
        let plan = policy.evaluate(&catalog);

        assert!(plan.extensions.is_empty());
        assert_eq!(plan.blocked.len(), 1);
        assert!(plan.blocked[0].reason.contains("disabled in Pi settings"));
    }

    #[test]
    fn skills_prompts_themes_pass_through() {
        let catalog = make_catalog(vec![
            make_resource(
                "/home/user/.pi/agent/skills/my-skill/SKILL.md",
                "auto",
                PiResourceType::Skills,
                true,
            ),
            make_resource(
                "/home/user/.pi/agent/prompts/my-prompt.md",
                "auto",
                PiResourceType::Prompts,
                true,
            ),
            make_resource(
                "/home/user/.pi/agent/themes/dark.json",
                "auto",
                PiResourceType::Themes,
                true,
            ),
        ]);
        let policy = ResourcePolicy::default();
        let plan = policy.evaluate(&catalog);

        assert_eq!(plan.skills.len(), 1);
        assert_eq!(plan.prompts.len(), 1);
        assert_eq!(plan.themes.len(), 1);
        assert!(plan.blocked.is_empty());
    }

    #[test]
    fn append_args_produces_correct_flags() {
        let mut plan = PiLaunchPlan::default();
        plan.extensions.push(PathBuf::from("/ext/a.ts"));
        plan.skills.push(PathBuf::from("/skills/b/SKILL.md"));

        let mut args = Vec::new();
        plan.append_args(&mut args);

        assert!(args.contains(&"--no-extensions".to_string()));
        assert!(args.contains(&"--no-skills".to_string()));
        assert!(args.contains(&"--no-prompt-templates".to_string()));
        assert!(args.contains(&"--no-themes".to_string()));
        assert!(args.contains(&"--extension".to_string()));
        assert!(args.contains(&"/ext/a.ts".to_string()));
        assert!(args.contains(&"--skill".to_string()));
        assert!(args.contains(&"/skills/b/SKILL.md".to_string()));
    }

    #[test]
    fn blocked_summary_is_none_when_empty() {
        let plan = PiLaunchPlan::default();
        assert!(plan.blocked_summary().is_none());
    }

    #[test]
    fn blocked_summary_lists_resources() {
        let mut plan = PiLaunchPlan::default();
        plan.blocked.push(BlockedResource {
            path: PathBuf::from("/ext/bad.ts"),
            source: "npm:bad".to_owned(),
            resource_type: PiResourceType::Extensions,
            reason: "test reason".to_owned(),
        });
        let summary = plan.blocked_summary().unwrap();
        assert!(summary.contains("bad.ts"));
        assert!(summary.contains("test reason"));
    }

    // ── check_explicit_path tests ─────────────────────────────────────────

    #[test]
    fn explicit_path_blocks_pi_tool_display() {
        let policy = ResourcePolicy::default();
        let reason = policy
            .check_explicit_path("/home/user/.pi/agent/npm/node_modules/pi-tool-display/index.ts");
        assert!(reason.is_some());
        assert!(reason.unwrap().contains("pi-tool-display"));
    }

    #[test]
    fn explicit_path_allows_normal_extension() {
        let policy = ResourcePolicy::default();
        assert!(policy.check_explicit_path("/home/user/my-ext/index.ts").is_none());
    }

    #[test]
    fn explicit_path_allow_overrides_block() {
        let policy = ResourcePolicy {
            allow: vec!["pi-tool-display".to_owned()],
            ..Default::default()
        };
        assert!(policy
            .check_explicit_path("/home/user/.pi/agent/npm/node_modules/pi-tool-display/index.ts")
            .is_none());
    }

    #[test]
    fn explicit_path_user_block() {
        let policy = ResourcePolicy {
            block: vec!["my-custom-renderer".to_owned()],
            ..Default::default()
        };
        let reason = policy.check_explicit_path("/ext/my-custom-renderer/index.ts");
        assert!(reason.is_some());
        assert!(reason.unwrap().contains("user policy"));
    }

    #[test]
    fn explicit_path_heuristic_blocks_renderer_name() {
        let policy = ResourcePolicy::default();
        let reason = policy.check_explicit_path("/ext/custom-header-widget/index.ts");
        assert!(reason.is_some());
        assert!(reason.unwrap().contains("custom-header"));
    }

    #[test]
    fn explicit_path_disable_heuristics_allows_renderer_name() {
        let policy = ResourcePolicy {
            disable_heuristics: true,
            ..Default::default()
        };
        assert!(policy.check_explicit_path("/ext/custom-header-widget/index.ts").is_none());
    }

    // ── TOML persistence tests ────────────────────────────────────────────

    #[test]
    fn toml_round_trip_preserves_policy() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");

        let policy = ResourcePolicy {
            allow: vec!["npm:pi-tool-display".to_owned()],
            block: vec!["my-bad-ext".to_owned()],
            disable_heuristics: true,
        };
        policy.save_to_path(&path).unwrap();

        let loaded = ResourcePolicy::load_from_path(&path);
        assert_eq!(loaded.allow, policy.allow);
        assert_eq!(loaded.block, policy.block);
        assert!(loaded.disable_heuristics);
    }

    #[test]
    fn toml_load_missing_file_returns_default() {
        let policy = ResourcePolicy::load_from_path(Path::new("/nonexistent/config.toml"));
        assert!(policy.allow.is_empty());
        assert!(policy.block.is_empty());
        assert!(!policy.disable_heuristics);
    }

    #[test]
    fn toml_save_preserves_sibling_tables() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "[ui]\ntheme = \"dark\"\n").unwrap();

        let policy = ResourcePolicy {
            block: vec!["bad-ext".to_owned()],
            ..Default::default()
        };
        policy.save_to_path(&path).unwrap();

        let body = std::fs::read_to_string(&path).unwrap();
        assert!(body.contains("theme"), "sibling [ui] should be preserved");
        assert!(body.contains("bad-ext"));
    }

    #[test]
    fn toml_load_partial_table_fills_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "[pi.resources]\nallow = [\"npm:foo\"]\n").unwrap();

        let policy = ResourcePolicy::load_from_path(&path);
        assert_eq!(policy.allow, vec!["npm:foo"]);
        assert!(policy.block.is_empty());
        assert!(!policy.disable_heuristics);
    }
}
