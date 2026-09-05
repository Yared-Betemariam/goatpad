## General Update Notes

Here are some commands and utility functions that will assist in the process of pushing updates.

1. `gh release create vX.Y.Z dist\Goatpad-X.Y.Z-x64.msi --title "vX.Y.Z" --notes "What changed"`

2. `(Get-FileHash .\dist\Goatpad-X.Y.Z-x64.msi -Algorithm SHA256).Hash.ToLower()`
