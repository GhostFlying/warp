# Fork CLI Agent Tab Color

## Goal

Show transient tab colors for Codex and Claude CLI agent activity:

- running sessions render yellow;
- finished sessions render green;
- the transient state does not replace the user's assigned tab color or the
  directory-derived tab color;
- finished green clears when the user activates the tab again.

## Implementation Notes

- Add a non-serialized `TabColorOverlay` field to `TabData`.
- Resolve effective tab color in this order:
  1. agent overlay;
  2. user-selected tab color;
  3. directory-derived tab color.
- Update the existing workspace subscription to `CLIAgentSessionsModelEvent`.
  Only Codex and Claude events for terminal views in the workspace affect
  overlays.
- Bubble a private OSC through the terminal event path:
  `OSC 9281 ; cli-agent-tab-color ; running|finished|clear`.
- Keep persistence unchanged by continuing to serialize only selected and
  directory colors.

## Verification

- Add unit coverage for overlay precedence and workspace event transitions.
- Add ANSI parser coverage for the private OSC payloads and invalid payloads.
- Run the narrow affected Rust tests on Linux.
