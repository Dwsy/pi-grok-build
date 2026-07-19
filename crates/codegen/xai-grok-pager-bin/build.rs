use std::process::Command;

fn main() {
    for path in ["HEAD", "refs/tags"] {
        if let Some(path) = git_path(path) {
            println!("cargo:rerun-if-changed={path}");
        }
    }
    println!("cargo:rerun-if-env-changed=GROK_PI_VERSION");

    let commit = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    // Product version for `grok-pi --version` and update checks.
    // Prefer release env (set by CI from the v* tag), then git describe,
    // never the upstream workspace CARGO_PKG_VERSION (0.1.220-alpha.*).
    let version = product_version();

    println!("cargo:rustc-env=GROK_PI_VERSION={version}");
    println!("cargo:rustc-env=VERSION_WITH_COMMIT={version} ({commit})");
}

fn git_path(path: &str) -> Option<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--git-path", path])
        .output()
        .ok()
        .filter(|output| output.status.success())?;
    String::from_utf8(output.stdout)
        .ok()
        .map(|path| path.trim().to_string())
        .filter(|path| !path.is_empty())
}

fn product_version() -> String {
    if let Ok(v) = std::env::var("GROK_PI_VERSION") {
        let v = v.trim().trim_start_matches('v').to_string();
        if !v.is_empty() {
            return v;
        }
    }

    // Local / non-release builds: nearest annotated or lightweight v* tag.
    if let Some(tag) = git_describe_version() {
        return tag;
    }

    "0.0.0-dev".to_string()
}

fn git_describe_version() -> Option<String> {
    let output = Command::new("git")
        .args([
            "describe",
            "--tags",
            "--match",
            "v*",
            "--abbrev=0",
            "--dirty=-dirty",
        ])
        .output()
        .ok()
        .filter(|o| o.status.success())?;
    let tag = String::from_utf8(output.stdout).ok()?;
    let tag = tag.trim().trim_start_matches('v');
    if tag.is_empty() {
        None
    } else {
        Some(tag.to_string())
    }
}
