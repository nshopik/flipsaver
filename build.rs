use std::env;
use std::fs::File;
use std::io::{self, Write};
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
    // Write manifest XML with PerMonitorV2 DPI + comctl32 v6 dependency.
    // Version derived from CARGO_PKG_VERSION; floor is Windows 10 1703+ (spec requirement).
    let version = env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "0.1.0".to_string());
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
      <supportedOS Id="{{35138b9a-5d96-4fbd-8e2d-a2440225f93a}}"/>
      <supportedOS Id="{{4a2f28e3-53b9-4441-ba9c-d69d4a4a6e38}}"/>
      <supportedOS Id="{{1f676c76-80e1-4239-95bb-83d0f6d0da78}}"/>
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

    let manifest_path = out_dir.join("app.manifest");
    let mut f = File::create(&manifest_path)?;
    f.write_all(manifest_xml.as_bytes())?;
    drop(f);

    // Write RC file: resource script with RT_MANIFEST (type 24) pointing to the manifest.
    // Format: `<id> <type> "<filename>"`
    let rc_script = r#"1 24 "app.manifest"
"#;
    let rc_path = out_dir.join("app.rc");
    let mut f = File::create(&rc_path)?;
    f.write_all(rc_script.as_bytes())?;
    drop(f);

    // Compile RC to COFF resource object via llvm-rc.
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

    // Manifest (PMv2 DPI awareness + comctl32 v6 for the dialog) embedded as COFF resource.
    // Use llvm-rc to compile manifest to .res; this works on all hosts and bypasses mt.exe.
    // SetProcessDpiAwarenessContext at startup (main.rs) provides belt-and-braces fallback.

    if std::env::var("CARGO_CFG_WINDOWS").is_ok() {
        match env::var("OUT_DIR") {
            Ok(out_dir) => {
                let out_path = PathBuf::from(out_dir);
                if let Err(e) = embed_manifest_via_coff(&out_path) {
                    panic!("failed to embed manifest via llvm-rc: {}", e);
                }
            }
            Err(_) => panic!("OUT_DIR env var not set"),
        }
    }
}
