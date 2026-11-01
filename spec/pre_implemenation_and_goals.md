Before implementing anything, I want you to thoroughly inspect the existing application and understand its architecture.

I am particularly concerned about preserving the existing Ratatui/libghostty terminal integration, the multi-layer architecture, the existing GUI implementation, and the current configuration infrastructure.

I do not want you to rebuild, simplify, replace, or refactor working infrastructure just to accommodate the new visual design.

I want you to work with the architecture that exists.

First, identify the actual UI framework, existing design system, configuration mechanisms, component hierarchy, and terminal integration boundaries.

Then show me:

1. What already exists and should be preserved.
2. What existing UI components should be redesigned.
3. What new UI components are needed.
4. What configuration capabilities need extending.
5. Any architectural changes that are genuinely necessary.
6. A proposed implementation sequence.

Do not modify source files until this assessment is complete and presented.

I want a significant visual and UX transformation for this option, not a superficial theme change. At the same time, I want zero unnecessary architectural disruption. And, I want the existing functionality, visual, and UX to remain accessible. These are accessible from command line switches and in-line commands such as the COLON commands ":ui-style ..."

My final recommendation
There is one thing I would particularly emphasize: don't let Antigravity interpret Liquid Glass as merely a visual skin.

The real opportunity is to create a cohesive visual system in which:
- Panes have consistent material and focus behavior.
- Drawers feel like extensions of the workspace.
- Floating tools feel integrated rather than bolted on.
- Navigation is contextual and unobtrusive.
- Themes control the entire application's appearance.
- Users can independently adjust transparency, blur, borders, density, spacing, and typography.
That is a much more meaningful transformation than simply applying translucent backgrounds and rounded corners.

One thing I'd like to establish before going further: Is your existing GUI built primarily with native Rust rendering (Ratatui or another Rust GUI framework), or does it use a separate frontend framework with Rust handling the terminal/backend? That distinction materially changes how I would instruct Antigravity to implement the Liquid Glass effects.
