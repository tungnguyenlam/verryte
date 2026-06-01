use crate::*;
use verryte_core::Rng;

#[test]
fn test_perimeter_points() {
    let grid = TileGrid::new(3, 3, '.');
    assert!(grid.is_on_edge(Point::new(0, 0)));
    assert!(grid.is_on_edge(Point::new(2, 2)));
    assert!(!grid.is_on_edge(Point::new(1, 1)));

    let perim = grid.perimeter_points();
    assert_eq!(perim.len(), 8);
    assert!(perim.contains(&Point::new(0, 0)));
    assert!(perim.contains(&Point::new(2, 2)));
    assert!(!perim.contains(&Point::new(1, 1)));
}

#[test]
fn point_steps_by_direction() {
    let p = Point::new(4, 4);
    assert_eq!(p.step(Direction::North), Point::new(4, 3));
    assert_eq!(p.step(Direction::South), Point::new(4, 5));
    assert_eq!(p.step(Direction::East), Point::new(5, 4));
    assert_eq!(p.step(Direction::West), Point::new(3, 4));
    assert_eq!(Direction::North.opposite(), Direction::South);
    assert_eq!(p.manhattan_distance(Point::new(1, 8)), 7);
    assert_eq!(
        p.neighbors4(),
        [
            Point::new(4, 3),
            Point::new(4, 5),
            Point::new(5, 4),
            Point::new(3, 4),
        ]
    );
}

#[test]
fn point_saturating_offset_clamps_on_overflow() {
    let p = Point::new(5, 3);
    assert_eq!(p.saturating_offset(2, 1), Point::new(7, 4));

    // Saturating at i16::MAX.
    let p = Point::new(i16::MAX, i16::MAX);
    assert_eq!(p.saturating_offset(1, 1), Point::new(i16::MAX, i16::MAX));

    // Saturating at i16::MIN.
    let p = Point::new(i16::MIN, i16::MIN);
    assert_eq!(p.saturating_offset(-1, -1), Point::new(i16::MIN, i16::MIN));
}

#[test]
fn direction_rotate_cw_cycles() {
    assert_eq!(Direction::North.rotate_cw(), Direction::East);
    assert_eq!(Direction::East.rotate_cw(), Direction::South);
    assert_eq!(Direction::South.rotate_cw(), Direction::West);
    assert_eq!(Direction::West.rotate_cw(), Direction::North);
}

#[test]
fn direction_rotate_ccw_cycles() {
    assert_eq!(Direction::North.rotate_ccw(), Direction::West);
    assert_eq!(Direction::West.rotate_ccw(), Direction::South);
    assert_eq!(Direction::South.rotate_ccw(), Direction::East);
    assert_eq!(Direction::East.rotate_ccw(), Direction::North);
}

#[test]
fn direction_rotate_cw_four_times_is_identity() {
    for d in Direction::ALL {
        assert_eq!(d.rotate_cw().rotate_cw().rotate_cw().rotate_cw(), d);
    }
}

#[test]
fn direction_from_offset_roundtrips() {
    for d in Direction::ALL {
        let (dx, dy) = d.delta();
        assert_eq!(Direction::from_offset(dx, dy), Some(d));
    }
}

#[test]
fn direction_from_offset_rejects_non_unit() {
    assert_eq!(Direction::from_offset(0, 0), None);
    assert_eq!(Direction::from_offset(2, 0), None);
    assert_eq!(Direction::from_offset(1, 1), None);
    assert_eq!(Direction::from_offset(-1, -1), None);
}

#[test]
fn direction8_from_offset_roundtrips() {
    for d in Direction8::ALL {
        let (dx, dy) = d.delta();
        assert_eq!(Direction8::from_offset(dx, dy), Some(d));
    }
}

#[test]
fn direction8_from_offset_rejects_invalid() {
    assert_eq!(Direction8::from_offset(0, 0), None);
    assert_eq!(Direction8::from_offset(2, 0), None);
    assert_eq!(Direction8::from_offset(0, -2), None);
}

#[test]
fn size_contains_rejects_negative_and_edge_points() {
    let size = Size::new(3, 2);
    assert!(size.contains(Point::new(2, 1)));
    assert!(!size.contains(Point::new(3, 1)));
    assert!(!size.contains(Point::new(1, 2)));
    assert!(!size.contains(Point::new(-1, 1)));
}

#[test]
fn tile_grid_get_set_and_points_are_row_major() {
    let mut grid = TileGrid::new(3, 2, '.');
    assert_eq!(grid.len(), 6);
    assert!(grid.in_bounds(Point::new(0, 0)));
    assert!(!grid.in_bounds(Point::new(3, 0)));
    assert!(!grid.in_bounds(Point::new(0, 2)));

    assert!(grid.set(Point::new(1, 1), '#'));
    assert_eq!(grid.get(Point::new(1, 1)), Some(&'#'));
    assert!(!grid.set(Point::new(3, 1), '!'));

    let points: Vec<Point> = grid.points().collect();
    assert_eq!(
        points,
        vec![
            Point::new(0, 0),
            Point::new(1, 0),
            Point::new(2, 0),
            Point::new(0, 1),
            Point::new(1, 1),
            Point::new(2, 1),
        ]
    );
}

#[test]
fn tile_grid_bounds_matches_size() {
    let grid = TileGrid::new(3, 2, '.');
    assert_eq!(grid.bounds(), Bounds::new(0, 0, 3, 2));
}

#[test]
fn tile_grid_points_in_clips_to_bounds() {
    let grid = TileGrid::new(3, 2, '.');
    let points: Vec<Point> = grid.points_in(Bounds::new(1, 0, 4, 3)).collect();
    assert_eq!(
        points,
        vec![
            Point::new(1, 0),
            Point::new(2, 0),
            Point::new(1, 1),
            Point::new(2, 1),
        ]
    );
}

#[test]
fn tile_grid_points_in_returns_empty_when_out_of_bounds() {
    let grid = TileGrid::new(2, 2, '.');
    let points: Vec<Point> = grid.points_in(Bounds::new(5, 5, 2, 2)).collect();
    assert!(points.is_empty());
}

#[test]
fn tile_grid_neighbors4_clip_to_bounds() {
    let grid = TileGrid::from_vec(3, 2, vec![0, 1, 2, 3, 4, 5]).unwrap();
    let neighbors = grid.neighbors4(Point::new(1, 0));
    assert_eq!(
        neighbors,
        vec![
            (Point::new(1, 1), &4),
            (Point::new(2, 0), &2),
            (Point::new(0, 0), &0),
        ]
    );
}

#[test]
fn from_vec_validates_tile_count() {
    let err = TileGrid::from_vec(2, 2, vec![1, 2, 3]).unwrap_err();
    assert_eq!(
        err,
        GridError::WrongTileCount {
            expected: 4,
            actual: 3
        }
    );
}

#[test]
fn contains_point_checks_bounds_and_predicate() {
    let grid = TileGrid::from_vec(3, 2, vec!['.', '#', '.', '.', '#', '.']).unwrap();

    assert!(grid.contains_point(Point::new(0, 0), |&t| t == '.'));
    assert!(grid.contains_point(Point::new(1, 0), |&t| t == '#'));
    assert!(!grid.contains_point(Point::new(0, 0), |&t| t == '#'));
    assert!(!grid.contains_point(Point::new(5, 0), |&t| t == '.'));
}

#[test]
fn line_between_includes_endpoints() {
    assert_eq!(
        line_between(Point::new(1, 1), Point::new(4, 2)),
        vec![
            Point::new(1, 1),
            Point::new(2, 1),
            Point::new(3, 2),
            Point::new(4, 2),
        ]
    );
}

#[test]
fn line_iter_yields_same_points_as_line_between() {
    let start = Point::new(0, 0);
    let end = Point::new(5, 3);
    let iter_points: Vec<Point> = LineIter::new(start, end).collect();
    let vec_points = line_between(start, end);
    assert_eq!(iter_points, vec_points);
}

#[test]
fn line_iter_handles_single_point() {
    let p = Point::new(3, 3);
    let mut iter = LineIter::new(p, p);
    assert_eq!(iter.next(), Some(p));
    assert_eq!(iter.next(), None);
}

