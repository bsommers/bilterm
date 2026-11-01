# EXISTING TERMINAL APPLICATION — UI MODERNIZATION AND DESIGN SYSTEM TRANSFORMATION

## PRIMARY DIRECTIVE

Modernize the existing terminal application's user interface, visual design system, interaction model, and user-facing configurability.

This is an EXISTING APPLICATION ENHANCEMENT PROJECT.

It is NOT a new application development project.

The application already has a substantial, functioning, multi-layer terminal architecture, including existing terminal infrastructure built around Ratatui and libghostty, as well as an existing GUI and configuration infrastructure.

Your responsibility is to elevate the user experience and visual sophistication of the existing application while preserving its underlying architecture and functionality.

The desired outcome is a premium, minimalist, highly configurable desktop terminal environment inspired by ZenTerm, modern macOS interface design, and Apple's Liquid Glass visual language.

The finished application should feel purpose-built, cohesive, sophisticated, and exceptionally well engineered.

---

# 1. NON-NEGOTIABLE ARCHITECTURAL CONSTRAINTS

Before making any modifications, inspect and understand the existing repository.

The existing application is the source of truth.

Do not assume that its architecture needs to be replaced or simplified.

## 1.1 Preserve existing infrastructure

Preserve the existing:

- Ratatui integration.
- libghostty integration.
- Terminal rendering architecture.
- Terminal session management.
- PTY infrastructure.
- Shell process management.
- Multi-layer application architecture.
- Existing GUI framework.
- Existing GUI component architecture.
- Configuration and preferences infrastructure.
- Workspace and session management.
- Terminal input/output handling.
- Keyboard event processing.
- Existing inter-process communication.
- Application lifecycle management.
- Existing persistence mechanisms.
- Existing error handling and recovery mechanisms.

Do not replace working functionality with mock implementations.

Do not introduce xterm.js, Electron, React, Tauri, or another framework simply because it is suggested by a generic terminal application architecture.

Do not introduce an alternative terminal engine.

Do not replace existing application services with newly invented equivalents.

Do not duplicate existing functionality.

Do not change the underlying terminal architecture to accommodate a visual redesign.

If an existing architectural limitation genuinely prevents a desired interface improvement, document the limitation and propose the smallest possible compatible modification.

## 1.2 Architectural modification policy

Use the existing application abstractions and patterns.

Prefer:

- Extending existing components.
- Improving existing component styling.
- Reusing existing configuration models.
- Adding new visual components where necessary.
- Improving layout composition.
- Extending existing theme infrastructure.
- Adding configuration options to existing settings.
- Improving existing interactions.

Avoid:

- Wholesale rewrites.
- Unnecessary abstraction layers.
- Duplicate state management.
- Parallel configuration systems.
- Replacing established dependencies.
- Introducing unnecessary dependencies.
- Breaking existing interfaces.

All modifications must remain compatible with the existing architecture.

## 1.3 Mandatory initial repository assessment

Before implementation, inspect the codebase and produce an architecture assessment.

Identify:

1. Existing frontend/UI framework.
2. Main application entry points.
3. Application shell and window structure.
4. Existing component hierarchy.
5. Existing styling system.
6. Existing theme implementation.
7. Existing configuration model.
8. Existing settings interface.
9. Existing terminal rendering integration.
10. Existing pane and layout management.
11. Existing session and workspace management.
12. Existing drawer and floating panel implementations, if present.
13. Existing keyboard shortcut infrastructure.
14. Existing persistence implementation.
15. Existing application state management.

Identify which capabilities already exist and which require enhancement.

Create a mapping between the existing components and the proposed redesigned components.

Do not implement functionality that already exists merely because it appears in this specification.

The objective is to improve the existing product, not recreate it.

---

# 2. DESIGN VISION

Transform the existing interface into a modern, premium terminal workspace.

The visual direction should combine:

- ZenTerm's workspace-oriented interaction model.
- Apple's Liquid Glass material language.
- The restraint and precision of modern developer applications.
- Minimalist desktop application design.
- High-quality typography.
- Sophisticated layering and depth.
- Highly configurable visual presentation.

The design must prioritize the terminal itself.

The surrounding application chrome should provide functionality without competing for attention.

## 2.1 Core design philosophy

Maximum functionality.

Minimum visual noise.

The interface should feel:

- Clean.
- Minimal.
- Refined.
- Spacious.
- Modern.
- Responsive.
- Configurable.
- Professional.
- Visually cohesive.

Avoid the appearance of a traditional terminal application with decorative panels added around it.

