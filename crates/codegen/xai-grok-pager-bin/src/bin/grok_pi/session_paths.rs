use std::path::{Path, PathBuf};

use xai_grok_pager::pi_resource_config::resolve_pi_agent_dir;

pub(super) fn pi_session_dir(pi_args: &[String], cwd: &Path) -> PathBuf {
    let configured = pi_args
        .windows(2)
        .filter(|args| args[0] == "--session-dir")
        .map(|args| args[1].as_str())
        .next_back()
        .map(|path| resolve_pi_path(path, cwd))
        .or_else(|| {
            std::env::var("PI_CODING_AGENT_SESSION_DIR")
                .ok()
                .filter(|path| !path.trim().is_empty())
                .map(|path| resolve_pi_path(&path, cwd))
        });
    configured.unwrap_or_else(|| {
        // Match Pi getSessionsDir(): join(getAgentDir(), "sessions")
        let agent_dir = resolve_pi_agent_dir().unwrap_or_else(|_| {
            std::env::var_os("HOME")
                .map(|home| PathBuf::from(home).join(".pi/agent"))
                .unwrap_or_else(|| PathBuf::from(".pi/agent"))
        });
        agent_dir.join("sessions")
    })
}

fn resolve_pi_path(path: &str, cwd: &Path) -> PathBuf {
    let path = path.trim();
    let expanded = path
        .strip_prefix("~/")
        .and_then(|suffix| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(suffix)))
        .or_else(|| (path == "~").then(|| std::env::var_os("HOME").map(PathBuf::from)).flatten())
        .unwrap_or_else(|| PathBuf::from(path));
    if expanded.is_absolute() {
        expanded
    } else {
        cwd.join(expanded)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_dir_uses_the_last_pi_session_dir_argument() {
        let cwd = PathBuf::from("/project");
        let args = vec![
            "--session-dir".to_string(),
            "old".to_string(),
            "--session-dir".to_string(),
            "sessions".to_string(),
        ];
        assert_eq!(
            pi_session_dir(&args, &cwd),
            PathBuf::from("/project/sessions")
        );
    }

    #[test]
    fn session_dir_defaults_under_resolved_pi_agent_home() {
        let cwd = PathBuf::from("/project");
        let dir = pi_session_dir(&[], &cwd);
        assert!(
            dir.ends_with("sessions"),
            "expected .../sessions, got {}",
            dir.display()
        );
        // Parent should be the Pi agent home (env or ~/.pi/agent).
        let agent = resolve_pi_agent_dir().expect("agent dir");
        assert_eq!(dir, agent.join("sessions"));
    }
}
