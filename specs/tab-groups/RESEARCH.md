# Tab Groups — Research Brief

> Audience: spec writers and implementers building Chrome-like tab grouping in Warp.
> Read top-to-bottom once; thereafter, treat as a citation index. Every claim is anchored to
> `path:line` in this worktree. Read the linked source before changing it.

---

## 0. Naming collision warning (READ FIRST)

The string `tab_group` / `TabGroup` is **already used** in this codebase for two unrelated concepts.
Pick a different word for the new feature (`TabBundle`, `TabSection`, `TabCollection`, …) or be
extremely careful:

- `app/src/workspace/view/vertical_tabs.rs:735` — `enum TabGroupColorMode`, used by the visual
  rendering of a *single* tab's panes inside the vertical tabs panel. Synonym for "the row that
  represents one TabData", not "a Chrome tab group".
- `app/src/workspace/view/vertical_tabs.rs:818` — `struct TabGroupDragState`.
- `app/src/workspace/view/vertical_tabs.rs:1696` — `fn render_tab_group(...)` renders one TabData.
- `app/src/workspace/view/vertical_tabs.rs:4170` — `fn compute_tab_group_color_mode`.
- `app/src/code/view.rs:230` — `tab_group: Vec<TabData>` field on `CodeView`. This is the file-tab
  bar inside a code editor pane (open files), entirely separate from workspace-level tabs.
- `app/src/code/view.rs:176-177` — actions `ClearEditorTabGroupDragPositions`,
  `ClearWorkspaceTabGroupDragPositions`.

If the new feature is named `TabGroup`, downstream readers will conflate these. Strongly
recommend a different identifier; the rest of this brief uses **"tab group"** in prose to refer
to the *new* Chrome-like grouping feature only.

---

## 1. Tab data model

### 1.1 In-memory: `TabData`

Defined at `app/src/tab.rs:134` (`#[derive(Clone)]`). Fields:

```
pub pane_group: ViewHandle<PaneGroup>,    // app/src/tab.rs:135
pub tab_mouse_state: MouseStateHandle,    // :136
pub close_mouse_state: MouseStateHandle,  // :137
pub tooltip_mouse_state: MouseStateHandle,// :138
pub draggable_state: DraggableState,      // :139  — ties the tab into Draggable<…>
pub default_directory_color: Option<AnsiColorIdentifier>, // :141 — derived from cwd→color map
pub selected_color: SelectedTabColor,     // :143  — explicit user override (Unset/Cleared/Color)
pub indicator_hover_state: MouseStateHandle, // :144
pub detached: bool,                       // :146 — set when the tab moved into a detached window
```

`TabData` is a thin wrapper: the **content** of a tab lives entirely on the `PaneGroup`
(`crate::pane_group::PaneGroup`, declared at `app/src/pane_group/mod.rs:832`). All persistent
session, layout, panel-open state, custom title, etc. are accessed via
`tab.pane_group.as_ref(ctx)` or `…read(ctx, |pg, _| …)`. See helpers
`Workspace::active_tab_pane_group` at `app/src/workspace/view.rs:4848`,
`Workspace::tab_views` at `:4763`.

`TabData::new(pane_group)` (`app/src/tab.rs:153`) is the only constructor; everything else
defaults.

`TabData::color()` (`app/src/tab.rs:168`) resolves user override > directory default. The color
type is `crate::themes::theme::AnsiColorIdentifier` (used as the precedent for per-tab metadata).

### 1.2 There is **no stable Tab ID**. Read this carefully

- The `Vec<TabData>` is **indexed by position only** (`Workspace::tabs: Vec<TabData>` at
  `app/src/workspace/view.rs:892`).
- All tab actions take `usize` indexes: `ActivateTab(usize)`, `CloseTab(usize)`,
  `MoveTabLeft(usize)`, `MoveTabRight(usize)`, `RenameTab(usize)`, `ToggleTabRightClickMenu
  { tab_index, … }`, `DragTab { tab_index, … }`, `ToggleTabColor { tab_index, … }`. See
  `app/src/workspace/action.rs:99-258`.
- The DB primary key `tabs.id` is **not stable across runs**. Persistence does
  `delete from tabs` then re-insert (`app/src/persistence/sqlite.rs:828` and `:907`); restored
  ordering is by ascending row id (`:2666`). DB id is never surfaced into in-memory state.
- The closest thing to a stable handle is `tab.pane_group.id() : EntityId`
  (`crates/warpui_core/src/core/entity.rs:13`). `EntityId` is in-memory only — it is allocated
  per-process and discarded on shutdown.

This drives the single biggest architectural decision for tab groups (see §8).

### 1.3 Persisted shape: `Tab` row + `TabSnapshot`

DB table (`crates/persistence/src/schema.rs:357`):

```
tabs (id) {
    id           -> Integer,
    window_id    -> Integer,
    custom_title -> Nullable<Text>,
    color        -> Nullable<Text>,   // serde_yaml of SelectedTabColor
}
```

Rust models `Tab` + `NewTab` at `crates/persistence/src/model.rs:344-359`. Belongs-to
relationship to `Window` (`crates/persistence/src/model.rs:345`).

In-memory restore representation `TabSnapshot` at `app/src/app_state.rs:62`:

```
pub struct TabSnapshot {
    pub custom_title: Option<String>,
    pub root: PaneNodeSnapshot,
    pub default_directory_color: Option<AnsiColorIdentifier>,
    pub selected_color: SelectedTabColor,
    pub left_panel: Option<LeftPanelSnapshot>,
    pub right_panel: Option<RightPanelSnapshot>,
}
```

