# Tab Groups

## Summary
Users can organize tabs in a Warp window into named, color-coded **Tab Groups**, modeled on Chrome's tab grouping. A group can be created from one or more tabs, named, recolored from a fixed palette, collapsed to a single chip in the horizontal tab bar (or to a single section header in the vertical tabs panel), reordered as a unit, and closed in one action. Groups persist across app restarts and live entirely within the window that owns them.

## Problem
A power user with many concurrent tabs in a single Warp window has no way to visually cluster related work — e.g. three tabs for a deploy investigation alongside two tabs for an unrelated review, alongside a long-running training job. Today, every tab sits in a flat strip with the only organization tools being per-tab color and per-tab rename. As tab counts grow, users either lose tabs in the strip, open new windows (losing the affordances of one workspace), or close work to reduce clutter. Tab Groups give users a lightweight, in-place way to keep clusters of tabs visually and operationally separate without forcing them into separate windows.

## Goals
- Users can group one or more tabs in the same window into a named, colored Tab Group.
- Users can collapse a Tab Group so its members occupy a single chip's worth of space (horizontal) or a single collapsed section's worth of space (vertical).
- Users can manage a group as a unit: rename, recolor, collapse/expand, ungroup, close, and reorder relative to other tabs and other groups.
- Users can move individual tabs into and out of groups by drag and by right-click menu.
- Tab Groups behave consistently in both the horizontal tab bar and the vertical tabs panel.
- Tab Groups persist across app restarts within the window that owns them.

## Non-Goals
- **Cross-window groups.** A group lives in exactly one window. When a tab is handed off to another window (existing tab-handoff behavior), it leaves its group; the group does not transfer.
- **Saved or named group templates.** Groups exist only as long as their member tabs do. Closing or ungrouping the last member dissolves the group; there is no library of "saved groups" to re-instantiate later.
- **Drive / cloud sync of groups.** Groups are window-local UI state, like the vertical-tabs panel toggle. They do not sync across devices.
- **Custom group colors.** v1 ships a fixed palette of 8 named colors. Hex pickers and per-group custom colors are out of scope.
- **Nested groups.** A group cannot contain another group; tabs are leaves.
- **Per-group keyboard shortcuts.** v1 ships no new user-rebindable keybindings for group operations. Group commands are reachable from the right-click menus and (where they map cleanly) the command palette.
- **Auto-grouping.** v1 does not propose, suggest, or auto-create groups based on cwd, project, or activity.

## Figma
Figma: none provided.

## User Experience

### Vocabulary

- **Tab Group** (capitalized, also "group" in running prose): a named, colored container that holds one or more tabs in a single window.
- **Chip**: in the horizontal tab bar, the leading element of a group — a colored, rounded label that shows the group's name (or its tab count when collapsed) and acts as the group's anchor and right-click target.
- **Section header**: the vertical-tabs equivalent of the chip — a row at the top of a group's section showing the group's color, name, and a collapse/expand affordance.

### Group model

1. A group has: a stable identity, a user-editable name (may be empty), one of 8 palette colors, a collapsed/expanded flag, and an ordered list of one or more member tabs.
2. A tab belongs to **at most one** group at a time. A tab not in any group is "ungrouped" and renders as it does today.
3. A group must contain **at least one tab**. Removing the last member tab from a group (by ungrouping it, dragging it out, closing it, or transferring it to another window) dissolves the group; the group's identity, name, and color are discarded.
4. Within a window, every group's members are contiguous in tab order. The user cannot interleave a non-member tab between two members of the same group.
5. Group order is determined by the position of the group's first member tab. Reordering a group as a unit reorders the contiguous run of its members.

### Color palette

6. v1 ships exactly 8 group colors, presented in this order: **Grey, Blue, Red, Yellow, Green, Pink, Purple, Cyan**. The exact hex values are part of the visual design (see TECH.md); they map to existing Warp design-system colors where a sensible match exists.
7. Group colors are independent of the existing per-tab color (`SelectedTabColor`). A tab's per-tab color, if any, remains the tab's own indicator; the group's color decorates the chip / section header and the band along member tabs (see "Visual treatment" below).
8. New groups default to the next palette color in round-robin order, skipping a color already used by another group in the same window when possible. Once all 8 are in use, color reuse is allowed.

### Creating a group

