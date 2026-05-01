# Tab Groups — Technical Spec

> Status: implementation-ready.
> Sibling: `specs/tab-groups/PRODUCT.md` (approved). Cite numbered invariants from there as `PRODUCT §<n>`.
> Companion: `specs/tab-groups/RESEARCH.md` (citation index for current code).
> Style: every claim about current code carries a `file_path:line_number` citation.

---

## 1. Problem

Warp lets a user accumulate many tabs in a single window but offers only per-tab color and per-tab rename to manage them. PRODUCT.md asks for Chrome-style **Tab Groups**: a window-local container with a name, a palette color, a collapsed/expanded flag, and a contiguous run of one or more member tabs, surfaced both in the horizontal tab bar and in the vertical-tabs panel, persisted across restarts, and never crossing window boundaries.

This spec translates that product intent into concrete edits to the existing tab/workspace code. The interesting constraints are:

- Workspace `tabs: Vec<TabData>` is positionally indexed; every existing tab action takes a `usize` index (`app/src/workspace/action.rs:99-258`).
- Persistence regenerates `tabs.id` on every save — `delete from tabs` then re-insert at `app/src/persistence/sqlite.rs:828, :907`. Any join from `tabs` to a new `tab_groups` table therefore must use a **stable UUID**, not the integer primary key.
- The horizontal tab bar is a single `for i in 0..self.tabs.len()` loop (`app/src/workspace/view.rs:17221`); adding a chip element that is **not** a `TabData` requires re-shaping the loop.
- The token `tab_group` / `TabGroup` is already used in the codebase for two unrelated concepts (see §4). The new feature must live in a fresh module with a unique name to avoid identifier conflicts and grep-noise.
- All product invariants — contiguity (PRODUCT §4, §40), dissolve-on-empty (§3, §51-54), active-tab-pinning (§27-28, §47-49), cross-window handoff strips groups (§60-61) — must be enforced at the workspace-state layer, not just respected by the UI.

## 2. Relevant Code

> All citations are line numbers in this worktree. Read the cited spans before editing.

### Tab data model
- `app/src/tab.rs:134` — `pub struct TabData` (the in-memory tab).
- `app/src/tab.rs:153` — `TabData::new(pane_group)` (the only constructor).
- `app/src/tab.rs:168` — `TabData::color()` resolves user override > directory default.
- `app/src/tab.rs:106-112` — `pub enum TabTelemetryAction` (extended in §13).
- `app/src/tab.rs:182-209` — `TabData::menu_items_with_pane_name_target` (extended in §9).
- `app/src/tab.rs:1462-1672` — `TabComponent::build` and `Draggable::new(...)` wiring; horizontal drop target at `:1446-1452`.

### Workspace state and tab lifecycle
- `app/src/workspace/view.rs:890` — `pub struct Workspace` (will gain `tab_groups: TabGroupRegistry`).
- `app/src/workspace/view.rs:892-894` — `tabs: Vec<TabData>`, `active_tab_index: usize`, `hovered_tab_index: Option<TabBarHoverIndex>`.
- `app/src/workspace/view.rs:880-888` — `pub struct TransferredTab` (cross-window handoff payload, edited in §11).
- `app/src/workspace/view.rs:4774` — `Workspace::get_tab_transfer_info` (handoff source side).
- `app/src/workspace/view.rs:4863-4942` — `activate_tab` / `activate_tab_internal` / `set_active_tab_index` (extended in §7 for collapse-on-activate behavior).
- `app/src/workspace/view.rs:10122-10198` — `Workspace::remove_tab` (gains group dissolve-on-empty hook).
- `app/src/workspace/view.rs:10205-10240` — `Workspace::adopt_transferred_pane_group` (receiver side; assertions in §11).
- `app/src/workspace/view.rs:10257-10455` — `close_tabs`, `close_tab`, `close_other_tabs`, `close_tabs_direction` (group-close pipeline reuses these).
- `app/src/workspace/view.rs:11886-11907` — `on_tab_drag` (member-tab drag inside groups; group-aware in §10).
- `app/src/workspace/view.rs:11919-11989` — `calculate_updated_tab_index{,_vertical}` (single-index midpoint math).
- `app/src/workspace/view.rs:11992-12013` — `move_tab` and its `MoveActiveTab`/`MoveTab` telemetry sends.
- `app/src/workspace/view.rs:17029-17249` — `render_tab_bar_contents` (horizontal tab bar; new chip rendering in §9).
- `app/src/workspace/view.rs:17221` — the `for i in 0..self.tabs.len()` loop that becomes group-aware.
- `app/src/workspace/view.rs:17541-17547` — outer `DropTarget<TabBarDropTargetData>` (gains `AfterGroup` variant).
- `app/src/workspace/view.rs:6488-6520` — `toggle_tab_right_click_menu` and `toggle_vertical_tabs_pane_context_menu` (entry points for menu construction; extended in §8).

### Drop targets
- `app/src/workspace/mod.rs:1531-1546` — `TabBarDropTargetData`, `VerticalTabsPaneDropTargetData`, `TabBarLocation` (all extended in §10).
- `app/src/pane_group/mod.rs:736` — `pub enum TabBarHoverIndex { BeforeTab(usize), OverTab(usize) }` (gains `OverGroupChip(TabGroupId)`).

### Vertical tabs panel
- `app/src/workspace/view/vertical_tabs.rs:1490-1665` — `render_groups` (the per-tab loop; group sections introduced here in §10).
- `app/src/workspace/view/vertical_tabs.rs:1696` — `fn render_tab_group(...)` (existing identifier, see §4).
- `app/src/workspace/view/vertical_tabs.rs:2024-2077` — drag/drop wiring on the per-tab row.
- `app/src/workspace/view/vertical_tabs.rs:735, 791, 818, 4170` — existing `TabGroupColorMode`, `TabGroupDragState`, `compute_tab_group_color_mode` (the naming-collision sources).

### Code editor file-tab bar
- `app/src/code/view.rs:230` — `tab_group: Vec<TabData>` (an unrelated file-tab list; 46 in-file references).
- `app/src/code/view.rs:176-177` — `ClearEditorTabGroupDragPositions`, `ClearWorkspaceTabGroupDragPositions` (public action variants).

### Action enum
- `app/src/workspace/action.rs:99-258` — `pub enum WorkspaceAction` (new variants enumerated in §8).
- `app/src/workspace/mod.rs:101-1525` — `init(app: &mut App)` registers fixed/editable bindings (PRODUCT §6 explicitly defers user-rebindable shortcuts; we still register palette-searchable bindings without keys).

### Persistence
- `crates/persistence/src/schema.rs:357-364` — `tabs` table.
- `crates/persistence/src/model.rs:344-359` — `Tab`, `NewTab` Diesel models.
- `app/src/persistence/sqlite.rs:828` — wholesale `delete from tabs` (the reason `tabs.id` is unstable).
- `app/src/persistence/sqlite.rs:891-941` — save path; `:907-916` is where new `group_uuid` flows into `NewTab`.
- `app/src/persistence/sqlite.rs:2650-2735` — load path; `:2683-2716` is where `TabSnapshot` is hydrated.
- `app/src/app_state.rs:43-69` — `WindowSnapshot` and `TabSnapshot` (extended in §6).
- `crates/persistence/migrations/2022-08-24-040424_tab_color/up.sql` — precedent for adding a per-tab column.
- `crates/persistence/migrations/2026-04-14-150000_add_code_pane_tabs/up.sql` — precedent for a multi-statement migration that introduces a child table referencing `tabs`.
- `diesel.toml:5,11` — schema generation config and patch file.

### Telemetry
- `app/src/server/telemetry/events.rs:1466-1476` — existing `TabRenamed`, `MoveActiveTab`, `MoveTab`, `DragAndDropTab`, `TabOperations` variants.
- `app/src/server/telemetry/events.rs:3081-3084` — JSON payload arms for those variants.
- `app/src/server/telemetry/events.rs:5217-5221` — `EnablementState` arms.
- `app/src/server/telemetry/events.rs:5433` — feature-flag-gated example (`AddTabWithShell => Flag(FeatureFlag::ShellSelector)`).
- `app/src/server/telemetry/events.rs:5720-5724` — human-readable name arms.
- `app/src/server/telemetry/events.rs:6320-6326` — description arms.

### Feature flags
- `crates/warp_features/src/lib.rs:9-847` — `pub enum FeatureFlag` (new `TabGroups` variant added alphabetically near other tab flags such as `TabConfigs:795`).
- `crates/warp_features/src/lib.rs:868-926` — `DOGFOOD_FLAGS` (`TabGroups` listed here for v1).
- `app/Cargo.toml` — features stanza (new `tab_groups` feature).
- `app/src/lib.rs` — `#[cfg(feature = "tab_groups")]` plumbing.

### Theme / color
- `crates/warp_core/src/ui/theme/mod.rs:542-566` — `pub enum AnsiColorIdentifier { Black, Red, Green, Yellow, Blue, Magenta, Cyan, White }`.
- `crates/warp_core/src/ui/theme/color.rs:138-156` — `ui_error_color`, `ui_warning_color`, `ui_yellow_color`, `ui_green_color`, `outline`, `font_color` (semantic accessors).
- `app/src/ui_components/color_dot.rs:18-25` — existing 6-color `TAB_COLOR_OPTIONS` (deliberately distinct from the 8-color group palette; see §6).

### Integration tests
- `app/src/integration_testing/tab/{mod,step,assertion}.rs` — current home of `assert_tab_title`, `assert_pane_title`. Group assertions added here.
- `app/src/integration_testing/workspace/{mod,step,assertions.rs}` — workspace-level steps.
- `crates/integration` — `Builder`/`TestStep` framework (per `warp-integration-test` skill).
- `app/src/workspace/view_test.rs`, `app/src/workspace/action_tests.rs` — synchronous unit-test homes for `Workspace` state.

## 3. Current State

