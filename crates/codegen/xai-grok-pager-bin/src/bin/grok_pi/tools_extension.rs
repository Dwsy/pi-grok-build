use anyhow::{Context, Result};
use std::{fs::File, io::Write};
use tempfile::NamedTempFile;

/// Materialize the bridge extension that applies the F2-selected Pi built-in
/// tools without changing Pi's source or filtering extension/custom tools.
pub(super) fn write_tools_extension() -> Result<NamedTempFile> {
    let mut file = tempfile::Builder::new()
        .prefix("pi-grok-tools-")
        .suffix(".ts")
        .tempfile()
        .context("create Pi tools extension tempfile")?;
    const SOURCE: &str = include_str!("../../../../../../extensions/pi-grok-tools/index.ts");
    file.write_all(SOURCE.as_bytes())
        .context("write Pi tools extension source")?;
    file.flush().context("flush Pi tools extension source")?;
    File::open(file.path())
        .and_then(|source| source.sync_all())
        .ok();
    Ok(file)
}

pub(super) fn configured_builtin_tools() -> String {
    let defaults = ["read", "bash", "edit", "write"];
    let Ok(config) = xai_grok_shell::config::load_effective_config() else {
        return defaults.join(",");
    };
    let Some(tools) = config
        .get("ui")
        .and_then(|ui| ui.get("pi_builtin_tools"))
        .and_then(toml::Value::as_table)
    else {
        return defaults.join(",");
    };
    ["read", "bash", "edit", "write", "grep", "find", "ls"]
        .into_iter()
        .filter(|name| {
            tools
                .get(*name)
                .and_then(toml::Value::as_bool)
                .unwrap_or(matches!(*name, "read" | "bash" | "edit" | "write"))
        })
        .collect::<Vec<_>>()
        .join(",")
}

pub(super) fn has_explicit_tools_arg(args: &[String]) -> bool {
    args.iter().any(|arg| arg == "--tools" || arg == "-t")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tools_extension_source_is_loadable_typescript_module() {
        let file = write_tools_extension().expect("write tools extension");
        let source = std::fs::read_to_string(file.path()).expect("read extension");
        assert!(source.contains("PI_GROK_BUILTIN_TOOLS"));
        assert!(source.contains("setActiveTools"));
        assert_eq!(
            file.path().extension().and_then(|value| value.to_str()),
            Some("ts")
        );
    }

    #[test]
    fn detects_explicit_tools_allowlist() {
        assert!(has_explicit_tools_arg(&[
            "--tools".into(),
            "read,grep".into()
        ]));
        assert!(has_explicit_tools_arg(&["-t".into(), "read,grep".into()]));
        assert!(!has_explicit_tools_arg(&[
            "--exclude-tools".into(),
            "bash".into()
        ]));
    }
}
