# Attribution

This independent Linux implementation is based on TinyHand by Kamran Ahmed,
https://github.com/nilbuild/tinyhand, licensed under MIT.

The 36 `assets/hand_*.png` files are downsampled from the original artwork.
All six `assets/sounds/tap_*.wav` samples and the icon originate in the same repository.
The original license is preserved at `assets/LICENSE.upstream` and included
in every distribution package.

The Rust settings interface is adapted from the original `SettingsView.swift`
and `SettingsWindow.swift` design, reviewed at commit
`c5fb36b7c4d7c87480958872a724b30a52427a47`. Original Swift/AppKit code is not
compiled into the Linux application.