The application should feel like a unified environment.

## 2.2 Visual references

Use the following as design references, not implementation templates.

ZenTerm:
https://zenterm.io/

Apple Liquid Glass:
https://developer.apple.com/documentation/technologyoverviews/liquid-glass

Zed:
https://zed.dev/

Ghostty:
https://ghostty.org/

The references serve different purposes:

- ZenTerm: workspace organization and terminal interaction patterns.
- Liquid Glass: material, translucency, depth, and layering.
- Zed: visual restraint, typography, and developer-oriented UX.
- Ghostty: terminal behavior and terminal-engine philosophy.

Do not copy any application's interface literally.

Develop a consistent design language appropriate to this application.

---

# 3. LIQUID GLASS DESIGN SYSTEM

Create or substantially enhance the existing design system to support a configurable Liquid Glass aesthetic.

This must be implemented as a reusable design system, not a collection of individually styled components.

## 3.1 Material system

Establish reusable material definitions.

Suggested materials:

### Material 1: Background

The foundational application surface.

Properties:
- Deep graphite or configurable neutral color.
- Minimal visual texture.
- Optional subtle gradient.
- Configurable opacity.

### Material 2: Primary Glass

Used for navigation bars, primary controls, and major interface surfaces.

Properties:
- Translucent background.
- Background blur.
- Fine border.
- Subtle shadow.
- Optional edge illumination.

### Material 3: Secondary Glass

Used for drawers, sidebars, contextual panels, and secondary controls.

Properties:
- Greater translucency.
- Softer separation.
- Subtle depth.
- Configurable blur.

### Material 4: Floating Glass

Used for command palettes, popovers, contextual menus, and floating tool windows.

Properties:
- Higher visual separation.
- Translucent surface.
- Soft shadow.
- Fine border.
- Optional backdrop blur.

### Material 5: Terminal Surface

Used for actual terminal content.

This material must prioritize readability and rendering performance.

It should support:
- Solid backgrounds.
- Slightly translucent backgrounds where supported.
- Independent terminal background colors.
- Configurable text contrast.
- Independent focused/unfocused appearance.

Do not sacrifice terminal readability to achieve glass effects.

## 3.2 Design tokens

Create centralized, reusable design tokens for:

- Background colors.
- Surface colors.
- Foreground colors.
- Accent colors.
- Border colors.
- Focus colors.
- Selection colors.
- Opacity.
- Blur.
- Shadows.
- Corner radii.
- Spacing.
- Typography.
- Animation durations.
- Elevation.
- Material definitions.

All UI components must use these tokens.

Avoid hardcoded visual values scattered throughout the codebase.

## 3.3 Glass effect configuration

Provide user-configurable controls for:

- Overall glass intensity.
- Background blur.
- Surface transparency.
- Border opacity.
- Shadow intensity.
- Edge illumination.
- Gradient intensity.
- Background dimming.
- Surface separation.

Include a master glass-effects toggle.

When disabled, the application should transition to a clean opaque interface without breaking layouts.

Support reduced-transparency and reduced-motion accessibility preferences.

Where the existing GUI framework or operating system does not support true backdrop blur, implement an appropriate visual fallback.

Do not simulate expensive effects that materially reduce rendering performance.

---

# 4. MAIN APPLICATION INTERFACE

Redesign the main application shell.

The terminal workspace should receive the majority of the available screen area.

## 4.1 Application chrome

Create a compact, visually restrained top navigation area.

Potential elements:

- Native window controls.
- Workspace selector.
- Session/tab navigation.
- Split-pane controls.
- Command palette.
- Appearance controls.
- Settings access.
- Optional connection indicators.

Avoid permanently displaying every possible control.

Use contextual visibility, hover states, and command-palette access.

Make toolbar contents configurable.

Support:
- Adjustable height.
- Adjustable opacity.
- Background material selection.
- Configurable button visibility.
- Optional auto-hide.
- Compact and expanded modes.

## 4.2 Display modes

Provide three interface modes.

**Standard Mode**

Displays the primary navigation and configured controls.

**Minimal Mode**

Reduces the interface chrome and prioritizes the terminal.

**Zen Mode**

Provides an immersive terminal experience.

In Zen Mode:
- Hide unnecessary navigation.
- Remove nonessential status indicators.
- Maximize available terminal space.
- Reveal controls contextually.
- Preserve access to all application functionality through keyboard shortcuts and appropriate gestures.

Transitions between modes should be smooth and configurable.

