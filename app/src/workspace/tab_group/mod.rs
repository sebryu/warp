//! Tab Groups (Chrome-like). Distinct from:
//! - `vertical_tabs::render_tab_group` and `TabGroupColorMode` — the per-tab row in the
//!   vertical-tabs panel (see app/src/workspace/view/vertical_tabs.rs).
//! - `code::view::CodeView::tab_group` — the file-tab bar inside a code-editor pane
//!   (see app/src/code/view.rs).
//! Grep `crate::workspace::tab_group::` for hits in this feature.
//!
//! See `specs/tab-groups/PRODUCT.md` and `specs/tab-groups/TECH.md`.

use std::collections::HashMap;

use enum_iterator::{all, Sequence};
use pathfinder_color::ColorU;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use warp_core::ui::theme::WarpTheme;
use warpui::elements::{DraggableState, Fill, MouseStateHandle};

pub mod chip;
pub mod vertical_section;

/// Distinguishes how a tab-group operation was initiated, so telemetry can
/// separate menu-driven vs. drag-driven actions (TECH.md §12.3).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum TabGroupOperationSource {
    Menu,
    Drag,
    /// Source-side effect of a cross-window handoff (TECH.md §11.5). Distinct
    /// so dashboards can isolate handoff-driven dissolutions.
    Handoff,
}

/// Stable per-group identifier. Minted at group creation and round-tripped through
/// persistence. Distinct from any DB integer PK because `tabs.id` is regenerated on
/// every save (see app/src/persistence/sqlite.rs save_app_state).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TabGroupId(pub Uuid);

impl TabGroupId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for TabGroupId {
    fn default() -> Self {
        Self::new()
    }
}

/// Fixed 8-color palette for tab groups. Order matches PRODUCT §6
/// (Grey, Blue, Red, Yellow, Green, Pink, Purple, Cyan).
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
    /// User-facing name of the palette color.
    pub fn display_name(self) -> &'static str {
        match self {
            TabGroupColor::Grey => "Grey",
            TabGroupColor::Blue => "Blue",
            TabGroupColor::Red => "Red",
            TabGroupColor::Yellow => "Yellow",
            TabGroupColor::Green => "Green",
            TabGroupColor::Pink => "Pink",
            TabGroupColor::Purple => "Purple",
            TabGroupColor::Cyan => "Cyan",
        }
    }

    /// Default color for a brand-new group when no palette colors are in use.
    pub fn default_palette_first() -> Self {
        TabGroupColor::Grey
    }

    /// All palette entries in PRODUCT §6 order.
    pub fn all_in_order() -> Vec<Self> {
        all::<Self>().collect()
    }

    /// Resolves the palette entry to a concrete `Fill` for chip / section
    /// header / member-tab band rendering. The mapping follows TECH.md
    /// Appendix A: prefer existing semantic theme accessors where they fit
    /// (so dark/light themes both work), fall back to a Chrome-tab-group-
    /// inspired hex literal for hues with no Warp semantic equivalent.
    ///
    /// Hex literals here are flagged for designer review before Stable
    /// promotion (TECH.md §15.1, §16). When designers land semantic
    /// accessors in `crates/warp_core/src/ui/theme/color.rs`, swap the
    /// arms below.
    pub fn to_fill(self, theme: &WarpTheme) -> Fill {
        match self {
            // Semantic accessors — theme-keyed so dark/light tone-match.
            TabGroupColor::Red => Fill::Solid(theme.ui_error_color()),
            TabGroupColor::Yellow => Fill::Solid(theme.ui_yellow_color()),
            TabGroupColor::Green => Fill::Solid(theme.ui_green_color()),
            // Hex literals — Chrome-tab-group palette. See Appendix A.
            TabGroupColor::Grey => Fill::Solid(ColorU::new(0x5F, 0x63, 0x68, 0xFF)),
            TabGroupColor::Blue => Fill::Solid(ColorU::new(0x1A, 0x73, 0xE8, 0xFF)),
            TabGroupColor::Pink => Fill::Solid(ColorU::new(0xD0, 0x18, 0x84, 0xFF)),
            TabGroupColor::Purple => Fill::Solid(ColorU::new(0xA1, 0x42, 0xF4, 0xFF)),
            TabGroupColor::Cyan => Fill::Solid(ColorU::new(0x00, 0x7B, 0x83, 0xFF)),
        }
    }

    /// The solid `ColorU` underlying `to_fill`. Useful where downstream
    /// code wants a `ColorU` (e.g. `WarpTheme::font_color`) without
    /// re-extracting from the `Fill::Solid(_)` arm.
    pub fn to_color_u(self, theme: &WarpTheme) -> ColorU {
        match self.to_fill(theme) {
            Fill::Solid(c) => c,
            // The mapping above is exhaustively `Fill::Solid(_)`; this arm
            // is unreachable in practice but kept defensive.
            _ => ColorU::new(0x5F, 0x63, 0x68, 0xFF),
        }
    }
}

