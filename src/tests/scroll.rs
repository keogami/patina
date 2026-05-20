use crate::application::utils::{
    ScrollState, selected_scroll_with_anchor, selected_scroll_with_direction,
};

#[test]
fn stateful_initial_state_tracks_selection_and_window() {
    let state = ScrollState::new();
    let (range, new_state) = selected_scroll_with_direction(10, 3, Some(0), state);

    assert_eq!(range, 0..3);
    assert_eq!(new_state.last_selected, Some(0));
    assert_eq!(new_state.window_start, 0);
}

#[test]
fn stateful_scrolling_down_updates_window_start() {
    let state = ScrollState {
        last_selected: Some(3),
        window_start: 1,
    };

    let (range, new_state) = selected_scroll_with_direction(10, 3, Some(5), state);

    assert_eq!(range, 3..6);
    assert_eq!(new_state.last_selected, Some(5));
    assert_eq!(new_state.window_start, 3);
}

#[test]
fn stateful_scrolling_up_updates_window_start() {
    let state = ScrollState {
        last_selected: Some(5),
        window_start: 3,
    };

    let (range, new_state) = selected_scroll_with_direction(10, 3, Some(2), state);

    assert_eq!(range, 2..5);
    assert_eq!(new_state.last_selected, Some(2));
    assert_eq!(new_state.window_start, 2);
}

#[test]
fn stateful_scroll_with_none_selection_resets_to_top() {
    let state = ScrollState {
        last_selected: Some(5),
        window_start: 4,
    };

    let (range, new_state) = selected_scroll_with_direction(10, 3, None, state);
    assert_eq!(range, 0..3);
    assert_eq!(new_state.last_selected, None);
    assert_eq!(new_state.window_start, 0);
}

#[test]
fn stateful_scroll_maintains_boundaries() {
    let items = 10;
    let max_items = 3;

    let state = ScrollState {
        last_selected: Some(7),
        window_start: 6,
    };

    let (range, _) = selected_scroll_with_direction(items, max_items, Some(9), state);

    assert!(range.end <= items);
    assert_eq!(range.len(), max_items);
    assert!(range.contains(&9));
}

#[test]
fn anchor_keeps_window_when_selection_inside() {
    let (range, start) = selected_scroll_with_anchor(20, 5, Some(7), 5);
    assert_eq!(range, 5..10);
    assert_eq!(start, 5);
}

#[test]
fn anchor_moves_window_up_when_selection_is_above() {
    let (range, start) = selected_scroll_with_anchor(20, 5, Some(3), 8);
    assert_eq!(range, 3..8);
    assert_eq!(start, 3);
}

#[test]
fn anchor_moves_window_down_when_selection_is_below() {
    let (range, start) = selected_scroll_with_anchor(20, 5, Some(12), 5);
    assert_eq!(range, 8..13);
    assert_eq!(start, 8);
}

#[test]
fn anchor_clamps_window_at_end() {
    let (range, start) = selected_scroll_with_anchor(10, 4, Some(99), 6);
    assert_eq!(range, 6..10);
    assert_eq!(start, 6);
}