#[test]
fn line_iter_handles_vertical_line() {
    let points: Vec<Point> = LineIter::new(Point::new(2, 0), Point::new(2, 3)).collect();
    assert_eq!(
        points,
        vec![
            Point::new(2, 0),
            Point::new(2, 1),
            Point::new(2, 2),
            Point::new(2, 3),
        ]
    );
}

#[test]
fn line_iter_can_short_circuit_early() {
    let mut iter = LineIter::new(Point::new(0, 0), Point::new(10, 10));
    assert_eq!(iter.next(), Some(Point::new(0, 0)));
    assert_eq!(iter.next(), Some(Point::new(1, 1)));
}

#[test]
#[allow(deprecated)]
fn visible_points_respect_radius_and_blockers() {
    let grid = TileGrid::from_vec(
        5,
        3,
        vec![
            '.', '.', '#', '.', '.', '.', '.', '#', '.', '.', '.', '.', '.', '.', '.',
        ],
    )
    .unwrap();
    let visible = grid.visible_points(Point::new(0, 1), 4, |tile| *tile == '#');

    assert!(visible.contains(&Point::new(0, 1)));
    assert!(
        visible.contains(&Point::new(2, 1)),
        "blocking tile is visible"
    );
    assert!(
        !visible.contains(&Point::new(3, 1)),
        "blocked tile beyond wall is hidden"
    );
    assert!(
        !visible.contains(&Point::new(4, 2)),
        "outside Manhattan radius"
    );
}

#[test]
fn shortest_path4_finds_cardinal_path_around_walls() {
    let grid = TileGrid::from_vec(
        5,
        4,
        vec![
            '.', '.', '.', '.', '.', '.', '#', '#', '#', '.', '.', '.', '.', '.', '.', '#', '#',
            '#', '.', '.',
        ],
    )
    .unwrap();

    let path = grid
        .shortest_path4(Point::new(0, 0), Point::new(4, 3), |_, tile| *tile == '.')
        .unwrap();

    assert_eq!(path.first(), Some(&Point::new(0, 0)));
    assert_eq!(path.last(), Some(&Point::new(4, 3)));
    assert_eq!(path.len(), 8);
    assert!(path
        .windows(2)
        .all(|pair| pair[0].manhattan_distance(pair[1]) == 1));
}

#[test]
fn shortest_path4_returns_none_when_goal_is_blocked_or_out_of_bounds() {
    let grid = TileGrid::from_vec(3, 1, vec!['.', '#', '.']).unwrap();

    assert_eq!(
        grid.shortest_path4(Point::new(0, 0), Point::new(2, 0), |_, tile| *tile == '.'),
        None
    );
    assert_eq!(
        grid.shortest_path4(Point::new(0, 0), Point::new(3, 0), |_, tile| *tile == '.'),
        None
    );
}

#[test]
fn test_shortest_path_weighted_avoids_mud() {
    let grid = TileGrid::from_vec(3, 3, vec!['.', 'M', '.', '.', '.', '.', '.', '.', '.']).unwrap();

    let path = grid
        .shortest_path4_weighted(
            Point::new(0, 0),
            Point::new(2, 0),
            |_, _| true,
            |_, _, &tile| if tile == 'M' { 10 } else { 1 },
        )
        .unwrap();

    assert_eq!(
        path,
        vec![
            Point::new(0, 0),
            Point::new(0, 1),
            Point::new(1, 1),
            Point::new(2, 1),
            Point::new(2, 0),
        ]
    );
}

#[test]
fn test_shortest_path8_weighted_avoids_mud() {
    let grid = TileGrid::from_vec(3, 3, vec!['.', 'M', '.', '.', '.', '.', '.', '.', '.']).unwrap();

    let path = grid
        .shortest_path8_weighted(
            Point::new(0, 0),
            Point::new(2, 0),
            |_, _| true,
            |_, _, &tile| if tile == 'M' { 10 } else { 1 },
        )
        .unwrap();

    assert_eq!(
        path,
        vec![Point::new(0, 0), Point::new(1, 1), Point::new(2, 0),]
    );
}

#[test]
fn nearest_path4_chooses_shortest_reachable_target() {
    let grid = TileGrid::from_vec(
        5,
        3,
        vec![
            '.', '.', '.', '.', '.', '.', '#', '#', '#', '.', '.', '.', '.', '.', '.',
        ],
    )
    .unwrap();

    let path = grid
        .nearest_path4(
            Point::new(0, 0),
            [Point::new(4, 0), Point::new(2, 2)],
            |_, tile| *tile == '.',
        )
        .unwrap();

    assert_eq!(path.first(), Some(&Point::new(0, 0)));
    assert_eq!(path.last(), Some(&Point::new(4, 0)));
    assert_eq!(path.len(), 5);
}

#[test]
fn distance_to_nearest4_reports_shortest_reachable_distance() {
    let grid = TileGrid::from_vec(
        5,
        3,
        vec![
            '.', '.', '.', '.', '.', '.', '#', '#', '#', '.', '.', '.', '.', '.', '.',
        ],
    )
    .unwrap();

    let distance = grid.distance_to_nearest4(
        Point::new(0, 0),
        [Point::new(4, 0), Point::new(2, 2)],
        |_, tile| *tile == '.',
    );
    assert_eq!(distance, Some(4));
}

#[test]
fn distance_to_nearest4_returns_none_when_targets_unreachable_or_empty() {
    let grid = TileGrid::from_vec(3, 1, vec!['.', '#', '.']).unwrap();
    assert_eq!(
        grid.distance_to_nearest4(Point::new(0, 0), [Point::new(2, 0)], |_, tile| *tile == '.'),
        None
    );
    assert_eq!(
        grid.distance_to_nearest4(Point::new(0, 0), Vec::<Point>::new(), |_, tile| *tile
            == '.'),
        None
    );
}

#[test]
fn reachable_points4_walks_passable_region_in_bfs_order() {
    let grid = TileGrid::from_vec(
        4,
        3,
        vec!['.', '.', '#', '.', '.', '#', '.', '.', '.', '.', '.', '#'],
    )
    .unwrap();

    let reachable = grid.reachable_points4(Point::new(0, 0), |_, tile| *tile == '.');

    assert_eq!(
        reachable,
        vec![
            Point::new(0, 0),
            Point::new(0, 1),
            Point::new(1, 0),
            Point::new(0, 2),
            Point::new(1, 2),
            Point::new(2, 2),
            Point::new(2, 1),
            Point::new(3, 1),
            Point::new(3, 0),
        ]
    );
}

#[test]
fn flood_fill4_fills_connected_region() {
    // 5x3 grid with a vertical wall at column 2 in rows 0-1.
    // Row 2 is fully open, connecting both sides.
    let grid = TileGrid::from_vec(
        5,
        3,
        vec![
            '.', '.', '#', '.', '.', '.', '.', '#', '.', '.', '.', '.', '.', '.', '.',
        ],
    )
    .unwrap();

    // From (0,0), flood fill reaches left side + bottom row + right side via bottom.
    let region = grid.flood_fill4(Point::new(0, 0), |_, tile| *tile == '.');
    assert_eq!(region.len(), 13);
    assert!(region.contains(&Point::new(0, 0)));
    assert!(region.contains(&Point::new(4, 2)));
}

#[test]
fn flood_fill4_is_blocked_by_walls_on_all_sides() {
    // 5x3 grid with walls fully enclosing (1,1).
    let grid = TileGrid::from_vec(
        5,
        3,
        vec![
            '#', '#', '#', '#', '#', '#', '.', '#', '#', '#', '#', '#', '#', '#', '#',
        ],
    )
    .unwrap();

    let region = grid.flood_fill4(Point::new(1, 1), |_, tile| *tile == '.');
    assert_eq!(region.len(), 1);
    assert_eq!(region[0], Point::new(1, 1));
}

#[test]
fn flood_fill4_returns_empty_for_non_matching_start() {
    let grid = TileGrid::from_vec(3, 1, vec!['.', '#', '.']).unwrap();
    let region = grid.flood_fill4(Point::new(1, 0), |_, tile| *tile == '.');
    assert!(region.is_empty());
}