/// In-memory representation of a tab group. The list of member tabs is **not**
/// stored here — membership is stored on `TabData::group_id` and member positions
/// are derived from `Workspace::tabs`. Contiguity is enforced at the workspace
/// layer (see TECH.md §7).
#[derive(Clone)]
pub struct TabGroup {
    pub id: TabGroupId,
    /// User-editable name. May be empty (PRODUCT §12, §14); when empty the
    /// chip / section header falls back to the color name at reduced contrast.
    pub name: String,
    pub color: TabGroupColor,
    pub collapsed: bool,
    /// Drag state for the chip / section header. Preserved across renders so
    /// drag interactions don't reset on every frame.
    pub draggable_state: DraggableState,
    /// Hover state for the chip / section header.
    pub hover_state: MouseStateHandle,
}

impl std::fmt::Debug for TabGroup {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TabGroup")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("color", &self.color)
            .field("collapsed", &self.collapsed)
            .finish()
    }
}

impl PartialEq for TabGroup {
    fn eq(&self, other: &Self) -> bool {
        // Drag/hover state is render-only; equality is based on persisted fields.
        self.id == other.id
            && self.name == other.name
            && self.color == other.color
            && self.collapsed == other.collapsed
    }
}

impl TabGroup {
    pub fn new(name: String, color: TabGroupColor) -> Self {
        Self {
            id: TabGroupId::new(),
            name,
            color,
            collapsed: false,
            draggable_state: DraggableState::default(),
            hover_state: MouseStateHandle::default(),
        }
    }
}

/// Owned by `Workspace`. Tracks all groups in this window. Round-trip persisted
/// via `WindowSnapshot::tab_groups: Vec<TabGroupSnapshot>` (TECH.md §6).
#[derive(Default, Clone)]
pub struct TabGroupRegistry {
    /// Insertion-stable map. Order is irrelevant for serialization; group order
    /// in the tab bar derives from the position of the group's first member tab.
    groups: HashMap<TabGroupId, TabGroup>,
}

impl std::fmt::Debug for TabGroupRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TabGroupRegistry")
            .field("len", &self.groups.len())
            .finish()
    }
}

impl TabGroupRegistry {
    pub fn get(&self, id: TabGroupId) -> Option<&TabGroup> {
        self.groups.get(&id)
    }

    pub fn get_mut(&mut self, id: TabGroupId) -> Option<&mut TabGroup> {
        self.groups.get_mut(&id)
    }

    pub fn insert(&mut self, group: TabGroup) -> TabGroupId {
        let id = group.id;
        self.groups.insert(id, group);
        id
    }

    pub fn remove(&mut self, id: TabGroupId) -> Option<TabGroup> {
        self.groups.remove(&id)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&TabGroupId, &TabGroup)> {
        self.groups.iter()
    }

    pub fn len(&self) -> usize {
        self.groups.len()
    }

    pub fn is_empty(&self) -> bool {
        self.groups.is_empty()
    }

    pub fn contains(&self, id: TabGroupId) -> bool {
        self.groups.contains_key(&id)
    }

    /// PRODUCT §8: round-robin pick of the next default color, skipping any
    /// color already used by an existing group when possible. After all 8 are
    /// in use, reuse is allowed (returns the next palette entry in order).
    pub fn next_default_color(&self) -> TabGroupColor {
        let used: std::collections::HashSet<TabGroupColor> =
            self.groups.values().map(|g| g.color).collect();
        let palette = TabGroupColor::all_in_order();
        for c in &palette {
            if !used.contains(c) {
                return *c;
            }
        }
        // All 8 used: reuse, picking the palette entry whose count is lowest;
        // fall back to palette order.
        let mut counts: HashMap<TabGroupColor, usize> = HashMap::new();
        for g in self.groups.values() {
            *counts.entry(g.color).or_insert(0) += 1;
        }
        palette
            .into_iter()
            .min_by_key(|c| counts.get(c).copied().unwrap_or(0))
            .unwrap_or(TabGroupColor::Grey)
    }
}

#[cfg(test)]
mod tests;
