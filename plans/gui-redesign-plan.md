# GUI Redesign Plan — Remote Code Pro

Based on research of Claude Code Desktop and Codex Desktop official apps.

## Phase 1: Design System Foundation (DONE)
- CSS custom properties tokens (light + dark themes)
- Tailwind config with semantic tokens
- ThemeProvider with dark mode toggle + system preference
- All core components migrated from hardcoded hex to semantic tokens

## Phase 2: Layout Overhaul (NEXT)
- ActivityBar (48px icon navigation)
- SplitPane (resizable panels)
- StatusBar (bottom status bar)
- Tab-based sidebar

## Phase 3: Chat Experience
- Streaming message animation
- Inline diff viewer
- Slash command autocomplete
- Message actions (copy/retry/edit)

## Phase 4: Integrated Tools
- Terminal pane (xterm.js)
- Diff pane (side-by-side)
- File tree pane
- Preview pane

## Phase 5: Advanced Features
- Command palette (Cmd+Shift+P)
- Keyboard shortcuts
- Skills picker
