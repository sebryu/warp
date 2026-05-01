//! Tab Group section header for the vertical tabs panel.
//!
//! When `render_groups` encounters a tab whose `group_id` is set we emit a
//! section: a header row (chevron + color swatch + name + count) followed
//! by the contiguous run of member tab rows. Member rows are indented and
//! decorated with a left stripe in the group's color (PRODUCT §22).
//! Collapsed groups (and groups whose collapsed state is overridden by an
//! active search query — PRODUCT §63-64) are rendered without members.
//!
//! See TECH.md §10.2.

use pathfinder_color::ColorU;
use warpui::elements::{
    ConstrainedBox, Container, CornerRadius, CrossAxisAlignment, DragAxis, Draggable, DropTarget,
    Element, Empty, Fill, Flex, Hoverable, MainAxisSize, Padding, ParentElement, Radius, Text,
};
use warpui::ui_components::components::{Coords, UiComponent, UiComponentStyles};
use warpui::ui_components::text_input::TextInput;
use warpui::{AppContext, ViewHandle};

use crate::appearance::Appearance;
use crate::editor::EditorView;
use crate::features::FeatureFlag;
use crate::pane_group::TabBarHoverIndex;
use crate::ui_components::icons::Icon;
use crate::workspace::action::WorkspaceAction;
use crate::workspace::tab_group::TabGroup;
use crate::workspace::{TabBarLocation, VerticalTabsPaneDropTargetData};

/// Diameter of the leading color swatch in the section header (px).
const SECTION_SWATCH_DIAMETER: f32 = 8.0;
/// Width of the leading color stripe on member rows (px).
const SECTION_MEMBER_STRIPE_WIDTH: f32 = 3.0;
/// Indent applied to member rows so the stripe + offset visually convey
/// "this row belongs to the group above" (px).
const SECTION_MEMBER_INDENT: f32 = 12.0;
/// Inner horizontal padding of the section header (px).
const SECTION_HEADER_HORIZONTAL_PADDING: f32 = 8.0;
/// Inner vertical padding of the section header (px). Approximates a
/// 28px-tall row given the surrounding text size.
const SECTION_HEADER_VERTICAL_PADDING: f32 = 6.0;
/// Alpha applied to the name label when it is the color-name fallback.
const SECTION_FALLBACK_NAME_ALPHA: u8 = 153; // ~60%

