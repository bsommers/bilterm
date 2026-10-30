# Session Checkpoint (2026-10-30)

**Status:** PAUSED — release asset backfill partly done; Windows + some Linux archives still missing. Last tag: `v2.4.0` (Latest).

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

## Release asset matrix (remaining gaps)
| Release | macOS | Windows | Linux |
|---|---|---|---|
| v2.4.0 (Latest) | ✅ | **missing** | gnu, musl x86_64; **aarch64 musl missing** |
| v2.3.0 | ✅ | **missing** | gnu, musl x86_64; **aarch64 musl missing** |
| v2.2.0 | ✅ | msvc | complete |
| v2.1.2 | ✅ | **missing** | **missing** (notes say "not attached yet") |
| v2.1.1 | ✅ | gnu (+ `windows-msvc.zip` alias is a copy of the gnu build — same sha) | complete |

## Known issues (not fixed)
- `cargo test` (`tests/full_suite_battle_and_perf.rs`) rewrites 5 `docs/assets/*.svg` → dirty tree after every test run. Proposed: write under `target/` or behind an env var.
- `scripts/build-release-assets.sh` hardcodes a stale `/tmp/claude-1000/...` zig path; needs `zig` on PATH.
- Local backup of pre-merge working tree: `~/bilterm-local-backup.patch` (safe to delete).

## Exact next command
`gh workflow run release-binaries.yml -f tag=v2.4.0` (first real run — watch it; expect fixes). Then repeat for v2.3.0, v2.1.2; Linux gaps via `scripts/build-release-assets.sh <tag>` on the Linux box; after v2.1.2 is complete, drop its "not attached yet" note.
