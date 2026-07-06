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

    // Manifest (PMv2 DPI awareness + comctl32 v6 for the dialog) embedding requires
    // Windows tools: either mt.exe (for /MANIFEST:EMBED) or rc.exe/llvm-rc (for COFF .res).
    // Cross-compilation targets cannot use these tools on non-Windows hosts.
    // Tested approaches:
    // - Approach #1 (llvm-rc): No RC tool available on this Linux host.
    // - Approach #2 (lld-link /MANIFEST:EMBED): lld-link still requires mt.exe, fails.
    // Therefore, we use the spec's declared functional backstop: SetProcessDpiAwarenessContext
    // at startup (already in main.rs). This provides DPI awareness; the /c dialog loses
    // comctl32 v6 visual theming but remains fully functional.

    if std::env::var("CARGO_CFG_WINDOWS").is_ok() {
        if std::env::consts::OS == "windows" {
            // Native Windows build: use embed_manifest with mt.exe.
            if let Err(e) = embed_manifest::embed_manifest(embed_manifest::new_manifest("flipsaver"))
            {
                eprintln!("Warning: failed to embed manifest: {e}");
            }
        } else {
            // Cross-compile: manifest embedding tools (mt.exe, rc.exe, llvm-rc) unavailable.
            // Runtime SetProcessDpiAwarenessContext call is the functional backstop.
            println!(
                "cargo:warning=cross-compile manifest skipped (host={}, target=windows-msvc); \
                 lld-link needs mt.exe for /MANIFEST:EMBED, not available on non-Windows; \
                 SetProcessDpiAwarenessContext at startup provides functional DPI awareness",
                std::env::consts::OS
            );
        }
    }
}
