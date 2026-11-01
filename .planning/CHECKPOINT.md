# Bilterm Session Checkpoint (2026-11-01)

## User decisions (verbatim)
- "commit the baseline and update the roadmap with phases 43-48; relax ARCH-R04 ... fuzzy target of ~800-1000 lines ... full use of libghostty if it makes sense. add a path, but do not remove or rewire existing paths ... paths living in harmony"
- "yes, start Phase 44 design previews"
- "show me the dispatch plan so I can run the correct stuff here in claude and give the other to agy in another window"

## State
- Branch main. Commits: c1da655 (v2.5.4 baseline), 67b9c64 (phases 43-48, ARCH-R04 soft, 1200 backstop), ec30c3d (design_preview example + 30 baseline PNGs + roadmap subagent model).
- Uncommitted: PROPOSAL.html, this checkpoint.
- Phase 44: baseline renders done (`cargo run -q --example design_preview -- <dir>`); proposal artifact https://claude.ai/artifact/DHtLf6cfQAUwST9vKqRL8v. AWAITING user sign-off (gate G1) and 4 open decisions in the proposal.
- swe-skills-2 skills identical to ~/.claude/skills (no sync needed). agy at ~/.local/bin/agy supports `-p/--print`, `--model`, `--mode`, `--dangerously-skip-permissions`.
- No background jobs. Dead end: codegraph not indexed for this repo.

## Next
After sign-off: Phase 45-01 (token API, orchestrator/opus, serial), then API FROZEN gate, then 45-03 fan-out.
