# Session Checkpoint (2026-10-30, Windows 11 ARM64 / win-devvm)

**Status:** PAUSED. Windows ARM64 build + VM pipeline working and verified; nothing pushed, no release assets uploaded.

## User request (verbatim)
"extend https://github.com/bsommers/win-devvm to use local Paralells on Mac, provision Windows 11 dev VM, build bilterm for Windows 11 ARM. update build and release scripts. update win-devvm repo. install bilterm in windows 11 vm, test. report back to me"

## Local commits (NOT pushed)
- bilterm branch `feat/windows-arm64`: `8a5f08e` ARM64 build flags, `build-windows-vm.sh`, `windows-smoke-test.ps1`, CI `windows-arm64` job, install.ps1 native arch + `-Archive`; `84b732d` accurate `-Prefix '??'` bug note; `439a5c3` spec-compliant zips (no `\` entries), ctl endpoint in smoke test; `a9bc7fe` Windows-portable page-hold test, unix-gated import.
- win-devvm (`~/src/tools/win-devvm`) branch `feat/parallels-arm64`: `7a1e361` Parallels backend + all fixes + docs.

## Verified (Windows 11 ARM64 26100.4349, Parallels Standard, VM `win-devvm`, 4 vCPU / 8 GB)
- `scripts/build-windows-vm.sh v2.4.0 --arch all --test`: builds aarch64 + x86_64 MSVC in the VM (warm ~35 s), archives in `target/release-dist/v2.4.0/`; smoke test 9/9 PASS for both under Windows PowerShell 5.1.
- `cargo test --release --no-fail-fast` natively on ARM64: 288 passed, 0 failed (after the test fix in `a9bc7fe`; before: 287/1, `/bin/sh` test).
- Interactive TUI in ConPTY over `ssh -tt`: command round-trip in a pane, Ctrl+Q exit. `bilterm --gui`: Win32 window opens in the desktop session.
- `install.ps1` from main: `-Prefix` fails under PS 5.1 (`??` passed to Resolve-Path); fixed version works.

## Not done / open
1. **Clean reprovision proof** of the last win-devvm fixes (EULA screen: removed SkipMachineOOBE/SkipUserOOBE + oobeSystem locale; `winget --source winget`; installed-check). Run: `cd ../win-devvm && WIN_ISO=/Volumes/mini-m4-2tb/ISOs/26100.4349.250607-1500.ge_release_svc_refresh_CLIENTCONSUMER_RET_A64FRE_en-us.iso ./provision.sh --force && ./vm.sh wait` — expect no clicks and `tools: DONE`. (Destroys the current VM.)
2. libvirt (Linux) path of win-devvm refactored but **untested** — run `./provision.sh` once on the Linux host.
3. Push both branches + open PRs (ask user first).
4. Optional: attach v2.4.0 Windows archives — `scripts/build-release-mac-win.sh v2.4.0 --windows --windows-arm64 --no-build --upload` (ask user first; v2.4.0 has no Windows assets today).
5. CI `windows-arm64` job (windows-11-arm runner) not yet run on GitHub.
6. Minor: unused imports in `tests/live_daemon_detach_attach.rs` on Windows (warning only).

## Parallels Standard facts (hard-won)
- prlctl only `list`/`register`/`unregister` work. Start = `open` an *unregistered* bundle (pre-registered → opens stopped + "already registered" dialog). Stop = `shutdown` over SSH; kill = SIGKILL `prl_vm_app --uuid {…}`.
- "Press any key" avoided by remastering with `efisys_noprompt.bin` (`hdiutil makehybrid -udf … -eltorito-platform 0xEF`).
- Parallels disconnects the install ISO after its Tools install → payload staged to `C:\win-devvm` in specialize.
- Guest IP from `/Library/Preferences/Parallels/parallels_dhcp_leases` by MAC.

## Previous checkpoint (release asset backfill) — still open
v2.4.0/v2.3.0: Windows missing (now buildable, see item 4), aarch64 musl missing; v2.1.2: Windows + Linux missing. `gh workflow run release-binaries.yml -f tag=v2.4.0` never run.
