# Goatpad Release Runbook

This is the complete release process for Goatpad Windows MSI releases. Replace
`X.Y.Z` with the release version, for example `1.1.0`.

## Release Checklist

- [ ] Confirm the release version and release notes.
- [ ] Confirm the working tree contains the intended changes.
- [ ] Update the version in `Cargo.toml`.
- [ ] Run formatting, checks, and tests.
- [ ] Build and test the MSI.
- [ ] Calculate the MSI SHA-256 checksum.
- [ ] Commit the version change and create the release tag.
- [ ] Push the branch and tag.
- [ ] Create the GitHub release and upload the MSI.
- [ ] Update and publish `updates/manifest.json`.
- [ ] Test the in-app update flow from the previous release.

## 1. Prerequisites

Run all commands from the repository root in PowerShell.

Required tools:

- A stable Rust toolchain with Cargo
- Visual Studio C++ build tools
- The .NET SDK
- WiX Toolset 7
- Git
- GitHub CLI (`gh`), authenticated to the repository

Install WiX if needed:

```powershell
dotnet tool install --global wix --version 7.0.0
```

Check the tools:

```powershell
cargo --version
wix --version
git --version
gh --version
gh auth status
```

Before starting, close Goatpad and make sure the intended changes are complete.

## 2. Choose the Version

Use a valid semantic version such as `1.1.0`.

Update only the package version in `Cargo.toml`:

```toml
version = "X.Y.Z"
```

Do not manually edit `Cargo.lock`. Cargo updates the package entry when a
normal Cargo command runs.

The MSI builder reads the version from `Cargo.toml` and creates:

```text
dist\Goatpad-X.Y.Z-x64.msi
```

## 3. Validate the Source

Run these commands before packaging:

```powershell
cargo fmt -- --check
cargo check
cargo test
```

All commands must finish successfully. Fix failures before continuing.

## 4. Build the Release MSI

Build the optimized executable and installer:

```powershell
.\build-msi.cmd
```

The script performs these actions:

1. Builds `target\release\goatpad.exe`.
2. Ensures the WiX UI extension is available.
3. Creates `dist\Goatpad-X.Y.Z-x64.msi`.

Confirm the file exists:

```powershell
Test-Path .\dist\Goatpad-X.Y.Z-x64.msi
```

For a local installation test, run:

```powershell
.\build-msi.cmd --install
```

Close Goatpad before installing or upgrading. The installer is 64-bit and
installs the application under `Program Files`.

## 5. Test the Packaged Application

Test the actual release build, not only `cargo run`.

At minimum, verify:

- Goatpad launches successfully.
- Existing notes, settings, and themes are preserved during an upgrade.
- Opening, editing, saving, importing, and exporting notes work.
- The Start menu and desktop shortcuts work.
- The application icon is present.
- Settings and the update controls open normally.

The installer does not remove user data from:

```text
%LOCALAPPDATA%\Goatpad
```

## 6. Calculate the SHA-256 Checksum

Run:

```powershell
(Get-FileHash .\dist\Goatpad-X.Y.Z-x64.msi -Algorithm SHA256).Hash.ToLower()
```

Copy the complete lowercase hash. It must be placed in `updates/manifest.json`
exactly as generated.

## 7. Commit and Tag the Source Release

Review the changes before committing:

```powershell
git diff -- Cargo.toml Cargo.lock
git status
```

Commit the version change and any related release documentation:

```powershell
git add Cargo.toml Cargo.lock README.md updates/NOTES.md
git commit -m "release: vX.Y.Z"
git tag -a vX.Y.Z -m "vX.Y.Z"
```

Only stage files that actually belong to the release. Do not include unrelated
work.

## 8. Push the Source and Tag

Push the release commit and tag:

```powershell
git push origin main
git push origin vX.Y.Z
```

If the default branch is different, replace `main` with the correct branch.

## 9. Create the GitHub Release

Create the GitHub release and upload the MSI:

```powershell
gh release create vX.Y.Z `
	.\dist\Goatpad-X.Y.Z-x64.msi `
	--title "vX.Y.Z" `
	--notes "Describe the changes in vX.Y.Z here."
```

The repository is currently configured as:

```text
Yared-Betemariam/goatpad
```

Confirm that the release asset is downloadable before publishing the update
manifest:

```text
https://github.com/Yared-Betemariam/goatpad/releases/download/vX.Y.Z/Goatpad-X.Y.Z-x64.msi
```

## 10. Update the In-App Update Manifest

Edit `updates/manifest.json` with the new version, release asset URL, checksum,
and notes:

```json
{
  "version": "X.Y.Z",
  "msi_url": "https://github.com/Yared-Betemariam/goatpad/releases/download/vX.Y.Z/Goatpad-X.Y.Z-x64.msi",
  "sha256": "PASTE_THE_LOWERCASE_SHA256_HERE",
  "notes": "Describe the changes in vX.Y.Z here."
}
```

Important requirements:

- `version` must be newer than the installed version.
- Both URLs must use HTTPS.
- `msi_url` must exactly match the GitHub release asset name.
- `sha256` must match the MSI byte-for-byte.
- Keep the JSON valid.

The application reads this manifest from:

```text
https://raw.githubusercontent.com/Yared-Betemariam/goatpad/main/updates/manifest.json
```

Commit and push the manifest only after the GitHub release asset is available:

```powershell
git add updates/manifest.json
git commit -m "update: publish vX.Y.Z update manifest"
git push origin main
```

## 11. Verify the Published Manifest

Open the raw manifest URL in a browser or fetch it from PowerShell:

```powershell
Invoke-WebRequest `
	https://raw.githubusercontent.com/Yared-Betemariam/goatpad/main/updates/manifest.json |
	Select-Object -ExpandProperty Content
```

Check that it contains the expected version, MSI URL, checksum, and notes.

## 12. Test the In-App Update Flow

Use an installed copy of the previous version, such as v1.0.0.

1. Start the previous version.
2. Use **Settings -> Updates** or the File menu to check for updates.
3. Confirm that vX.Y.Z is detected.
4. Start the download.
5. Confirm that checksum verification succeeds.
6. Confirm that Goatpad closes and the elevated MSI installer starts.
7. Launch the upgraded application.
8. Confirm that notes, settings, themes, and session data remain intact.

## Release Order

The safe order is:

```text
Build MSI -> calculate hash -> create GitHub release -> upload MSI
-> update manifest -> push manifest -> test in-app update
```

Do not publish the manifest before the MSI is available. Existing users can
fetch the manifest immediately, and a premature manifest would point them to a
missing release asset.

## Troubleshooting

### WiX is not found

Install WiX and ensure the global .NET tools directory is on `PATH`:

```powershell
dotnet tool install --global wix --version 7.0.0
```

### The MSI has the wrong version

Check the `version` field in `Cargo.toml`, then run `build-msi.cmd` again. The
MSI filename and installer product version both come from that field.

### The update is not detected

Check that:

- The manifest URL is reachable.
- The manifest version is greater than the installed version.
- The JSON is valid.
- The release asset URL is correct.
- The manifest change has been pushed to `main`.

### SHA-256 verification fails

Regenerate the hash from the exact MSI uploaded to GitHub. Do not rebuild the
MSI after calculating the hash unless you calculate the hash again, because any
binary change produces a different checksum.

### The installer does not start the upgrade

Close all Goatpad windows and verify that the MSI is a 64-bit build. Also check
that Windows has permission to run the elevated installer.