Save path: `app/src/persistence/sqlite.rs:891-920` (constructs `NewTab`, inserts, then queries
ids back — note **DB id is regenerated on every save**).
Load path: `app/src/persistence/sqlite.rs:2650-2735` (reads windows, groups Tabs by window,
hydrates each into `TabSnapshot`).

### 1.4 Migration precedent for adding per-tab columns

`crates/persistence/migrations/2022-08-24-040424_tab_color/up.sql` is the canonical example for
"add a column to `tabs`":

```
ALTER TABLE tabs ADD color TEXT;
```

`down.sql` is `ALTER TABLE tabs DROP COLUMN color;`. The earlier
`2022-05-16-235820_add_tab_title_to_session_restoration/up.sql` (`ALTER TABLE tabs ADD
custom_title TEXT;`) is the same shape.

After authoring a new migration, regenerate `schema.rs`. `diesel.toml:5` configures
`crates/persistence/src/schema.rs` and a `schema.patch` (`diesel.toml:11`) at
`crates/persistence/schema.patch` which currently only patches the `object_metadata` table — if
the new `tabs` columns need a non-default Diesel type, extend the patch (see §6).

---

## 2. Workspace state and ownership

### 2.1 The Entity / View / Handle system (one-paragraph primer)

Warp uses an in-house GPUI-style framework called `warpui`. Long-lived objects are **Entities**
(`crates/warpui_core/src/core/entity.rs:39`, `EntityId` at `:13`) and the user-visible subset are
**Views** (`crates/warpui_core/src/core/view/mod.rs:61` — `pub trait View: Entity`). To touch a
view from another view you hold a **`ViewHandle<T>`** (`crates/warpui_core/src/core/view/handle.rs:21`):

- `handle.as_ref(ctx) -> &T` — read access through `&AppContext`.
- `handle.read(ctx, |view, ctx| …) -> S` — ergonomic read.
- `handle.update(ctx, |view, ctx| …)` — mutable access through `ViewContext` (`…/context.rs:36`).
- `handle.id() -> EntityId`, `handle.downgrade() -> WeakViewHandle<T>`.

The relevant graph for tabs:

- `Workspace` (`app/src/workspace/view.rs:890`) is a `View` owned by the window. It holds
  `tabs: Vec<TabData>` (`:892`) and `active_tab_index: usize` (`:893`). One `Workspace` per window.
- Each `TabData` holds `ViewHandle<PaneGroup>` (`app/src/tab.rs:135`).
- `PaneGroup` (`app/src/pane_group/mod.rs:832`) is itself a `View`, owning the tree of panes,
  panel open/closed flags, custom title, etc. Lifetimes are managed by handle ref-counting; when
  the last `ViewHandle<PaneGroup>` drops, the entity is freed.

### 2.2 Tab lifecycle on `Workspace`

| Operation | Function | Reference |
| --- | --- | --- |
| Construct an empty workspace | `tabs: Vec::new(),` | `app/src/workspace/view.rs:3037` |
| Append a new tab | `self.tabs.push(TabData::new(new_pane_group))` | `:3940`, `:4035`, `:10838`, `:10844`, `:10912`, `:11450` |
| Insert at index (restore-closed) | `self.tabs.insert(tab_index, tab_data)` | `:10529` (also `:10915`) |
| Activate | `Workspace::activate_tab` → `activate_tab_internal` → `set_active_tab_index` | `:4863`, `:4870`, `:4942` |
| Move (left/right) | `Workspace::move_tab` swaps in-place | `:11992` |
| Drag-reorder (live) | `Workspace::on_tab_drag` calls `calculate_updated_tab_index{,_vertical}` and `self.tabs.swap(...)` | `:11886`, `:11919`, `:11956` |
| Remove | `Workspace::remove_tab` (with undo+detach plumbing) | `:10122` (`:10172` is the actual `tabs.remove(index)`) |
| Close (with confirmation pipeline) | `Workspace::close_tab`, `close_tabs`, `close_other_tabs`, `close_tabs_direction` | `:10360`, `:10257`, `:10394`, `:10424` |
| Add (high-level) | `add_terminal_tab`, `add_welcome_tab`, `add_get_started_tab`, `add_ambient_agent_tab`, `add_tab_with_shell`, `add_tab_with_pane_layout`, `add_tab_from_existing_pane`, `add_tab_for_cloud_notebook`, `add_tab_for_cloud_workflow`, `add_tab_for_file_notebook`, `add_tab_for_assisted_autoupdate`, `add_tab_for_code_file`, `add_tab_for_new_code_file`, `add_tab_for_joining_shared_session` | `:10541`, `:10553`, `:10573`, `:10632`, `:10654`, `:10791`, `:10890`, `:10920`, `:10938`, `:10956`, `:10971`, `:11017`, `:11038`, `:3919` |

Every mutation that should cross a save boundary calls
`ctx.dispatch_global_action("workspace:save_app", ())` (e.g.
`app/src/workspace/view.rs:10196` after `remove_tab`, `:10238` after adopt-transferred). New
group operations must do the same to survive a restart.

### 2.3 Cross-window tab handoff (already implemented, will affect groups)

- `pub struct TransferredTab` at `app/src/workspace/view.rs:880`. Carries
  `pane_group, color, custom_title, left_panel_open, vertical_tabs_panel_open,
  right_panel_open, is_right_panel_maximized` between windows.
- Workspace flags `pending_pane_group_transfer` (`:1026`) and `is_drag_preview_workspace`
  (`:1027`). Constructor
  `Workspace::get_tab_transfer_info` at `:4774` and `Workspace::adopt_transferred_pane_group`
  at `:10205`.
