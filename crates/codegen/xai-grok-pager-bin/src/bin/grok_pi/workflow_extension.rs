use anyhow::{Context, Result};
use std::{fs::File, io::Write};
use tempfile::NamedTempFile;

/// Materialize the Pi workflow spawn executor (xai-workflow host backend).
pub(super) fn write_workflow_extension() -> Result<NamedTempFile> {
    let mut file = tempfile::Builder::new()
        .prefix("pi-grok-workflows-")
        .suffix(".ts")
        .tempfile()
        .context("create Pi workflow extension tempfile")?;
    const SOURCE: &str = include_str!("../../../../../../extensions/pi-grok-workflows/index.ts");
    file.write_all(SOURCE.as_bytes())
        .context("write Pi workflow extension source")?;
    file.flush().context("flush Pi workflow extension source")?;
    File::open(file.path())
        .and_then(|source| source.sync_all())
        .ok();
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workflow_extension_source_loads() {
        let file = write_workflow_extension().expect("write");
        let source = std::fs::read_to_string(file.path()).expect("read");
        assert!(source.contains("__pi_workflow_spawn"));
        assert!(source.contains("createAgentSession"));
        assert!(source.contains("pi-grok-workflow/v1"));
        assert!(
            source.contains("responsePath"),
            "tool must wait on host response file so parent gets the report"
        );
    }
}
