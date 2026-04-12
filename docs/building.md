# Building Molt

## Linux / macOS (recommended)

```bash
cargo build --release
sudo cp target/release/molt /usr/local/bin/molt
molt --version
```

No extra configuration needed.

## Windows — MSVC toolchain

Requires [Visual Studio Build Tools](https://visualstudio.microsoft.com/downloads/#build-tools-for-visual-studio-2022) with the "C++ build tools" workload.

```powershell
rustup target add x86_64-pc-windows-msvc
cargo build --release
```

## Windows — GNU toolchain (current dev machine)

This setup is used when VS Build Tools are not available. It uses msys2's GCC as the linker.

### 1. Install msys2

Download from https://www.msys2.org/ and install to `C:\msys64\`.

In an MSYS2 terminal:
```bash
pacman -S mingw-w64-x86_64-gcc
```

### 2. Add Rust GNU target

```powershell
rustup target add x86_64-pc-windows-gnu
```

### 3. Build

```powershell
# Option A: use the target flag
rustup run stable-x86_64-pc-windows-gnu cargo build --release

# Option B: add [build] section to .cargo/config.toml (local only, don't commit)
# [build]
# target = "x86_64-pc-windows-gnu"
```

The `.cargo/config.toml` in this repo already includes the GNU linker path for msys2 at `C:/msys64/mingw64/bin/gcc.exe`. If your msys2 is installed elsewhere, set `CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER` env var.

## Cross-compilation (Linux → Windows)

```bash
rustup target add x86_64-pc-windows-gnu
sudo apt install gcc-mingw-w64-x86_64
cargo build --release --target x86_64-pc-windows-gnu
```

## Running tests

```bash
cargo test
```

No external services are needed for unit tests. Integration tests (Feishu API, Anthropic API) require credentials in `~/.molt/config.yaml`.