- Actions: `WorkspaceAction::HandoffPendingTransfer` and `ReverseHandoff`
  (`app/src/workspace/action.rs:249`, `:253`); also `DragTabsToWindows` feature flag
  (referenced from `app/src/tab.rs:1668`).

Open question for spec writers: when a grouped tab is dragged into a new window, does the
new window also receive the group, the group split, or does the tab leave the group? See §8.

### 2.4 `WorkspaceState` (per-window UI flags)

The `current_workspace_state: WorkspaceState` field (`app/src/workspace/view.rs:928`) tracks
flags like `is_tab_being_dragged` (`:20278`, `:20664`) and `is_tab_being_renamed`
(`is_any_tab_renaming` is read at `:17216`). Hovered tab feedback comes through
`hovered_tab_index: Option<TabBarHoverIndex>` (`:894`), updated by
`pane_group::Event::UpdateHoveredTabIndex` / `ClearHoveredTabIndex` events
(`app/src/workspace/view.rs:13762`, `:13766`; emitted from `app/src/pane_group/mod.rs:1147,1155,
1167-1192`).

`TabBarHoverIndex` enum: `BeforeTab(usize)` / `OverTab(usize)` (`app/src/pane_group/mod.rs:736`).
**The new feature will likely need an additional variant, or a parallel "hovered group" enum.**

---

## 3. Render path — horizontal tab bar

### 3.1 Where it actually lives

Although there is no `tab_bar/` module, the horizontal tab bar is rendered entirely **inside
`Workspace`'s view** via methods on `Workspace`:

- Top-level entrypoint: `Workspace::render_tab_bar` at `app/src/workspace/view.rs:17532`.
  Wraps everything in a `DropTarget<TabBarDropTargetData>` keyed
  `TabBarLocation::AfterTabIndex(self.tabs.len())` (`:17541-17547`), wraps the result in a
  `TAB_BAR_HEIGHT` `ConstrainedBox` (`:17549`) and applies a bottom `Border` of
  `TAB_BAR_BORDER_HEIGHT` (`:17552`). Adds `on_back_mouse_down` → `ActivatePrevTab` and
  `on_forward_mouse_down` → `ActivateNextTab` (`:17557-17563`).
- Body: `Workspace::render_tab_bar_contents` at `:17029`. Branching:
  - WASM simplified mode (`:17042-17125`).
  - Vertical-tabs mode (`:17127-17202`) — the horizontal tab bar collapses to a thin header
    with traffic lights + search; the actual tab list moves into the vertical panel (§4).
  - Default horizontal mode (`:17203-17249`):
    - Builds a `TabBarState` (`tab_count`, `active_tab_index`, `is_any_tab_renaming`,
      `is_any_tab_dragging`, `hover_fixed_width`) at `:17213`.
    - Iterates `for i in 0..self.tabs.len()` (`:17221`), inserting a hover indicator
      (`render_tab_hover_indicator`, `:16955`) before the tab if
      `hovered_tab_index == BeforeTab(i)` (`:17222-17230`).
    - Renders each tab via `render_tab_in_tab_bar(i, tab_bar_state, ctx)` (`:16611`), which is a
      thin wrapper that builds `TabComponent::new(...)` and calls `.build().finish()`.
    - Appends an "after the last tab" hover indicator (`:17234-17244`).
    - Appends `render_new_session_button(ctx)` (`:17617`) when `ContextFlag::CreateNewSession`
      is enabled.
- Right side controls: `add_configurable_right_side_tab_bar_controls(...)` (called at
  `:17254`) inserts the configurable header toolbar (notifications, code review, agent
  management, etc.).

### 3.2 The tab itself: `TabComponent`

Defined at `app/src/tab.rs:587`. `impl UiComponent for TabComponent<'_>` at `:1462`. Highlights:

- `TabBarState` (`app/src/tab.rs:546`) is the bag of cross-tab state passed in.
- Active styling is selected by comparing `Some(self.tab_index) == self.tab_bar.active_tab_index`
  (`app/src/tab.rs:792`); active styles are layered onto default styles via
  `TabStyles::merge` (`:621`). New border + background are computed at `:1198-1232`
  (NewTabStyling on) or `:1234-1261` (legacy).
- Mouse routing on `tab` element is wired in `UiComponent::build` (`:1462`):
  - `on_mouse_down → WorkspaceAction::ActivateTab(tab_index)` (`:1617-1625`), suppressed when
    the close-button is hovered or the tab is being renamed.
  - `on_double_click → WorkspaceAction::RenameTab(tab_index)` (`:1627`).
  - `on_right_click → WorkspaceAction::ToggleTabRightClickMenu { tab_index, anchor:
    TabContextMenuAnchor::Pointer(position) }` (`:1632-1637`).
  - `on_middle_click → WorkspaceAction::CloseTab(tab_index)` (`:1640-1643`).
- The full tab is wrapped in `Draggable::new(...)` (`:1659-1672`) firing `StartTabDrag` /
  `DragTab { tab_index, tab_position }` / `DropTab`. `DragAxis::HorizontalOnly` unless
  `FeatureFlag::DragTabsToWindows` is on (`:1668`).
- The drop target for a *neighboring* tab is built inside `render_tab_container_internal`
  (`:1190`) — at `:1446-1452` `DropTarget::new(tab.finish(), TabBarDropTargetData {
  tab_bar_location: TabBarLocation::TabIndex(self.tab_index) })`.