#[test]
fn flood_fill4_stops_at_boundaries() {
    // 5x3 grid: top two rows are '.', bottom row is '#'.
    let grid = TileGrid::from_vec(
        5,
        3,
        vec![
            '.', '.', '.', '.', '.', '.', '.', '.', '.', '.', '#', '#', '#', '#', '#',
        ],
    )
    .unwrap();
    let region = grid.flood_fill4(Point::new(0, 0), |_, tile| *tile == '.');
    assert_eq!(region.len(), 10);
}

#[test]
fn flood_fill8_reaches_diagonals() {
    let grid = TileGrid::from_vec(
        3,
        3,
        vec![
            '#', '.', '#',
            '.', '#', '.',
            '#', '.', '#',
        ],
    )
    .unwrap();

    let region4 = grid.flood_fill4(Point::new(1, 0), |_, tile| *tile == '.');
    assert_eq!(region4.len(), 1);

    let region8 = grid.flood_fill8(Point::new(1, 0), |_, tile| *tile == '.');
    assert_eq!(region8.len(), 4);
    assert!(region8.contains(&Point::new(1, 0)));
    assert!(region8.contains(&Point::new(0, 1)));
    assert!(region8.contains(&Point::new(2, 1)));
    assert!(region8.contains(&Point::new(1, 2)));
}

#[test]
fn count_regions4_counts_disconnected_areas() {
    let grid = TileGrid::from_vec(7, 1, vec!['.', '.', '#', '.', '.', '.', '#']).unwrap();

    assert_eq!(grid.count_regions4(|_, tile| *tile == '.'), 2);
}

#[test]
fn count_regions4_returns_one_for_fully_connected() {
    let grid = TileGrid::from_vec(3, 3, vec!['.'; 9]).unwrap();
    assert_eq!(grid.count_regions4(|_, tile| *tile == '.'), 1);
}

#[test]
fn count_regions4_returns_zero_for_no_matches() {
    let grid = TileGrid::from_vec(3, 3, vec!['#'; 9]).unwrap();
    assert_eq!(grid.count_regions4(|_, tile| *tile == '.'), 0);
}

#[test]
fn direction8_deltas_and_opposites() {
    assert_eq!(Direction8::North.delta(), (0, -1));
    assert_eq!(Direction8::NorthEast.delta(), (1, -1));
    assert_eq!(Direction8::SouthWest.delta(), (-1, 1));
    assert_eq!(Direction8::North.opposite(), Direction8::South);
    assert_eq!(Direction8::NorthEast.opposite(), Direction8::SouthWest);
    assert!(Direction8::North.is_cardinal());
    assert!(!Direction8::NorthEast.is_cardinal());
    assert_eq!(
        Direction8::to_direction(Direction8::North),
        Some(Direction::North)
    );
    assert_eq!(Direction8::to_direction(Direction8::NorthEast), None);
    assert_eq!(
        Direction8::from_direction(Direction::East),
        Direction8::East
    );
}

#[test]
fn point_neighbors8_includes_diagonals() {
    let p = Point::new(3, 3);
    let n = p.neighbors8();
    assert_eq!(n.len(), 8);
    assert!(n.contains(&Point::new(3, 2))); // North
    assert!(n.contains(&Point::new(4, 2))); // NorthEast
    assert!(n.contains(&Point::new(2, 4))); // SouthWest
    assert!(n.contains(&Point::new(4, 4))); // SouthEast
}

#[test]
fn chebyshev_distance_is_king_move_count() {
    let a = Point::new(0, 0);
    assert_eq!(a.chebyshev_distance(Point::new(3, 3)), 3);
    assert_eq!(a.chebyshev_distance(Point::new(5, 2)), 5);
    assert_eq!(a.chebyshev_distance(Point::new(0, 7)), 7);
}

#[test]
fn euclidean_distance_is_straight_line() {
    let a = Point::new(0, 0);
    assert_eq!(a.euclidean_distance(Point::new(3, 4)), 5.0);
    assert_eq!(a.euclidean_distance(Point::new(0, 0)), 0.0);
    assert!((a.euclidean_distance(Point::new(1, 1)) - std::f32::consts::SQRT_2).abs() < 1e-5);
}

#[test]
fn tile_grid_neighbors8_clips_to_bounds() {
    let grid = TileGrid::from_vec(3, 3, vec![0, 1, 2, 3, 4, 5, 6, 7, 8]).unwrap();
    let neighbors = grid.neighbors8(Point::new(1, 1));
    assert_eq!(neighbors.len(), 8);
    let vals: Vec<i32> = neighbors.iter().map(|(_, v)| **v).collect();
    assert!(vals.contains(&0));
    assert!(vals.contains(&8));
}

#[test]
fn shortest_path8_finds_diagonal_path() {
    let grid = TileGrid::from_vec(5, 5, vec!['.'; 25]).unwrap();

    let path = grid
        .shortest_path8(Point::new(0, 0), Point::new(4, 4), |_, tile| *tile == '.')
        .unwrap();

    assert_eq!(path.first(), Some(&Point::new(0, 0)));
    assert_eq!(path.last(), Some(&Point::new(4, 4)));
    // Diagonal path should be 5 steps (all diagonals).
    assert_eq!(path.len(), 5);
    // Each step should be adjacent in 8-directional sense.
    assert!(path
        .windows(2)
        .all(|pair| pair[0].chebyshev_distance(pair[1]) == 1));
}

#[test]
fn shortest_path8_prefers_diagonal_when_shorter() {
    let grid = TileGrid::from_vec(3, 3, vec!['.'; 9]).unwrap();

    // Cardinal path would be 5 steps; diagonal is 3.
    let path = grid
        .shortest_path8(Point::new(0, 0), Point::new(2, 2), |_, tile| *tile == '.')
        .unwrap();

    assert_eq!(path.len(), 3);
    assert_eq!(path[1], Point::new(1, 1));
}

#[test]
fn shortest_path8_handles_same_start_and_goal() {
    let grid = TileGrid::from_vec(3, 3, vec!['.'; 9]).unwrap();
    let path = grid
        .shortest_path8(Point::new(1, 1), Point::new(1, 1), |_, tile| *tile == '.')
        .unwrap();
    assert_eq!(path, vec![Point::new(1, 1)]);
}

#[test]
fn shortest_path8_returns_none_when_blocked() {
    let grid = TileGrid::from_vec(3, 3, vec!['.', '#', '.', '#', '#', '#', '.', '#', '.']).unwrap();
    assert_eq!(
        grid.shortest_path8(Point::new(0, 0), Point::new(2, 2), |_, tile| *tile == '.'),
        None
    );
}

#[test]
fn nearest_path8_chooses_closest_target_diagonally() {
    let grid = TileGrid::from_vec(5, 5, vec!['.'; 25]).unwrap();

    let path = grid
        .nearest_path8(
            Point::new(0, 0),
            [Point::new(4, 4), Point::new(2, 0)],
            |_, tile| *tile == '.',
        )
        .unwrap();

    assert_eq!(path.first(), Some(&Point::new(0, 0)));
    assert_eq!(path.last(), Some(&Point::new(2, 0)));
    assert_eq!(path.len(), 3);
}

#[test]
fn nearest_path8_returns_none_when_all_targets_unreachable() {
    let grid = TileGrid::from_vec(5, 5, vec!['.'; 25]).unwrap();
    assert_eq!(
        grid.nearest_path8(Point::new(0, 0), Vec::<Point>::new(), |_, tile| *tile
            == '.'),
        None
    );
}

#[test]
fn reachable_points8_walks_passable_region_with_diagonals() {
    let grid = TileGrid::from_vec(
        5,
        3,
        vec![
            '.', '.', '#', '.', '.', '.', '.', '#', '.', '.', '.', '.', '.', '.', '.',
        ],
    )
    .unwrap();

    let reachable = grid.reachable_points8(Point::new(0, 0), |_, tile| *tile == '.');

    // With 8-directional movement, the wall at column 2 can be bypassed diagonally
    // through rows that connect (row 2 is fully open).
    assert!(reachable.contains(&Point::new(0, 0)));
    assert!(reachable.contains(&Point::new(4, 2)));
    // All 13 open tiles should be reachable with 8-directional movement.
    assert_eq!(reachable.len(), 13);
}