## 4.3 Workspace navigation

Improve the visual presentation of existing workspaces and sessions.

If workspace functionality already exists, extend it rather than implementing another workspace system.

The interface should make it easy to identify:

- Current workspace.
- Active terminal.
- Other terminal sessions.
- Available workspaces.
- Running sessions.

Support compact visual navigation.

Avoid excessive permanent tab bars or large navigation panels.

---

# 5. TERMINAL PANE VISUAL DESIGN

The terminal is the central element of the application.

Preserve its existing rendering and session infrastructure.

Concentrate on its surrounding presentation and integration with the GUI.

## 5.1 Pane appearance

Each terminal pane should have:

- Configurable background.
- Configurable corner radius.
- Subtle border.
- Adjustable internal padding where supported.
- Minimal visual separation.
- Clear focus indication.
- Consistent alignment.

Suggested default appearance:

- 10–14px corner radius.
- 1px subtle border.
- Low-contrast separator.
- Minimal focused-pane illumination.
- No permanent decorative toolbar.

## 5.2 Focus behavior

The active pane should be identifiable immediately.

Use subtle visual cues:

- Slightly brighter border.
- Configurable accent.
- Small edge illumination.
- Optional title or session indicator.

Unfocused panes should remain readable.

Do not use strong glowing effects.

## 5.3 Pane interactions

Where supported by the existing infrastructure, expose contextual pane actions:

- Split.
- Maximize.
- Restore.
- Close.
- Rename.
- Move.
- Swap.
- Copy session.
- Change pane appearance.

Controls should appear on hover, focus, or through a contextual menu.

Do not introduce unnecessary permanent controls into the terminal area.

## 5.4 Pane layout configuration

Expose existing pane layout capabilities through the GUI.

Where supported, configure:

- Pane spacing.
- Pane borders.
- Corner radius.
- Split separators.
- Focus indicators.
- Default split direction.
- Pane padding.
- Pane title visibility.

Preserve existing layout and resizing behavior.

---

# 6. DRAWERS, SIDE PANELS, AND FLOATING WINDOWS

Modernize the presentation of existing auxiliary terminal surfaces.

Do not duplicate existing drawer or session management functionality.

## 6.1 Bottom drawer

If supported by the current architecture, provide a polished bottom drawer for:

- Secondary terminal sessions.
- Build output.
- Development servers.
- Log monitoring.
- Background tasks.

Visual requirements:

- Translucent glass surface.
- Subtle separation.
- Configurable height.
- Smooth transitions.
- Minimal header.
- Optional auto-hide.
- Configurable border and corner treatment.

## 6.2 Right drawer

Provide an optional contextual right-side panel.

Potential content:

- Session overview.
- Secondary terminal.
- Workspace information.
- Configured tools.
- System information.
- Existing application utilities.

Support:
- Configurable width.
- Collapsed and expanded modes.
- Smooth transitions.
- Independent appearance settings.

## 6.3 Floating panels

Modernize existing floating window and popup functionality.

Floating panels should have:

- Translucent surfaces.
- Subtle shadows.
- Fine borders.
- Rounded corners.
- Compact controls.
- Configurable dimensions.
- Consistent typography.

Ensure floating panels maintain appropriate focus and stacking behavior.

---

# 7. ADVANCED VISUAL CONFIGURATION

This is a major feature requirement.

The existing application already has configuration infrastructure. Extend it to support a comprehensive visual configuration system.

Do not create a separate, disconnected configuration framework.

## 7.1 Appearance settings

Create a cohesive appearance settings interface.

Organize settings into sections.

### General appearance

- Dark mode.
- Light mode.
- System appearance.
- Interface density.
- Overall visual intensity.
- Accent color.
- Background color.

### Glass materials

- Enable glass.
- Glass intensity.
- Background blur.
- Surface opacity.
- Border opacity.
- Shadow intensity.
- Edge illumination.
- Gradient intensity.

### Layout

- Corner radius.
- Pane spacing.
- Panel spacing.
- Sidebar width.
- Toolbar height.
- Drawer dimensions.
- Floating window dimensions.

### Typography

- Interface font.
- Terminal font, using existing terminal configuration.
- Interface font size.
- Terminal font size, using existing terminal configuration.
- Font weight.
- Line height.
- Letter spacing.
- Ligatures, where supported.

### Interface elements

- Toolbar visibility.
- Sidebar visibility.
- Status bar visibility.
- Pane titles.
- Focus indicators.
- Contextual controls.
- Window borders.

## 7.2 Live preview