- Compact (icon-only) variant kicks in below `COMPACT_TAB_WIDTH_THRESHOLD = 42.0`
  (`app/src/tab.rs:73`) via `SizeConstraintSwitch` (`:1409-1415`).
- Indicator slot: `render_indicator` (`:1065`) renders one of `Indicator` (`:556`):
  `UnsavedChanges`, `Synced`, `Error`, `Shared`, `Maximized`, `Shell`, `Agent`, `AmbientAgent`.
  Tab groups likely need a parallel "group color/badge" slot — there is space on the left of
  the title where the indicator currently lives.
- Tooltip layered overlay at `:1490-1610`.

The tab is laid out in a `Flex::row()` with `MainAxisAlignment::Center` (`:1264-1281`); width is
`max_width: 200.0` (`app/src/tab.rs:1655`) unless the tab bar pinned a fixed width during a
close-button hover (`hover_fixed_width`, `:1647`).

### 3.3 Active-tab highlight

`TabComponent::is_active_tab` (`app/src/tab.rs:791`) drives:
- Background fill in `render_tab_container_internal` (`:1196-1232`).
- Border fill (`:1226-1230`) via `internal_colors::fg_overlay_2` for active vs.
  `fg_overlay_1` otherwise.
- Style merge for the title text (`render_tab_content`, `:925-977`) — uses
  `theme.active_ui_text_color()` and `Weight::Medium` for the active row.

For new-tab-styling builds the first tab gets a left border inset
(`is_first_tab`, `:1422-1428`).

---

## 4. Render path — vertical tabs

File: `app/src/workspace/view/vertical_tabs.rs` (5996 lines; declared `mod vertical_tabs;` at
`app/src/workspace/view.rs:18`). `pub mod telemetry;` at `vertical_tabs.rs:1`.

### 4.1 Mode gate

Rendered only when both `FeatureFlag::VerticalTabs.is_enabled()` and
`*TabSettings::as_ref(ctx).use_vertical_tabs` (helper `crate::tab::uses_vertical_tabs` at
`app/src/tab.rs:60`). When active, the horizontal tab bar still renders but skips the tab
list (`app/src/workspace/view.rs:17128-17202`).

### 4.2 Top-down layout

- `render_vertical_tabs_panel` at `vertical_tabs.rs:1420` is the panel root.
- It hosts `render_control_bar` (`:1159`) — search input + new-tab button + settings popup
  trigger (`render_settings_button`, `:1265`; `render_new_tab_button`, `:1331`).
- `render_groups` (`:1490`) iterates `workspace.tabs` (`:1518`), filters by search query, and
  emits a `render_tab_group(...)` per visible tab (`:1652`).
- `render_tab_group` (`:1696`) is what spec writers should think of as "**the row that
  represents one tab in vertical mode**". It builds:
  - Group header if the tab has a custom title or is being renamed (`:1906-1917`,
    `render_group_header` at `:2157`).
  - One row per visible pane in the tab via `render_pane_row` (`:2355`) or
    `render_compact_pane_row` (`:5784`); summary mode uses `render_summary_tab_item` (`:3467`).
  - Active styling via `is_active = tab_index == workspace.active_tab_index` (`:1764`).
  - Hover/drag chrome through `compute_tab_group_color_mode` (`:4170`) and
    `add_vertical_tab_insertion_target_overlay`/`render_vertical_tab_hover_indicator`
    (`:1090`, `:1117`).

### 4.3 Vertical mouse routing

- Right-click on the group dispatches `WorkspaceAction::ToggleVerticalTabsPaneContextMenu
  { tab_index, target: VerticalTabsPaneContextMenuTarget::ActivePane(...), position }`
  (`:2024-2030`). The context menu is the same horizontal one with extra pane-rename items —
  see §5.
- Middle-click closes the tab unless it's the last one (`:2034-2038`).
- Drag is `DragAxis::VerticalOnly` (`:2055`); start/move/drop dispatch the same
  `StartTabDrag`/`DragTab`/`DropTab` actions as horizontal (`:2042-2054`).
- Drop target wraps the group element at `:2070-2077` with `VerticalTabsPaneDropTargetData`
  (`app/src/workspace/mod.rs:1537`) carrying both `tab_bar_location: TabBarLocation::TabIndex(…)`
  and a `tab_hover_index: TabBarHoverIndex::OverTab(…)`.

### 4.4 Helpers used by drop-position math

- `vertical_tabs_tab_bar_location(insert_index, tab_count)` at
  `vertical_tabs.rs:1082` — converts an insertion site to `TabBarLocation::TabIndex(_)` or
  `AfterTabIndex(_)`.
- `Workspace::calculate_updated_tab_index_vertical` at `app/src/workspace/view.rs:11956`
  (uses neighbor midpoints to avoid jitter).

### 4.5 What groups will need to hook into

The vertical panel is the most natural surface for full Chrome-like group rendering (collapse
toggle, group color band, group name). Concretely, group-aware code will live in
`render_groups` (insert a wrapper container around contiguous tabs sharing a group) and
`render_tab_group` (left color stripe; indent inside the wrapper).

---

## 5. Tab actions (context menu, drag-and-drop, close, reorder)

### 5.1 The action enum

`WorkspaceAction` is the canonical enum: `app/src/workspace/action.rs:99`. Tab-related
variants (`:100-258`):

