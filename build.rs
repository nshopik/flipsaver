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

    // Manifest (PMv2 DPI awareness + comctl32 v6 for the dialog) embedding via linker
    // resource only works when both target and host are Windows (mt.exe / manifest tool).
    // Cross-compiling (Linux → Windows via cargo-xwin) cannot embed, so we rely on the
    // declared functional backstop: SetProcessDpiAwarenessContext(PERMONITORAWAREV2) at
    // startup in main(). This provides DPI awareness; the /c dialog loses comctl32 v6
    // visual theming but remains functional.
    if std::env::var("CARGO_CFG_WINDOWS").is_ok() {
        if std::env::consts::OS == "windows" {
            if let Err(e) = embed_manifest::embed_manifest(embed_manifest::new_manifest("flipsaver"))
            {
                eprintln!("Warning: failed to embed manifest: {e}");
            }
        } else {
            println!(
                "cargo:warning=cross-compile manifest skipped (host={}, target=windows-msvc); \
                 SetProcessDpiAwarenessContext at startup is the functional backstop",
                std::env::consts::OS
            );
        }
    }
}
