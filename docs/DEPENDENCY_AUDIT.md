# Dependency Security & Supply Chain Audit Report (`dependency-audit`)

> **Date:** `2026-10-25`  
> **Repository:** `bilterm` (`v1.10.0`)  
> **Ecosystem:** Rust (`Cargo.toml` / `Cargo.lock`, Edition 2024, MSRV `1.88`)  
> **Total Resolved Crates Audited:** `247` (down from `260` after pruning `vt100-ctt` default `tui-term` feature)  
> **Security Status:** **Clean (`0` CVE Vulnerabilities, `1` Informational Upstream Advisory)**

---

## 1. 🚨 Vulnerability & Advisory Summary (`cargo audit`)

Scanned against the RustSec Advisory Database (`1,352` advisories loaded):
- **Critical / High / Medium / Low CVEs**: **`0`**

### Remediated Advisories in This Audit

| Crate | Version | Advisory ID | Type | Resolution Applied |
| :--- | :---: | :--- | :---: | :--- |
| `paste` | `1.0.15` | `RUSTSEC-2024-0436` | `unmaintained` | ✅ **Removed from `Cargo.lock`** by setting `default-features = false` on `vt100-ctt` (which previously pulled `tui-term` + duplicate `ratatui 0.29.0`) |
| `lru` | `0.12.5` | `RUSTSEC-2026-0002`, `RUSTSEC-2026-0253` | `unsound` | ✅ **Removed from `Cargo.lock`** via `vt100-ctt` `default-features = false` |

### Remaining Informational Upstream Warning

| Crate | Version | Transitive Path | Advisory ID | Risk Assessment |
| :--- | :---: | :--- | :--- | :--- |
| `lru` | `0.16.4` | `ratatui 0.30.0` $\rightarrow$ `ratatui-core 0.1.0` $\rightarrow$ `lru 0.16.4` | `RUSTSEC-2026-0253` | **Informational / Not Exploitable in Bilterm**: Triggered only if `LruCache::pop()` is called with a user key type whose `Borrow::borrow` / `Hash` / `Eq` panics mid-probe. `ratatui-core` uses `LruCache` solely for internal `Layout` geometry caching with infallible integer/tuple keys and never calls `pop()`. |

---

## 2. 📦 Direct Dependencies Scorecard

| Direct Dependency | Pinned Spec | Resolved Version | Purpose | Maintenance & Health |
| :--- | :---: | :---: | :--- | :--- |
| `ratatui` | `0.30.0` | `0.30.0` | Immediate-mode TUI cell compositor | Active (Latest `0.30` series) |
| `crossterm` | `0.29.0` | `0.29.0` | Host terminal raw mode, SGR mouse, Kitty keys | Active (Paired with `ratatui 0.30`) |
| `portable-pty` | `0.9.0` | `0.9.0` | Cross-platform POSIX/ConPTY process management | Active (WezTerm upstream) |
| `vt100-ctt` | `0.17.1` (`default-features = false`) | `0.17.1` | VT100/ANSI screen state machine | Active (`tui-term` bloat removed) |
| `tokio` | `1.44.0` | `1.52.1` | Async runtime, `select!`, timers, IPC server | Active |
| `futures` | `0.3.31` | `0.3.32` | `EventStream` async stream combinators | Active |
| `nucleo-matcher` | `0.3.1` | `0.3.1` | Lock-free fuzzy matcher (`Ctrl+R`, `Ctrl+P`) | Active (Helix upstream) |
| `serde` / `toml` | `1.0.219` / `0.8.20` | `1.0.228` / `0.8.23` | `config.toml`, sessions, playbooks | Active |
| `color-eyre` / `thiserror` | `0.6.3` / `2.0.12` | `0.6.5` / `2.0.18` | Panic hook terminal restore & error types | Active |
| `tracing` / `tracing-subscriber` / `tracing-appender` | `0.1.41` / `0.3.19` / `0.2.3` | `0.1.44` / `0.3.23` / `0.2.4` | Non-blocking `bilterm.log` telemetry | Active |
| `unicode-width` | `0.2.0` | `0.2.2` | Wide CJK/Emoji grapheme width calculation | Active |
| `libc` | `0.2.170` | `0.2.185` | `termios`, `tcgetpgrp`, `TIOCGWINSZ`, `chmod` | Active |
| `libghostty-vt` *(optional)* | `0.2.2` | `0.2.2` | Opt-in `--features ghostty-vt` C-ABI backend | Experimental (gated) |

---

## 3. 📜 License & Supply Chain Compliance

- **Bilterm Package License**: `Apache-2.0`
- **Resolved Dependency Licenses (`247` crates)**:
  - `MIT OR Apache-2.0` / `MIT` / `Apache-2.0`: `235` crates (`95.1%`)
  - `Zlib` / `BSD-2-Clause` / `0BSD` / `Unlicense` / `Unicode-3.0`: `11` crates (`4.5%`)
  - `MPL-2.0` (weak file-level copyleft, compatible with `Apache-2.0` binary distribution): `1` crate (`option-ext 0.2.0`)
- **Strong Copyleft (`GPL` / `AGPL`) Count**: **`0` crates**
