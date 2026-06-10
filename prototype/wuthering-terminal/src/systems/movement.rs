use crate::components::{Position, Team};
use crate::map::{TacticalMap, Tile};

use verryte_core::World;

pub fn visibility_system(world: &mut World) {
    let player_positions: Vec<Position> = world
        .query2::<Position, Team>()
        .iter()
        .filter(|(_, _, team)| **team == Team::Player)
        .map(|(_, pos, _)| **pos)
        .collect();

    let map_tiles = world
        .resource::<TacticalMap>()
        .expect("TacticalMap resource must be registered")
        .tiles
        .clone();

    let darkness_reduction = super::environment::floor_modifier_visibility_reduction(world);
    let base_radius = 8i32;
    let radius = (base_radius - darkness_reduction).max(2) as u16;

    let visibility = world
        .resource_mut::<verryte_map::VisibilityMap>()
        .expect("VisibilityMap resource must be registered");

    visibility.clear_visible();

    for pos in player_positions {
        visibility.compute_fov_incremental(pos, radius, |p| {
            map_tiles
                .get(p)
                .map(|t| matches!(t, Tile::Wall))
                .unwrap_or(true)
        });
    }
}

pub fn is_occupied_except(world: &World, pos: Position, except: verryte_core::Entity) -> bool {
    for (e, p) in world.query::<Position>() {
        if e != except && *p == pos && world.get::<Team>(e).is_some() {
            return true;
        }
    }
    false
}

pub fn get_tile_dimensions(_world: &World) -> (u16, u16) {
    let (term_w, term_h) = verryte_tty::terminal_size();
    let tier = verryte_terminal::ResolutionTier::from_size(term_w, term_h);
    tier.tile_dimensions()
}

pub fn get_tile_center_pixels(world: &World, pos: Position) -> (f32, f32) {
    let (tile_w, tile_h) = get_tile_dimensions(world);
    let cx = pos.x as f32 * tile_w as f32 + (tile_w as f32 / 2.0);
    let cy = pos.y as f32 * tile_h as f32 + (tile_h as f32 / 2.0);
    (cx, cy)
}

pub fn play_spatial_sfx(world: &mut World, name: &str, emitter_pos: Position) {
    let listener = world
        .resource::<crate::components::GameState>()
        .map(|s| s.cursor)
        .unwrap_or(Position::new(12, 8));
    let dx = (emitter_pos.x - listener.x) as f32;
    let dy = (emitter_pos.y - listener.y) as f32;
    let dist = (dx * dx + dy * dy).sqrt();
    let max_range = 15.0_f32;
    let volume = (1.0 - (dist / max_range)).clamp(0.0, 1.0);
    let pan = if max_range > 0.0 {
        (dx / max_range).clamp(-1.0, 1.0)
    } else {
        0.0
    };
    if let Some(events) = world.resource_mut::<verryte_core::Events<verryte_core::AudioEvent>>() {
        events.send(
            verryte_core::AudioEvent::play(name)
                .with_volume(volume)
                .with_pan(pan),
        );
    }
}
