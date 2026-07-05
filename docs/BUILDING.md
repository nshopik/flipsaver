# Building flipsaver

Cross-compiled for Windows from WSL2. No Windows toolchain required.

## One-time setup

    rustup target add x86_64-pc-windows-msvc
    cargo install cargo-xwin --locked
    sudo apt-get install -y clang lld

Pinned versions (splat layout drifts across xwin releases — if a build
breaks after reinstalling, reinstall exactly these):

- cargo-xwin: 0.23.0
- rustc: 1.93.1 (01f6ddf75 2026-02-11)

First build downloads + splats the MSVC CRT and Windows SDK (~1.5 GB)
and requires accepting the Microsoft license (set `XWIN_ACCEPT_LICENSE=1`
for non-interactive builds).

## Build

    cargo xwin build --release --target x86_64-pc-windows-msvc

Output: `target/x86_64-pc-windows-msvc/release/flipsaver.exe` (< 1 MB).

## Test (Linux host)

    cargo test

Only pure modules (arg parsing, INI, layout math) compile on the host;
all Win32 code is `#[cfg(windows)]`-gated.