#[test]
fn reachable_points8_returns_empty_for_out_of_bounds() {
    let grid = TileGrid::from_vec(3, 3, vec!['.'; 9]).unwrap();
    assert!(grid
        .reachable_points8(Point::new(-1, 0), |_, tile| *tile == '.')
        .is_empty());
}

#[test]
fn random_walk_fill4_carves_floor_from_seed() {
    let mut grid = TileGrid::new(10, 10, '#');
    grid.random_walk_fill4(Point::new(5, 5), 100, '.', 42);

    // Start position should be floor.
    assert_eq!(grid.get(Point::new(5, 5)), Some(&'.'));
    // At least some floor tiles should exist.
    let floor_count = grid.tiles().iter().filter(|&&t| t == '.').count();
    assert!(
        floor_count > 10,
        "expected >10 floor tiles, got {floor_count}"
    );
    // All floor tiles should be connected (single random walk).
    let region = grid.flood_fill4(Point::new(5, 5), |_, &t| t == '.');
    assert_eq!(region.len(), floor_count);
}

#[test]
fn random_walk_fill4_is_reproducible_with_same_seed() {
    let mut grid1 = TileGrid::new(10, 10, '#');
    grid1.random_walk_fill4(Point::new(5, 5), 50, '.', 123);

    let mut grid2 = TileGrid::new(10, 10, '#');
    grid2.random_walk_fill4(Point::new(5, 5), 50, '.', 123);

    assert_eq!(grid1, grid2);
}

#[test]
fn random_walk_fill4_produces_different_results_with_different_seeds() {
    let mut grid1 = TileGrid::new(10, 10, '#');
    grid1.random_walk_fill4(Point::new(5, 5), 50, '.', 1);

    let mut grid2 = TileGrid::new(10, 10, '#');
    grid2.random_walk_fill4(Point::new(5, 5), 50, '.', 999);

    assert_ne!(grid1, grid2);
}

#[test]
fn random_walk_fill4_does_nothing_for_out_of_bounds_start() {
    let mut grid = TileGrid::new(5, 5, '#');
    grid.random_walk_fill4(Point::new(-1, -1), 100, '.', 42);
    assert!(grid.tiles().iter().all(|&t| t == '#'));
}

#[test]
fn is_line_of_sight_clear_returns_true_when_no_blockers() {
    let grid = TileGrid::from_vec(5, 1, vec!['.', '.', '.', '.', '.']).unwrap();
    assert!(grid.is_line_of_sight_clear(Point::new(0, 0), Point::new(4, 0), |t| *t == '#'));
}

#[test]
fn is_line_of_sight_clear_returns_false_when_blocked() {
    let grid = TileGrid::from_vec(5, 1, vec!['.', '.', '#', '.', '.']).unwrap();
    assert!(!grid.is_line_of_sight_clear(Point::new(0, 0), Point::new(4, 0), |t| *t == '#'));
}

#[test]
fn is_line_of_sight_clear_ignores_endpoints() {
    // The target tile itself is a wall, but LOS should still be clear
    // because the target is the thing being looked at.
    let grid = TileGrid::from_vec(3, 1, vec!['.', '.', '#']).unwrap();
    assert!(grid.is_line_of_sight_clear(Point::new(0, 0), Point::new(2, 0), |t| *t == '#'));
}

#[test]
fn is_line_of_sight_clear_returns_false_for_out_of_bounds() {
    let grid = TileGrid::from_vec(3, 3, vec!['.'; 9]).unwrap();
    assert!(!grid.is_line_of_sight_clear(Point::new(0, 0), Point::new(5, 5), |t| *t == '#'));
    assert!(!grid.is_line_of_sight_clear(Point::new(-1, 0), Point::new(2, 2), |t| *t == '#'));
}

#[test]
fn is_line_of_sight_clear_handles_same_point() {
    let grid = TileGrid::from_vec(3, 3, vec!['.'; 9]).unwrap();
    assert!(grid.is_line_of_sight_clear(Point::new(1, 1), Point::new(1, 1), |t| *t == '#'));
}

#[test]
fn is_line_of_sight_clear_works_for_diagonal_lines() {
    // 3x3 grid with a blocker at (1,1).
    let grid = TileGrid::from_vec(3, 3, vec!['.', '.', '.', '.', '#', '.', '.', '.', '.']).unwrap();
    // Diagonal from (0,0) to (2,2) passes through (1,1).
    assert!(!grid.is_line_of_sight_clear(Point::new(0, 0), Point::new(2, 2), |t| *t == '#'));
    // But (0,0) to (2,0) is clear.
    assert!(grid.is_line_of_sight_clear(Point::new(0, 0), Point::new(2, 0), |t| *t == '#'));
}

#[test]
fn bsp_dungeon_carves_rooms_and_corridors() {
    let mut grid = TileGrid::new(30, 20, '#');
    let centers = grid.generate_bsp_dungeon('#', '.', 3, 42);

    // BSP should produce multiple rooms.
    assert!(
        centers.len() >= 2,
        "expected >= 2 rooms, got {}",
        centers.len()
    );

    // All room centers should be floor.
    for &center in &centers {
        assert_eq!(grid.get(center), Some(&'.'));
    }

    // All rooms should be connected (single flood-fill from first center).
    let floor_count = grid.tiles().iter().filter(|&&t| t == '.').count();
    assert!(
        floor_count > 20,
        "expected >20 floor tiles, got {floor_count}"
    );
    let region = grid.flood_fill4(centers[0], |_, &t| t == '.');
    assert_eq!(
        region.len(),
        floor_count,
        "all floor tiles should be connected"
    );
}

#[test]
fn bsp_dungeon_is_reproducible_with_same_seed() {
    let mut grid1 = TileGrid::new(25, 25, '#');
    grid1.generate_bsp_dungeon('#', '.', 3, 99);

    let mut grid2 = TileGrid::new(25, 25, '#');
    grid2.generate_bsp_dungeon('#', '.', 3, 99);

    assert_eq!(grid1, grid2);
}

#[test]
fn bsp_dungeon_differs_with_different_seeds() {
    let mut grid1 = TileGrid::new(25, 25, '#');
    grid1.generate_bsp_dungeon('#', '.', 3, 1);

    let mut grid2 = TileGrid::new(25, 25, '#');
    grid2.generate_bsp_dungeon('#', '.', 3, 2);

    assert_ne!(grid1, grid2);
}

#[test]
fn bsp_dungeon_returns_empty_for_too_small_grid() {
    let mut grid = TileGrid::new(2, 2, '#');
    let centers = grid.generate_bsp_dungeon('#', '.', 3, 42);
    assert!(centers.is_empty());
}

#[test]
fn find_matching_returns_first_match() {
    let grid = TileGrid::from_vec(3, 2, vec!['#', '.', '.', '#', '.', '#']).unwrap();
    assert_eq!(grid.find_matching(|_, &t| t == '.'), Some(Point::new(1, 0)));
    assert_eq!(grid.find_matching(|_, &t| t == 'x'), None);
}

#[test]
fn points_matching_collects_all_matches_in_order() {
    let grid = TileGrid::from_vec(4, 1, vec!['#', '.', '#', '.']).unwrap();
    assert_eq!(
        grid.points_matching(|_, &t| t == '.'),
        vec![Point::new(1, 0), Point::new(3, 0)]
    );
}

#[test]
fn count_matching_returns_correct_count() {
    let grid = TileGrid::from_vec(5, 1, vec!['.', '.', '#', '.', '#']).unwrap();
    assert_eq!(grid.count_matching(|_, &t| t == '.'), 3);
    assert_eq!(grid.count_matching(|_, &t| t == '#'), 2);
    assert_eq!(grid.count_matching(|_, &t| t == 'x'), 0);
}

#[test]
fn density_returns_fraction() {
    let grid = TileGrid::from_vec(4, 1, vec!['.', '.', '#', '#']).unwrap();
    assert!((grid.density(|_, &t| t == '.') - 0.5).abs() < f32::EPSILON);
    assert!((grid.density(|_, &t| t == '#') - 0.5).abs() < f32::EPSILON);
    assert!((grid.density(|_, &t| t == 'x') - 0.0).abs() < f32::EPSILON);
}

