use crate::menu::{Dir, MenuGrid, NavOptions, Repeat, first_focusable, step};

const OFF: NavOptions = NavOptions { wrap: false, skip_disabled: false };
const WRAP: NavOptions = NavOptions { wrap: true, skip_disabled: false };

fn grid(columns: usize, len: usize) -> MenuGrid {
    MenuGrid { columns, enabled: vec![true; len] }
}

#[test]
fn list_moves_up_and_down_and_ignores_left_right() {
    let g = grid(1, 4);
    assert_eq!(step(&g, 0, Dir::Down, OFF), 1);
    assert_eq!(step(&g, 2, Dir::Up, OFF), 1);
    assert_eq!(step(&g, 2, Dir::Left, OFF), 2);
    assert_eq!(step(&g, 2, Dir::Right, OFF), 2);
}

#[test]
fn row_moves_left_right_only() {
    let g = grid(4, 4);
    assert_eq!(step(&g, 1, Dir::Right, OFF), 2);
    assert_eq!(step(&g, 1, Dir::Left, OFF), 0);
    assert_eq!(step(&g, 1, Dir::Up, OFF), 1);
    assert_eq!(step(&g, 1, Dir::Down, OFF), 1);
}

#[test]
fn grid_3x2_moves_in_four_directions() {
    let g = grid(3, 6);
    assert_eq!(step(&g, 0, Dir::Right, OFF), 1);
    assert_eq!(step(&g, 1, Dir::Down, OFF), 4);
    assert_eq!(step(&g, 4, Dir::Left, OFF), 3);
    assert_eq!(step(&g, 3, Dir::Up, OFF), 0);
}

#[test]
fn down_into_incomplete_last_row_lands_on_its_last_item() {
    let g = grid(3, 7);
    assert_eq!(step(&g, 4, Dir::Down, OFF), 6);
    assert_eq!(step(&g, 3, Dir::Down, OFF), 6);
}

#[test]
fn wrap_off_stays_at_edges() {
    let g = grid(3, 6);
    assert_eq!(step(&g, 2, Dir::Right, OFF), 2);
    assert_eq!(step(&g, 3, Dir::Left, OFF), 3);
    assert_eq!(step(&g, 1, Dir::Up, OFF), 1);
    assert_eq!(step(&g, 4, Dir::Down, OFF), 4);
}

#[test]
fn wrap_on_goes_to_the_other_end_of_the_row_and_column() {
    let g = grid(3, 6);
    assert_eq!(step(&g, 2, Dir::Right, WRAP), 0);
    assert_eq!(step(&g, 3, Dir::Left, WRAP), 5);
    assert_eq!(step(&g, 1, Dir::Up, WRAP), 4);
    assert_eq!(step(&g, 4, Dir::Down, WRAP), 1);
    let list = grid(1, 3);
    assert_eq!(step(&list, 2, Dir::Down, WRAP), 0);
    assert_eq!(step(&list, 0, Dir::Up, WRAP), 2);
}

#[test]
fn skip_disabled_jumps_over_and_stays_when_none_left() {
    let skip = NavOptions { wrap: false, skip_disabled: true };
    let g = MenuGrid { columns: 1, enabled: vec![true, false, true, false] };
    assert_eq!(step(&g, 0, Dir::Down, skip), 2);
    assert_eq!(step(&g, 2, Dir::Down, skip), 2);
    assert_eq!(step(&g, 0, Dir::Down, OFF), 1);
    assert_eq!(first_focusable(&g, 1, skip), Some(2));
    assert_eq!(first_focusable(&g, 1, OFF), Some(1));
    let none = MenuGrid { columns: 2, enabled: vec![false, false] };
    assert_eq!(first_focusable(&none, 0, skip), None);
}

#[test]
fn empty_and_out_of_range_never_panic() {
    let empty = grid(1, 0);
    assert_eq!(step(&empty, 0, Dir::Down, WRAP), 0);
    assert_eq!(first_focusable(&empty, 0, OFF), None);
    let g = grid(0, 3);
    assert_eq!(step(&g, 9, Dir::Down, OFF), 2);
    assert_eq!(step(&g, 9, Dir::Right, WRAP), 2);
    assert_eq!(first_focusable(&g, 9, OFF), Some(0));
}

#[test]
fn repeat_fires_on_press_then_after_0_4_then_every_0_12() {
    let mut r = Repeat::default();
    assert_eq!(r.update(Some(Dir::Down), 0.0), Some(Dir::Down));
    assert_eq!(r.update(Some(Dir::Down), 0.39), None);
    assert_eq!(r.update(Some(Dir::Down), 0.40), Some(Dir::Down));
    assert_eq!(r.update(Some(Dir::Down), 0.45), None);
    assert_eq!(r.update(Some(Dir::Down), 0.52), Some(Dir::Down));
    assert_eq!(r.update(None, 0.53), None);
    assert_eq!(r.update(Some(Dir::Up), 0.54), Some(Dir::Up));
    assert_eq!(r.update(Some(Dir::Left), 0.55), Some(Dir::Left));
    assert_eq!(r.update(Some(Dir::Left), f64::NAN), None);
}