```
ActivateTab(usize), ActivatePrevTab, ActivateNextTab, ActivateLastTab,
CyclePrevSession, CycleNextSession,
MoveActiveTabLeft, MoveActiveTabRight, MoveTabLeft(usize), MoveTabRight(usize),
RenameTab(usize), ResetTabName(usize), RenameActiveTab, SetActiveTabName(String),
SetActiveTabColor(SelectedTabColor),
ToggleTabRightClickMenu { tab_index, anchor: TabContextMenuAnchor },
ToggleVerticalTabsPaneContextMenu { tab_index, target: VerticalTabsPaneContextMenuTarget,
    position: Vector2F },
TabHoverWidthStart { width: f32 }, TabHoverWidthEnd,
ToggleTabBarOverflowMenu,
CloseTab(usize), CloseActiveTab, CloseOtherTabs(usize), CloseNonActiveTabs,
CloseTabsRight(usize), CloseTabsRightActiveTab,
AddDefaultTab, AddTerminalTab { hide_homepage }, AddTabWithShell { shell, source },
AddGetStartedTab, AddAmbientAgentTab, AddAgentTab, AddDockerSandboxTab,
OpenNewSessionMenu { position }, ToggleNewSessionMenu { position, is_vertical_tabs },
SelectNewSessionMenuItem(NewSessionMenuItem),
ToggleTabConfigsMenu, ActivateTabByNumber(usize),
ToggleTabColor { color, tab_index }, SelectTabConfig(TabConfig),
StartTabDrag, DragTab { tab_index, tab_position: RectF }, DropTab, FinalizeDropTab,
HandoffPendingTransfer { target_window_id, insertion_index },
ReverseHandoff { target_window_id, target_insertion_index },
OpenShareSessionModal(usize), StopSharingSessionFromTabMenu { terminal_view_id },
StopSharingAllSessionsInTab { pane_group }, CopySharedSessionLinkFromTab { tab_index },
ToggleSyncTerminalInputsInTab, …
```

The big switch that handles them is `Workspace::handle_action` (matching by variant in
`app/src/workspace/view.rs` starting near `:19750`; tab branches are at `:19750-19770` for
activation and rename, `:20275-20665` for drag/drop, and the close/move dispatches are
adjacent — search for the variant name to find each arm). Important specific lines:

- `StartTabDrag` arm at `app/src/workspace/view.rs:20275-20279` — sets
  `is_tab_being_dragged = true` and finishes any in-progress rename.
- `DragTab { tab_index, tab_position }` arm at `:20659-20662` calls `on_tab_drag(...)` which
  performs the actual swap.
- `DropTab` arm at `:20663-20666` — clears flag and emits
  `TelemetryEvent::DragAndDropTab`.
- `ToggleTabRightClickMenu { tab_index, anchor }` arm at `:19769` calls
  `Workspace::toggle_tab_right_click_menu` (`:6488`).
- The vertical-tabs variant `ToggleVerticalTabsPaneContextMenu` is dispatched and handled by
  `Workspace::toggle_vertical_tabs_pane_context_menu` (`app/src/workspace/view.rs:6510`).

### 5.2 Where new actions get registered

There are three flavours of registration; pick the one matching the action's command-palette
and keybinding semantics. All examples are in `app/src/workspace/mod.rs::init` (starts at
`:101`):

- `app.register_fixed_bindings([FixedBinding::empty(...)])` — internal/dev shortcuts; e.g.
  `:133-136` for `DumpDebugInfo`. These are not user-rebindable but show in the palette.
- `app.register_editable_bindings([EditableBinding::new("workspace:foo", "Description",
  WorkspaceAction::Foo).with_context_predicate(id!("Workspace"))])` — user-bindable keymap
  entries, also command-palette searchable. Heavy use across `mod.rs:730-1525`.
- `BindingDescription::new(...).with_dynamic_override(...)` adapts the displayed label by
  context (e.g. "Move Tab Right" vs "Move Tab Down" in vertical mode at `mod.rs:833`,
  `:847`).

For tab groups, expect to add:
- `WorkspaceAction::CreateTabGroup { tab_index }` (or `…ForActiveTab`),
  `RenameTabGroup { group_id }`, `SetTabGroupColor { group_id, color }`,
  `MoveTabIntoGroup { tab_index, group_id }`, `RemoveTabFromGroup { tab_index }`,
  `CollapseTabGroup { group_id }`, `CloseTabGroup { group_id }`, …
- Each new variant must be added to the enum, the `handle_action` match in `view.rs`, and a
  registration line in `workspace/mod.rs::init`.

### 5.3 Context menu construction (already wired)

`TabData::menu_items` (`app/src/tab.rs:173`) and
`TabData::menu_items_with_pane_name_target` (`:182`) produce the menu items, calling four
section helpers and intercalating separators:
- `session_sharing_menu_items` (`:211`)
- `modify_tab_menu_items` (`:289`) — Rename / Reset name / Move / pane-name items.
- `pane_name_menu_items` (`:345`)
- `close_tab_menu_items` (`:376`)
- `save_config_menu_items` (`:414`) — gated by `FeatureFlag::TabConfigs`.
- `color_option_menu_items` (`:423`) → `dot_color_option_menu_items` (`:437`) /
  `legacy_color_option_menu_items` (`:513`).

The menu is shown via `Menu<WorkspaceAction>`:
- `Workspace::tab_right_click_menu: ViewHandle<Menu<WorkspaceAction>>`
  (`view.rs:908`), `show_tab_right_click_menu: Option<(usize, TabContextMenuAnchor)>` (`:909`).
- `Workspace::toggle_tab_right_click_menu` (`:6488`) populates with `tab.menu_items(tab_index,
  self.tabs.len(), ctx)` (`:6500-6504`) and focuses the menu.
- `Workspace::toggle_vertical_tabs_pane_context_menu` (`:6510`) is the vertical-tabs analogue
  with `pane_name_target` injected.
