# Goatpad

Goatpad is a native Windows-first Markdown and plain-text editor built with Rust and egui. It automatically saves changes, restores its workspace and session, and keeps documents in the local Goatpad app-data folder.

## Portable build

Build the release executable with:

```powershell
cargo build --release
```

The portable application is `target\release\goatpad.exe`. It needs no installer; copy that file to a folder of your choice and run it. User documents, themes, settings, and session data remain under the Windows Goatpad app-data directory.

## Current features

- Persistent Markdown and plain-text tabs with session restore
- Background, atomic autosave (400 ms debounce; 2 s maximum dirty window)
- Markdown highlighting, type switching, formatting shortcuts, and configurable themes
- Keyboard-first tab controls: `Ctrl+T`, `Ctrl+Tab`, `Ctrl+Shift+Tab`, `Ctrl+Shift+W`, and `Ctrl+,`
- Windows window/executable icon and an active-document title bar
- In-app error notifications for failed writes and unreadable document files

## Known gaps

- There is no import/export flow yet.
- The Phase 4 visual Markdown-highlighting QA remains outstanding.
- Packaging is currently a portable `.exe`, not an MSI installer.
- macOS and Linux packaging are intentionally deferred to Phase 8.
