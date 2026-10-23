# Session Checkpoint (`v1.5.0`)

**Updated:** `2026-10-22T23:22:11-04:00`
**Branch:** `main`
**HEAD Commit:** `bfe59ef` (`tag: v1.5.0`, pushed to `origin/main`)
**Baseline Verification:** `cargo test --all-targets && cargo clippy --all-targets -- -D warnings && cargo build --release` (`91/91` tests passing, `0` warnings)

---

## 1. Completed Goals & User Decisions

1. **License Migration (`dd84700`)**:
   - Switched project license to `Apache-2.0` (`LICENSE`, `Cargo.toml`, `README.md`, `flake.nix`).
2. **Readline / ZLE Prompt Stability & 3-Way Prompt Mode Toggle (`19f1a1b`, `0055f00`, `bfe59ef`)**:
   - Eliminated command prompt flashing under Bash Readline / Zsh ZLE / Oh-My-Bash (`!ICANON && !ECHO` when `fg_pgrp == shell_pgid`).
   - Added 3-way prompt mode toggle (`Floating Prompt` -> `Fixed Prompt` -> `Regular Terminal`) via `F4` / `Alt+E` / `Ctrl+X e` / `:mode [float|fixed|terminal|cycle]` or clicking the bottom OIA status bar.
   - Fixed `sudo` / `sudo-rs` / `ssh` non-canonical `!ECHO` password prompt input capture (`fg_pgrp != shell_pgid && !ECHO` + canvas password-prompt fallback `[sudo` / `Password:` / `passphrase:`) so password prompts in the output pane immediately switch to `RawPtyPassthrough` (`🔒 !ECHO LOCK`) and capture keyboard input.
3. **macOS `Option` Key Normalization & Universal `Ctrl+X` Leader Prefix (`c6ecfa9`)**:
   - Added configurable `leader_key` (`"Ctrl+X"` default) that works in both `CommandPrompt` and `RawPtyPassthrough` modes.
   - Added `normalize_macos_option_key` (`macos_option_as_alt = true`) and Readline word-wise navigation (`Alt+Left`/`Right`, `Alt+B`/`F`, `Alt+Backspace`/`Ctrl+W`, `Alt+D`).
4. **Focal Spotlight Shader (`bfe59ef`, `src/ui/cursor.rs`)**:
   - Added toggleable, resizable **Circular** and **Elliptical** Focal Spotlight (`Alt+O` / `Ctrl+X L` / `:spotlight`) with:
     - **Smooth Fuzzy Mode (`spotlight_pixelated = false`)**: Hermite `smoothstep` radial falloff and core phosphor bloom.
     - **Pixelated Character-Block Mode (`spotlight_pixelated = true`)**: `2×1` character-cell block quantization (`x & !1`) and stepped `▒`/`░` dither rings.
     - Dynamic radius resizing via `Alt+Shift+]` (`Alt+}`) / `Alt+Shift+[` (`Alt+{`) or `:spotlight [+|-|size <rx> [ry]|feather <pct>|full|pane|off]`.
5. **Focal Zoom (Fisheye Magnifying Lens) Shader (`bfe59ef`, `src/ui/cursor.rs`)**:
   - Added toggleable, resizable **Focal Zoom Magnifying Lens** (`Alt+Shift+O` / `:focalzoom` / `:lens`) with smooth Hermite `smoothstep` radial magnification falloff across multiple magnification levels (`1.25x`, `1.50x`, `2.00x`, `3.00x`, or custom `1.05x..=4.00x`), anti-stutter primary-cell glyph placement, continuous box-drawing line stretching, and pixelated `2×1` character-block mode (`:focalzoom pixel`).

---

## 2. Working-Tree & Remote State

- **Git Branch:** `main` (`bfe59ef`, synced with `origin/main` and tagged `v1.5.0` on `origin`).
- **Background Processes:** None.
- **Release Binary:** `./target/release/bilterm` (`v1.5.0`).

---

## 3. Exact Resume Verification Command

```bash
git status --short && cargo test --all-targets && cargo clippy --all-targets -- -D warnings
```
