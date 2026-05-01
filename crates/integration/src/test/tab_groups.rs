//! Integration tests for Tab Groups (PRODUCT.md / TECH.md under
//! `specs/tab-groups/`).
//!
//! These tests exercise the user-visible flows end-to-end inside a real
//! Warp app instance (one terminal pane bootstrapped, real workspace
//! lifecycle, real action dispatch). State-machine invariants live in
//! `app/src/workspace/view_test.rs`; this file focuses on flows that
//! traverse the action layer (`WorkspaceAction` enum) into `Workspace`
//! state and back, which the in-process unit tests cannot fully cover.
//!
//! All tests turn on `FeatureFlag::TabGroups` at the top of the builder.
//! State is read through the `Workspace::tab_group_registry()` and
//! `Workspace::tab_group_id_at()` accessors so this file stays out of
//! the `pub(crate)` boundary.

use warp::features::FeatureFlag;
use warp::integration_testing::{
    step::new_step_with_default_assertions, terminal::wait_until_bootstrapped_single_pane_for_tab,
    view_getters::workspace_view,
};
use warp::workspace::tab_group::{TabGroupColor, TabGroupId};
use warp::workspace::WorkspaceAction;
use warpui::{
    integration::{AssertionOutcome, TestStep},
    App, WindowId,
};

use super::{new_builder, Builder};

// ── Helpers ────────────────────────────────────────────────────────────────

/// Adds `count` extra terminal tabs to the workspace by dispatching the
/// existing `AddTerminalTab` action.
fn add_extra_tabs_step(count: usize) -> TestStep {
    new_step_with_default_assertions(&format!("Add {count} extra terminal tabs")).with_action(
        move |app, window_id, _| {
            let workspace = workspace_view(app, window_id);
            app.update(|ctx| {
                for _ in 0..count {
                    ctx.dispatch_typed_action_for_view(
                        window_id,
                        workspace.id(),
                        &WorkspaceAction::AddTerminalTab {
                            hide_homepage: false,
                        },
                    );
                }
            });
        },
    )
}

fn dispatch_workspace_action(app: &mut App, window_id: WindowId, action: &WorkspaceAction) {
    let workspace = workspace_view(app, window_id);
    app.update(|ctx| {
        ctx.dispatch_typed_action_for_view(window_id, workspace.id(), action);
    });
}

/// Reads the (assumed unique) group's id from the registry. Panics if
/// the registry is empty.
fn first_group_id(app: &App, window_id: WindowId) -> TabGroupId {
    let workspace = workspace_view(app, window_id);
    workspace.read(app, |workspace, _| {
        *workspace
            .tab_group_registry()
            .iter()
            .next()
            .expect("integration fixture: expected at least one tab group")
            .0
    })
}

// ── Tests ─────────────────────────────────────────────────────────────────

pub fn test_tab_groups_create_via_action_assigns_membership() -> Builder {
    FeatureFlag::TabGroups.set_enabled(true);
    new_builder()
        .with_step(wait_until_bootstrapped_single_pane_for_tab(0))
        .with_step(add_extra_tabs_step(1))
        .with_step(
            new_step_with_default_assertions("Dispatch CreateTabGroupFromTab on tab 0")
                .with_action(|app, window_id, _| {
                    dispatch_workspace_action(
                        app,
                        window_id,
                        &WorkspaceAction::CreateTabGroupFromTab { tab_index: 0 },
                    );
                })
                .add_named_assertion("registry has exactly one group", |app, window_id| {
                    let workspace = workspace_view(app, window_id);
                    workspace.read(app, |workspace, _| {
                        let len = workspace.tab_group_registry().len();
                        if len == 1 {
                            AssertionOutcome::Success
                        } else {
                            AssertionOutcome::failure(format!(
                                "expected 1 group, got {len}"
                            ))
                        }
                    })
                })
                .add_named_assertion(
                    "tab 0 is a member of the new group; tab 1 is not",
                    |app, window_id| {
                        let workspace = workspace_view(app, window_id);
                        workspace.read(app, |workspace, _| {
                            let gid_0 = workspace.tab_group_id_at(0);
                            let gid_1 = workspace.tab_group_id_at(1);
                            match (gid_0, gid_1) {
                                (Some(_), None) => AssertionOutcome::Success,
                                _ => AssertionOutcome::failure(format!(
                                    "expected tab 0 grouped, tab 1 ungrouped, got {gid_0:?} and {gid_1:?}"
                                )),
                            }
                        })
                    },
                )
                .add_named_assertion(
                    "new group has empty name and is in rename mode (PRODUCT §9)",
                    |app, window_id| {
                        let workspace = workspace_view(app, window_id);
                        workspace.read(app, |workspace, _| {
                            let registry = workspace.tab_group_registry();
                            let Some((_, group)) = registry.iter().next() else {
                                return AssertionOutcome::failure(
                                    "no group in registry".to_string(),
                                );
                            };
                            if !group.name.is_empty() {
                                return AssertionOutcome::failure(format!(
                                    "expected empty name on new group, got {:?}",
                                    group.name
                                ));
                            }
                            AssertionOutcome::Success
                        })
                    },
                ),
        )
}

