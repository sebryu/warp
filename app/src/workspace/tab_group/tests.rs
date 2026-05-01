//! Unit tests for the `tab_group` module — registry-only behavior. State-machine
//! tests that involve `Workspace` live in `workspace/view_test.rs`.

use super::*;

#[test]
fn tab_group_registry_starts_empty() {
    let r = TabGroupRegistry::default();
    assert!(r.is_empty());
    assert_eq!(r.len(), 0);
}

#[test]
fn tab_group_registry_next_default_color_round_robin() {
    let mut r = TabGroupRegistry::default();
    let palette = TabGroupColor::all_in_order();
    assert_eq!(palette.len(), 8);

    // First 8 calls return the palette in order, each followed by registering the
    // returned color so the next call sees it as "used".
    for expected in &palette {
        let next = r.next_default_color();
        assert_eq!(next, *expected);
        r.insert(TabGroup::new(String::new(), next));
    }

    // 9th call: all 8 are in use; the helper still returns a palette entry
    // (PRODUCT §8: "Once all 8 are in use, color reuse is allowed.").
    let next = r.next_default_color();
    assert!(palette.contains(&next));
}

#[test]
fn tab_group_registry_skips_used_colors_when_possible() {
    let mut r = TabGroupRegistry::default();
    r.insert(TabGroup::new("a".into(), TabGroupColor::Blue));
    r.insert(TabGroup::new("b".into(), TabGroupColor::Red));
    r.insert(TabGroup::new("c".into(), TabGroupColor::Green));

    let next = r.next_default_color();
    assert!(![
        TabGroupColor::Blue,
        TabGroupColor::Red,
        TabGroupColor::Green
    ]
    .contains(&next));
}

#[test]
fn tab_group_id_is_unique() {
    let a = TabGroupId::new();
    let b = TabGroupId::new();
    assert_ne!(a, b);
}

#[test]
fn tab_group_color_display_names_match_palette_order() {
    let names: Vec<&'static str> = TabGroupColor::all_in_order()
        .into_iter()
        .map(|c| c.display_name())
        .collect();
    assert_eq!(
        names,
        vec!["Grey", "Blue", "Red", "Yellow", "Green", "Pink", "Purple", "Cyan"],
    );
}
