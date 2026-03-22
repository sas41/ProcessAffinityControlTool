# Development

This document covers source builds, debugging, and VS Code setup.

## Rust setup

Install Rust using the official guide:

- https://rust-lang.org/learn/get-started/

## VS Code setup (Windows)

For reliable Windows debugging in this repository:

1. Install Rust Analyzer (`rust-lang.rust-analyzer`).
2. Install the Microsoft C/C++ Extension Pack:
   - https://marketplace.visualstudio.com/items?itemName=ms-vscode.cpptools-extension-pack
3. Run VS Code as Administrator when debugging elevated processes.

The workspace includes tasks and launch configurations in:

- `.vscode/tasks.json`
- `.vscode/launch.json`

## Cross-compilation prerequisites

### Linux to Windows

The Windows platform provider uses `Win32_System_SystemInformation` which
requires MinGW's `dlltool` to generate import libraries at build time.

**Arch / CachyOS:**

```bash
sudo pacman -S mingw-w64-toolchain
rustup target add x86_64-pc-windows-gnu
```

**Debian / Ubuntu:**

```bash
sudo apt install mingw-w64
rustup target add x86_64-pc-windows-gnu
```

**Fedora:**

```bash
sudo dnf install mingw64-gcc
rustup target add x86_64-pc-windows-gnu
```

Building natively on Windows with MSVC does not require MinGW.

## Build from source

Run from repository root:

```bash
cargo build
```

Useful commands:

```bash
cargo check
cargo test
cargo run --bin process_affinity_control_tool
```

Optional diagnostics:

```bash
cargo run --bin process_affinity_control_tool -- --topology-report
```

## Publishing a Windows executable on Windows

Use the publish script with the Windows target from a Windows shell:

```bash
./publish.sh windows
```

Notes:

- On Windows host, the script now builds `x86_64-pc-windows-msvc`.
- Output binary is written to `publish/windows/process_affinity_control_tool.exe`.
- If your shell does not support `./publish.sh`, run it from Git Bash, or call Bash explicitly.

## Elevated debug workflow (Windows)

Recommended flow in VS Code:

1. Start `Debug (Windows Elevated Attach)` from the debugger dropdown.
2. Approve UAC when prompted by the pre-launch task.
3. Select `process_affinity_control_tool.exe` in the attach process picker.

If needed, use `Debug (Windows Attach Only)` to attach to an already running elevated instance.