The settings interface must provide a live preview.

When users adjust appearance settings:

- Changes should appear immediately.
- The entire interface should update consistently.
- The terminal should not restart.
- Existing sessions should remain active.
- Configuration should persist through the existing persistence mechanism.

Avoid requiring application restarts for purely visual changes.

## 7.3 Theme profiles

Provide several initial visual profiles.

**Obsidian**
Deep graphite, muted silver, restrained highlights.

**Midnight**
Dark navy, subtle blue accents, cool neutral borders.

**Graphite**
Neutral charcoal, gray surfaces, monochromatic appearance.

**Arctic**
Light surfaces, cool gray borders, pale blue accents.

**Aurora**
Dark glass, restrained purple and teal accents.

**Monochrome**
Black, white, and grayscale.

**Liquid**
Translucent graphite surfaces, subtle blue highlights, layered glass.

**Custom**
User-defined appearance.

Allow users to:
- Select a profile.
- Duplicate a profile.
- Modify a profile.
- Save a profile.
- Import a profile.
- Export a profile.
- Reset a profile.

Changing a profile must not modify terminal session state.

## 7.4 Configuration consistency

Ensure that:

- GUI settings and underlying configuration remain synchronized.
- Existing configuration values are preserved.
- Missing new configuration values receive sensible defaults.
- Existing user configurations migrate safely.
- Invalid configuration values are handled gracefully.
- Theme changes do not overwrite unrelated settings.

Do not break backward compatibility with existing configuration files.

---

# 8. COMMAND PALETTE AND CONTEXTUAL INTERACTION

If the existing application already has a command palette, modernize its appearance and interaction model.

Otherwise, implement it using existing application command infrastructure.

The command palette should have:

- Centered floating panel.
- Glass material.
- Search input.
- Fuzzy search.
- Grouped commands.
- Keyboard navigation.
- Shortcut display.
- Smooth appearance and dismissal.

Suggested command groups:

APPLICATION
- Open Settings.
- Toggle Zen Mode.
- Toggle Minimal Mode.
- Toggle Sidebar.

WORKSPACE
- Switch Workspace.
- Create Workspace.
- Rename Workspace.

TERMINAL
- New Terminal.
- Split Pane.
- Maximize Pane.
- Close Pane.
- Search Terminal.

APPEARANCE
- Change Theme.
- Toggle Glass.
- Change Accent Color.
- Open Appearance Settings.

All commands must invoke existing application functionality wherever it exists.

Do not implement parallel command execution pathways.

---

# 9. KEYBOARD AND INTERACTION DESIGN

Preserve existing keyboard behavior.

The visual redesign must not break terminal keyboard input, shell shortcuts, or existing application bindings.

Where the existing shortcut infrastructure allows, improve:

- Shortcut discoverability.
- Shortcut configuration.
- Contextual shortcut display.
- Command palette integration.
- Navigation between panes.
- Navigation between workspaces.

Avoid intercepting keyboard events that should be delivered to the terminal.

All interactions should have clear focus and hover states.

---

# 10. ANIMATION AND MICRO-INTERACTIONS

Use restrained, high-quality animation.

Suggested durations:

- Hover transitions: 100–150ms.
- Button feedback: 100–150ms.
- Drawer transitions: 180–250ms.
- Floating panels: 150–220ms.
- Dialogs: 150–200ms.
- Theme transitions: 200–300ms.

Use consistent easing curves.

Animations should communicate state changes, not provide decoration.

Support reduced-motion preferences.

Avoid animations that interfere with terminal input, rendering, or resizing.

---

# 11. PERFORMANCE AND RENDERING REQUIREMENTS

This is a terminal application, not an animated dashboard.

Visual improvements must not compromise the application's existing performance characteristics.

Requirements:

- Preserve existing terminal rendering performance.
- Avoid unnecessary component re-rendering.
- Avoid unnecessary terminal redraws.
- Avoid coupling terminal output events to unrelated UI updates.
- Avoid excessive blur and compositing layers.
- Avoid expensive background animations.
- Avoid unnecessary allocations in frequently executed rendering paths.
- Preserve responsive keyboard input.
- Preserve smooth scrolling.
- Maintain appropriate separation between terminal rendering and GUI presentation.

Profile changes should affect only the relevant UI components.

Visual state changes must not recreate terminal sessions.

---

# 12. IMPLEMENTATION PROCESS

Implement the project incrementally.

Do not attempt a wholesale redesign in a single uncontrolled code-generation pass.