- Menu construction (`add_typed_action_view`, subscription) at `:1745-1748`. Event handler
  `handle_tab_right_click_menu_event` at `:8499`.

To add tab-group menu items, extend `TabData::menu_items_with_pane_name_target` with a new
section helper (e.g. `tab_group_menu_items(...)`) and dispatch new `WorkspaceAction` variants
from those `MenuItemFields::with_on_select_action(...)` calls.

### 5.4 Drag-and-drop wiring

- Each tab's `Draggable::new(draggable_state, content)` at `app/src/tab.rs:1659-1673` (horizontal)
  and `app/src/workspace/view/vertical_tabs.rs:2042-2054` (vertical).
- Drop targets:
  - **Whole bar**: `DropTarget<TabBarDropTargetData { TabBarLocation::AfterTabIndex(len) }>`
    at `app/src/workspace/view.rs:17541-17547`.
  - **Per tab (horizontal)**: `DropTarget<TabBarDropTargetData { TabBarLocation::TabIndex(i) }>`
    at `app/src/tab.rs:1446-1452`.
  - **Per tab (vertical)**: `DropTarget<VerticalTabsPaneDropTargetData { TabBarLocation,
    TabBarHoverIndex }>` at `app/src/workspace/view/vertical_tabs.rs:2070-2077`.
- Drop-target **data structs**: `TabBarDropTargetData` at `app/src/workspace/mod.rs:1532`,
  `VerticalTabsPaneDropTargetData` at `:1537`. `TabBarLocation` enum at `:1543`. New
  group-aware drop targets (e.g. dropping into the empty area of a group, or above/below a
  collapsed group) probably need parallel structs, or a new variant on `TabBarLocation`.
- Drag math: `Workspace::on_tab_drag` (`view.rs:11886`) →
  `calculate_updated_tab_index` (`:11919`) for horizontal, `_vertical` (`:11956`) for vertical.
  Both compare midpoints against neighbor positions resolved via `ctx.element_position_by_id`
  on `tab_position_id(idx)` (`tab_position_id` is at `app/src/tab.rs:1458`).

### 5.5 Close / reorder

- Reorder (action): `move_tab` at `view.rs:11992` swaps `self.tabs[index]` with neighbor and
  re-points active index if needed. Telemetry: `MoveActiveTab` / `MoveTab` (see §7).
- Close (action): `close_tab` (`:10360`) → `close_tabs` (`:10257`) → `remove_tab`
  (`:10122` / `tabs.remove(index)` at `:10172`) with `UndoCloseStack` integration at `:10174-10180`.
  Special "last tab → close window" path at `:10141-10148`.

---

## 6. Persistence

### 6.1 Migration directory

All Diesel migrations are under `crates/persistence/migrations/<YYYY-MM-DD-HHMMSS>_<name>/`,
each containing `up.sql` and `down.sql`. List of relevant ones (chronological):

- `2021-10-18-232826_create_windows_and_tabs/up.sql` — original `windows` and `tabs` tables.
  `tabs(id PK, window_id FK, cwd TEXT)`.
- `2022-05-16-235820_add_tab_title_to_session_restoration/up.sql` —
  `ALTER TABLE tabs ADD custom_title TEXT;`.
- `2022-08-24-040424_tab_color/up.sql` — `ALTER TABLE tabs ADD color TEXT;` (the precedent
  flagged by the task description).
- `2024-12-30-232544_add_workspace_tables/up.sql` — `workspaces` table (note: distinct from
  the `Workspace` view; this is a Drive-level concept tied to remote sync).
- `2025-01-08-010739_add_current_workspace` — adds `is_selected` to `workspaces`.
- `2025-11-07-005740_create_panels_table` — `panels` (left/right panel snapshot per tab).
- `2026-01-29-210900_add_left_panel_open_to_windows`,
  `2026-03-27-075600_add_vertical_tabs_panel_open_to_windows` — recent per-window booleans.
- `2026-04-14-150000_add_code_pane_tabs/up.sql` — example of a multi-statement migration that
  *creates a child table and backfills from an existing column* (relevant if tab-groups needs a
  new `tab_groups` table linked to `tabs`).
- `2026-04-17-020439_add_custom_vertical_tabs_title_to_pane_leaves/up.sql` — most recent column
  addition.

### 6.2 Recommended migration shape for tab groups

Two reasonable paths; pick one in TECH.md.

(a) **`tab_groups` table** (Chrome model — group is a first-class object):

```sql
-- up.sql
CREATE TABLE tab_groups (
    id INTEGER PRIMARY KEY NOT NULL,
    window_id INTEGER NOT NULL,
    name TEXT,
    color TEXT,                 -- serde_yaml of AnsiColorIdentifier (matches tabs.color)
    is_collapsed INTEGER NOT NULL DEFAULT 0,
    sort_order INTEGER NOT NULL,
    FOREIGN KEY (window_id) REFERENCES windows(id) ON DELETE CASCADE
);
ALTER TABLE tabs ADD group_id INTEGER REFERENCES tab_groups(id) ON DELETE SET NULL;
```

(b) **Embed group on `tabs` directly** (simpler — just `tabs.group_id` + a JSON-encoded
`tab_group_meta` column on `windows`). Less surgery; loses referential integrity; group rename
becomes a JSON edit.

Either way:

1. Add `up.sql` + `down.sql` under
   `crates/persistence/migrations/<TS>_add_tab_groups/`.
2. Run `diesel migration run` (or whatever the workflow-blessed task is — see WARP.md /
   `script/`) to regenerate `crates/persistence/src/schema.rs` (configured in `diesel.toml:5`).
