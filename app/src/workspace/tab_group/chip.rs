//! Tab Group chip element for the horizontal tab bar.
//!
//! The chip is the leading element of a group's run in the horizontal tab
//! bar. It shows the group's color as a fill, its name (or color name
//! fallback) as text, and — when collapsed — a count of hidden member
//! tabs. Left-click toggles collapse, right-click opens the group menu,
//! double-click enters inline rename. Drag is whole-group reorder. Drop
//! targeting the chip adds the dragged tab to the group at the run's end.
//!
//! See PRODUCT §18-20, §29, §33, §40, §69 and TECH.md §9.2.

use pathfinder_color::ColorU;
use warpui::elements::{
    Border, ConstrainedBox, Container, CornerRadius, CrossAxisAlignment, DragAxis, Draggable,
    DropTarget, Element, Empty, Fill, Flex, Hoverable, MainAxisSize, Padding, ParentElement,
    Radius, Text,
};
use warpui::ui_components::components::{Coords, UiComponent, UiComponentStyles};
use warpui::ui_components::text_input::TextInput;
use warpui::{AppContext, ViewHandle};

use crate::appearance::Appearance;
use crate::editor::EditorView;
use crate::features::FeatureFlag;
use crate::ui_components::icons::Icon;
use crate::workspace::action::WorkspaceAction;
use crate::workspace::tab_group::TabGroup;
use crate::workspace::{TabBarDropTargetData, TabBarLocation};

/// Chip rounded-rect corner radius (px).
const CHIP_CORNER_RADIUS: f32 = 6.0;
/// Diameter of the small leading dot in the chip (px).
const CHIP_LEADING_DOT_DIAMETER: f32 = 6.0;
/// Gap between the leading dot and the name label (px).
const CHIP_DOT_TEXT_GAP: f32 = 4.0;
/// Inner horizontal padding inside the chip (px).
const CHIP_HORIZONTAL_PADDING: f32 = 8.0;
/// Inner vertical padding inside the chip (px). Matches the vertical
/// padding on `TabComponent` (`tab.rs:1419`) so the chip aligns with
/// neighboring tabs in the tab bar.
const CHIP_VERTICAL_PADDING: f32 = 2.0;
/// Minimum chip width before the label is allowed to truncate (px).
const CHIP_MIN_WIDTH: f32 = 60.0;
/// Maximum chip width before the label truncates (px).
const CHIP_MAX_WIDTH: f32 = 140.0;
/// Alpha applied to the label color when it is the color-name fallback,
/// approximating "reduced contrast" per PRODUCT §18.
const CHIP_FALLBACK_NAME_ALPHA: u8 = 153; // ~60%