9. From the **right-click menu on a tab**, the "Add tab to group" submenu offers:
   - **New group** — creates a new group containing just this tab. The group is created with an empty name and the next default palette color, and immediately enters rename mode (see "Renaming" below) so the user can type a name without an extra step.
   - **{existing group name}** — one entry per existing group in this window. Selecting one moves the tab into that group at the end of the group's run.
   - **Remove from group** — present only when the tab is currently a member of a group. Removes the tab from its group (see "Removing a tab from a group").
10. From the **right-click menu on a chip / section header**, "Add tab to group" is not offered (that menu manages the group itself; see "Group menu").
11. Multi-tab grouping in v1 happens by creating a group from one tab and then adding tabs to it — either by drag (see "Drag and drop") or by right-click → "Add tab to group" → {group name}. There is no multi-select-and-group gesture in v1.

### Naming and renaming

12. A group's name is a single line of text; it may be empty. Names are not constrained to be unique within a window.
13. To rename a group, the user invokes **Rename** from the chip / section header right-click menu, or **double-clicks** the chip / section header. The chip / section header text becomes an inline editor pre-filled with the current name.
14. The rename editor commits on Enter or focus loss and cancels on Escape. Committing an empty string leaves the group nameless.
15. While a group is being renamed, all other rename editors (other groups, tabs) close. A group rename and a tab rename cannot be active at the same time, mirroring the existing single-rename-at-a-time invariant for tabs.

### Recoloring

16. The chip / section header right-click menu offers a **Recolor** submenu listing the 8 palette colors with their names, with a check next to the group's current color.
17. Selecting a color updates the chip / section header color and the color band on member tabs immediately. The change persists across app restarts.

### Visual treatment — horizontal tab bar

18. When **expanded**, a group renders as a contiguous run of its member tabs preceded by the chip:
    - The chip sits to the left of the first member tab. It shows a colored fill in the group's palette color, a small leading dot or icon in a contrasting fill, and the group's name. If the name is empty, the chip falls back to the color name (e.g. "Blue") at reduced contrast so the chip is still right-clickable and visible.
    - A thin colored band in the group's color sits along one consistent edge (top or underline — exact edge is a visual-design decision, see TECH.md) of every member tab in the run. Non-member tabs do not show this band.
    - The chip and the run move together when the group is dragged.
19. When **collapsed**, the group's member tabs are hidden from the horizontal tab bar. Only the chip is visible, and the chip displays the group's name and a count of hidden tabs (e.g. "Deploy · 4"). The chip retains its color.
20. The chip itself is **not** a tab — it has no associated `PaneGroup` and clicking it does not activate a tab. Left-clicking an expanded chip toggles the group to collapsed; left-clicking a collapsed chip toggles it back to expanded. Right-click opens the group menu.
21. A group does not "show" or take focus on its own. The window's active tab is always one of the (visible or hidden) member tabs or an ungrouped tab; the chip never becomes the active element.

### Visual treatment — vertical tabs panel

22. In the vertical tabs panel, a group renders as a section:
    - A **section header row** at the top of the section shows a chevron (collapse/expand affordance), the group's color as a leading swatch, the group's name, and a tab count.
    - When expanded, the member tabs render below the header, indented to make membership visible, with a thin colored stripe in the group's color along the leading (left) edge of each member row.
    - When collapsed, the section header row remains visible; member rows are hidden.
23. Right-clicking the section header opens the same group menu as the horizontal chip. Right-clicking a member row opens the existing per-tab right-click menu (extended with the "Add tab to group" submenu — see item 9).
24. The vertical-tabs view's three modes (Compact, Expanded, and the resolved sub-modes Panes / FocusedSession / Summary) all support groups. In any mode where a tab renders, its group section header renders above it; in Summary mode the section header is the only group-level chrome and members render as summary items.

### Collapse / expand

25. Each group has its own collapsed/expanded state. The state is per-group, per-window, and persists across app restarts.
26. Collapsing a group hides its member tabs from the horizontal tab bar (or from the vertical tabs panel section). It does not close, suspend, or modify those tabs; they continue to run.
27. **If the active tab is inside a group and that group becomes collapsed (by user action or restored as collapsed), the group automatically expands so the active tab remains visible.** A collapsed group never hides the active tab.
28. Conversely, activating a tab — by command palette, keybinding, click on an external surface, or any programmatic activation — that lives inside a collapsed group expands its group as a side effect of the activation. The group remains expanded until the user collapses it again.
29. Toggling collapsed state via the chip's left click (item 20) and via the section header's chevron (item 22) is the same toggle as **Collapse** / **Expand** in the group right-click menu.