#[test]
fn density_returns_zero_for_empty_grid() {
    let grid: TileGrid<char> = TileGrid::new(0, 0, '.');
    assert!((grid.density(|_, &t| t == '.') - 0.0).abs() < f32::EPSILON);
}

#[test]
fn bounding_box_of_returns_none_when_no_match() {
    let grid = TileGrid::from_vec(3, 3, vec!['.'; 9]).unwrap();
    assert!(grid.bounding_box_of(|_, &t| t == '#').is_none());
}

#[test]
fn bounding_box_of_returns_tight_rect() {
    // 5x5 grid with '#' at (1,1), (3,1), (1,3), (3,3).
    let mut grid = TileGrid::new(5, 5, '.');
    grid.set(Point::new(1, 1), '#');
    grid.set(Point::new(3, 1), '#');
    grid.set(Point::new(1, 3), '#');
    grid.set(Point::new(3, 3), '#');

    let bounds = grid.bounding_box_of(|_, &t| t == '#').unwrap();
    assert_eq!(bounds.x, 1);
    assert_eq!(bounds.y, 1);
    assert_eq!(bounds.width, 3);
    assert_eq!(bounds.height, 3);
}

#[test]
fn bounds_contains_and_center() {
    let b = Bounds::new(2, 3, 5, 7);
    assert_eq!(b.right(), 7);
    assert_eq!(b.bottom(), 10);
    assert!(b.contains(Point::new(2, 3)));
    assert!(b.contains(Point::new(6, 9)));
    assert!(!b.contains(Point::new(1, 3)));
    assert!(!b.contains(Point::new(7, 3)));
    assert_eq!(b.center(), Point::new(4, 6));
}

#[test]
fn bounds_clamp_point_handles_empty_and_out_of_range() {
    let empty = Bounds::new(0, 0, 0, 5);
    assert_eq!(empty.clamp_point(Point::new(3, 4)), None);

    let bounds = Bounds::new(2, 3, 4, 2);
    assert_eq!(bounds.clamp_point(Point::new(3, 4)), Some(Point::new(3, 4)));
    assert_eq!(
        bounds.clamp_point(Point::new(-5, 10)),
        Some(Point::new(2, 4))
    );
}

#[test]
fn bounds_intersects_detects_overlap() {
    let a = Bounds::new(0, 0, 4, 4);
    let b = Bounds::new(3, 3, 4, 4);
    assert!(a.intersects(b));
    assert!(b.intersects(a));
}

#[test]
fn bounds_intersection_returns_overlap() {
    let a = Bounds::new(0, 0, 4, 4);
    let b = Bounds::new(2, 1, 3, 4);
    assert_eq!(a.intersection(b), Some(Bounds::new(2, 1, 2, 3)));
}

#[test]
fn bounds_intersection_none_for_disjoint_or_empty() {
    let a = Bounds::new(0, 0, 4, 4);
    let b = Bounds::new(5, 5, 2, 2);
    assert!(!a.intersects(b));
    assert_eq!(a.intersection(b), None);

    let empty = Bounds::new(0, 0, 0, 4);
    assert!(!empty.intersects(a));
    assert_eq!(empty.intersection(a), None);
}

#[test]
fn rect_area_and_is_empty() {
    let r = Rect::new(1, 2, 3, 4);
    assert_eq!(r.area(), 12);
    assert!(!r.is_empty());

    let empty = Rect::new(0, 0, 0, 5);
    assert_eq!(empty.area(), 0);
    assert!(empty.is_empty());

    let empty2 = Rect::new(0, 0, 5, 0);
    assert!(empty2.is_empty());
}

#[test]
fn field_of_view_includes_origin() {
    let grid = TileGrid::from_vec(5, 5, vec!['.'; 25]).unwrap();
    let fov = grid.field_of_view(Point::new(2, 2), 10, |t| *t == '#');
    assert!(fov.contains(&Point::new(2, 2)));
}

#[test]
fn field_of_view_sees_all_in_open_area() {
    let grid = TileGrid::from_vec(5, 5, vec!['.'; 25]).unwrap();
    let fov = grid.field_of_view(Point::new(2, 2), 3, |t| *t == '#');
    // Within radius 3 from center, all tiles should be visible.
    for point in grid.points() {
        if point.manhattan_distance(Point::new(2, 2)) <= 3 {
            assert!(fov.contains(&point), "point {point:?} should be visible");
        }
    }
}

#[test]
fn field_of_view_blocks_behind_wall() {
    // 7x1 grid with a wall at position 3.
    let grid = TileGrid::from_vec(7, 1, vec!['.', '.', '.', '#', '.', '.', '.']).unwrap();

    let fov = grid.field_of_view(Point::new(0, 0), 6, |t| *t == '#');

    // Wall itself should be visible.
    assert!(fov.contains(&Point::new(3, 0)));
    // Tiles behind the wall should NOT be visible.
    assert!(!fov.contains(&Point::new(4, 0)));
    assert!(!fov.contains(&Point::new(5, 0)));
    assert!(!fov.contains(&Point::new(6, 0)));
}

#[test]
fn field_of_view_respects_radius() {
    let grid = TileGrid::from_vec(11, 11, vec!['.'; 121]).unwrap();
    let fov = grid.field_of_view(Point::new(5, 5), 2, |t| *t == '#');

    // All visible tiles should be within radius 2.
    for &point in &fov {
        assert!(
            point.manhattan_distance(Point::new(5, 5)) <= 2,
            "point {point:?} exceeds radius"
        );
    }
}

#[test]
fn field_of_view_returns_empty_for_out_of_bounds() {
    let grid = TileGrid::from_vec(5, 5, vec!['.'; 25]).unwrap();
    let fov = grid.field_of_view(Point::new(-1, -1), 5, |t| *t == '#');
    assert!(fov.is_empty());
}

#[test]
fn spatial_hash_insert_and_query() {
    let mut hash = SpatialHash::<u32>::new(3);
    hash.insert(Point::new(0, 0), 1);
    hash.insert(Point::new(1, 0), 2);
    hash.insert(Point::new(10, 10), 3);

    let nearby: Vec<u32> = hash.query(Point::new(0, 0), 2).copied().collect();
    assert_eq!(nearby.len(), 2);
    assert!(nearby.contains(&1));
    assert!(nearby.contains(&2));
    assert!(!nearby.contains(&3));
}

#[test]
fn spatial_hash_remove() {
    let mut hash = SpatialHash::<u32>::new(3);
    hash.insert(Point::new(0, 0), 1);
    hash.insert(Point::new(0, 0), 2);
    assert_eq!(hash.query(Point::new(0, 0), 1).count(), 2);

    hash.remove(Point::new(0, 0), &1);
    let nearby: Vec<u32> = hash.query(Point::new(0, 0), 1).copied().collect();
    assert_eq!(nearby, vec![2]);
}

#[test]
fn spatial_hash_cell_size_affects_grouping() {
    let mut hash = SpatialHash::<u32>::new(5);
    hash.insert(Point::new(0, 0), 1);
    hash.insert(Point::new(4, 4), 2); // Same cell (0,0) with cell_size=5
    hash.insert(Point::new(5, 5), 3); // Different cell (1,1)

    // Query with radius 8 to include (4,4) which is manhattan distance 8 from (0,0).
    let nearby: Vec<u32> = hash.query(Point::new(0, 0), 8).copied().collect();
    assert_eq!(nearby.len(), 2);
    assert!(nearby.contains(&1));
    assert!(nearby.contains(&2));
    assert!(!nearby.contains(&3));
}

#[test]
fn spatial_hash_handles_empty_query() {
    let hash = SpatialHash::<u32>::new(3);
    assert_eq!(hash.query(Point::new(0, 0), 5).count(), 0);
}

#[test]
fn spatial_hash_clear_removes_all() {
    let mut hash = SpatialHash::<u32>::new(3);
    hash.insert(Point::new(0, 0), 1);
    hash.insert(Point::new(100, 100), 2);
    hash.clear();
    assert_eq!(hash.query(Point::new(0, 0), 200).count(), 0);
}