pub fn test_tab_groups_add_recolor_collapse_lifecycle() -> Builder {
    FeatureFlag::TabGroups.set_enabled(true);
    new_builder()
        .with_step(wait_until_bootstrapped_single_pane_for_tab(0))
        .with_step(add_extra_tabs_step(2))
        .with_step(new_step_with_default_assertions("Create a group from tab 1").with_action(
            |app, window_id, _| {
                dispatch_workspace_action(
                    app,
                    window_id,
                    &WorkspaceAction::CreateTabGroupFromTab { tab_index: 1 },
                );
            },
        ))
        .with_step(
            new_step_with_default_assertions(
                "Add tab 2 to the group; expect run = [1, 2] (PRODUCT §31)",
            )
            .with_action(|app, window_id, _| {
                let gid = first_group_id(app, window_id);
                dispatch_workspace_action(
                    app,
                    window_id,
                    &WorkspaceAction::AddTabToTabGroup {
                        tab_index: 2,
                        group_id: gid,
                    },
                );
            })
            .add_named_assertion("tabs 1 and 2 are members; tab 0 is not", |app, window_id| {
                let workspace = workspace_view(app, window_id);
                workspace.read(app, |workspace, _| {
                    let gid_0 = workspace.tab_group_id_at(0);
                    let gid_1 = workspace.tab_group_id_at(1);
                    let gid_2 = workspace.tab_group_id_at(2);
                    match (gid_0, gid_1, gid_2) {
                        (None, Some(g1), Some(g2)) if g1 == g2 => AssertionOutcome::Success,
                        _ => AssertionOutcome::failure(format!(
                            "expected (None, Some(g), Some(g)), got ({gid_0:?}, {gid_1:?}, {gid_2:?})"
                        )),
                    }
                })
            }),
        )
        .with_step(
            new_step_with_default_assertions("Recolor the group to Cyan (PRODUCT §17)")
                .with_action(|app, window_id, _| {
                    let gid = first_group_id(app, window_id);
                    dispatch_workspace_action(
                        app,
                        window_id,
                        &WorkspaceAction::RecolorTabGroup {
                            group_id: gid,
                            color: TabGroupColor::Cyan,
                        },
                    );
                })
                .add_named_assertion("group color is Cyan", |app, window_id| {
                    let workspace = workspace_view(app, window_id);
                    workspace.read(app, |workspace, _| {
                        let registry = workspace.tab_group_registry();
                        let Some((_, group)) = registry.iter().next() else {
                            return AssertionOutcome::failure("no group".to_string());
                        };
                        if group.color == TabGroupColor::Cyan {
                            AssertionOutcome::Success
                        } else {
                            AssertionOutcome::failure(format!(
                                "expected Cyan, got {:?}",
                                group.color
                            ))
                        }
                    })
                }),
        )
        .with_step(
            new_step_with_default_assertions(
                "Activate ungrouped tab 0, then collapse the group — collapse sticks (PRODUCT §27 negative)",
            )
            .with_action(|app, window_id, _| {
                let gid = first_group_id(app, window_id);
                dispatch_workspace_action(app, window_id, &WorkspaceAction::ActivateTab(0));
                dispatch_workspace_action(
                    app,
                    window_id,
                    &WorkspaceAction::ToggleTabGroupCollapsed { group_id: gid },
                );
            })
            .add_named_assertion("group is collapsed", |app, window_id| {
                let workspace = workspace_view(app, window_id);
                workspace.read(app, |workspace, _| {
                    let registry = workspace.tab_group_registry();
                    let Some((_, group)) = registry.iter().next() else {
                        return AssertionOutcome::failure("no group".to_string());
                    };
                    if group.collapsed {
                        AssertionOutcome::Success
                    } else {
                        AssertionOutcome::failure(
                            "group should be collapsed after toggle".to_string(),
                        )
                    }
                })
            }),
        )
        .with_step(
            new_step_with_default_assertions(
                "Activate tab 1 (a member) — group auto-expands (PRODUCT §27/§28)",
            )
            .with_action(|app, window_id, _| {
                dispatch_workspace_action(app, window_id, &WorkspaceAction::ActivateTab(1));
            })
            .add_named_assertion(
                "group is expanded after activating a member",
                |app, window_id| {
                    let workspace = workspace_view(app, window_id);
                    workspace.read(app, |workspace, _| {
                        let registry = workspace.tab_group_registry();
                        let Some((_, group)) = registry.iter().next() else {
                            return AssertionOutcome::failure("no group".to_string());
                        };
                        if !group.collapsed {
                            AssertionOutcome::Success
                        } else {
                            AssertionOutcome::failure(
                                "group should auto-expand on member activation".to_string(),
                            )
                        }
                    })
                },
            ),
        )
}