### Group right-click menu

30. The right-click menu on a chip (horizontal) or section header (vertical) contains, in order:
    - **Rename** — enters inline rename mode (item 13).
    - **Recolor** — submenu with the 8 palette colors (item 16).
    - **Collapse** when the group is expanded, or **Expand** when collapsed (items 25–29).
    - **Ungroup** — dissolves the group; member tabs become ungrouped and remain in their current order at the group's former position. No confirmation prompt.
    - **Close group** — closes every member tab in order (see "Closing a group").

### Adding tabs to a group

31. From the **right-click menu on a tab**, "Add tab to group" → "{group name}" moves the tab into the chosen group. The tab is appended at the end of the group's run. If the tab was previously a member of a different group, it is first removed from that group.
32. From the **right-click menu on a tab**, "Add tab to group" → "New group" creates a new group containing just this tab and immediately opens its rename editor.
33. By **drag**, dragging a tab and dropping it onto a group's chip, section header, or anywhere within an existing group's run / section adds the tab to that group at the drop position. The drop preview communicates which group the tab will land in.
34. The tab order within a group is meaningful and is preserved by drag-and-drop (the user can reorder members inside the group by dragging) and by add-to-group operations (item 31 appends at the end).

### Removing a tab from a group

35. From the **right-click menu on a tab in a group**, "Add tab to group" → "Remove from group" removes the tab from its group. The tab becomes ungrouped and remains at its current position relative to other tabs (the group shrinks; if the removed tab was at the start or end of the run, the group's position shifts accordingly).
36. By **drag**, dragging a member tab out of its group's run — to a position before the chip, after the group's last member tab, or onto another non-group neighbor — removes it from the group as part of the drop.
37. **Removing the active tab from its group does not deactivate it.** The tab remains the active tab; it simply becomes ungrouped.
38. If removal leaves the group empty, the group dissolves immediately (item 3).

### Reordering

39. Within a group, member tabs reorder by drag, the same way ungrouped tabs reorder today.
40. A group as a whole reorders by dragging its **chip** (horizontal) or **section header** (vertical). The entire run / section moves together. The drop targets are: before another ungrouped tab, after another ungrouped tab, before another group, or after another group. A group cannot be dropped between two members of another group (item 4 — group runs are contiguous).
41. Dragging a chip into another window is not supported in v1. (The existing single-tab cross-window handoff is unaffected; see "Cross-window handoff" below.)

### Closing a group

42. **Close group** closes every member tab of the group in order, then dissolves the group.
43. The close pipeline is the same per-tab close pipeline used today. If any member tab has unsaved or shared state that triggers a close confirmation when closed individually, that confirmation appears as part of the group close. The user can cancel; cancellation aborts the entire group close (no partial close), leaving the group and all its members intact.
44. If the active tab is a member of the closed group, activation moves to the nearest remaining tab using the existing single-tab close behavior (the same fall-through used when closing a single active tab today).
45. Group close events are individually undoable through the existing close-tab undo behavior — undoing restores the most recently closed member tab. v1 does not introduce a single "undo close group" that restores the entire group at once.

### Active-tab interactions (summary)

46. The active tab is always exactly one of the window's tabs, grouped or not. Group state never leaves the window without an active tab.
47. **Active tab removed from a group** (items 35–37): tab stays active and becomes ungrouped.
48. **Active tab in a group that the user collapses**: the group expands automatically (item 27); collapse only "sticks" when the active tab moves elsewhere.
49. **Active tab in a group whose group is closed** (item 42): activation falls through to the nearest remaining tab via the existing close behavior.
50. **Active tab in a group whose group is ungrouped** (item 30 → Ungroup): tab stays active in its same window position; only the group decoration disappears.

### Empty-group invariants

51. A group cannot exist with zero members. Any operation that would leave a group empty dissolves it as part of the same operation.
52. Closing the second-to-last and then the last member of a group dissolves the group on the close of the last member.
53. Dragging the only member tab out of its group dissolves the group on drop.
54. Ungrouping a single-member group is equivalent to "Remove from group" on that member: the group dissolves and the tab becomes ungrouped at the group's position.

### Persistence

55. The following are persisted per window across app restarts:
    - Each group's identity, name, color, and collapsed/expanded state.
    - The mapping of tabs to groups.
    - The order of groups and of tabs within each group, consistent with overall tab order.
56. On restore, every persisted group has at least one member (item 3 is enforced at write time and at read time; a corrupt empty group is silently dropped).
57. On restore, each group's collapsed/expanded state is honored, except where the active tab is a member of a group persisted as collapsed — in that case the group expands on restore (item 27).
58. Tabs in groups participate in the same restore behavior as ungrouped tabs (custom title, color, panel state, pane layout). Group membership is additional metadata, not a replacement for any existing tab persistence.

### Cross-window handoff

59. Tab Groups are **window-local**. A group lives in exactly one window and is not visible from any other window.
60. When a tab is handed off to another window (existing handoff via drag-to-other-window or `HandoffPendingTransfer` action), the tab **leaves its source group as part of the handoff**. The receiving window receives the tab as ungrouped. The source group remains in the source window with the handed-off tab removed; if that leaves the source group empty, the source group dissolves (item 3).
61. The reverse handoff (`ReverseHandoff`) follows the same rule: the tab returns to the source window as ungrouped. The user can re-add it to a group via right-click or drag.
62. There is no v1 affordance to "send a whole group to another window".

### Vertical tabs panel — additional behaviors

63. The vertical tabs panel's existing search input filters the visible tab list. Search applies to member tab titles as well as ungrouped tab titles. A group whose name matches the query, or any of whose member tabs match, remains visible; non-matching members are filtered out, but the section header remains visible while at least one match is present in the section. A group with zero matches is hidden entirely while the search has a query.
64. While search is active, all matching groups are temporarily expanded so matches are visible regardless of stored collapse state. Clearing the search restores the stored collapse state.
65. The vertical tabs new-tab affordance (the existing per-section new-tab button or the panel's global new-tab button) creates a tab outside any group by default. To add a new tab to a specific group, the user creates the tab and then drags or right-clicks it into the group.

### Right-click menu summary

66. **On an ungrouped tab**: existing menu items (rename, color, close, share, etc.) plus an "Add tab to group" submenu listing **New group** and each existing group in this window.
67. **On a tab in a group**: same as ungrouped, except "Add tab to group" also includes **Remove from group** as the first item, and the group the tab is currently in is shown but disabled (or omitted) in the list of existing groups.
68. **On a chip (horizontal) or section header (vertical)**: Rename, Recolor, Collapse/Expand, Ungroup, Close group (item 30). No tab-level items appear here.
69. The chip / section header menus do not appear on left-click, hover, or focus alone — only on right-click. Left-click of a chip toggles collapse (item 20); left-click of a section header is reserved for the chevron toggle (item 22).

### Visual and theming consistency

70. Group color, chip styling, section header styling, color band, and rename editor all use existing Warp design-system primitives (theme accessors, button themes, menu themes). No bespoke per-feature theme is introduced; the 8 palette colors map to existing semantic / named colors where possible (see TECH.md for exact mapping).
71. Active-tab styling continues to win visually: when a member tab is active, its active-tab styling (background, text weight) reads through the group's color band rather than being suppressed by it.
72. The group's color band on member tabs is decorative; it never replaces or hides the existing per-tab indicator slot (unsaved changes, synced, error, shared, maximized, etc.). A grouped tab with unsaved changes still shows its unsaved-changes indicator.

### Settings

73. v1 introduces no new user-facing settings. Tab Groups are always-on once the feature flag is enabled. Future toggles (e.g. "show group color band on tabs") are out of scope.

### Telemetry expectations

74. The product surface emits telemetry on: create group, rename group, recolor group, collapse group, expand group, add tab to group (by menu vs. by drag, where distinguishable), remove tab from group, ungroup, close group, drag-reorder a group as a unit. Exact event names live in TECH.md.

### Out-of-scope items, restated for clarity

- No multi-window groups, group transfer, or "move group to new window" affordance.
- No saved/named group templates; groups are ephemeral metadata over a current set of tabs.
- No Drive / cloud sync of groups.
- No custom group colors beyond the 8 palette entries.
- No nested groups.
- No new user-rebindable keybindings for group operations in v1.
- No auto-grouping by cwd, project, or activity.
- No "undo close group" as a single action; per-tab undo close continues to work (item 45).