#[test]
fn spatial_hash_len_counts_all_entries() {
    let mut hash = SpatialHash::<u32>::new(3);
    hash.insert(Point::new(0, 0), 1);
    hash.insert(Point::new(0, 0), 2);
    hash.insert(Point::new(10, 10), 3);
    assert_eq!(hash.len(), 3);
}

#[test]
fn spatial_hash_nearest_finds_closest() {
    let mut hash = SpatialHash::<Point>::new(3);
    let origin = Point::new(0, 0);
    let far = Point::new(10, 10);
    let near = Point::new(2, 0);
    hash.insert(origin, origin);
    hash.insert(far, far);
    hash.insert(near, near);

    // Find nearest to (5, 0). Expected: near (2,0) at distance 3, not origin (0,0) at distance 5.
    let center = Point::new(5, 0);
    let nearest = hash.nearest(center, 20, |a, b, c| {
        a.manhattan_distance(c).cmp(&b.manhattan_distance(c))
    });
    assert_eq!(nearest, Some(&near));
}

#[test]
fn spatial_hash_chebyshev_and_euclidean_queries() {
    let mut hash = SpatialHash::<u32>::new(3);
    // Put elements at different distances from center (0,0):
    // (1, 1): Manhattan = 2, Chebyshev = 1, Euclidean = 1.414
    // (2, 2): Manhattan = 4, Chebyshev = 2, Euclidean = 2.828
    // (0, 3): Manhattan = 3, Chebyshev = 3, Euclidean = 3.0
    hash.insert(Point::new(1, 1), 1);
    hash.insert(Point::new(2, 2), 2);
    hash.insert(Point::new(0, 3), 3);

    // Chebyshev query at (0,0) with radius 2.
    // Should include (1,1) (dist 1) and (2,2) (dist 2). Should NOT include (0,3) (dist 3).
    let cheb_nearby: Vec<u32> = hash.query_chebyshev(Point::new(0, 0), 2).copied().collect();
    assert_eq!(cheb_nearby.len(), 2);
    assert!(cheb_nearby.contains(&1));
    assert!(cheb_nearby.contains(&2));
    assert!(!cheb_nearby.contains(&3));

    // Euclidean query at (0,0) with radius 2.0.
    // Should include (1,1) (dist 1.414 <= 2.0). Should NOT include (2,2) (dist 2.828) or (0,3) (dist 3.0).
    let euc_nearby1: Vec<u32> = hash
        .query_euclidean(Point::new(0, 0), 2.0)
        .copied()
        .collect();
    assert_eq!(euc_nearby1, vec![1]);

    // Euclidean query at (0,0) with radius 3.0.
    // Should include (1,1), (2,2), and (0,3).
    let euc_nearby2: Vec<u32> = hash
        .query_euclidean(Point::new(0, 0), 3.0)
        .copied()
        .collect();
    assert_eq!(euc_nearby2.len(), 3);
    assert!(euc_nearby2.contains(&1));
    assert!(euc_nearby2.contains(&2));
    assert!(euc_nearby2.contains(&3));
}

#[test]
fn place_rooms_carves_rooms_on_wall_background() {
    let mut grid = TileGrid::from_vec(20, 20, vec!['.'; 400]).unwrap();
    let mut rng = Rng::seed(42);
    let centers = grid.place_rooms(5, 3, 6, || '#', || '.', &mut || rng.next_u64());

    assert!(!centers.is_empty());
    // Room centers should be within bounds.
    for c in &centers {
        assert!(grid.in_bounds(*c));
    }
    // At least one room center should have floor under it.
    let has_floor = centers.iter().any(|c| *grid.get(*c).unwrap() == '.');
    assert!(has_floor);
}

#[test]
fn place_rooms_returns_deterministic_results() {
    let mut grid1 = TileGrid::from_vec(20, 20, vec!['.'; 400]).unwrap();
    let mut grid2 = TileGrid::from_vec(20, 20, vec!['.'; 400]).unwrap();
    let mut rng1 = Rng::seed(99);
    let mut rng2 = Rng::seed(99);

    let c1 = grid1.place_rooms(3, 2, 5, || '#', || '.', &mut || rng1.next_u64());
    let c2 = grid2.place_rooms(3, 2, 5, || '#', || '.', &mut || rng2.next_u64());

    assert_eq!(c1, c2);
}

#[test]
fn place_rooms_respects_max_rooms() {
    let mut grid = TileGrid::from_vec(50, 50, vec!['.'; 2500]).unwrap();
    let mut rng = Rng::seed(42);
    let centers = grid.place_rooms(3, 3, 5, || '#', || '.', &mut || rng.next_u64());
    assert!(centers.len() <= 3);
}

#[test]
fn fill_rect_clips_to_bounds() {
    let mut grid = TileGrid::new(5, 5, '.');
    grid.fill_rect(1, 1, 3, 3, '#');

    assert_eq!(*grid.get(Point::new(0, 0)).unwrap(), '.');
    assert_eq!(*grid.get(Point::new(1, 1)).unwrap(), '#');
    assert_eq!(*grid.get(Point::new(3, 3)).unwrap(), '#');
    assert_eq!(*grid.get(Point::new(4, 4)).unwrap(), '.');
    assert_eq!(*grid.get(Point::new(2, 2)).unwrap(), '#');
}

#[test]
fn fill_rect_handles_negative_start() {
    let mut grid = TileGrid::new(3, 3, '.');
    grid.fill_rect(-1, -1, 4, 4, '#');

    // Should fill the entire 3x3 grid.
    for y in 0..3 {
        for x in 0..3 {
            assert_eq!(*grid.get(Point::new(x, y)).unwrap(), '#');
        }
    }
}

#[test]
fn map_in_place_transforms_all_tiles() {
    let mut grid = TileGrid::new(3, 2, 0);
    grid.set(Point::new(1, 0), 5);
    grid.set(Point::new(2, 1), 3);

    grid.map_in_place(|p, &tile| tile + p.x as i32 + p.y as i32);

    assert_eq!(*grid.get(Point::new(0, 0)).unwrap(), 0);
    assert_eq!(*grid.get(Point::new(1, 0)).unwrap(), 6);
    assert_eq!(*grid.get(Point::new(2, 0)).unwrap(), 2);
    assert_eq!(*grid.get(Point::new(0, 1)).unwrap(), 1);
    assert_eq!(*grid.get(Point::new(1, 1)).unwrap(), 2);
    assert_eq!(*grid.get(Point::new(2, 1)).unwrap(), 6);
}

#[test]
fn swap_exchanges_two_tiles() {
    let mut grid = TileGrid::new(3, 2, '.');
    grid.set(Point::new(0, 0), 'A');
    grid.set(Point::new(2, 1), 'B');

    assert!(grid.swap(Point::new(0, 0), Point::new(2, 1)));
    assert_eq!(*grid.get(Point::new(0, 0)).unwrap(), 'B');
    assert_eq!(*grid.get(Point::new(2, 1)).unwrap(), 'A');
}

#[test]
fn swap_returns_false_for_out_of_bounds() {
    let mut grid = TileGrid::new(2, 2, '.');
    assert!(!grid.swap(Point::new(0, 0), Point::new(5, 0)));
    assert!(!grid.swap(Point::new(5, 0), Point::new(0, 0)));
}

#[test]
fn swap_same_point_is_noop() {
    let mut grid = TileGrid::new(2, 2, '.');
    grid.set(Point::new(1, 1), 'X');
    assert!(grid.swap(Point::new(1, 1), Point::new(1, 1)));
    assert_eq!(*grid.get(Point::new(1, 1)).unwrap(), 'X');
}

#[test]
fn cellular_automata_carves_a_cave() {
    let mut grid = TileGrid::new(30, 20, '#');
    let floors = grid.cellular_automata_cave('#', '.', 0.45, 5, 4, 42);
    // A cave should have a meaningful number of floor tiles.
    assert!(floors > 0);
    // Borders should remain walls.
    for x in 0..30 {
        assert_eq!(*grid.get(Point::new(x, 0)).unwrap(), '#');
        assert_eq!(*grid.get(Point::new(x, 19)).unwrap(), '#');
    }
    for y in 0..20 {
        assert_eq!(*grid.get(Point::new(0, y)).unwrap(), '#');
        assert_eq!(*grid.get(Point::new(29, y)).unwrap(), '#');
    }
}

