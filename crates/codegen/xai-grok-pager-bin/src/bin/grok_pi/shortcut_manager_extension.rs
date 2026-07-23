use anyhow::{Context, Result};
use std::{fs::File, io::Write};
use tempfile::NamedTempFile;

/// Materialize the shortcut manager extension before user extensions load.
///
/// Captures all Pi extension-registered shortcuts (pi.registerShortcut) into a
/// global registry and dispatches them in the Remote TUI key path. Provides
/// /shortcuts command for listing, enabling/disabling, remapping, and diagnostics.
pub(super) fn write_shortcut_manager_extension() -> Result<NamedTempFile> {
    let mut file = tempfile::Builder::new()
        .prefix("pi-grok-shortcut-manager-")
        .suffix(".ts")
        .tempfile()
        .context("create Pi shortcut manager extension tempfile")?;
    const SOURCE: &str =
        include_str!("../../../../../../extensions/pi-grok-shortcut-manager/index.ts");
    file.write_all(SOURCE.as_bytes())
        .context("write Pi shortcut manager extension source")?;
    file.flush()
        .context("flush Pi shortcut manager extension source")?;
    File::open(file.path())
        .and_then(|source| source.sync_all())
        .ok();
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortcut_manager_extension_captures_extension_shortcuts_only() {
        let file = write_shortcut_manager_extension().expect("write extension");
        let source = std::fs::read_to_string(file.path()).expect("read extension");
        assert!(source.contains("__piGrokShortcutIntercept"));
        assert!(source.contains("core/extensions/runner.js"));
        assert!(source.contains("registerShortcut"));
        assert!(source.contains("/shortcuts"));
        // Must NOT manage grok-pi built-in keys
        assert!(!source.contains("RESERVED_KEYS"));
        assert!(!source.contains("app.interrupt"));
    }
}