/// Renders the chip for `group`. `member_count` is the number of tabs
/// currently in the group; passed in (rather than re-derived from a
/// `Workspace` reference) so the chip stays a small, self-contained
/// element. `is_active_member` is `true` when the workspace's active tab
/// belongs to this group; this draws a defensive accent ring per
/// TECH.md §9.4 (steady-state I3 prevents the underlying state combo
/// from happening, but the cue avoids a "where did my tab go?" moment
/// during state restoration).
pub fn render_tab_group_chip(
    group: &TabGroup,
    member_count: usize,
    is_active_member: bool,
    is_being_renamed: bool,
    rename_editor: ViewHandle<EditorView>,
    appearance: &Appearance,
    _ctx: &AppContext,
) -> Box<dyn Element> {
    let theme = appearance.theme();
    let group_id = group.id;
    let is_collapsed = group.collapsed;
    let chip_fill = group.color.to_fill(theme);
    let chip_color_u = group.color.to_color_u(theme);

    // Label: prefer the user-supplied name (PRODUCT §12). When empty,
    // fall back to the color name at reduced contrast (PRODUCT §18) so
    // the chip is still visible and right-clickable. When the group is
    // collapsed, append the member count (PRODUCT §19, e.g. "Deploy · 4").
    let (label_base, is_fallback_label) = if group.name.is_empty() {
        (group.color.display_name().to_string(), true)
    } else {
        (group.name.clone(), false)
    };
    let label_text = if is_collapsed {
        format!("{label_base} · {member_count}")
    } else {
        label_base
    };

    // Text color: pick the foreground that contrasts with the chip fill,
    // dimmed for the fallback color-name case.
    let mut text_color: ColorU = theme.font_color(chip_color_u).into();
    if is_fallback_label {
        text_color.a = CHIP_FALLBACK_NAME_ALPHA;
    }

    // Inner row: optional leading dot + label.
    let mut row = Flex::row()
        .with_main_axis_size(MainAxisSize::Min)
        .with_cross_axis_alignment(CrossAxisAlignment::Center);

    // Leading dot — omitted in the fallback case to avoid stutter (the
    // color-name label already conveys the color).
    if !is_fallback_label {
        let dot_color = theme.font_color(chip_color_u);
        let dot = ConstrainedBox::new(Icon::Ellipse.to_warpui_icon(dot_color).finish())
            .with_width(CHIP_LEADING_DOT_DIAMETER)
            .with_height(CHIP_LEADING_DOT_DIAMETER)
            .finish();
        row.add_child(dot);
        // Spacer between dot and text.
        row.add_child(
            ConstrainedBox::new(Empty::new().finish())
                .with_width(CHIP_DOT_TEXT_GAP)
                .finish(),
        );
    }

    if is_being_renamed {
        // PRODUCT §13-15: chip text becomes an inline editor while
        // rename mode is active. The editor is the workspace's
        // `tab_group_rename_editor`; commit / cancel handlers are wired
        // there. We strip the surrounding chrome (background, border,
        // padding) on the TextInput to keep it visually flush with
        // the chip's existing background.
        row.add_child(
            ConstrainedBox::new(
                TextInput::new(
                    rename_editor,
                    UiComponentStyles::default()
                        .set_background(Fill::None)
                        .set_border_radius(CornerRadius::with_all(Radius::Pixels(0.)))
                        .set_border_width(0.)
                        .set_font_color(text_color),
                )
                .with_style(UiComponentStyles {
                    margin: Some(Coords::default()),
                    ..Default::default()
                })
                .build()
                .finish(),
            )
            .with_min_width(40.)
            .finish(),
        );
    } else {
        row.add_child(
            Text::new_inline(
                label_text,
                appearance.ui_font_family(),
                appearance.ui_font_size(),
            )
            .with_color(text_color)
            .finish(),
        );
    }

    // Chip container — colored rounded rect.
    let mut chip_container = Container::new(row.finish())
        .with_background(chip_fill)
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(CHIP_CORNER_RADIUS)))
        .with_padding(
            Padding::default()
                .with_horizontal(CHIP_HORIZONTAL_PADDING)
                .with_vertical(CHIP_VERTICAL_PADDING),
        );

    // Defensive accent ring when the active tab is a member of a
    // collapsed group. I3 normally makes this combination unreachable.
    if is_active_member && is_collapsed {
        chip_container =
            chip_container.with_border(Border::all(1.0).with_border_fill(theme.accent()));
    }

    let chip_element: Box<dyn Element> = chip_container.finish();

    // Width constraint — chip stays in a reasonable range so long names
    // don't blow up the tab bar.
    let constrained = ConstrainedBox::new(chip_element)
        .with_min_width(CHIP_MIN_WIDTH)
        .with_max_width(CHIP_MAX_WIDTH)
        .finish();

    // Mouse routing.
    //  - Left mouse-down toggles collapse (PRODUCT §20).
    //  - Right click opens the group menu (PRODUCT §69).
    //  - Double-click enters inline rename (PRODUCT §13).
    // While the chip is in rename mode, the click handlers are
    // suppressed so the editor can take focus / receive clicks
    // without firing collapse.
    let mut chip_with_handlers =
        Hoverable::new(group.hover_state.clone(), move |_state| constrained);
    if !is_being_renamed {
        chip_with_handlers = chip_with_handlers
            .on_mouse_down(move |ctx, _, _| {
                ctx.dispatch_typed_action(WorkspaceAction::ToggleTabGroupCollapsed { group_id });
            })
            .on_double_click(move |ctx, _, _| {
                ctx.dispatch_typed_action(WorkspaceAction::RenameTabGroup { group_id });
            });
    }
    chip_with_handlers = chip_with_handlers.on_right_click(move |ctx, _, position| {
        ctx.dispatch_typed_action(WorkspaceAction::ToggleTabGroupContextMenu {
            group_id,
            position,
        });
    });

    // Whole-group drag (PRODUCT §40). v1 chip cross-window drag is out of
    // scope (PRODUCT §41) — we mirror the per-tab Draggable's
    // `DragTabsToWindows` gate (`tab.rs:1668`) so the same flag flips both.
    let chip_with_drag = {
        let drag = Draggable::new(group.draggable_state.clone(), chip_with_handlers.finish())
            .on_drag_start(move |ctx, _, _| {
                ctx.dispatch_typed_action(WorkspaceAction::StartTabGroupDrag { group_id });
            })
            .on_drag(move |ctx, _, rect, _| {
                ctx.dispatch_typed_action(WorkspaceAction::DragTabGroup {
                    group_id,
                    position: rect,
                });
            })
            .on_drop(move |ctx, _, _, _| {
                ctx.dispatch_typed_action(WorkspaceAction::DropTabGroup { group_id });
            });
        if FeatureFlag::DragTabsToWindows.is_enabled() {
            drag
        } else {
            drag.with_drag_axis(DragAxis::HorizontalOnly)
        }
    };

    // Drop target on the chip itself — adds the dragged tab to this
    // group at the run's end (PRODUCT §33).
    DropTarget::new(
        chip_with_drag.finish(),
        TabBarDropTargetData {
            tab_bar_location: TabBarLocation::OnGroupChip(group_id),
        },
    )
    .finish()
}
