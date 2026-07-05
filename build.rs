use std::process::Command;

fn git(args: &[&str]) -> Option<String> {
    let out = Command::new("git").args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8(out.stdout).ok()?.trim().to_string();
    (!s.is_empty()).then_some(s)
}

fn main() {
    // Version-line convention: tag "dev" for untagged/head builds.
    let tag = git(&["describe", "--tags", "--exact-match", "HEAD"]).unwrap_or_else(|| "dev".into());
    let sha = git(&["rev-parse", "--short", "HEAD"]).unwrap_or_else(|| "unknown".into());
    println!("cargo:rustc-env=FLIPSAVER_VERSION_TAG={tag}");
    println!("cargo:rustc-env=FLIPSAVER_GIT_SHA={sha}");
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs");

    // Manifest (PMv2 DPI awareness + comctl32 v6 for the dialog) via a
    // linker resource. Requires:
    // - CARGO_CFG_WINDOWS: target platform is Windows (for /MANIFEST:EMBED)
    // - Host OS is Windows: mt.exe (manifest tool) only available on Windows
    // When cross-compiling (e.g., Linux → Windows via cargo-xwin), mt.exe
    // is unavailable, so embedding fails. In that case,
    // SetProcessDpiAwarenessContext at startup is the functional backstop.
    if std::env::var("CARGO_CFG_WINDOWS").is_ok() && std::env::consts::OS == "windows" {
        if let Err(e) = embed_manifest::embed_manifest(embed_manifest::new_manifest("flipsaver"))
        {
            eprintln!("Warning: failed to embed manifest: {e}");
        }
    }
}