#[test]
fn cellular_automata_is_reproducible_with_same_seed() {
    let mut g1 = TileGrid::new(25, 15, '#');
    let mut g2 = TileGrid::new(25, 15, '#');
    let f1 = g1.cellular_automata_cave('#', '.', 0.42, 4, 4, 777);
    let f2 = g2.cellular_automata_cave('#', '.', 0.42, 4, 4, 777);
    assert_eq!(f1, f2);
    for y in 0..15 {
        for x in 0..25 {
            assert_eq!(g1.get(Point::new(x, y)), g2.get(Point::new(x, y)));
        }
    }
}

#[test]
fn cellular_automata_differs_with_different_seeds() {
    let mut g1 = TileGrid::new(25, 15, '#');
    let mut g2 = TileGrid::new(25, 15, '#');
    g1.cellular_automata_cave('#', '.', 0.45, 5, 4, 111);
    g2.cellular_automata_cave('#', '.', 0.45, 5, 4, 999);
    // With different seeds, at least some tiles should differ.
    let mut differs = false;
    for y in 0..15 {
        for x in 0..25 {
            if g1.get(Point::new(x, y)) != g2.get(Point::new(x, y)) {
                differs = true;
                break;
            }
        }
    }
    assert!(differs);
}

#[test]
fn cellular_automata_returns_empty_for_tiny_grid() {
    let mut grid = TileGrid::new(2, 2, '#');
    let floors = grid.cellular_automata_cave('#', '.', 0.45, 5, 4, 42);
    assert_eq!(floors, 0);
}

#[test]
fn from_ascii_parses_multiline_string() {
    let ascii = "\
#####
#...#
#.@.#
#...#
#####";
    let grid = TileGrid::from_ascii(ascii, |ch, _x, _y| ch);
    assert_eq!(grid.width(), 5);
    assert_eq!(grid.height(), 5);
    assert_eq!(*grid.get(Point::new(0, 0)).unwrap(), '#');
    assert_eq!(*grid.get(Point::new(2, 2)).unwrap(), '@');
    assert_eq!(*grid.get(Point::new(1, 1)).unwrap(), '.');
}

#[test]
fn from_ascii_handles_ragged_lines() {
    let ascii = "###\n#.\n#####";
    let grid = TileGrid::from_ascii(ascii, |ch, _x, _y| ch);
    assert_eq!(grid.width(), 5);
    assert_eq!(grid.height(), 3);
    // Short line is padded with space.
    assert_eq!(*grid.get(Point::new(3, 1)).unwrap(), ' ');
    assert_eq!(*grid.get(Point::new(0, 2)).unwrap(), '#');
}

#[test]
fn from_ascii_empty_input_produces_empty_grid() {
    let grid = TileGrid::from_ascii("", |ch, _x, _y| ch);
    assert_eq!(grid.width(), 0);
    assert_eq!(grid.height(), 0);
    assert!(grid.is_empty());
}

#[test]
fn from_ascii_receives_coordinates() {
    let ascii = "AB\nCD";
    let mut coords = Vec::new();
    let _grid = TileGrid::from_ascii(ascii, |ch, x, y| {
        coords.push((ch, x, y));
        ch
    });
    assert_eq!(
        coords,
        vec![('A', 0, 0), ('B', 1, 0), ('C', 0, 1), ('D', 1, 1)]
    );
}

#[test]
fn from_fn_creates_grid_from_closure() {
    let grid: TileGrid<i32> = TileGrid::from_fn(3, 2, |x, y| (x + y) as i32);
    assert_eq!(grid.width(), 3);
    assert_eq!(grid.height(), 2);
    assert_eq!(*grid.get(Point::new(0, 0)).unwrap(), 0);
    assert_eq!(*grid.get(Point::new(2, 0)).unwrap(), 2);
    assert_eq!(*grid.get(Point::new(1, 1)).unwrap(), 2);
}

#[test]
fn from_fn_zero_dimensions_produces_empty_grid() {
    let grid: TileGrid<bool> = TileGrid::from_fn(0, 0, |_, _| true);
    assert!(grid.is_empty());
}

#[test]
fn map_tiles_transforms_to_different_type() {
    let grid = TileGrid::from_ascii("#.@", |ch, _x, _y| ch);
    let mapped: TileGrid<u8> = grid.map_tiles(|_, &ch| match ch {
        '#' => 1,
        '.' => 2,
        '@' => 3,
        _ => 0,
    });
    assert_eq!(mapped.width(), 3);
    assert_eq!(mapped.height(), 1);
    assert_eq!(*mapped.get(Point::new(0, 0)).unwrap(), 1);
    assert_eq!(*mapped.get(Point::new(1, 0)).unwrap(), 2);
    assert_eq!(*mapped.get(Point::new(2, 0)).unwrap(), 3);
}

#[test]
fn map_tiles_preserves_dimensions() {
    let grid = TileGrid::new(4, 3, 10u32);
    let mapped: TileGrid<bool> = grid.map_tiles(|_, &v| v > 5);
    assert_eq!(mapped.width(), 4);
    assert_eq!(mapped.height(), 3);
    assert!(*mapped.get(Point::new(0, 0)).unwrap());
}

#[test]
fn crop_extracts_sub_region() {
    let ascii = "ABCDE\nFGHIJ\nKLMNO";
    let grid = TileGrid::from_ascii(ascii, |ch, _x, _y| ch);
    let cropped = grid.crop(1, 1, 3, 2, 'X');
    assert_eq!(cropped.width(), 3);
    assert_eq!(cropped.height(), 2);
    assert_eq!(*cropped.get(Point::new(0, 0)).unwrap(), 'G');
    assert_eq!(*cropped.get(Point::new(1, 0)).unwrap(), 'H');
    assert_eq!(*cropped.get(Point::new(2, 0)).unwrap(), 'I');
    assert_eq!(*cropped.get(Point::new(0, 1)).unwrap(), 'L');
    assert_eq!(*cropped.get(Point::new(1, 1)).unwrap(), 'M');
    assert_eq!(*cropped.get(Point::new(2, 1)).unwrap(), 'N');
}

#[test]
fn crop_out_of_bounds_uses_fill() {
    let grid = TileGrid::new(3, 3, 'A');
    let cropped = grid.crop(-1, -1, 3, 3, 'X');
    assert_eq!(cropped.width(), 3);
    assert_eq!(cropped.height(), 3);
    // Top-left corner is out of bounds → fill.
    assert_eq!(*cropped.get(Point::new(0, 0)).unwrap(), 'X');
    // (1,1) in cropped maps to (0,0) in source → 'A'.
    assert_eq!(*cropped.get(Point::new(1, 1)).unwrap(), 'A');
    assert_eq!(*cropped.get(Point::new(2, 2)).unwrap(), 'A');
}

#[test]
fn crop_full_grid_returns_clone() {
    let ascii = "#.@";
    let grid = TileGrid::from_ascii(ascii, |ch, _x, _y| ch);
    let cropped = grid.crop(0, 0, 3, 1, 'X');
    assert_eq!(cropped, grid);
}

#[test]
fn crop_zero_dimensions_returns_empty() {
    let grid = TileGrid::new(5, 5, 0u32);
    let cropped = grid.crop(0, 0, 0, 0, 99);
    assert_eq!(cropped.width(), 0);
    assert_eq!(cropped.height(), 0);
    assert!(cropped.is_empty());
}

#[test]
fn point_display() {
    assert_eq!(format!("{}", Point::new(3, 5)), "3,5");
    assert_eq!(format!("{}", Point::new(-1, 0)), "-1,0");
    assert_eq!(format!("{}", Point::ZERO), "0,0");
}

#[test]
fn point_from_tuple_roundtrip() {
    let p = Point::from((7, -3));
    assert_eq!(p, Point::new(7, -3));
    let (x, y): (i16, i16) = p.into();
    assert_eq!((x, y), (7, -3));
}

#[test]
fn direction_display() {
    assert_eq!(format!("{}", Direction::North), "North");
    assert_eq!(format!("{}", Direction::East), "East");
}