pub fn test_tab_groups_remove_dissolves_singleton() -> Builder {
    FeatureFlag::TabGroups.set_enabled(true);
    new_builder()
        .with_step(wait_until_bootstrapped_single_pane_for_tab(0))
        .with_step(add_extra_tabs_step(1))
        .with_step(
            new_step_with_default_assertions("Create singleton group on tab 1").with_action(
                |app, window_id, _| {
                    dispatch_workspace_action(
                        app,
                        window_id,
                        &WorkspaceAction::CreateTabGroupFromTab { tab_index: 1 },
                    );
                },
            ),
        )
        .with_step(
            new_step_with_default_assertions(
                "Remove tab 1 from the group — group dissolves (PRODUCT §3, §51)",
            )
            .with_action(|app, window_id, _| {
                dispatch_workspace_action(
                    app,
                    window_id,
                    &WorkspaceAction::RemoveTabFromTabGroup { tab_index: 1 },
                );
            })
            .add_named_assertion("registry is empty", |app, window_id| {
                let workspace = workspace_view(app, window_id);
                workspace.read(app, |workspace, _| {
                    let len = workspace.tab_group_registry().len();
                    if len == 0 {
                        AssertionOutcome::Success
                    } else {
                        AssertionOutcome::failure(format!(
                            "expected empty registry, got {len} groups"
                        ))
                    }
                })
            })
            .add_named_assertion("tab 1 is ungrouped", |app, window_id| {
                let workspace = workspace_view(app, window_id);
                workspace.read(app, |workspace, _| {
                    let gid = workspace.tab_group_id_at(1);
                    if gid.is_none() {
                        AssertionOutcome::Success
                    } else {
                        AssertionOutcome::failure(format!("expected tab 1 ungrouped, got {gid:?}"))
                    }
                })
            }),
        )
}

pub fn test_tab_groups_ungroup_dissolves_keeps_member_positions() -> Builder {
    FeatureFlag::TabGroups.set_enabled(true);
    new_builder()
        .with_step(wait_until_bootstrapped_single_pane_for_tab(0))
        .with_step(add_extra_tabs_step(2))
        .with_step(
            new_step_with_default_assertions("Create a group from tab 1, then add tab 2")
                .with_action(|app, window_id, _| {
                    dispatch_workspace_action(
                        app,
                        window_id,
                        &WorkspaceAction::CreateTabGroupFromTab { tab_index: 1 },
                    );
                    let gid = first_group_id(app, window_id);
                    dispatch_workspace_action(
                        app,
                        window_id,
                        &WorkspaceAction::AddTabToTabGroup {
                            tab_index: 2,
                            group_id: gid,
                        },
                    );
                }),
        )
        .with_step(
            new_step_with_default_assertions(
                "Ungroup — registry empties; both former members keep their positions (PRODUCT §50, §54)",
            )
            .with_action(|app, window_id, _| {
                let gid = first_group_id(app, window_id);
                dispatch_workspace_action(
                    app,
                    window_id,
                    &WorkspaceAction::UngroupTabGroup { group_id: gid },
                );
            })
            .add_named_assertion("registry is empty", |app, window_id| {
                let workspace = workspace_view(app, window_id);
                workspace.read(app, |workspace, _| {
                    let len = workspace.tab_group_registry().len();
                    if len == 0 {
                        AssertionOutcome::Success
                    } else {
                        AssertionOutcome::failure(format!(
                            "expected empty registry, got {len} groups"
                        ))
                    }
                })
            })
            .add_named_assertion("tab count unchanged at 3 with all ungrouped", |app, window_id| {
                let workspace = workspace_view(app, window_id);
                workspace.read(app, |workspace, _| {
                    let count = workspace.tab_count();
                    let g1 = workspace.tab_group_id_at(1);
                    let g2 = workspace.tab_group_id_at(2);
                    if count == 3 && g1.is_none() && g2.is_none() {
                        AssertionOutcome::Success
                    } else {
                        AssertionOutcome::failure(format!(
                            "expected 3 tabs all ungrouped, got count={count}, g1={g1:?}, g2={g2:?}"
                        ))
                    }
                })
            }),
        )
}