3. If a column type needs a non-default mapping (e.g. `BigInt` instead of `Integer`), extend
   `crates/persistence/schema.patch` (`diesel.toml:11`) — current patch only modifies
   `object_metadata.revision_ts`. Tab-group columns can almost certainly use defaults.
4. Add `Insertable`/`Queryable`/`Identifiable` Rust models in
   `crates/persistence/src/model.rs` next to `Tab`/`NewTab` (`model.rs:344-359`).
5. Update save/load in `app/src/persistence/sqlite.rs`:
   - Save: alongside `diesel::insert_into(schema::tabs::dsl::tabs)` at `:907-909`, insert
     `tab_groups` rows; populate `tabs.group_id` per-tab. **Crucially, the current code does a
     wholesale `delete from tabs` at `:828` then re-inserts everything, so DB ids are not stable
     across saves**. Group identifiers must be stored on the snapshot side as a stable
     in-memory value (e.g. UUID) before they are persisted, otherwise a Drive sync round-trip
     could shuffle them.
   - Load: extend the `Tab::belonging_to(&db_windows)` block at `:2665-2735` to also load
     `tab_groups` and decorate the `TabSnapshot`.
6. Extend `TabSnapshot` (`app/src/app_state.rs:62`) with the per-tab group reference, and
   `WindowSnapshot` (`:44`) with the per-window group catalog.

### 6.3 Save/load entry points (one-page tour)

- Save trigger: `ctx.dispatch_global_action("workspace:save_app", ())` (e.g.
  `app/src/workspace/view.rs:10196`, `:10238`, `:382` — searched widely). Ends in
  `app/src/persistence/sqlite.rs::save_app_state` (insert begins at `:828`).
- Load: `read_sqlite_data` at `app/src/persistence/sqlite.rs:2650`.
- The "what becomes a tab in memory" lifecycle is:
  `Tab` (DB row) → `TabSnapshot` (`app_state.rs:62`) → `add_tab_with_pane_layout(...)`
  (`view.rs:10791`) which constructs a `PaneGroup` view and `self.tabs.push(TabData::new(pg))`.

---

## 7. Telemetry

### 7.1 The pattern

Telemetry events are defined in **one big enum** at `app/src/server/telemetry/events.rs`
(`pub enum TelemetryEvent`). Adding a new event requires touching **four places** in that file:

1. The variant on `TelemetryEvent` (e.g. `:1467-1476`).
2. The JSON-payload arm in the `event_data` / `payload` match (e.g. `:3082-3084`).
3. The `EnablementState` arm (`:5218-5224`) deciding whether the event is always on or gated
   by a feature flag.
4. The human-readable name arm (`:5721-5724`) and the description arm (`:6321-6324`).

For events behind a flag also touch `events.rs:5433` (`Self::AddTabWithShell =>
EnablementState::Flag(FeatureFlag::ShellSelector)` is the template).

### 7.2 Existing tab telemetry (use as templates)

| Event | Variant @ events.rs | Emitted from | When |
| --- | --- | --- | --- |
| `TabRenamed(TabRenameEvent)` | `:1466` | `view.rs:1311`, `:5051`, `:5084`, `:5195` | open editor, set new name, clear name |
| `MoveActiveTab { direction }` | `:1467` | `view.rs:12004` | active-tab moved via menu/keybinding |
| `MoveTab { direction }` | `:1470` | `view.rs:12011` | inactive tab moved |
| `DragAndDropTab` | `:1473` | `view.rs:20665` | `DropTab` action |
| `TabOperations { action: TabTelemetryAction }` | `:1474` | `view.rs:10384` (close), `:10413` (close-others), `:10450/+` (close-direction), `:5114` (rename color section) | close / close-other / close-right / set-color / reset-color (`TabTelemetryAction` enum at `tab.rs:106`) |
| `AddTabWithShell { source, shell }` | `:2229` (gated by `FeatureFlag::ShellSelector`) | `view.rs:10660` | Shell selector or palette |

Tab activation does NOT currently emit a telemetry event. If tab-groups needs "switched into a
grouped tab" instrumentation, it's a new event.

### 7.3 Suggested events for tab groups

Pattern — extend `TabTelemetryAction` (currently `app/src/tab.rs:106-112`: `CloseTab`,
`CloseOtherTabs`, `CloseTabsToRight`, `SetColor`, `ResetColor`) with `CreateGroup`,
`RenameGroup`, `SetGroupColor`, `RemoveFromGroup`, `CloseGroup`, `CollapseGroup`, …, and emit
via the existing `TelemetryEvent::TabOperations { action }` variant. Keeps the dashboard schema
stable.

For a new top-level event, mirror `MoveTab`/`DragAndDropTab` — short variant,
`EnablementState::Always` (or `Flag(FeatureFlag::TabGroups)`), one-line description.

### 7.4 Macro

Use `send_telemetry_from_ctx!(TelemetryEvent::Foo { … }, ctx)` (defined via
`crate::send_telemetry_from_ctx` at `app/src/server/telemetry/events.rs:375`).
Examples in `view.rs:10384`, `:12004`, `:20665`.

---

## 8. Open questions and risks for spec writers

1. **Stable group identity vs. positional indexes.** Tabs themselves have no stable identity —
   `tabs: Vec<TabData>` indexed by `usize`, all actions take `usize` indexes
   (`app/src/workspace/action.rs:99-258`), DB row ids are regenerated on every save
   (`app/src/persistence/sqlite.rs:828, :907`). For a group to survive close/reorder/restart,
   the group has to carry its own identifier (a UUID stored in `TabSnapshot` and the new
   `tab_groups` table). **The very first design question for TECH.md is: introduce a stable
   per-tab UUID alongside group ids, or only stabilize the group?** Without one, "move tab N
   into group G" actions will be racy under concurrent operations.

