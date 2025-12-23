# Building Windows Executable

This guide explains how to compile `spotify-tui` to a Windows executable (`.exe` file).

## Table of Contents
- [Prerequisites](#prerequisites)
- [Quick Start](#quick-start)
- [Method 1: Using Make](#method-1-using-make)
- [Method 2: Using the Build Script](#method-2-using-the-build-script)
- [Method 3: Manual Build](#method-3-manual-build)
- [Troubleshooting](#troubleshooting)

## Prerequisites

Before building, ensure you have:

1. **Rust installed** - Install from [rustup.rs](https://rustup.rs/)
2. **Windows target** - Will be installed automatically by the build scripts

## Quick Start

The easiest way to build a Windows executable:

```bash
make build-windows
```

The executable will be located at: `target/x86_64-pc-windows-gnu/release/spt.exe`

**Note:** By default, the build uses the `x86_64-pc-windows-gnu` target which works better for cross-compilation from Linux. On Windows, you may prefer to use the MSVC target instead (see [Manual Build](#method-3-manual-build)).

## Method 1: Using Make

If you have `make` installed:

```bash
# Build Windows executable
make build-windows

# View all available commands
make help
```

## Method 2: Using the Build Script

Run the provided build script:

```bash
# Make it executable (first time only)
chmod +x build-windows.sh

# Run the script
./build-windows.sh
```

The script will:
- Install the Windows target if not already present
- Compile the project for Windows (using GNU target by default)
- Show the location of the generated `.exe` file

You can override the target:
```bash
TARGET=x86_64-pc-windows-msvc ./build-windows.sh
```

## Method 3: Manual Build

If you prefer to build manually:

### Step 1: Add the Windows target

For GNU toolchain (recommended for Linux cross-compilation):
```bash
rustup target add x86_64-pc-windows-gnu
```

For MSVC toolchain (recommended when building on Windows):
```bash
rustup target add x86_64-pc-windows-msvc
```

### Step 2: Build the project

With GNU toolchain:
```bash
cargo build --release --target x86_64-pc-windows-gnu
```

With MSVC toolchain:
```bash
cargo build --release --target x86_64-pc-windows-msvc
```

### Step 3: Find your executable

The Windows executable will be at:
```
target/x86_64-pc-windows-gnu/release/spt.exe
```
or
```
target/x86_64-pc-windows-msvc/release/spt.exe
```

## Alternative Windows Targets

If you're building on Linux and don't have MSVC toolchain, you can use GNU target:

```bash
# Add GNU target
rustup target add x86_64-pc-windows-gnu

# Build with GNU toolchain
cargo build --release --target x86_64-pc-windows-gnu
```

**Note:** You may need to install `mingw-w64` on Linux:
- Ubuntu/Debian: `sudo apt-get install mingw-w64`
- Fedora: `sudo dnf install mingw64-gcc`
- Arch: `sudo pacman -S mingw-w64-gcc`

## Troubleshooting

### "linker not found" error

If you encounter linker errors when building on Linux, install the MinGW toolchain:

```bash
# Ubuntu/Debian
sudo apt-get install mingw-w64

# Fedora
sudo dnf install mingw64-gcc

# Arch Linux
sudo pacman -S mingw-w64-gcc
```

Then use the GNU target instead:
```bash
cargo build --release --target x86_64-pc-windows-gnu
```

### Dependencies on Linux

For building on Linux, you may also need these development packages:

```bash
# Ubuntu/Debian
sudo apt-get install -y pkg-config libssl-dev libxcb1-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev

# Fedora
sudo dnf install -y pkg-config openssl-devel libxcb-devel

# Arch Linux
sudo pacman -S pkg-config openssl libxcb
```

### Cross-compilation from macOS

Cross-compiling to Windows from macOS requires additional setup. The easiest approach is to:

1. Use the MSVC target with `xwin`:
```bash
cargo install xwin
cargo build --release --target x86_64-pc-windows-msvc
```

2. Or set up a Windows VM or use GitHub Actions for building Windows binaries.

## Testing the Executable

After building, you can test the executable on a Windows machine or using Wine on Linux:

```bash
# Install Wine (on Linux)
sudo apt-get install wine64

# Run the executable
wine target/x86_64-pc-windows-msvc/release/spt.exe --help
```

## CI/CD Integration

This project uses GitHub Actions to automatically build Windows executables on every release. See `.github/workflows/cd.yml` for the automated build configuration.

## Distribution

The Windows executable can be distributed as-is or packaged in various ways:

- **ZIP archive** - Simple compression for distribution
- **Installer** - Using tools like Inno Setup or WiX
- **Scoop package** - For the Scoop package manager (already supported)
- **Chocolatey package** - For Chocolatey package manager

## Additional Resources

- [Rust Cross-Compilation Guide](https://rust-lang.github.io/rustup/cross-compilation.html)
- [Cargo Documentation](https://doc.rust-lang.org/cargo/)
- [Project Releases](https://github.com/Rigellute/spotify-tui/releases) - Pre-built binaries