#[test]
fn direction_from_into_direction8() {
    let d8: Direction8 = Direction::South.into();
    assert_eq!(d8, Direction8::South);
}

#[test]
fn direction8_display() {
    assert_eq!(format!("{}", Direction8::NorthEast), "NE");
    assert_eq!(format!("{}", Direction8::SouthWest), "SW");
}

#[test]
fn direction8_try_into_direction_cardinal() {
    let d: Direction = Direction8::East.try_into().unwrap();
    assert_eq!(d, Direction::East);
}

#[test]
fn direction8_try_into_direction_diagonal_fails() {
    let result: Result<Direction, ()> = Direction8::NorthEast.try_into();
    assert!(result.is_err());
}

#[test]
fn size_display() {
    assert_eq!(format!("{}", Size::new(80, 24)), "80x24");
}

#[test]
fn size_from_tuple_roundtrip() {
    let s = Size::from((10, 20));
    assert_eq!(s, Size::new(10, 20));
    let (w, h): (u16, u16) = s.into();
    assert_eq!((w, h), (10, 20));
}

#[test]
fn bounds_display() {
    let b = Bounds::new(10, 5, 30, 20);
    assert_eq!(format!("{}", b), "(10,5 30x20)");
}

#[test]
fn test_tilegrid_raycast() {
    let grid = TileGrid::new(5, 5, '.');
    let path = grid.raycast(Point::new(0, 0), Point::new(3, 3));
    assert_eq!(path.len(), 4);
    assert_eq!(path[0], Point::new(0, 0));
    assert_eq!(path[3], Point::new(3, 3));
}

#[test]
fn test_tilegrid_raycast_opaque() {
    let ascii = "\
.....
.###.
.....";
    let grid = TileGrid::from_ascii(ascii, |ch, _, _| ch);
    let (path, blocked) =
        grid.raycast_opaque(Point::new(0, 1), Point::new(4, 1), |_, &tile| tile == '#');
    assert!(blocked);
    assert_eq!(path.len(), 2);
    assert_eq!(path[0], Point::new(0, 1));
    assert_eq!(path[1], Point::new(1, 1));
}

#[test]
fn test_tilegrid_flood_fill() {
    let closed_ascii = "\
###
#.#
###";
    let closed_grid = TileGrid::from_ascii(closed_ascii, |ch, _, _| ch);
    let filled = closed_grid.flood_fill(Point::new(1, 1), |_, &tile| tile == '.');
    assert_eq!(filled.len(), 1);
    assert!(filled.contains(&Point::new(1, 1)));

    let open_ascii = "\
...
..#
###";
    let open_grid = TileGrid::from_ascii(open_ascii, |ch, _, _| ch);
    let open_filled = open_grid.flood_fill4(Point::new(0, 0), |_, &tile| tile == '.');
    assert_eq!(open_filled.len(), 5);
}

#[test]
fn test_dijkstra_map() {
    let passable = |p: Point| !(p.x == 1 && p.y == 1);

    let map = DijkstraMap::compute(3, 3, &[Point::new(0, 0)], passable, false);
    assert_eq!(map.get(Point::new(0, 0)), Some(0));
    assert_eq!(map.get(Point::new(1, 0)), Some(1));
    assert_eq!(map.get(Point::new(2, 0)), Some(2));
    assert_eq!(map.get(Point::new(0, 1)), Some(1));
    assert_eq!(map.get(Point::new(1, 1)), None);
    assert_eq!(map.get(Point::new(2, 1)), Some(3));
    assert_eq!(map.get(Point::new(0, 2)), Some(2));
    assert_eq!(map.get(Point::new(1, 2)), Some(3));
    assert_eq!(map.get(Point::new(2, 2)), Some(4));

    let chase = map.chase_direction(Point::new(2, 2), false);
    assert!(chase == Some(Point::new(1, 2)) || chase == Some(Point::new(2, 1)));

    let flee = map.flee_direction(Point::new(0, 0), false);
    assert!(flee == Some(Point::new(1, 0)) || flee == Some(Point::new(0, 1)));
}

#[test]
fn test_dijkstra_map_path_to() {
    let passable = |p: Point| !(p.x == 1 && p.y == 1);
    let map = DijkstraMap::compute(3, 3, &[Point::new(0, 0)], passable, false);

    // Start from (2, 2) and path to (0, 0)
    let path = map.path_to(Point::new(2, 2), false);
    assert_eq!(path.len(), 5);
    assert_eq!(path[0], Point::new(2, 2));
    assert_eq!(path[4], Point::new(0, 0));

    // Start from an unreachable point (1, 1) which is not passable
    let path_unreachable = map.path_to(Point::new(1, 1), false);
    assert!(path_unreachable.is_empty());
}

#[test]
fn test_astar_pathfinding() {
    let grid = TileGrid::new(5, 5, '.');
    let start = Point::new(0, 0);
    let goal = Point::new(4, 4);

    // Standard 4-directional A*
    let path4 = grid.astar4(start, goal, |_, _| true).unwrap();
    assert_eq!(path4.first().copied(), Some(start));
    assert_eq!(path4.last().copied(), Some(goal));
    assert_eq!(path4.len(), 9); // 4 down + 4 right + 1 start = 9 points

    // Standard 8-directional A*
    let path8 = grid.astar8(start, goal, |_, _| true).unwrap();
    assert_eq!(path8.first().copied(), Some(start));
    assert_eq!(path8.last().copied(), Some(goal));
    assert_eq!(path8.len(), 5); // 4 diagonals = 5 points

    // Test with obstacles
    let passable = |p: Point, _tile: &char| {
        p != Point::new(1, 1) && p != Point::new(1, 0) && p != Point::new(0, 1)
    };
    let blocked_path = grid.astar4(start, goal, passable);
    assert!(blocked_path.is_none());
}

#[test]
fn test_maze_generation() {
    let mut grid = TileGrid::new(11, 11, '#');
    let seed = 42u64;
    grid.generate_maze('#', '.', seed);

    // Bounds should still contain walls
    for x in 0..11 {
        assert_eq!(grid.get(Point::new(x, 0)).copied(), Some('#'));
        assert_eq!(grid.get(Point::new(x, 10)).copied(), Some('#'));
    }
    for y in 0..11 {
        assert_eq!(grid.get(Point::new(0, y)).copied(), Some('#'));
        assert_eq!(grid.get(Point::new(10, y)).copied(), Some('#'));
    }

    // Inside should contain at least some paths '.'
    let path_count = grid.tiles().iter().filter(|&&t| t == '.').count();
    assert!(path_count > 10);
}

#[test]
fn test_dijkstra_range_methods() {
    let passable = |_p: Point| true;
    let map = DijkstraMap::compute(5, 5, &[Point::new(2, 2)], passable, false);

    // Test find_all_within_range
    let within_2 = map.find_all_within_range(2);
    // Distance 0: (2,2) -> 1 cell
    // Distance 1: (2,1), (2,3), (1,2), (3,2) -> 4 cells
    // Distance 2: (2,0), (2,4), (0,2), (4,2), (1,1), (3,1), (1,3), (3,3) -> 8 cells
    // Total cells at distance <= 2 should be 1 + 4 + 8 = 13
    assert_eq!(within_2.len(), 13);
    for &(p, dist) in &within_2 {
        assert!(dist <= 2);
        assert_eq!(map.get(p), Some(dist));
    }

    // Test chase_path_to_range (from far away: 4,4 has distance 4 from 2,2)
    let path = map
        .chase_path_to_range(Point::new(4, 4), 1, 2, false)
        .unwrap();
    assert!(!path.is_empty());
    let end = *path.last().unwrap();
    let end_dist = map.get(end).unwrap();
    assert!(end_dist >= 1 && end_dist <= 2);

    // Test chase_path_to_range (too close: 2,2 has distance 0, min_range=1)
    let path_flee = map
        .chase_path_to_range(Point::new(2, 2), 1, 2, false)
        .unwrap();
    assert!(!path_flee.is_empty());
    let end_flee = *path_flee.last().unwrap();
    let end_flee_dist = map.get(end_flee).unwrap();
    assert!(end_flee_dist >= 1 && end_flee_dist <= 2);
}
