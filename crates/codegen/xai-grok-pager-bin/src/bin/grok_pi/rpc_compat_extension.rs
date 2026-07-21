use anyhow::{Context, Result};
use std::{fs::File, io::Write};
use tempfile::NamedTempFile;

/// Materialize the Remote TUI extension-mode facade before user extensions load.
///
/// Pi remains in JSONL RPC mode. The extension changes only the mode exposed by
/// Pi's ExtensionRunner after Remote TUI has installed the custom-component host.
pub(super) fn write_rpc_compat_extension() -> Result<NamedTempFile> {
    let mut file = tempfile::Builder::new()
        .prefix("pi-grok-rpc-compat-")
        .suffix(".ts")
        .tempfile()
        .context("create Pi RPC compatibility extension tempfile")?;
    const SOURCE: &str = include_str!("../../../../../../extensions/pi-grok-rpc-compat/index.ts");
    file.write_all(SOURCE.as_bytes())
        .context("write Pi RPC compatibility extension source")?;
    file.flush()
        .context("flush Pi RPC compatibility extension source")?;
    File::open(file.path())
        .and_then(|source| source.sync_all())
        .ok();
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rpc_compat_extension_patches_only_the_extension_mode_boundary() {
        let file = write_rpc_compat_extension().expect("write extension");
        let source = std::fs::read_to_string(file.path()).expect("read extension");
        assert!(source.contains("PI_GROK_EXTENSION_TUI_COMPAT"));
        assert!(source.contains("core/extensions/runner.js"));
        assert!(source.contains("setUIContext"));
        assert!(source.contains("mode === \"rpc\" ? \"tui\" : mode"));
        assert!(!source.contains("process.argv ="));
    }
}
