# Goatpad Release Process

Use this process for each Windows MSI release. Replace `X.Y.Z` with the new
version, for example `1.2.0`.

## Before you start

Required tools:

- Rust and Cargo
- Visual Studio C++ build tools
- .NET SDK
- WiX Toolset 7
- Git and GitHub CLI (`gh`), authenticated to the repository

Install WiX if needed:

```powershell
dotnet tool install --global wix --version 7.0.0
```

The in-app update manifest is read from:

```text
https://raw.githubusercontent.com/Yared-Betemariam/goatpad/main/updates/manifest.json
```

## Release order

The release commit and tag must exist before the GitHub release can be
published. Follow this order:

```text
Finish update -> validate -> build and test MSI -> update manifest
-> make one release commit -> tag and push -> publish GitHub release
-> verify the in-app update
```

## 1. Finish the update

Complete and review every code, asset, documentation, and configuration change
that belongs in the release. Do not start the release until the update is ready.

## 2. Set the version and validate it

Update the package version in `Cargo.toml`:

```toml
version = "X.Y.Z"
```

Run the checks:

```powershell
cargo fmt -- --check
cargo check
cargo test
```

Fix any failures before continuing.

## 3. Build and test the MSI

Build the release installer:

```powershell
.\build-msi.cmd
```

This creates:

```text
dist\Goatpad-X.Y.Z-x64.msi
```

Install and test the MSI. Confirm that the app launches, the main features
work, and existing notes, settings, themes, and session data are preserved.

Calculate the checksum of the exact MSI that will be published:

```powershell
(Get-FileHash .\dist\Goatpad-X.Y.Z-x64.msi -Algorithm SHA256).Hash.ToLower()
```

## 4. Prepare the release files

Update `updates/manifest.json` with the new version, GitHub release asset URL,
lowercase SHA-256 checksum, and release notes:

```json
{
  "version": "X.Y.Z",
  "msi_url": "https://github.com/Yared-Betemariam/goatpad/releases/download/vX.Y.Z/Goatpad-X.Y.Z-x64.msi",
  "sha256": "PASTE_THE_LOWERCASE_SHA256_HERE",
  "notes": "Describe the changes in vX.Y.Z here."
}
```

Check that the manifest is valid and that the MSI URL uses the exact version
and filename.

## 5. Create one release commit

Review the final changes, then create one commit containing everything required
for this update, including the version, manifest, source changes, assets, and
release documentation:

```powershell
git add -A
git commit -m "Release: VX.Y.Z Out"
git tag -a vX.Y.Z -m "vX.Y.Z"
git push origin main --follow-tags
```

Do not create separate commits for the version, manifest, documentation, or
other parts of the same release. Keep unrelated work out of the release commit.

## 6. Publish and verify

Create the GitHub release and upload the exact MSI:

```powershell
gh release create vX.Y.Z `
  .\dist\Goatpad-X.Y.Z-x64.msi `
  --title "vX.Y.Z" `
  --notes "Describe the changes in vX.Y.Z here."
```

After publishing, verify that:

- The MSI downloads from the release URL.
- The raw manifest contains the expected version, URL, checksum, and notes.
- An older installed version detects the update.
- Checksum verification succeeds and the upgrade preserves user data.