/// Renders the section header for a Tab Group in the vertical tabs panel.
///
/// `member_count` is passed by the caller (it derives membership from
/// `Workspace::group_member_range`). `effective_collapsed` is the value
/// the renderer should use — caller MAY override `group.collapsed` to
/// `false` while a search query is active (PRODUCT §64) so the stored
/// collapse state is preserved on the group itself.
pub fn render_section_header(
    group: &TabGroup,
    member_count: usize,
    effective_collapsed: bool,
    is_being_renamed: bool,
    rename_editor: ViewHandle<EditorView>,
    appearance: &Appearance,
    _ctx: &AppContext,
) -> Box<dyn Element> {
    let theme = appearance.theme();
    let group_id = group.id;
    let swatch_color: ColorU = group.color.to_color_u(theme);

    // Label fallback to color name (PRODUCT §22 mirrors PRODUCT §18).
    let (label_text, is_fallback_label) = if group.name.is_empty() {
        (group.color.display_name().to_string(), true)
    } else {
        (group.name.clone(), false)
    };
    let mut text_color: ColorU = theme.foreground().into();
    if is_fallback_label {
        text_color.a = SECTION_FALLBACK_NAME_ALPHA;
    }
    let count_text = format!("({member_count})");
    let mut count_color: ColorU = theme.sub_text_color(theme.background()).into();
    count_color.a = 200;

    // Chevron — the existing stack uses Icon::ChevronDown / ChevronRight
    // for collapsible sections; reuse them so the disclosure affordance
    // matches other vertical-tabs UI.
    let chevron_icon = if effective_collapsed {
        Icon::ChevronRight
    } else {
        Icon::ChevronDown
    };
    let chevron_color = theme.font_color(theme.background().into_solid());
    let chevron = ConstrainedBox::new(chevron_icon.to_warpui_icon(chevron_color).finish())
        .with_width(12.)
        .with_height(12.)
        .finish();

    // Color swatch (small filled circle).
    let swatch = ConstrainedBox::new(Icon::Ellipse.to_warpui_icon(swatch_color.into()).finish())
        .with_width(SECTION_SWATCH_DIAMETER)
        .with_height(SECTION_SWATCH_DIAMETER)
        .finish();

    let mut row = Flex::row()
        .with_main_axis_size(MainAxisSize::Max)
        .with_cross_axis_alignment(CrossAxisAlignment::Center);
    row.add_child(chevron);
    row.add_child(
        ConstrainedBox::new(Empty::new().finish())
            .with_width(6.)
            .finish(),
    );
    row.add_child(swatch);
    row.add_child(
        ConstrainedBox::new(Empty::new().finish())
            .with_width(6.)
            .finish(),
    );
    if is_being_renamed {
        // PRODUCT §13-15: section header label becomes an inline editor
        // while rename mode is active.
        row.add_child(
            warpui::elements::Shrinkable::new(
                1.,
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
    // Spacer to push the count label to the right.
    row.add_child(warpui::elements::Shrinkable::new(1., Empty::new().finish()).finish());
    row.add_child(
        Text::new_inline(
            count_text,
            appearance.ui_font_family(),
            appearance.ui_font_size(),
        )
        .with_color(count_color)
        .finish(),
    );

    let header_container = Container::new(row.finish())
        .with_padding(
            Padding::default()
                .with_horizontal(SECTION_HEADER_HORIZONTAL_PADDING)
                .with_vertical(SECTION_HEADER_VERTICAL_PADDING),
        )
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.)))
        .finish();

    // While renaming, suppress the click handlers that would steal
    // focus from the editor.
    let mut header_with_handlers =
        Hoverable::new(group.hover_state.clone(), move |_state| header_container);
    if !is_being_renamed {
        header_with_handlers = header_with_handlers
            .on_mouse_down(move |ctx, _, _| {
                // Section header click — same toggle as the chevron
                // (PRODUCT §22, §29).
                ctx.dispatch_typed_action(WorkspaceAction::ToggleTabGroupCollapsed { group_id });
            })
            .on_double_click(move |ctx, _, _| {
                ctx.dispatch_typed_action(WorkspaceAction::RenameTabGroup { group_id });
            });
    }
    header_with_handlers = header_with_handlers.on_right_click(move |ctx, _, position| {
        ctx.dispatch_typed_action(WorkspaceAction::ToggleTabGroupContextMenu {
            group_id,
            position,
        });
    });

    // Whole-section drag — vertical only (PRODUCT §40, §41).
    let header_with_drag = {
        let drag = Draggable::new(group.draggable_state.clone(), header_with_handlers.finish())
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
            drag.with_drag_axis(DragAxis::VerticalOnly)
        }
    };

    DropTarget::new(
        header_with_drag.finish(),
        VerticalTabsPaneDropTargetData {
            tab_bar_location: TabBarLocation::OnGroupChip(group_id),
            tab_hover_index: TabBarHoverIndex::OverGroupChip(group_id),
        },
    )
    .finish()
}

/// Wraps a member tab row with the indent and the leading colored stripe
/// (PRODUCT §22). Caller passes the already-rendered member row.
pub fn wrap_member_row(
    row: Box<dyn Element>,
    group: &TabGroup,
    appearance: &Appearance,
) -> Box<dyn Element> {
    let stripe_fill = group.color.to_fill(appearance.theme());

    let stripe = ConstrainedBox::new(
        Container::new(Empty::new().finish())
            .with_background(stripe_fill)
            .finish(),
    )
    .with_width(SECTION_MEMBER_STRIPE_WIDTH)
    .finish();

    let mut row_with_stripe = Flex::row()
        .with_main_axis_size(MainAxisSize::Max)
        .with_cross_axis_alignment(CrossAxisAlignment::Stretch);
    row_with_stripe.add_child(stripe);
    row_with_stripe.add_child(
        ConstrainedBox::new(Empty::new().finish())
            .with_width(SECTION_MEMBER_INDENT - SECTION_MEMBER_STRIPE_WIDTH)
            .finish(),
    );
    row_with_stripe.add_child(warpui::elements::Shrinkable::new(1., row).finish());

    Container::new(row_with_stripe.finish()).finish()
}

/// Convenience: a column wrapper holding the section header + (optional)
/// member rows. Caller produces this as a single element to slot into
/// the existing `render_groups` column. Does not handle drop indicators
/// between members — those keep using the existing per-row drop targets.
pub fn build_section_column(
    header: Box<dyn Element>,
    member_rows: Vec<Box<dyn Element>>,
) -> Box<dyn Element> {
    let mut column = Flex::column()
        .with_main_axis_size(MainAxisSize::Min)
        .with_cross_axis_alignment(CrossAxisAlignment::Stretch);
    column.add_child(header);
    for row in member_rows {
        column.add_child(row);
    }
    column.finish()
}
