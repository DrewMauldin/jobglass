# Install JobGlass

JobGlass v0.1.0 is distributed as unsigned community packages. Every release includes `SHA256SUMS`, a CycloneDX SBOM, and GitHub build provenance. Code signing and notarisation are not claimed.

## Verify the download

Download the package and `SHA256SUMS` from the same [GitHub release](https://github.com/DrewMauldin/jobglass/releases/latest). In a terminal opened in the download directory:

### macOS or Linux

```bash
shasum -a 256 PATH_TO_PACKAGE
grep 'PATH_TO_PACKAGE$' SHA256SUMS
```

Compare the two complete hexadecimal values. If you download every release asset with `gh release download v0.1.0 --repo DrewMauldin/jobglass`, you can instead run `shasum -a 256 --check SHA256SUMS`. You can also verify GitHub's build attestation with a current GitHub CLI:

```bash
gh attestation verify PATH_TO_PACKAGE --repo DrewMauldin/jobglass
```

### Windows PowerShell

```powershell
Get-FileHash .\PATH_TO_PACKAGE -Algorithm SHA256
Get-Content .\SHA256SUMS
```

Compare the complete hexadecimal value with the matching line. Do not install when it differs.

## macOS 13 or later

1. Open the `.dmg` and drag JobGlass into Applications.
2. Because the package is unsigned, Gatekeeper may block the first launch.
3. If you accept that limitation after verifying the checksum and provenance, open **System Settings → Privacy & Security** and use the system-provided **Open Anyway** control for JobGlass.

Do not use a terminal command that disables Gatekeeper globally. JobGlass does not require Full Disk Access, Accessibility control, administrator access, or a background helper. Definitions unavailable to the current user remain marked as permission-limited.

## Linux

Choose the `.deb` for Debian/Ubuntu-family systems or the `.AppImage` for a portable package. Desktop WebKit and distribution policy still apply.

```bash
sudo apt install ./jobglass_0.1.0_amd64.deb
```

For an AppImage, make only that downloaded file executable and run it:

```bash
chmod +x JobGlass_0.1.0_amd64.AppImage
./JobGlass_0.1.0_amd64.AppImage
```

JobGlass never invokes `sudo`. Running the application as root changes the evidence and privacy boundary and is unsupported.

## Windows 10 or 11

Use either the `.msi` or NSIS `.exe` from the release. Windows SmartScreen may warn because the package is unsigned. Continue only after verifying the checksum and accepting that publisher identity is not established.

JobGlass queries only the local Task Scheduler with the current user's token. It does not require an administrator console, stored credentials, or remote Task Scheduler access.

## Build from source

Source builds are the strongest option when unsigned packages do not meet your trust requirements. Follow [Development](development.md), check out the exact release tag, and run:

```bash
git checkout v0.1.0
npm ci
npm run quality -- full
npm run tauri:build
```

## Uninstall

Use the platform's normal application removal path. JobGlass creates no account, daemon, scheduler job, or cloud data. WebView preferences may remain in the current user's normal application-data location and can be removed separately if desired.