2. **Cross-window handoff.** `TransferredTab` (`view.rs:880`) and the
   `HandoffPendingTransfer`/`ReverseHandoff` actions (`action.rs:249-256`) move a tab between
   windows. Should the destination window receive the source's group? A copy of it? Should the
   group split (one half stays in source window, other half moves)? `DragTabsToWindows` flag
   (`tab.rs:1668`) gates this.

3. **Drive (cloud) synchronization.** There is a `workspaces` table
   (`schema.rs:496`, distinct from the `Workspace` view) and team-level workflows for sync. Tab
   groups likely should *not* sync to Drive (they are window-local UI state, like
   `vertical_tabs_panel_open`), but confirm by skimming `app/src/drive/`. If they do sync,
   migrations need to be replicated server-side too — out of scope for this brief; ask.

4. **Naming collision** with existing `tab_group`/`TabGroup` identifiers (see §0). A
   color-coded grep of new code should be part of QA.

5. **Render slot in `TabComponent`.** `TabComponent` is already busy: indicator slot, color
   stripe (gradient/border), close button, tooltip overlay. The visual design must specify
   exactly where the group color band goes (left edge? top? underline?), and whether collapsed
   groups appear as a single pill in the horizontal bar. Affects `render_tab_container_internal`
   (`tab.rs:1190`). For NewTabStyling builds the borders are already side-aware
   (`tab.rs:1422-1428`).

6. **Vertical-tabs interplay.** The vertical panel already has its own visual grouping
   (`render_tab_group` at `vertical_tabs.rs:1696` is "the row for one tab"). The new feature
   must define whether a tab group renders as a wrapper around N existing
   `render_tab_group` blocks, with its own collapse toggle and color band, or replaces the
   per-tab block when collapsed. There are also three view modes
   (`VerticalTabsViewMode::{Compact, Expanded}`, `VerticalTabsResolvedMode::{Panes,
   FocusedSession, Summary}`) that each need defined behavior.

7. **Drop targets.** Currently two structs (`TabBarDropTargetData`,
   `VerticalTabsPaneDropTargetData` at `mod.rs:1532, :1537`) and one location enum
   (`TabBarLocation` at `:1543`). The new feature needs at minimum: "drop on a group" (creates
   group / adds to group), "drop into a group" (insertion site within), "drop after a group"
   (peer-of-group). Decide whether to extend `TabBarLocation` (`TabIndex` /
   `AfterTabIndex` / `IntoGroup(group_id)` / `AfterGroup(group_id)`) or introduce a separate
   enum.

8. **Active-tab activation when the active tab leaves a group / a group collapses.** Existing
   logic in `set_active_tab_index` (`view.rs:4942`) clamps to `tab_count`. If a collapse hides
   tabs, do we re-activate to the group's first visible tab? Specify.

9. **Undo stack.** `UndoCloseStack::handle(ctx)` (`view.rs:10174-10180`) only tracks
   close-tab events. Should creating/destroying a group be undoable? If so, extend
   `UndoCloseStack`.

10. **Keybindings.** Existing tab keybindings are registered in `workspace/mod.rs::init`
    (`:101+`). Decide which group ops deserve user-bindable keys (likely: `Group selected
    tabs`, `Toggle collapse`, `Cycle to next/prev group`).

---

## 9. Helpful pointers for implementers

- **Tab tests**:
  - Integration: `app/src/integration_testing/tab/{mod,step,assertion}.rs` — currently only
    `assert_tab_title` (`assertion.rs:32`) and `assert_pane_title` (`:6`); a tab-groups feature
    will likely add `assert_tab_group(tab_index, expected_group_name)` etc. The `TestStep`
    framework is the `warp-integration-test` skill's domain (already documented under
    `crates/integration`).
  - Workspace-level: `app/src/integration_testing/workspace/{step,assertions}.rs`.
  - Unit: `app/src/workspace/view_test.rs` and `app/src/workspace/action_tests.rs` are the home
    for synchronous tab-state tests.
- **Where the active tab's pane group is fetched**: `Workspace::active_tab_pane_group` at
  `view.rs:4848`, `Workspace::tab_count` at `:4759`, `Workspace::tab_views` at `:4763`,
  `Workspace::get_pane_group_view(idx)` at `:4749`.
- **Vertical-tabs rename plumbing** uses a separate editor:
  `Workspace::pane_rename_editor` (`view.rs:899`) vs `tab_rename_editor` (`:898`).
- **Tab settings** live in `app/src/workspace/tab_settings.rs` (`TabSettings`,
  `VerticalTabsDisplayGranularity`, `VerticalTabsTabItemMode`, `VerticalTabsViewMode`,
  `VerticalTabsPrimaryInfo`, `VerticalTabsCompactSubtitle`). The new feature will need
  user-facing settings (e.g. "Show group color band on tabs").
- **Feature flag** registration: `FeatureFlag::VerticalTabs`, `::NewTabStyling`,
  `::DragTabsToWindows`, `::TabConfigs`, `::DirectoryTabColors`, etc. live in
  `app/src/features.rs` (search for `pub enum FeatureFlag`). Tab groups will need a new
  variant; gate the entire UI surface behind it.

---

End of brief. If something here is wrong or stale, fix it in the same PR as the change that
makes it stale; downstream agents trust this file.
