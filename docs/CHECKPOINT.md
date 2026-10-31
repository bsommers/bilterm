# Session Checkpoint (2026-10-30)

**Status:** PAUSED — only remaining asset gap is **v2.3.0 Windows**. Last tag: `v2.4.0` (Latest).

## User decisions (verbatim)
- "make MacOS releases for this. also make a skill / command / scripts to do binary release per platform."
- "v2.1.2 - do release. make release page; add MacOS binaries. note that I have to build for Linux and Windows, too." / "v2.1.1 - build and attach v2.1.1. binaries for MacOS to it"
- Merge local vs remote without clobbering either → user: **"yes"** (to the stash / ff-pull / pop plan).
- Workflow should run the script from `main` + attach macOS to v2.2.0–v2.4.0 → user: **"yes"**.

## Done (committed + pushed)
- `066f4e5` fix `scripts/build-release-mac-win.sh --upload` on macOS: `ls bilterm-*.zip` failed under `pipefail` (no zips on mac) → nullglob + printf.
- `d0552c2` `.github/workflows/release-binaries.yml` checks out the default branch (`fetch-depth: 0`) instead of the tag; the script builds the tag in its own worktree, so script fixes now apply to old tags. **Not yet run on GitHub.**
- macOS archives (aarch64 + x86_64, verified arch / `--version` / system-only dylibs, re-downloaded and `SHA256SUMS`-checked) attached to v2.1.1, v2.1.2, v2.2.0, v2.3.0, v2.4.0. Existing Linux/Windows sums were merged, not overwritten.
- v2.1.2 release page created from CHANGELOG (`--latest=false`); v2.1.1 notes footer updated to list macOS.
- Skill `building-release-binaries` committed in swe-skills-2 (`28b79df`); its `build-binaries.sh` (bash 3.2-safe `REPO_ARGS`) is also in this repo as `scripts/build-binaries.sh`.

## Release asset matrix (2026-10-30 20:50, verified via `gh release view`)
| Release | macOS | Windows | Linux |
|---|---|---|---|
| v2.4.0 (Latest) | ✅ | x86_64 + aarch64 msvc | gnu, musl x86_64 + aarch64 musl ✅ |
| v2.3.0 | ✅ | **missing** | gnu, musl x86_64 + aarch64 musl ✅ |
| v2.2.0 | ✅ | msvc | complete |
| v2.1.2 | ✅ | msvc | complete (release notes updated to list all platforms) |
| v2.1.1 | ✅ | gnu (+ `windows-msvc.zip` alias is a copy of the gnu build — same sha) | complete |

- aarch64-unknown-linux-musl for v2.3.0/v2.4.0 built on Linux via `scripts/build-binaries.sh <tag> --targets aarch64-unknown-linux-musl --upload` (static ELF aarch64, embedded `BILTERM v<ver>` checked; not executed — no qemu-aarch64); `SHA256SUMS` merged and re-verified after download.

## Known issues (not fixed)
- ~~`cargo test` rewrites 5 `docs/assets/*.svg`~~ — fixed: screenshots now go to `target/tmp/screenshots/`; refresh committed SVGs with `BILTERM_UPDATE_SCREENSHOTS=1 cargo test -q --test full_suite_battle_and_perf`.
- `scripts/build-release-assets.sh` hardcodes a stale `/tmp/claude-1000/...` zig path; needs `zig` on PATH.
- Local backup of pre-merge working tree: `~/bilterm-local-backup.patch` (safe to delete).

## Lessons
- Never run `cargo fmt` repo-wide (repo is not rustfmt-clean → 100+ file churn; v2.4.0 needed a revert commit `0df2ed5`).
- Run the FULL integration suite (`timeout 900 cargo test -q --tests --no-fail-fast`) before releases, not just lib tests (v2.3.0 shipped a 1-row-grid crash).

## Exact next command
v2.3.0 Windows: build on the Windows VM (`scripts/build-binaries.sh v2.3.0 --upload`) or try the never-run workflow `gh workflow run release-binaries.yml -f tag=v2.3.0` (expect fixes).