## PHASE 0 — REPOSITORY AUDIT

Inspect the entire relevant codebase.

Deliver:

1. Existing architecture map.
2. Existing UI component inventory.
3. Existing theme and configuration assessment.
4. Existing terminal integration assessment.
5. Existing functionality inventory.
6. Proposed component modifications.
7. Proposed new components.
8. Potential architectural risks.
9. Implementation plan.

Identify what can be reused.

Identify what should be enhanced.

Identify what genuinely needs to be added.

Do not make destructive changes during this phase.

## PHASE 1 — DESIGN SYSTEM

Implement the foundational visual system.

Deliver:

- Centralized design tokens.
- Material definitions.
- Typography system.
- Color system.
- Spacing system.
- Corner-radius system.
- Shadow system.
- Animation system.
- Glass effect configuration.

Verify that all new styling uses the centralized system.

## PHASE 2 — APPLICATION SHELL REDESIGN

Modernize:

- Main window.
- Top navigation.
- Workspace navigation.
- Terminal container.
- Pane presentation.
- Sidebar.
- Status bar.

Preserve all existing functionality.

## PHASE 3 — AUXILIARY INTERFACE REDESIGN

Modernize:

- Bottom drawer.
- Right drawer.
- Floating panels.
- Contextual menus.
- Dialogs.
- Command palette.
- Popovers.

Preserve all existing session and process behavior.

## PHASE 4 — APPEARANCE CONFIGURATION

Implement:

- Theme selection.
- Glass controls.
- Transparency controls.
- Blur controls.
- Typography controls.
- Layout controls.
- Live preview.
- Custom profiles.
- Import/export.

Integrate with the existing configuration and persistence systems.

## PHASE 5 — POLISH AND VERIFICATION

Perform:

- Visual consistency review.
- Performance review.
- Accessibility review.
- Keyboard interaction review.
- Configuration persistence testing.
- Terminal session regression testing.
- Multi-pane regression testing.
- Window resizing tests.
- Multi-display and high-DPI testing.

Correct inconsistencies before considering the work complete.

---

# 13. TESTING AND REGRESSION REQUIREMENTS

Existing functionality must continue to work.

Explicitly verify:

- Terminal input and output.
- Shell startup.
- Shell termination.
- Multiple simultaneous sessions.
- Existing pane layouts.
- Pane resizing.
- Pane focus.
- Terminal scrolling.
- Text selection.
- Copy and paste.
- Keyboard shortcuts.
- Session restoration.
- Workspace restoration.
- Configuration loading.
- Configuration saving.
- Theme switching.
- Window resizing.
- Application shutdown.
- Application recovery.

Test with long-running terminal processes.

Test with multiple active panes.

Test with high-volume terminal output.

Test appearance changes while terminal sessions are active.

A visual change must never unexpectedly terminate, recreate, or reset a running terminal session.

---

# 14. COMPLETION CRITERIA

The project is complete when:

1. The application has a cohesive, modern visual identity.
2. The Liquid Glass-inspired design language is implemented consistently.
3. The interface is clean, sparse, and terminal-focused.
4. All primary UI elements use centralized design tokens.
5. Visual appearance is extensively configurable.
6. Theme changes apply immediately.
7. Existing terminal infrastructure remains intact.
8. Existing GUI functionality remains operational.
9. Existing configuration and persistence remain compatible.
10. Existing terminal functionality passes regression testing.
11. The interface performs smoothly under realistic terminal workloads.
12. The application does not require a wholesale architectural rewrite.

---

# 15. FINAL INSTRUCTIONS TO THE DEVELOPMENT AGENT

Treat this project as the modernization of an existing, mature terminal application.

Do not treat the repository as a blank starting point.

Do not rebuild the terminal.

Do not replace Ratatui or libghostty.

Do not replace the existing GUI framework.

Do not introduce alternative terminal engines.

Do not remove existing features to simplify implementation.

Do not replace functional components with mockups.

Do not create duplicate configuration, state-management, or session-management systems.

Do not make architectural changes without establishing why they are necessary.

Prioritize reusing existing components and infrastructure.

When a requested capability already exists, improve its presentation rather than implementing it again.

When a capability does not exist, implement it using established application patterns.

When a desired visual treatment conflicts with terminal readability or performance, prioritize readability and performance.

When uncertain about an architectural decision, inspect the codebase and document the decision rather than guessing.

**The final objective is to make the existing application look and feel like a premium, next-generation terminal environment without sacrificing any of the engineering investment already present in its underlying architecture.**