Framed as constraints on the design:

- **No stable tab identifier**: `Workspace::tabs` is a flat `Vec<TabData>` (`view.rs:892`). Every action carries `usize`. Persisted `tabs.id` is regenerated on every save (`sqlite.rs:828, :907`). Group identity therefore must be carried independently — by a UUID minted at group creation, persisted as text on both `tabs` (the per-row pointer) and `tab_groups` (the group's own row), and round-tripped through `TabSnapshot`.
- **Single chip-blind reorder loop**: `move_tab` swaps adjacent indices (`view.rs:11992-12013`), `on_tab_drag` calls `tabs.swap(...)` (`view.rs:11886-11907`), `calculate_updated_tab_index{,_vertical}` (`:11919, :11956`) compares one tab's drag midpoint to one neighbor. None of them know about contiguity. They will produce broken layouts (interleaved members) the moment a group exists, unless they all route through a new chokepoint that re-clamps to contiguous boundaries.
- **`for i in 0..self.tabs.len()` is the only render shape**: the horizontal bar (`view.rs:17221`) and the vertical panel (`vertical_tabs.rs:1518, 1648`) both walk the tabs vector in order. The horizontal chip and vertical section header are not `TabData` and have no `PaneGroup`. The render walk has to drive off a joint structure (registry + tabs) without minting fake `TabData` for chips.
- **Two existing identifiers named `tab_group`**:
  - `app/src/workspace/view/vertical_tabs.rs:735, 791, 818, 1696, 4170` — `enum TabGroupColorMode`, `struct TabGroupDragState`, `fn render_tab_group`, `fn compute_tab_group_color_mode`. All refer to the visual representation of a *single* `TabData`'s row in the vertical panel ("the row that draws one tab"). Module-private — only used inside `vertical_tabs.rs`.
  - `app/src/code/view.rs:230` — `tab_group: Vec<TabData>` field on `CodeView`. The file-tab list inside a code editor pane. 46 in-file references. Plus public action variants `ClearEditorTabGroupDragPositions`, `ClearWorkspaceTabGroupDragPositions` (`code/view.rs:176-177`).
  - These are pre-existing usages with no relation to the new feature. The total cost of renaming them is ~60 sites in two files plus action enum variants. The cost of namespacing the new feature is zero in existing code.
- **Cross-window handoff already exists**: `TransferredTab` (`view.rs:880-888`) carries `pane_group, color, custom_title, ...` between windows. There is no group field on it today, and PRODUCT §60-61 says there shouldn't be — but stripping must happen at the **source** (`get_tab_transfer_info` at `view.rs:4774`) so the destination cannot accidentally re-attach.
- **Persistence is delete-and-rewrite**: `sqlite.rs:828` wholesale-deletes the `tabs` table on every save. The `tab_groups` table will follow the same pattern. Any FK from `tabs.group_id INTEGER` to `tab_groups.id INTEGER` would dangle the moment we re-save. Therefore the join key is a UUID column on both sides.
- **Telemetry has the four-touchpoint pattern**: variant decl + payload arm + enablement arm + name arm + description arm — `events.rs:1466, 3082, 5218, 5721, 6321` for `MoveActiveTab` is the template. Some events also touch `:5433` for flag-gated enablement.

## 4. Naming

User-facing string is "Tab Group" (PRODUCT §1, §31, §66-69). Internally we need a unique Rust identifier so the feature can be grepped without matching unrelated existing code.

**Decision: namespace under a new module, do not rename existing usages.**

Concretely:
- New module: `app/src/workspace/tab_group/mod.rs`. Public types: `pub struct TabGroup`, `pub struct TabGroupId(uuid::Uuid)`, `pub enum TabGroupColor`, `pub struct TabGroupRegistry`.
- Anywhere outside that module, refer to the type qualified: `crate::workspace::tab_group::TabGroup`. Inside the module, the unqualified `TabGroup` is fine.
- New `WorkspaceAction` variants are namespaced via prefix: `CreateTabGroupFromTab`, `RenameTabGroup`, `RecolorTabGroup`, `CollapseTabGroup`, `ExpandTabGroup`, `UngroupTabGroup`, `CloseTabGroup`, `AddTabToTabGroup`, `RemoveTabFromGroup`, `ToggleTabGroupCollapsed`, `StartTabGroupDrag`, `DragTabGroup`, `DropTabGroup`, `ToggleTabGroupContextMenu` (full list in §8). The `TabGroup` infix disambiguates from any future tab-singular action.

**Why namespacing, not renaming:**

| Site | Renaming cost | Module-namespacing cost |
| --- | --- | --- |
| `app/src/workspace/view/vertical_tabs.rs:735, 791, 818, 1696, 4170` (5 identifiers, internal) | 5 type renames + ~20 use-site renames | 0 |
| `app/src/code/view.rs` (`tab_group: Vec<TabData>` + 46 references) | ~46 line edits, plus rename of struct field used across the module | 0 |
| `app/src/workspace/action.rs:176-177` (`ClearEditorTabGroupDragPositions`, `ClearWorkspaceTabGroupDragPositions`) | Public enum variants — every dispatcher and matcher renamed | 0 |

Renaming would touch ~60 lines across three files plus public action enum variants, for zero functional gain. The new module is self-contained at `app/src/workspace/tab_group/`; Rust's path resolution scopes `TabGroup` cleanly.

**Trade-off documented for future readers**: a contributor grepping `tab_group` will see hits in `vertical_tabs.rs` and `code/view.rs` that are unrelated to this feature. The module-doc comment at the top of `app/src/workspace/tab_group/mod.rs` MUST call this out:

```rust
//! Tab Groups (Chrome-like). Distinct from:
//! - `vertical_tabs::render_tab_group` and `TabGroupColorMode` — the per-tab row in the
//!   vertical-tabs panel (see app/src/workspace/view/vertical_tabs.rs:1696).
//! - `code::view::CodeView::tab_group` — the file-tab bar inside a code-editor pane
//!   (see app/src/code/view.rs:230).
//! Grep `crate::workspace::tab_group::` for hits in this feature.
```

## 5. Data model

All new types live in `app/src/workspace/tab_group/mod.rs`.

### 5.1 Identity

```rust
/// Stable per-group identifier. Minted at group creation and round-tripped through
/// persistence. Distinct from any DB integer PK because `tabs.id` is regenerated on
/// every save (see app/src/persistence/sqlite.rs:828).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TabGroupId(pub uuid::Uuid);

impl TabGroupId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}
```

### 5.2 Color

```rust
/// 8-color palette for tab groups. Order matches PRODUCT §6.
/// Round-robin default selection lives in `TabGroupRegistry::next_default_color` (§7).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Sequence)]
pub enum TabGroupColor {
    Grey,
    Blue,
    Red,
    Yellow,
    Green,
    Pink,
    Purple,
    Cyan,
}

impl TabGroupColor {
    pub fn display_name(self) -> &'static str { /* "Grey", "Blue", ... */ }

    /// Resolves to the concrete fill used by the chip / section-header / member-tab band.
    /// Theme-aware: prefers existing semantic accessors where they fit, falls back to
    /// the literal hex in `palette_hex(self)`. See §6 for the mapping.
    pub fn to_fill(self, theme: &WarpTheme) -> Fill { /* ... */ }
}
```

### 5.3 The group itself

```rust
#[derive(Clone, Debug, PartialEq)]
pub struct TabGroup {
    pub id: TabGroupId,
    pub name: String,                  // empty string allowed (PRODUCT §12, §14)
    pub color: TabGroupColor,
    pub collapsed: bool,
}
```

The list of member tabs is **not** stored on `TabGroup`. Membership is stored on `TabData` / `TabSnapshot` as `Option<TabGroupId>`, and member positions are derived from `Workspace::tabs`. Contiguity is an invariant enforced at the workspace layer (§7).

### 5.4 Registry

```rust
/// Owned by `Workspace`. Tracks all groups in this window. Round-trip persisted via
/// `WindowSnapshot::tab_groups: Vec<TabGroupSnapshot>` (§6).
#[derive(Default, Clone, Debug, PartialEq)]
pub struct TabGroupRegistry {
    /// Insertion-stable map. Order is irrelevant for serialization; group order in the
    /// tab bar derives from the position of the group's first member tab.
    groups: HashMap<TabGroupId, TabGroup>,
}

impl TabGroupRegistry {
    pub fn get(&self, id: TabGroupId) -> Option<&TabGroup>;
    pub fn get_mut(&mut self, id: TabGroupId) -> Option<&mut TabGroup>;
    pub fn insert(&mut self, group: TabGroup);
    pub fn remove(&mut self, id: TabGroupId) -> Option<TabGroup>;
    pub fn iter(&self) -> impl Iterator<Item = (&TabGroupId, &TabGroup)>;
    pub fn len(&self) -> usize;

    /// PRODUCT §8: round-robin pick of the next default color, skipping any color
    /// already used by an existing group when possible. After all 8 are in use,
    /// reuse is allowed (returns the next one in palette order regardless).
    pub fn next_default_color(&self) -> TabGroupColor;
}
```

### 5.5 `TabData` extension

Add **one** field to `TabData` (`app/src/tab.rs:134`):

```rust
pub struct TabData {
    // ... existing fields unchanged ...
    pub group_id: Option<TabGroupId>,
}
```

`TabData::new(pane_group)` (`tab.rs:153`) defaults `group_id: None`.

No method on `TabData` reads or writes `group_id` — all mutation goes through `Workspace` so the contiguity invariant is preserved (§7).

## 6. Persistence

### 6.1 Migration

New directory: `crates/persistence/migrations/2026-05-01-000000_add_tab_groups/`. Two files.

**`up.sql`** (full SQL, not pseudo-code):

```sql
-- Tab groups (Chrome-like): a window-local container of contiguous tabs.
-- Identity is a UUID stored as TEXT, NOT the integer PK, because `tabs.id`
-- and `tab_groups.id` are both regenerated on every save
-- (see app/src/persistence/sqlite.rs:828).
CREATE TABLE tab_groups (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    uuid TEXT NOT NULL,
    window_id INTEGER NOT NULL,
    name TEXT NOT NULL DEFAULT '',
    color TEXT NOT NULL,
    is_collapsed INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY (window_id) REFERENCES windows(id) ON DELETE CASCADE,
    UNIQUE (window_id, uuid)
);

-- Per-tab pointer to the tab's group, by stable UUID. Nullable: ungrouped tabs
-- have NULL. The integer FK from tabs.id to anything is unsafe because tab IDs
-- are regenerated on every save; the UUID join is stable across save cycles.
ALTER TABLE tabs ADD COLUMN group_uuid TEXT;
```

**`down.sql`**:

```sql
ALTER TABLE tabs DROP COLUMN group_uuid;
DROP TABLE tab_groups;
```

After running `diesel migration run`, regenerate `crates/persistence/src/schema.rs` (configured at `diesel.toml:5`). The current `schema.patch` (`diesel.toml:11`) only patches `object_metadata.revision_ts`; `tab_groups` and the new `tabs.group_uuid` use default Diesel types and require no patch addition.

### 6.2 Diesel models

In `crates/persistence/src/model.rs`, alongside `Tab`/`NewTab` (`:344-359`), add:

```rust
#[derive(Identifiable, Queryable, Associations)]
#[diesel(belongs_to(Window))]
#[diesel(table_name = tab_groups)]
pub struct TabGroupRow {
    pub id: i32,
    pub uuid: String,
    pub window_id: i32,
    pub name: String,
    pub color: String,         // serde_yaml of TabGroupColor
    pub is_collapsed: bool,    // SQLite stores 0/1 as INTEGER; Diesel maps via SqlType=Bool
}

#[derive(Insertable)]
#[diesel(table_name = tab_groups)]
pub struct NewTabGroupRow {
    pub uuid: String,
    pub window_id: i32,
    pub name: String,
    pub color: String,
    pub is_collapsed: bool,
}
```

Extend `Tab` (`:344-351`) and `NewTab` (`:353-359`):

```rust
pub struct Tab {
    pub id: i32,
    pub window_id: i32,
    pub custom_title: Option<String>,
    pub color: Option<String>,
    pub group_uuid: Option<String>,   // new
}

pub struct NewTab {
    pub window_id: i32,
    pub custom_title: Option<String>,
    pub color: Option<String>,
    pub group_uuid: Option<String>,   // new
}
```

### 6.3 Snapshot extensions

In `app/src/app_state.rs`:

```rust
#[derive(Clone, Debug, PartialEq)]
pub struct TabGroupSnapshot {
    pub id: TabGroupId,
    pub name: String,
    pub color: TabGroupColor,
    pub collapsed: bool,
}

pub struct WindowSnapshot {
    // ... existing fields unchanged ...
    pub tab_groups: Vec<TabGroupSnapshot>,   // new
}

pub struct TabSnapshot {
    // ... existing fields unchanged ...
    pub group_id: Option<TabGroupId>,        // new
}
```

`WindowSnapshot::tab_groups` is a `Vec` (not a `HashMap`) so the snapshot is `PartialEq`-by-content — order is irrelevant for correctness but consistent ordering simplifies tests. Sort by `TabGroupId` UUID bytes when constructing the snapshot.

### 6.4 Save path

In `app/src/persistence/sqlite.rs::save_app_state`, between the existing `delete from tabs` (`:828`) and `delete from windows` (`:829`), add:

```rust
diesel::delete(schema::tab_groups::dsl::tab_groups).execute(conn)?;
```

Inside the per-window loop after `let window_id: i32 = ... ;` (`:881`) and **before** the `let tabs: Vec<NewTab> = ...` block (`:891`):

```rust
let group_rows: Vec<NewTabGroupRow> = window
    .tab_groups
    .iter()
    .map(|g| NewTabGroupRow {
        uuid: g.id.0.to_string(),
        window_id,
        name: g.name.clone(),
        color: serde_yaml::to_string(&g.color).unwrap_or_default(),
        is_collapsed: g.collapsed,
    })
    .collect();
diesel::insert_into(schema::tab_groups::dsl::tab_groups)
    .values(group_rows)
    .execute(conn)?;
```

Then in the existing `tabs` mapping (`:891-905`), add:

```rust
group_uuid: tab.group_id.map(|id| id.0.to_string()),
```

### 6.5 Load path

In `read_sqlite_data` (`sqlite.rs:2650-2735`), before the existing `db_tabs = Tab::belonging_to(...)` block (`:2665`), load groups:

```rust
let db_groups = TabGroupRow::belonging_to(&db_windows)
    .load::<TabGroupRow>(conn)?
    .grouped_by(&db_windows);
```

Inside the per-window iteration (`:2680`), build the snapshot's `tab_groups`:

```rust
let tab_groups: Vec<TabGroupSnapshot> = groups_for_window
    .into_iter()
    .filter_map(|row| {
        Some(TabGroupSnapshot {
            id: TabGroupId(uuid::Uuid::parse_str(&row.uuid).ok()?),
            name: row.name,
            color: serde_yaml::from_str(&row.color).ok()?,
            collapsed: row.is_collapsed,
        })
    })
    .collect();
```

In the per-tab `filter_map` that builds `TabSnapshot` (`:2683-2716`), populate:

```rust
group_id: tab.group_uuid
    .as_deref()
    .and_then(|s| uuid::Uuid::parse_str(s).ok())
    .map(TabGroupId),
```

### 6.6 Read-time invariant repair

PRODUCT §56: "every persisted group has at least one member; a corrupt empty group is silently dropped."

After both vectors are loaded, in `read_sqlite_data` before assembling the `WindowSnapshot`:

```rust
let referenced: HashSet<TabGroupId> = saved_tabs
    .iter()
    .filter_map(|t| t.group_id)
    .collect();
let tab_groups: Vec<TabGroupSnapshot> = tab_groups
    .into_iter()
    .filter(|g| referenced.contains(&g.id))
    .collect();
let valid_ids: HashSet<TabGroupId> = tab_groups.iter().map(|g| g.id).collect();
for t in &mut saved_tabs {
    if let Some(id) = t.group_id {
        if !valid_ids.contains(&id) {
            t.group_id = None;
        }
    }
}
```

PRODUCT §57: collapsed groups whose active tab is a member must be expanded on restore. After invariant repair, before saving the snapshot:

```rust
if let Some(active_tab) = saved_tabs.get(active_tab_index) {
    if let Some(active_group_id) = active_tab.group_id {
        if let Some(g) = tab_groups.iter_mut().find(|g| g.id == active_group_id) {
            g.collapsed = false;
        }
    }
}
```

## 7. Workspace state and invariants

### 7.1 `Workspace` field

```rust
pub struct Workspace {
    // ... existing fields ...
    pub(crate) tab_groups: TabGroupRegistry,
}
```

Initialized to `TabGroupRegistry::default()` in the constructor (`view.rs:3037`).

### 7.2 Invariants enforced by `Workspace` methods

These are **always true** between message-pump ticks:

I1. **Contiguity** (PRODUCT §4): for any group `g`, `tabs.iter().enumerate().filter(|(_, t)| t.group_id == Some(g.id))` produces indices that form a contiguous run.
I2. **Non-empty** (PRODUCT §3, §51): for every `g` in the registry, at least one `tab` has `group_id == Some(g.id)`.
I3. **Active visibility** (PRODUCT §27, §47): if `tabs[active_tab_index].group_id == Some(g.id)` then `!tab_groups[g.id].collapsed`.
I4. **Group order** (PRODUCT §5): groups appear in tab-bar order according to the position of their first member; this is implicit because contiguity makes group order a function of `tabs` itself.
I5. **Single membership** (PRODUCT §2): each `TabData` has at most one `group_id` — enforced by Rust's `Option`.

`debug_assert_invariants(&self)` is called at the end of every mutating method on `Workspace` that touches groups or tab order, and asserts I1, I2, I3 hold. Released as no-op under `cfg(not(debug_assertions))`.

### 7.3 The reorder chokepoint

All in-place reordering (member drag, group drag, `MoveTabLeft/Right`) routes through:

```rust
impl Workspace {
    /// Move the tab at `from` to position `to`, snapping `to` so that the move
    /// preserves contiguity (I1). Returns the actual landing index, which may
    /// differ from `to` when `to` would split a group run that the dragged tab
    /// is not a member of. Updates `active_tab_index` if affected. Calls
    /// `debug_assert_invariants` on exit.
    fn reorder_tab(&mut self, from: usize, to: usize, ctx: &mut ViewContext<Self>) -> usize;
}
```

Snap rules (executed in order, the first one matched wins):

1. If `to == from`, no-op.
2. Compute `target_group = tabs[from].group_id`. Compute `dest_group = self.dest_group_at(to, exclude_idx = Some(from))`.
3. If `target_group == dest_group`, accept `to` directly: the tab is staying inside its own group's run (or moving between two ungrouped tabs).
4. Otherwise, snap `to` to the nearest **boundary** index of any group run not equal to `target_group`: either the run's start or the index immediately after the run's end, whichever is closer. This guarantees the moved tab lands ungrouped between groups, never interleaved.

After the snap, `self.tabs.swap` is replaced by a `Vec::remove + Vec::insert` (swap can break contiguity over multi-step shifts).

All existing call-sites are rewritten to call `reorder_tab`:

- `Workspace::move_tab` (`view.rs:11992-12013`): `self.reorder_tab(index, new_index_from_direction, ctx);`
- `Workspace::on_tab_drag` (`view.rs:11886-11907`): `self.reorder_tab(current_index, new_index_from_drag_math, ctx);`
- A new `on_chip_drag` (§10) for whole-group reorder calls `reorder_group(from_run, to_position)` — see §7.5.

### 7.4 Group lifecycle methods

Added to `Workspace`:

```rust
impl Workspace {
    /// PRODUCT §9, §32: create a new group from a single tab, immediately enter rename.
    pub fn create_tab_group_from_tab(&mut self, tab_idx: usize, ctx: &mut ViewContext<Self>);

    /// PRODUCT §31, §33: move a tab into an existing group at the run's end.
    /// Performs a `reorder_tab` to land the tab adjacent to the group's run, then
    /// sets the tab's `group_id`. If the tab was previously in a different group,
    /// it leaves first (which may dissolve the source group via `prune_empty_group`).
    pub fn move_tab_into_group(
        &mut self,
        tab_idx: usize,
        group_id: TabGroupId,
        ctx: &mut ViewContext<Self>,
    );

    /// PRODUCT §35-37: clear the tab's `group_id`. Tab keeps its position; if
    /// removing it leaves the source group with members on both sides of any
    /// non-member, the existing contiguity is already preserved (a group's run
    /// is always contiguous, so removal cannot split it). May dissolve the
    /// source group via `prune_empty_group`.
    pub fn remove_tab_from_group(&mut self, tab_idx: usize, ctx: &mut ViewContext<Self>);

    /// PRODUCT §30 → Ungroup: dissolve a group, leaving members in place
    /// without group_id.
    pub fn ungroup_tab_group(&mut self, group_id: TabGroupId, ctx: &mut ViewContext<Self>);

    /// PRODUCT §13-15: enter inline rename for the chip / section header.
    /// Cancels any in-progress tab/group rename first.
    pub fn rename_tab_group(&mut self, group_id: TabGroupId, ctx: &mut ViewContext<Self>);

    /// PRODUCT §17: apply a new color, persist via dispatch_global_action.
    pub fn recolor_tab_group(
        &mut self,
        group_id: TabGroupId,
        color: TabGroupColor,
        ctx: &mut ViewContext<Self>,
    );

    /// PRODUCT §25-29: toggle collapse. If `collapsed=true` and the active tab
    /// is a member of `group_id`, this is a no-op (chip remains expanded). The
    /// chip's left-click and the section-header chevron route through this.
    pub fn set_tab_group_collapsed(
        &mut self,
        group_id: TabGroupId,
        collapsed: bool,
        ctx: &mut ViewContext<Self>,
    );

    /// PRODUCT §42-45: close every member tab of the group in workspace-tab
    /// order via the existing close pipeline. Cancellation aborts the whole
    /// close (no partial). Group dissolves on close of last member via
    /// `prune_empty_group`.
    pub fn close_tab_group(&mut self, group_id: TabGroupId, ctx: &mut ViewContext<Self>);

    /// Internal: invariant-restoring helper called by every method that may
    /// orphan a group. Removes the group from the registry if no `TabData`
    /// references it. Idempotent.
    fn prune_empty_group(&mut self, group_id: TabGroupId);

    /// Helper: indices of the contiguous run of members of `group_id`.
    /// Empty slice when no members exist.
    fn group_member_range(&self, group_id: TabGroupId) -> std::ops::Range<usize>;
}
```

### 7.5 Group-as-unit reorder

```rust
impl Workspace {
    /// PRODUCT §40: reorder a whole group as a unit. Moves the contiguous run
    /// `group_member_range(group_id)` so that its first member lands at
    /// `target_first_index`, snapped to a group-boundary so we never split
    /// another group's run.
    fn reorder_group(
        &mut self,
        group_id: TabGroupId,
        target_first_index: usize,
        ctx: &mut ViewContext<Self>,
    );
}
```

Implementation: extract the run with `Vec::drain(start..end)`, snap `target_first_index` to the nearest non-group-interior index in the post-drain vector, and `Vec::splice` the run back in. Update `active_tab_index` by the same delta if the active tab was in the moved run.

### 7.6 Active-tab pinning at activation time

PRODUCT §28: activating a tab inside a collapsed group must expand the group as a side effect.

In `Workspace::set_active_tab_index` (`view.rs:4942`), at the **start** of the method, before any other field mutation:

```rust
if let Some(group_id) = self.tabs.get(new_index).and_then(|t| t.group_id) {
    if self.tab_groups.get(group_id).is_some_and(|g| g.collapsed) {
        self.tab_groups.get_mut(group_id).unwrap().collapsed = false;
        // Don't dispatch save here; set_active_tab_index already saves.
    }
}
```

Doing this in the same call (not via an event) ensures hover math and the next render see a consistent expanded state. Telemetry: emit `TabGroupExpanded` via `prune_empty_group`-style helper if expansion was a side effect of activation (PRODUCT §74's "expand group" event covers it).

Conversely, `set_tab_group_collapsed(_, true, _)` checks I3 and treats "active member ⇒ no-op" as the documented behavior (PRODUCT §27).

### 7.7 Save dispatch

Every group-mutating method ends with `ctx.dispatch_global_action("workspace:save_app", ())` (mirroring `remove_tab` at `view.rs:10196`). This includes: create, rename, recolor, collapse, expand, ungroup, add-tab-to-group, remove-tab-from-group, reorder-group, close-group.

## 8. Actions

### 8.1 New `WorkspaceAction` variants

Add to `app/src/workspace/action.rs:99-258`:

```rust
pub enum WorkspaceAction {
    // ... existing variants ...

    // ── Tab Groups (gated by FeatureFlag::TabGroups) ────────────────────

    /// Right-click → "Add tab to group" → "New group" (PRODUCT §9, §32).
    CreateTabGroupFromTab { tab_index: usize },

    /// Right-click → "Add tab to group" → "{name}" (PRODUCT §31).
    AddTabToTabGroup { tab_index: usize, group_id: TabGroupId },

    /// Right-click → "Add tab to group" → "Remove from group" (PRODUCT §35).
    RemoveTabFromTabGroup { tab_index: usize },

    /// Chip / section-header right-click → Rename (PRODUCT §13).
    RenameTabGroup { group_id: TabGroupId },

    /// Inline-rename commit handler (PRODUCT §14).
    SetTabGroupName { group_id: TabGroupId, name: String },

    /// Chip / section-header right-click → Recolor → {color} (PRODUCT §17).
    RecolorTabGroup { group_id: TabGroupId, color: TabGroupColor },

    /// Chip left-click, section-header chevron, right-click → Collapse/Expand
    /// (PRODUCT §20, §22, §29).
    ToggleTabGroupCollapsed { group_id: TabGroupId },

    /// Chip / section-header right-click → Ungroup (PRODUCT §30 Ungroup).
    UngroupTabGroup { group_id: TabGroupId },

    /// Chip / section-header right-click → Close group (PRODUCT §42).
    CloseTabGroup { group_id: TabGroupId },

    /// Right-click on chip / section header (PRODUCT §30, §69).
    ToggleTabGroupContextMenu {
        group_id: TabGroupId,
        position: Vector2F,
    },

    // ── Group drag (chip / section-header) ──────────────────────────────

    /// Begin a chip-drag (whole-group reorder, PRODUCT §40).
    StartTabGroupDrag { group_id: TabGroupId },

    /// Per-frame chip drag position update.
    DragTabGroup { group_id: TabGroupId, position: RectF },

    /// Drop a chip into its new position.
    DropTabGroup { group_id: TabGroupId },
}
```

### 8.2 Dispatch path

`Workspace::handle_action` (the `match` block in `view.rs` near `:19750`) gains arms that dispatch to the §7 methods. Each arm checks `FeatureFlag::TabGroups.is_enabled()` first — if disabled, log-and-drop. Pattern, mirroring the existing tab-action arms:

```rust
WorkspaceAction::CreateTabGroupFromTab { tab_index } => {
    if !FeatureFlag::TabGroups.is_enabled() { return; }
    self.create_tab_group_from_tab(tab_index, ctx);
}
WorkspaceAction::AddTabToTabGroup { tab_index, group_id } => {
    if !FeatureFlag::TabGroups.is_enabled() { return; }
    self.move_tab_into_group(tab_index, group_id, ctx);
}
// ... etc, one arm per variant ...
```

The drag arms (`StartTabGroupDrag`/`DragTabGroup`/`DropTabGroup`) live next to the existing `StartTabDrag`/`DragTab`/`DropTab` arms (`view.rs:20275-20666`) and call `Workspace::on_chip_drag` / `Workspace::reorder_group` analogously.

### 8.3 Binding registration

`app/src/workspace/mod.rs::init` (starts at `:101`) gets palette-searchable bindings (no key, per PRODUCT §6) for the group-level actions:

```rust
EditableBinding::new(
    "workspace:create_tab_group_from_active_tab",
    "Group active tab into new tab group",
    /* dispatch via ActiveTab variant - see below */
)
.with_context_predicate(id!("Workspace"))
.with_enabled(|| FeatureFlag::TabGroups.is_enabled()),
// ... similarly for ungroup-active, recolor (8 sub-bindings, one per color),
// rename-active-group, collapse-active-group, close-active-group ...
```

For the palette to dispatch group commands without an active group_id, also add **active-tab-derived** companion variants:

```rust
WorkspaceAction::CreateTabGroupFromActiveTab,
WorkspaceAction::UngroupActiveTabGroup,
WorkspaceAction::CollapseActiveTabGroup,
WorkspaceAction::ExpandActiveTabGroup,
WorkspaceAction::RenameActiveTabGroup,
WorkspaceAction::CloseActiveTabGroup,
WorkspaceAction::RecolorActiveTabGroup(TabGroupColor),
```

Each of these resolves to `tabs[active_tab_index].group_id` and dispatches the underlying variant. They are no-ops (with a log line) when the active tab has no `group_id` and the operation requires one.

### 8.4 Right-click menu on a tab

Extend `TabData::menu_items_with_pane_name_target` (`tab.rs:182-209`). Insert a new section helper between `modify_tab_menu_items` and `close_tab_menu_items`:

```rust
fn add_tab_to_group_submenu(
    &self,
    index: usize,
    workspace: &Workspace,
    ctx: &AppContext,
) -> Vec<MenuItem<WorkspaceAction>> {
    if !FeatureFlag::TabGroups.is_enabled() {
        return vec![];
    }

    let mut submenu = vec![
        MenuItemFields::new("New group")
            .with_on_select_action(WorkspaceAction::CreateTabGroupFromTab { tab_index: index })
            .into_item(),
    ];

    // PRODUCT §66-67: list all groups in this window. The tab's current group
    // is shown disabled.
    let current = self.group_id;
    for (gid, group) in workspace.tab_groups.iter() {
        let label = if group.name.is_empty() {
            group.color.display_name().to_string()
        } else {
            group.name.clone()
        };
        let item = MenuItemFields::new(label);
        let item = if Some(*gid) == current {
            item.disabled()
        } else {
            item.with_on_select_action(WorkspaceAction::AddTabToTabGroup {
                tab_index: index,
                group_id: *gid,
            })
        };
        submenu.push(item.into_item());
    }

    // PRODUCT §35 Remove from group, only when the tab is in a group.
    if current.is_some() {
        submenu.insert(0, MenuItem::Separator);   // visually separate above existing options
        submenu.insert(0,
            MenuItemFields::new("Remove from group")
                .with_on_select_action(WorkspaceAction::RemoveTabFromTabGroup { tab_index: index })
                .into_item(),
        );
    }

    vec![MenuItemFields::new("Add tab to group").with_submenu(submenu).into_item()]
}
```

Because `TabData` doesn't have access to `Workspace`, pass `workspace: &Workspace` through `menu_items_with_pane_name_target`'s signature (one extra parameter; both call sites — `Workspace::toggle_tab_right_click_menu` at `view.rs:6488-6504` and `Workspace::toggle_vertical_tabs_pane_context_menu` at `view.rs:6510` — already have `self`).

The submenu is appended to the existing menu chain at `tab.rs:193-198` between `modify_tab_menu_items` and `close_tab_menu_items`.

### 8.5 Right-click menu on a chip / section header

A new menu, owned by `Workspace::tab_group_right_click_menu: ViewHandle<Menu<WorkspaceAction>>` (mirrors `tab_right_click_menu` at `view.rs:908-909`). Population helper:

```rust
fn tab_group_menu_items(
    &self,
    group_id: TabGroupId,
    ctx: &AppContext,
) -> Vec<MenuItem<WorkspaceAction>> {
    let group = self.tab_groups.get(group_id)?;
    let mut items = vec![
        MenuItemFields::new("Rename")
            .with_on_select_action(WorkspaceAction::RenameTabGroup { group_id })
            .into_item(),
        MenuItem::Submenu {
            label: "Recolor".to_string(),
            items: TabGroupColor::iter()
                .map(|c| MenuItemFields::new(c.display_name())
                    .with_check(c == group.color)
                    .with_on_select_action(WorkspaceAction::RecolorTabGroup { group_id, color: c })
                    .into_item())
                .collect(),
        },
        MenuItemFields::new(if group.collapsed { "Expand" } else { "Collapse" })
            .with_on_select_action(WorkspaceAction::ToggleTabGroupCollapsed { group_id })
            .into_item(),
        MenuItem::Separator,
        MenuItemFields::new("Ungroup")
            .with_on_select_action(WorkspaceAction::UngroupTabGroup { group_id })
            .into_item(),
        MenuItemFields::new("Close group")
            .with_on_select_action(WorkspaceAction::CloseTabGroup { group_id })
            .into_item(),
    ];
    items
}
```

`Workspace::toggle_tab_group_context_menu` is the analogue of `toggle_tab_right_click_menu` at `view.rs:6488`. Subscription wiring at `view.rs:1745-1748` is duplicated for the new menu handle.

## 9. Render — horizontal tab bar

### 9.1 New render walk

Replace the `for i in 0..self.tabs.len()` loop in `Workspace::render_tab_bar_contents` (`view.rs:17221`) with a joint walk over the tabs vector and the registry:

```rust
let mut i = 0usize;
while i < self.tabs.len() {
    // Hover indicator before this tab (existing logic, unchanged).
    if matches!(self.hovered_tab_index, Some(TabBarHoverIndex::BeforeTab(idx)) if idx == i) {
        tab_bar.add_child(self.render_tab_hover_indicator(appearance));
    }

    match self.tabs[i].group_id {
        Some(gid) => {
            let group = self.tab_groups.get(gid).expect("registry invariant I2");
            // Render the chip exactly once per run, when entering it.
            tab_bar.add_child(self.render_tab_group_chip(gid, group, ctx));
            if group.collapsed {
                // Skip past the entire run — members are hidden.
                let run = self.group_member_range(gid);
                i = run.end;
                continue;
            }
            // Render the run's tabs.
            let run = self.group_member_range(gid);
            for j in run.clone() {
                tab_bar.add_child(self.render_tab_in_tab_bar(j, tab_bar_state, ctx));
            }
            i = run.end;
        }
        None => {
            tab_bar.add_child(self.render_tab_in_tab_bar(i, tab_bar_state, ctx));
            i += 1;
        }
    }
}
// Existing trailing hover indicator + new-session button block, unchanged.
```

Contiguity (I1) is what makes this loop sound: every group's members are guaranteed to occupy `run.start..run.end` once `i == run.start`.

### 9.2 The chip itself

`Workspace::render_tab_group_chip(group_id, group, ctx) -> Box<dyn Element>`. New file: `app/src/workspace/tab_group/chip.rs`. Layout:

- `Container` with a `CornerRadius::all(6.)` rounded rect, fill = `group.color.to_fill(theme)`.
- A `Flex::row()` content:
  - A small leading dot (4px) in `theme.font_color(group_color)` (contrasting fill); only when name is non-empty.
  - Text: `if group.name.is_empty() { group.color.display_name() } else { &group.name }`. Reduced opacity when falling back to the color name (PRODUCT §18, "at reduced contrast").
  - When collapsed, append `· {member_count}`.
- `Padding` left/right 8px, vertical 2px to match `TAB_BAR_HEIGHT` constraints (`tab.rs:54`).
- Width: ergonomic min of `~60px`, max of `~140px`, with `truncate_from_end` for long names.
- Wrap in a `Hoverable` that:
  - On left mouse down: `WorkspaceAction::ToggleTabGroupCollapsed { group_id }` (PRODUCT §20).
  - On right mouse down: `WorkspaceAction::ToggleTabGroupContextMenu { group_id, position }` (PRODUCT §69).
  - On double-click: `WorkspaceAction::RenameTabGroup { group_id }` (PRODUCT §13).
- Wrap in a `Draggable::new(group.draggable_state.clone(), ...)` firing `StartTabGroupDrag { group_id }` / `DragTabGroup { group_id, position }` / `DropTabGroup { group_id }`. `DragAxis::HorizontalOnly` unless `FeatureFlag::DragTabsToWindows` is on (mirrors `tab.rs:1668`; v1 chip-cross-window drag is out of scope per PRODUCT §41).
- Wrap in a `DropTarget<TabBarDropTargetData { tab_bar_location: TabBarLocation::OnGroupChip(group_id) }>` so that dropping a tab onto the chip adds it to the group (PRODUCT §33). See §10.

The `DraggableState` for the chip is stored on the `TabGroup` struct (added field `pub draggable_state: DraggableState`) so it is preserved across renders.

When the active tab is a member of the group **and** the group is collapsed (which can only happen transiently during a state restoration before the active-tab repair runs — I3 enforces it otherwise), render the chip with a thin focus ring `theme.accent_overlay(...)` so the user can see "active tab is in here."

### 9.3 Color band on member tabs

In `TabComponent::build` (`tab.rs:1462-1672`), when `tab.group_id.is_some()`, layer a `Border` along the bottom edge of the tab container (existing `Border` at `tab.rs:1226-1230`):

```rust
let group_band: Option<Fill> = self.tab.group_id
    .and_then(|gid| group_color_lookup(gid))
    .map(|color| color.to_fill(theme));
```

Add a `Border` of height `2px` along the bottom edge of `render_tab_container_internal` (`tab.rs:1190-1262`) with `group_band` as the fill. Bottom edge is chosen because it doesn't compete with the existing top-edge active-tab indicator and survives `NewTabStyling`'s side-aware borders (`tab.rs:1422-1428`). PRODUCT §18 leaves the exact edge to design — bottom is the explicit choice; flag for designer review.

Active styling (PRODUCT §71): the existing active-tab background gradient (`tab.rs:636-641`) reads through the band; do not suppress it. The band is a bottom border, not a fill replacement.

The existing per-tab indicator slot (`render_indicator` at `tab.rs:1065`) is unchanged — group color is decorative chrome around the indicator (PRODUCT §72).

### 9.4 Active-tab fall-through to chip when collapsed

I3 guarantees this never actually happens at steady state. As defensive code, `render_tab_group_chip` accepts an `is_active_member: bool` that draws an accent ring (see §9.2 last bullet). Tests cover this in §14.

## 10. Render — vertical tabs

### 10.1 New section walk

Replace the per-tab loop in `vertical_tabs::render_groups` (`vertical_tabs.rs:1648-1665`) with a similar joint walk:

```rust
let mut i = 0usize;
while i < visible_tabs.len() {
    let (tab_index, _) = visible_tabs[i];
    match workspace.tabs[tab_index].group_id {
        Some(gid) => {
            let group = workspace.tab_groups.get(gid).unwrap();
            let run = workspace.group_member_range(gid);
            let visible_run_end = visible_tabs[i..]
                .iter()
                .position(|(t, _)| !run.contains(t))
                .map(|n| i + n)
                .unwrap_or(visible_tabs.len());

            groups.add_child(render_tab_group_section(
                state, workspace, gid, group,
                &visible_tabs[i..visible_run_end],
                app,
            ));
            i = visible_run_end;
        }
        None => {
            groups.add_child(render_tab_group(
                state, workspace, tab_index,
                &workspace.tabs[tab_index],
                visible_tabs[i].1.as_deref(),
                TabGroupDragState { /* unchanged */ },
                app,
            ));
            i += 1;
        }
    }
}
```

`render_tab_group` (`vertical_tabs.rs:1696`) keeps its existing meaning ("draw one tab's row"), per the naming decision in §4. The new section wrapper is `render_tab_group_section` in `app/src/workspace/tab_group/vertical_section.rs`.

### 10.2 Section layout

`render_tab_group_section` produces:

```
┌─────────────────────────────┐
│ ▾ ●  Group name      (3)    │   ← section header (height ~28px)
├─────────────────────────────┤
│ │ ▸ Tab 1 ...               │   ← member rows, indented + left stripe
│ │ ▸ Tab 2 ...               │
│ │ ▸ Tab 3 ...               │
└─────────────────────────────┘
```

- Section header (PRODUCT §22):
  - Chevron icon (`▾` expanded, `▸` collapsed). Click toggles `WorkspaceAction::ToggleTabGroupCollapsed { group_id }`.
  - 8px swatch in `group.color.to_fill(theme)`.
  - Group name (or color name fallback at reduced opacity).
  - `(N)` count of member tabs.
  - Right-click → `WorkspaceAction::ToggleTabGroupContextMenu { group_id, position }`.
  - Double-click → `WorkspaceAction::RenameTabGroup { group_id }`.
  - Wrap in `Draggable::new` firing the `StartTabGroupDrag/DragTabGroup/DropTabGroup` actions, `DragAxis::VerticalOnly`.
  - Wrap in a `DropTarget<VerticalTabsPaneDropTargetData { tab_bar_location: TabBarLocation::OnGroupChip(group_id), tab_hover_index: TabBarHoverIndex::OverGroupChip(group_id) }>`.
- Member rows: each rendered by the existing `render_tab_group(...)` (`vertical_tabs.rs:1696`), wrapped in:
  - A `Padding::with_left(12.)` for indentation.
  - A `Border` on the leading (left) edge, 3px wide, fill = `group.color.to_fill(theme)`.
- When `group.collapsed`, omit member rows entirely; the section header alone is rendered.

### 10.3 Search interaction

PRODUCT §63-64. The query filter at `vertical_tabs.rs:1517-1620` is modified:

- A group-section is visible if **any** member tab matches OR `group.name.contains_ignore_ascii_case(query)`. Non-matching members within a visible group are hidden.
- While `!query.is_empty()`, group sections are rendered in **expanded** state regardless of `group.collapsed` (PRODUCT §64). The stored `collapsed` flag is **not** mutated; only the render path overrides. When the query is cleared, the next render uses the stored value.

Implementation: thread an `is_search_active: bool` flag into `render_tab_group_section`; when true, ignore `group.collapsed`.

### 10.4 New-tab button placement

PRODUCT §65: the panel-global new-tab button creates ungrouped tabs. No code change required — `render_new_tab_button` at `vertical_tabs.rs:1331` already calls `WorkspaceAction::AddTerminalTab { hide_homepage: false }` which appends with `group_id: None`. To add a tab to a specific group, the user creates the tab and drags or right-clicks it in.

### 10.5 Drop targets — both surfaces

`TabBarLocation` (`workspace/mod.rs:1543`) gains variants:

```rust
pub enum TabBarLocation {
    TabIndex(usize),                 // existing — drop adjacent to tab i
    AfterTabIndex(usize),             // existing — drop at end / after the run
    OnGroupChip(TabGroupId),          // new — drop onto a chip (add to group)
    BeforeGroup(TabGroupId),          // new — drop just before group's run
    AfterGroup(TabGroupId),           // new — drop just after group's run
}
```

`TabBarHoverIndex` (`pane_group/mod.rs:736`) gains `OverGroupChip(TabGroupId)`.

Drop-target data structs (`workspace/mod.rs:1531-1546`) are unchanged in shape — they already wrap `TabBarLocation`. Their existing `DropTargetData` impls cover the new variants for free.

### 10.6 Drop resolution

Routing into the existing `Workspace::handle_action` `DropTab`/`DropTabGroup` arms (`view.rs:20663-20666` and the new chip arms). The drop handler reads the active drop target's `TabBarLocation` and calls:

| Source | Drop target | Result |
| --- | --- | --- |
| Tab `t`, no source group | `TabIndex(j)` adjacent to ungrouped tab | `reorder_tab(t, j)` (existing behavior) |
| Tab `t`, no source group | `TabIndex(j)` adjacent to grouped tab | `reorder_tab` then `move_tab_into_group(t, dest_group)` |
| Tab `t`, no source group | `OnGroupChip(g)` | `move_tab_into_group(t, g)` (PRODUCT §33) |
| Tab `t`, no source group | `BeforeGroup(g)` / `AfterGroup(g)` | `reorder_tab` to that boundary, leave ungrouped |
| Tab `t`, in group `g` | `TabIndex(j)` inside the run of `g` | `reorder_tab(t, j)` (intra-group reorder, PRODUCT §39) |
| Tab `t`, in group `g` | `TabIndex(j)` outside `g`'s run | `remove_tab_from_group(t)` then `reorder_tab(t, j)`; may dissolve `g` (PRODUCT §53) |
| Tab `t`, in group `g` | `OnGroupChip(g')`, `g' != g` | `remove_tab_from_group(t)` then `move_tab_into_group(t, g')` |
| Tab `t`, in group `g` | `OnGroupChip(g)` | no-op |
| Chip `g` | `TabIndex(j)` / `BeforeGroup(g')` / `AfterGroup(g')` | `reorder_group(g, j)` snapped (PRODUCT §40) |
| Chip `g` | `OnGroupChip(g')` | no-op (cannot interleave groups, PRODUCT §4) |

This dispatch table lives in a new `Workspace::resolve_drop` function called from the `DropTab` and `DropTabGroup` handlers.

### 10.7 Drag math for chips

Existing `calculate_updated_tab_index{,_vertical}` (`view.rs:11919-11989`) operates on a single index. For chip drag, add:

```rust
fn calculate_updated_group_position(
    &self,
    group_id: TabGroupId,
    drag_position: RectF,
    is_vertical: bool,
    ctx: &mut ViewContext<Self>,
) -> usize;
```

The function:
1. Computes `run = self.group_member_range(group_id)`.
2. Probes `element_position_by_id(tab_position_id(run.start - 1))` (left/above neighbor) and `tab_position_id(run.end)` (right/below neighbor).
3. Returns the **first member's** target landing index, snapped to a valid boundary index where reordering to that index does not split another group.

`Workspace::on_chip_drag` calls this and `reorder_group`, mirroring `on_tab_drag` (`view.rs:11886-11907`). On `DropTabGroup`, emit `TelemetryEvent::DragAndDropTabGroup` (§13).

## 11. Cross-window handoff

PRODUCT §60-61: the tab leaves its group on handoff; receiver gets it ungrouped.

Edits, all on the **source** side so the receiver has nothing extra to handle:

### 11.1 `TransferredTab` is unchanged

The struct at `app/src/workspace/view.rs:880-888` does **not** gain a `group_id` field. This is the contract: the payload is ungrouped by construction.

### 11.2 `Workspace::get_tab_transfer_info`

At `app/src/workspace/view.rs:4774`. Before constructing `TransferredTab`, capture the source group:

```rust
let source_group_id = self.tabs[tab_index].group_id;
self.tabs[tab_index].group_id = None;
if let Some(g) = source_group_id {
    self.prune_empty_group(g);
}
// ... existing TransferredTab construction, unchanged ...
```

This ensures (a) `TransferredTab` is built from a now-ungrouped tab, and (b) if removing the tab orphans the source group, dissolution happens before the source workspace re-renders.

### 11.3 `HandoffPendingTransfer` and `ReverseHandoff` action arms

In `app/src/workspace/action.rs:249-256`, the variants are unchanged (no new payload fields). The handler arms in `view.rs` (search for the variant names) gain a `debug_assert_eq!(transferred.group_id, None)` style guard near the `TransferredTab` consumption — but since `TransferredTab` has no `group_id` field, this is enforced by the type system. No runtime check needed.

### 11.4 `Workspace::adopt_transferred_pane_group`

At `app/src/workspace/view.rs:10205-10240`. The receiver inserts a tab built from `TransferredTab` with `group_id: None` (the default for `TabData::new`). No additional edits required.

### 11.5 Telemetry hint

If a tab leaves a group via handoff, the source side emits `TelemetryEvent::TabOperations { action: TabTelemetryAction::HandoffRemoveFromGroup }` (new variant in §13). This is distinct from a user-initiated remove-from-group so dashboards can separate handoff-driven dissolutions.

## 12. Telemetry

Following the four-touchpoint pattern (RESEARCH §7; the `MoveActiveTab` template lives at `events.rs:1466, 3082, 5218, 5721, 6321`).

### 12.1 New top-level event

A new `TelemetryEvent::DragAndDropTabGroup` parallel to `DragAndDropTab` (`events.rs:1473`):

| Touchpoint | Edit |
| --- | --- |
| Variant decl (`events.rs:1466-1476`) | Add `DragAndDropTabGroup,` next to `DragAndDropTab` |
| Payload arm (`events.rs:3081-3084`) | Add `TelemetryEvent::DragAndDropTabGroup => None,` (no payload fields) |
| Enablement arm (`events.rs:5217-5221`) | Add `Self::DragAndDropTabGroup => EnablementState::Flag(FeatureFlag::TabGroups),` |
| Name arm (`events.rs:5720-5724`) | Add `Self::DragAndDropTabGroup => "Drag and Drop Tab Group",` |
| Description arm (`events.rs:6320-6326`) | Add `Self::DragAndDropTabGroup => "Tab group reordered as a unit by chip drag",` |

Emitted at the `DropTabGroup` action arm in `view.rs` (next to the existing `DragAndDropTab` emission at `:20665`).

### 12.2 Extend `TabTelemetryAction`

PRODUCT §74 enumerates 10 telemetry-bearing operations. We funnel them through the existing `TelemetryEvent::TabOperations { action }` (`events.rs:1474`) by extending `TabTelemetryAction` (`tab.rs:106-112`) with new variants:

```rust
pub enum TabTelemetryAction {
    // ── existing ──
    CloseTab,
    CloseOtherTabs,
    CloseTabsToRight,
    SetColor,
    ResetColor,
    // ── new (gated by FeatureFlag::TabGroups via the action paths) ──
    CreateGroup,
    RenameGroup,
    RecolorGroup,
    CollapseGroup,
    ExpandGroup,
    AddTabToGroupByMenu,       // PRODUCT §31
    AddTabToGroupByDrag,       // PRODUCT §33
    RemoveTabFromGroupByMenu,  // PRODUCT §35
    RemoveTabFromGroupByDrag,  // PRODUCT §36
    HandoffRemoveFromGroup,    // §11.5 (handoff-driven, distinct from user-initiated)
    UngroupGroup,              // PRODUCT §30 Ungroup
    CloseGroup,                // PRODUCT §42
}
```

All new variants serialize as standard JSON enum strings; the existing payload arm (`events.rs:3084`) covers them — no change needed there.

The description arm (`events.rs:6324-6326`) is extended:

```rust
Self::TabOperations => {
    "Took operation on a tab or tab group: change color, close tab, \
     close adjacent tabs, create group, rename group, recolor group, \
     collapse / expand group, add to / remove from group, ungroup, close group, etc."
}
```

### 12.3 Emission sites

Each group-mutating method on `Workspace` ends with a `send_telemetry_from_ctx!(TelemetryEvent::TabOperations { action: TabTelemetryAction::<variant> }, ctx)` matching the operation. Drag-vs-menu is distinguished by which call path entered the method:

- `Workspace::move_tab_into_group` is called from both menu (`AddTabToTabGroup` action) and drop resolution (`resolve_drop`). Add a `source: TabGroupOperationSource` parameter (`Menu | Drag`) so the right variant is emitted.
- `Workspace::remove_tab_from_group` likewise gains a `source` parameter.

Disable-aware: when `FeatureFlag::TabGroups` is off, the action arms early-return (§8.2), so no group-related telemetry can fire.

## 13. Feature flag

### 13.1 Add `TabGroups`

Per `add-feature-flag` skill:

1. **`crates/warp_features/src/lib.rs`** — add `TabGroups,` variant near `TabConfigs` (`:795`) and document:

   ```rust
   /// Enables Chrome-like Tab Groups: window-local containers with name, color,
   /// collapse/expand, and contiguous member tabs. See specs/tab-groups/.
   TabGroups,
   ```

2. **`crates/warp_features/src/lib.rs:868-926`** — add `FeatureFlag::TabGroups,` to `DOGFOOD_FLAGS`. (Per the task brief: dogfood is the right initial state for a new UI feature.)

3. **`app/Cargo.toml`** — add `tab_groups = []` to `[features]`, **and** add `"tab_groups",` to the `default = [...]` features list. The default-list inclusion is required because the lib.rs runtime-registry entry is gated by `#[cfg(feature = "tab_groups")]` (step 4); without the feature in the default set, the variant never reaches the runtime registry, so even with `TabGroups` in `DOGFOOD_FLAGS` the flag stays off in dev. Precedent: `tab_configs` is in both `[features]` and `default`. Stable users still see the feature off because `DOGFOOD_FLAGS` only auto-enables in WarpDev; promotion to Stable goes through the standard rollout path.

4. **`app/src/lib.rs`** — add `#[cfg(feature = "tab_groups")] FeatureFlag::TabGroups,` to the corresponding flag-feature plumbing list (next to `FeatureFlag::TabConfigs`).

### 13.2 Runtime guard sites

Use `FeatureFlag::TabGroups.is_enabled()` rather than `#[cfg(feature = "tab_groups")]` at every guarded site (per skill best-practices). Sites:

- All new `WorkspaceAction` arms (§8.2): early-return when off.
- `TabData::add_tab_to_group_submenu` (§8.4): returns empty when off.
- The chip render path in `render_tab_bar_contents` (§9.1): when off, the new while-loop reduces to the existing for-loop because no tab has `group_id == Some(_)` in practice (the action paths don't run, so no group is ever created); a defensive `if !FeatureFlag::TabGroups.is_enabled() { /* ignore registry */ }` sets the loop into legacy mode for an extra safety margin.
- `vertical_tabs::render_groups`: same guard.
- `Workspace::create_tab_group_from_tab` (and all sibling group-mutating methods): early-return when off.
- Persistence read path (§6.5-6.6): always parses existing rows even when the flag is off, so users who toggle the flag mid-session don't lose their groups.

### 13.3 Persistence interaction

Migration runs unconditionally. Save/load also runs unconditionally — toggling the flag off must **not** drop persisted groups. UI guards (§13.2) mean the user simply can't see or operate on them while off. When toggled back on, groups reappear.

## 14. Tests

### 14.1 Unit tests — workspace state invariants

Home: `app/src/workspace/view_test.rs` (sync state-machine tests). New file `app/src/workspace/tab_group/tests.rs` for registry-only tests.

Each test asserts I1-I5 on exit via `Workspace::debug_assert_invariants` (§7.2).

| Test name | Asserts |
| --- | --- |
| `tab_group_registry_next_default_color_round_robin` | First 8 calls return palette in order; 9th repeats. |
| `tab_group_registry_skips_used_colors_when_possible` | With 3 groups using Blue/Red/Green, the 4th group does not pick those. |
| `create_tab_group_from_single_tab_assigns_id_and_color` | Group exists in registry; `tabs[i].group_id` matches; default color used. |
| `create_tab_group_enters_rename_immediately` | `is_any_group_renaming` true; `rename_tab_group` was dispatched. |
| `move_tab_into_group_appends_to_run_end` | Tab lands at `group_member_range(g).end - 1`. |
| `move_tab_into_group_from_other_group_dissolves_source_when_singleton` | Source group with one member dissolves on departure. |
| `remove_tab_from_group_keeps_position` | Tab's index unchanged; `group_id` cleared. |
| `remove_last_member_dissolves_group` | Registry no longer has the id. |
| `ungroup_dissolves_group_keeps_member_positions` | All members retain index, `group_id == None`. |
| `move_tab_left_clamps_to_group_run_when_outside` | Member of group A cannot be moved to interleave between two members of group B. |
| `move_tab_right_clamps_to_group_run_when_outside` | Same, opposite direction. |
| `reorder_tab_chokepoint_preserves_contiguity_for_all_groups` | After arbitrary reorder, every group's run is contiguous. |
| `chip_drag_moves_whole_run_together` | After `reorder_group`, run elements stay adjacent. |
| `chip_drag_cannot_split_other_group` | Dropping group A inside group B's run snaps to nearest boundary. |
| `set_active_tab_index_expands_collapsed_member_group` | I3 enforced inside the same call (no event lag). |
| `collapse_group_with_active_member_is_no_op` | Group remains expanded; collapse never "sticks" while active. |
| `close_group_closes_all_members_in_order` | Each member's `close_tab` was called; group dissolved. |
| `close_group_cancellation_aborts_all` | If close confirmation cancels mid-run, no member is closed and group remains. |
| `feature_flag_off_drops_action` | With `TabGroups.is_enabled() == false`, group actions are no-ops. |

### 14.2 Unit tests — persistence round-trip

Home: `app/src/persistence/sqlite_test.rs` (existing harness). All tests use an in-memory SQLite instance.

| Test name | Asserts |
| --- | --- |
| `persistence_round_trip_tab_groups_basic` | Save a workspace with 2 groups; reload; assert registry equals, `tabs.group_id` populated. |
| `persistence_round_trip_collapsed_state` | Save group with `collapsed=true`; reload; assert preserved (when active tab is not a member). |
| `persistence_round_trip_active_member_force_expands_on_load` | PRODUCT §57: collapsed group whose active tab is a member expands on reload. |
| `persistence_drops_orphan_group_at_load` | DB has a `tab_groups` row with no referencing tab; load filters it out. |
| `persistence_clears_dangling_group_uuid_on_tab` | DB has `tabs.group_uuid` pointing to a non-existent group; load sets `group_id=None`. |
| `persistence_uuid_join_survives_save_round_trip` | Save → mutate `tabs.id` (delete+reinsert via second save) → reload; group_id correctly relinked via UUID. |
| `persistence_handles_legacy_db_without_tab_groups_table` | Migration not yet run (or down-migrated): save_app_state must not panic. (Covered by the migration test harness.) |
| `migration_up_then_down_is_clean` | Run `up.sql` then `down.sql`; schema returns to pre-migration state. |

### 14.3 Integration tests

Home: `app/src/integration_testing/tab/{step,assertion}.rs` and a new `app/src/integration_testing/tab_group/{mod,step,assertion}.rs`. Use the `Builder`/`TestStep` framework per `warp-integration-test` skill.

New step helpers in `tab_group/step.rs`:

```rust
pub fn create_tab_group_from_active_tab() -> TestStep;
pub fn add_tab_to_group(tab_index: usize, group_index: usize) -> TestStep;
pub fn remove_tab_from_group(tab_index: usize) -> TestStep;
pub fn collapse_tab_group(group_index: usize) -> TestStep;
pub fn expand_tab_group(group_index: usize) -> TestStep;
pub fn rename_tab_group(group_index: usize, new_name: String) -> TestStep;
pub fn recolor_tab_group(group_index: usize, color: TabGroupColor) -> TestStep;
pub fn ungroup_tab_group(group_index: usize) -> TestStep;
pub fn close_tab_group(group_index: usize) -> TestStep;
pub fn drag_tab_onto_chip(tab_index: usize, group_index: usize) -> TestStep;
pub fn drag_chip_to_position(group_index: usize, target_first_index: usize) -> TestStep;
```

Assertion helpers in `tab_group/assertion.rs`:

```rust
pub fn assert_tab_in_group(tab_index: usize, group_index: usize) -> AssertionCallback;
pub fn assert_tab_ungrouped(tab_index: usize) -> AssertionCallback;
pub fn assert_tab_group_count(expected: usize) -> AssertionCallback;
pub fn assert_tab_group_name(group_index: usize, expected: String) -> AssertionCallback;
pub fn assert_tab_group_collapsed(group_index: usize, expected: bool) -> AssertionCallback;
pub fn assert_tab_group_member_count(group_index: usize, expected: usize) -> AssertionCallback;
pub fn assert_tab_group_color(group_index: usize, expected: TabGroupColor) -> AssertionCallback;
pub fn assert_tab_group_run(group_index: usize, expected_indices: Vec<usize>) -> AssertionCallback;
```

Group-index is order-of-first-member to keep tests deterministic.

End-to-end integration tests, listed by name with what they assert:

| Test name | Scenario |
| --- | --- |
| `tab_groups_create_via_right_click_menu` | Right-click tab 0 → "Add tab to group" → "New group" → group registry has 1 entry, tab 0 is its sole member, rename editor open. |
| `tab_groups_add_tab_via_right_click_menu` | Two groups exist; right-click tab in group A, "Add tab to group" → group B; tab moves to end of group B's run. |
| `tab_groups_remove_tab_via_right_click_menu` | Tab in group: right-click, "Remove from group"; tab keeps position, group_id cleared. |
| `tab_groups_remove_dissolves_singleton` | Group of one: remove its only member; group dissolves. |
| `tab_groups_collapse_hides_members_horizontal` | Collapse group; member tabs not rendered in horizontal bar; chip remains. |
| `tab_groups_collapse_hides_members_vertical` | Same in vertical-tabs panel. |
| `tab_groups_active_tab_in_collapsed_group_force_expands` | Manually mutate state to collapsed-with-active-member, dispatch any render → group is expanded after the tick. |
| `tab_groups_chip_left_click_toggles` | Click chip → collapsed; click again → expanded. |
| `tab_groups_section_chevron_toggles` | Vertical: click chevron → collapsed; click again → expanded. |
| `tab_groups_drag_tab_into_group` | Drag tab T onto chip → T joins the group at run-end. |
| `tab_groups_drag_member_out_of_group` | Drag member M to before-chip → M leaves group; positioned just before group. |
| `tab_groups_drag_chip_reorders_run` | Drag chip from position 3 to position 0 → entire run moves to the front; active-tab tracks. |
| `tab_groups_recolor_persists_visually` | Recolor → chip and member band update. |
| `tab_groups_rename_inline_commit_on_enter` | Rename editor: type "Deploy", press Enter; chip text == "Deploy". |
| `tab_groups_rename_cancel_on_escape` | Type, press Escape; chip text reverts. |
| `tab_groups_rename_empty_falls_back_to_color_name` | Commit empty string; chip displays the color name at reduced opacity. |
| `tab_groups_close_group_closes_all_members_undoable_individually` | Close group of 3 → 3 separate items in `UndoCloseStack`; first undo restores last-closed member. |
| `tab_groups_close_group_cancel_aborts_all` | Close group; one member triggers confirm; cancel; all 3 members remain, group remains. |
| `tab_groups_handoff_strips_group` | Group of 2 in window A; drag one tab to window B; window B receives ungrouped tab; window A retains the group with one member. |
| `tab_groups_handoff_strips_singleton_dissolves` | Singleton group: handoff dissolves the source group. |
| `tab_groups_persistence_round_trip` | Create groups, save app state, restart, assert registry + memberships restored. |
| `tab_groups_persistence_collapsed_with_active_member_expands_on_restore` | Collapsed-with-active-member persists, but reloads expanded (PRODUCT §57). |
| `tab_groups_search_in_vertical_tabs_temporarily_expands` | Stored-collapsed group with matching member: while query active, group renders expanded; clearing query restores collapsed. |
| `tab_groups_feature_flag_off_no_chip_no_section` | With `TabGroups` flag off, no chip / section header rendered even if registry has entries (e.g. data left over from prior session with flag on). |
| `tab_groups_contiguity_invariant_after_arbitrary_reorder` | A randomized fuzz-style integration test that performs 50 random reorder/add/remove operations and asserts I1 still holds. |

### 14.4 Manual verification checklist

For each numbered PRODUCT invariant 1-72, name the test or the manual step that covers it. Maintained as a table in the PR description for review.

## 15. Migration / rollout notes

### 15.1 Dogfood

`TabGroups` is added to `DOGFOOD_FLAGS` (§13.1) so internal builds get the feature on by default. Stable users see no chip and no section header until promotion.

### 15.2 What to verify in dogfood

- Create / rename / recolor / collapse / close-group flows from horizontal bar.
- Same flows from vertical-tabs panel (`TabSettings::use_vertical_tabs` enabled).
- App restart preserves: registry, memberships, names, colors, collapsed state.
- App restart with active tab in a persisted-collapsed group: group expands on restore.
- Drag tab → window B handoff: tab leaves group; source group dissolves if singleton.
- 50+ tabs with 5+ groups: no rendering jank, no contiguity invariant violations.
- Telemetry: events arrive with correct variant labels in the events dashboard.
- Toggle `FeatureFlag::TabGroups` off mid-session: chip/section disappear, persisted data survives, toggle back on restores UI.
- Existing tab tests (`view_test.rs`, `action_tests.rs`) and `cargo nextest run -p app` pass unchanged.
- WASM build per repo convention.

### 15.3 Promotion to Preview / Stable

Out of scope for this spec; promote via the standard `promote-feature` workflow once dogfood feedback is positive.

## 16. Open questions for implementers

None — the resolutions in the task brief plus the §1-§15 specifications are sufficient to implement. If the implementer encounters a genuinely ambiguous case, file a follow-up against this spec rather than committing a silent decision.

---

## Appendix A — Color palette mapping table

PRODUCT §6 lists 8 colors in this order: Grey, Blue, Red, Yellow, Green, Pink, Purple, Cyan. Hex values are Chrome-tab-group-inspired and **flagged for designer review** before Stable promotion — they should land in the warp design-system theme files, not as literals. Where an existing semantic accessor on `WarpTheme` (`crates/warp_core/src/ui/theme/color.rs:138-156`) approximates the color, it is listed; otherwise the literal is the v1 source of truth.

| Palette name | Proposed hex | Existing semantic mapping (preferred when available) | Notes |
| --- | --- | --- | --- |
| Grey | `#5F6368` | `theme.outline()` (`color.rs:154`) ≈ neutral grey | Used for "no color" / muted group default. |
| Blue | `#1A73E8` | `theme.accent()` (`color.rs:92`) varies per theme | Use literal — `accent` is theme-keyed and would couple group color to user theme. |
| Red | `#D93025` | `theme.ui_error_color()` (`color.rs:142`) | Same hue family; semantic accessor preferred so dark/light themes both work. |
| Yellow | `#F9AB00` | `theme.ui_yellow_color()` (`color.rs:146`) | Semantic accessor preferred. |
| Green | `#188038` | `theme.ui_green_color()` (`color.rs:150`) | Semantic accessor preferred. |
| Pink | `#D01884` | _none_ | Literal hex — no Warp semantic color in this hue. **Designer review.** |
| Purple | `#A142F4` | _none_ | Literal hex — no Warp semantic color in this hue. **Designer review.** |
| Cyan | `#007B83` | `AnsiColorIdentifier::Cyan` via `theme.terminal_colors().normal.cyan` (`tab.rs:632-634` precedent) | Terminal-color accessor preferred so theme tone-matching applies. |

`TabGroupColor::to_fill(theme)` implements this table: tries the semantic accessor first, falls back to the literal hex when the theme accessor is `None` or unsuitable.

## Appendix B — Files added / modified, by section

| Section | New files | Modified files |
| --- | --- | --- |
| §4-§5 Data model | `app/src/workspace/tab_group/mod.rs` | `app/src/tab.rs` (add `group_id`), `app/src/workspace/view.rs` (declare `mod tab_group`) |
| §6 Persistence | `crates/persistence/migrations/2026-05-01-000000_add_tab_groups/{up,down}.sql` | `crates/persistence/src/{schema.rs,model.rs}`, `app/src/persistence/sqlite.rs`, `app/src/app_state.rs` |
| §7 State | — | `app/src/workspace/view.rs` |
| §8 Actions | — | `app/src/workspace/action.rs`, `app/src/workspace/mod.rs`, `app/src/tab.rs` (menu items) |
| §9 Horizontal render | `app/src/workspace/tab_group/chip.rs` | `app/src/workspace/view.rs`, `app/src/tab.rs` |
| §10 Vertical render | `app/src/workspace/tab_group/vertical_section.rs` | `app/src/workspace/view/vertical_tabs.rs`, `app/src/workspace/mod.rs` (`TabBarLocation` variants), `app/src/pane_group/mod.rs` (`TabBarHoverIndex` variant) |
| §11 Handoff | — | `app/src/workspace/view.rs` |
| §12 Telemetry | — | `app/src/server/telemetry/events.rs`, `app/src/tab.rs` |
| §13 Feature flag | — | `crates/warp_features/src/lib.rs`, `app/Cargo.toml`, `app/src/lib.rs` |
| §14 Tests | `app/src/workspace/tab_group/tests.rs`, `app/src/integration_testing/tab_group/{mod,step,assertion}.rs` | `app/src/workspace/view_test.rs`, `app/src/persistence/sqlite_test.rs`, `app/src/integration_testing/mod.rs` (re-export) |
