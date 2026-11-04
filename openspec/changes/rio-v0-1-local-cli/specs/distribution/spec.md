## Purpose

Distribution turns a tagged version into downloadable, verifiable binaries and gives users a one-line install on Unix (`install.sh`) and Windows (`install.ps1`) that never runs an unverified binary and never needs administrator rights.

## ADDED Requirements

### Requirement: Release artifacts
Pushing a tag `v<semver>` SHALL run a release workflow that builds `rio` for `x86_64-unknown-linux-musl`, `aarch64-unknown-linux-musl`, `x86_64-apple-darwin`, `aarch64-apple-darwin`, `x86_64-pc-windows-msvc` and `aarch64-pc-windows-msvc`. It SHALL publish one GitHub Release per tag, with one archive per target named `rio-<version>-<target>.tar.gz` (`.zip` for Windows targets) containing the binary, `LICENSE` and `README.md`, a `SHA256SUMS` file listing every archive, the `install.sh` and `install.ps1` from the tagged commit, and a build provenance attestation for each archive. The workflow SHALL publish nothing unless every target builds and the test suite passes on the tagged commit. Each binary SHALL report its version, target and commit through `rio --version`. Code signing and notarization are out of scope for this version.

#### Scenario: Tag produces a complete release
- **WHEN** tag `v0.1.0` is pushed and every build succeeds
- **THEN** the release `v0.1.0` contains six archives, `SHA256SUMS` with six entries matching them, `install.sh`, `install.ps1` and an attestation per archive

#### Scenario: One failing target blocks the release
- **WHEN** the `aarch64-pc-windows-msvc` build fails
- **THEN** no release is published and no asset is uploaded for that tag

#### Scenario: Linux binary runs without glibc
- **WHEN** the `x86_64-unknown-linux-musl` binary is run on an Alpine Linux container
- **THEN** `rio --version` prints the version, target and commit and exits 0

### Requirement: Unix installer
`install.sh` SHALL be a POSIX `sh` script that installs `rio` on Linux and macOS, runnable as `curl -fsSL https://github.com/pixelinfinito/rio/releases/latest/download/install.sh | sh`. It SHALL:
- map `uname -s` and `uname -m` to one of the four Unix release targets, and exit non-zero with the list of supported targets when there is no match;
- install the latest release, or the version given by `--version <v>` or `RIO_VERSION`;
- install into `$HOME/.local/bin`, or the directory given by `--install-dir <dir>` or `RIO_INSTALL_DIR`, creating it if needed, without `sudo` and without writing outside that directory and a temporary directory;
- download with `curl`, or `wget` when `curl` is absent, over HTTPS only;
- verify the archive's SHA-256 against `SHA256SUMS` using `sha256sum` or `shasum -a 256`, and install nothing if the checksum is missing or does not match;
- extract into a temporary directory removed on exit, then move the binary into place in a single rename so an interrupted install never leaves a partial binary;
- replace an existing `rio` in the install directory, so rerunning it upgrades or downgrades;
- wrap all logic in a function called on the last line, so a truncated download executes nothing;
- run the installed binary with `--version` and print the result;
- never edit shell startup files: when the install directory is not on `PATH`, print the line to add instead.

Any failure SHALL exit non-zero with a message naming the step that failed.

#### Scenario: Default install on macOS arm64
- **WHEN** `install.sh` runs with no options on an arm64 Mac whose `PATH` contains `$HOME/.local/bin`
- **THEN** the `aarch64-apple-darwin` archive is downloaded and verified, `rio` is installed at `$HOME/.local/bin/rio`, and the script prints the installed version

#### Scenario: Checksum mismatch installs nothing
- **WHEN** the downloaded archive's SHA-256 differs from its `SHA256SUMS` entry
- **THEN** the script exits non-zero, reports the mismatch, and the install directory is unchanged

#### Scenario: Unsupported platform
- **WHEN** `install.sh` runs on FreeBSD or on a 32-bit ARM Linux machine
- **THEN** the script exits non-zero and lists the supported targets without downloading anything

#### Scenario: Install directory not on PATH
- **WHEN** `install.sh --install-dir /opt/tools/bin` completes and `/opt/tools/bin` is not on `PATH`
- **THEN** the script prints the `export PATH=...` line to add and does not modify any shell startup file

### Requirement: Windows installer
`install.ps1` SHALL run on Windows PowerShell 5.1 and PowerShell 7 or later and install `rio` on Windows, runnable as `irm https://github.com/pixelinfinito/rio/releases/latest/download/install.ps1 | iex`. It SHALL:
- map the processor architecture to `x86_64-pc-windows-msvc` or `aarch64-pc-windows-msvc`, and stop with the list of supported targets otherwise;
- install the latest release, or the version given by `-Version` or `RIO_VERSION`;
- install into `$env:LOCALAPPDATA\Programs\rio\bin`, or the directory given by `-InstallDir` or `RIO_INSTALL_DIR`, without administrator rights;
- enable TLS 1.2 on Windows PowerShell 5.1 and download over HTTPS only;
- verify the archive with `Get-FileHash -Algorithm SHA256` against `SHA256SUMS`, and install nothing if the checksum is missing or does not match;
- extract into a temporary directory removed on exit, then replace `rio.exe` in one move; when the existing `rio.exe` is in use, stop with a message asking the user to close running `rio` processes;
- add the install directory to the user `PATH` (the current user's environment, never the machine's) unless it is already present or `-NoModifyPath` or `RIO_NO_MODIFY_PATH=1` is given, and tell the user to open a new terminal;
- run the installed binary with `--version` and print the result.

Parameters SHALL have environment-variable equivalents, because `irm | iex` cannot pass parameters. Any failure SHALL stop with a non-zero exit code and a message naming the step that failed.

#### Scenario: Default install on Windows x64
- **WHEN** `irm https://github.com/pixelinfinito/rio/releases/latest/download/install.ps1 | iex` runs in Windows PowerShell 5.1 on an x64 machine as a standard user
- **THEN** `rio.exe` is installed in `%LOCALAPPDATA%\Programs\rio\bin`, that directory is added to the user `PATH`, and the installed version is printed

#### Scenario: Checksum mismatch installs nothing
- **WHEN** the downloaded zip's SHA-256 differs from its `SHA256SUMS` entry
- **THEN** the script stops with a non-zero exit code, reports the mismatch, and the install directory is unchanged

#### Scenario: PATH left alone on request
- **WHEN** the script runs with `RIO_NO_MODIFY_PATH=1`
- **THEN** the user `PATH` is not modified and the script prints the directory to add

### Requirement: Installer download source override
Both installers SHALL accept `RIO_DOWNLOAD_BASE` to replace the default download base `https://github.com/pixelinfinito/rio/releases/download/<version>`, for mirrors and for testing. It SHALL accept an `https://` URL or a local directory path, and SHALL reject any other scheme. Checksum verification SHALL apply to every source.

#### Scenario: Install from a local directory
- **WHEN** `RIO_DOWNLOAD_BASE` points to a directory holding a build's archives and `SHA256SUMS`
- **THEN** the installer installs from that directory with the same verification and no network access

#### Scenario: Plain HTTP rejected
- **WHEN** `RIO_DOWNLOAD_BASE` is `http://mirror.example/rio`
- **THEN** the installer exits non-zero, stating that only HTTPS URLs or local paths are accepted
