use std::env;
use std::fs;
use std::io;
use std::path::PathBuf;
use std::process::Command;

fn git(args: &[&str]) -> Option<String> {
    let out = Command::new("git").args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8(out.stdout).ok()?.trim().to_string();
    (!s.is_empty()).then_some(s)
}

fn find_llvm_rc() -> io::Result<String> {
    // Allow override via LLVM_RC env var.
    if let Ok(rc) = env::var("LLVM_RC") {
        return Ok(rc);
    }
    // Probe standard LLVM RC names from newest to oldest.
    // -no-preprocess flag only available in llvm-rc 17+, so restrict probe accordingly.
    for name in &["llvm-rc-19", "llvm-rc-18", "llvm-rc-17", "llvm-rc"] {
        // Check if command can be executed (exit code may be non-zero for help).
        if Command::new(name).output().is_ok() {
            return Ok(name.to_string());
        }
    }
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        "llvm-rc not found: tried llvm-rc-19, llvm-rc-18, llvm-rc-17, and llvm-rc; set LLVM_RC to override",
    ))
}

fn embed_manifest_via_coff(out_dir: &PathBuf) -> io::Result<()> {
    // Floor is Windows 10 1703+ (spec requirement).
    let version = env!("CARGO_PKG_VERSION");
    let manifest_xml = format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" xmlns:asmv3="urn:schemas-microsoft-com:asm.v3" manifestVersion="1.0">
  <assemblyIdentity name="flipsaver" type="win32" version="{}.0"/>
  <dependency>
    <dependentAssembly>
      <assemblyIdentity language="*" name="Microsoft.Windows.Common-Controls" processorArchitecture="*" publicKeyToken="6595b64144ccf1df" type="win32" version="6.0.0.0"/>
    </dependentAssembly>
  </dependency>
  <compatibility xmlns="urn:schemas-microsoft-com:compatibility.v1">
    <application>
      <maxversiontested Id="10.0.18362.1"/>
      <supportedOS Id="{{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}}"/>
    </application>
  </compatibility>
  <asmv3:application>
    <asmv3:windowsSettings>
      <activeCodePage xmlns="http://schemas.microsoft.com/SMI/2019/WindowsSettings">UTF-8</activeCodePage>
      <dpiAwareness xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">permonitorv2</dpiAwareness>
      <longPathAware xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">true</longPathAware>
    </asmv3:windowsSettings>
  </asmv3:application>
  <asmv3:trustInfo>
    <asmv3:security>
      <asmv3:requestedPrivileges>
        <asmv3:requestedExecutionLevel level="asInvoker" uiAccess="false"/>
      </asmv3:requestedPrivileges>
    </asmv3:security>
  </asmv3:trustInfo>
</assembly>"#, version);

    fs::write(out_dir.join("app.manifest"), manifest_xml)?;

    // RT_MANIFEST is resource type 24; line format is `<id> <type> "<filename>"`.
    let rc_path = out_dir.join("app.rc");
    fs::write(&rc_path, "1 24 \"app.manifest\"\n")?;

    let llvm_rc = find_llvm_rc()?;
    let out_res = out_dir.join("out.res");
    let status = Command::new(&llvm_rc)
        .arg("/fo")
        .arg(&out_res)
        .arg("-no-preprocess")
        .arg(&rc_path)
        .current_dir(out_dir)
        .status()?;

    if !status.success() {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            format!("{} compilation failed with status {}", llvm_rc, status),
        ));
    }

    // Emit linker arg to link the .res file.
    println!(
        "cargo:rustc-link-arg-bins={}",
        out_res.canonicalize()?.display()
    );

    Ok(())
}

fn main() {
    // Version-line convention: tag "dev" for untagged/head builds.
    let tag = git(&["describe", "--tags", "--exact-match", "HEAD"]).unwrap_or_else(|| "dev".into());
    let sha = git(&["rev-parse", "--short", "HEAD"]).unwrap_or_else(|| "unknown".into());
    println!("cargo:rustc-env=FLIPSAVER_VERSION_TAG={tag}");
    println!("cargo:rustc-env=FLIPSAVER_GIT_SHA={sha}");
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs");
    println!("cargo:rerun-if-changed=.git/packed-refs");

    // Embed the manifest via llvm-rc (works on any host, no mt.exe); main.rs's
    // SetProcessDpiAwarenessContext is the fallback.
    if env::var("CARGO_CFG_WINDOWS").is_ok() {
        let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
        embed_manifest_via_coff(&out_dir).expect("failed to embed manifest via llvm-rc");
    }
}
