use crate::components::{
    incursion_shape, ActiveFloorModifiers, CharacterClass, DynamicFloorEvents, FloorEventKind,
    FloorEventRecord, FloorEventResponse, FloorModifier, GameEvent, GameState,
    IncursionFirstAttackDisrupted, IncursionMiniBoss, Inventory, Item, ItemEffect,
    PendingFloorEvent, Position, Stats, Team, TurnPhase, Weather, WeatherType,
};
use crate::game::Game;
use crate::map::{TacticalMap, Tile};
use crate::spawn::Spawner;

use verryte_core::{Entity, Events, Rng, World};
use verryte_terminal::{vfx::VfxSystem, Color};

use super::combat::{apply_heal, handle_defeat, log};
use super::movement::get_tile_center_pixels;

pub fn weather_ambient_system(world: &mut World) {
    let (current, already_started) = world
        .resource::<Weather>()
        .map(|weather| (weather.current, weather.ambient_started_for))
        .unwrap_or((WeatherType::Sunny, None));
    if already_started == Some(current) {
        return;
    }

    let ambient_name = match current {
        WeatherType::Rainy => "ambient_rain",
        WeatherType::LightningStorm => "ambient_thunder",
        WeatherType::Sunny => "ambient_birds",
        WeatherType::Snowing => "ambient_wind",
    };
    let volume: f32 = match current {
        WeatherType::Rainy => 0.4,
        WeatherType::LightningStorm => 0.5,
        WeatherType::Sunny => 0.3,
        WeatherType::Snowing => 0.35,
    };

    if let Some(events) = world.resource_mut::<Events<verryte_core::AudioEvent>>() {
        events.send(verryte_core::AudioEvent::loop_music(ambient_name).with_volume(volume));
        if let Some(weather) = world.resource_mut::<Weather>() {
            weather.ambient_started_for = Some(current);
        }
    }
}

pub const WEATHER_CYCLE_TURNS: u32 = 3;

pub fn weather_cycle_system(world: &mut World) {
    let turn = world.resource::<GameState>().map(|s| s.turn).unwrap_or(1);
    let (current, last_effect_turn) = world
        .resource::<Weather>()
        .map(|weather| (weather.current, weather.last_effect_turn))
        .unwrap_or((WeatherType::Sunny, None));
    if last_effect_turn == Some(turn) {
        return;
    }

    let next = if turn != 1 && turn % WEATHER_CYCLE_TURNS == 1 {
        match current {
            WeatherType::Sunny => WeatherType::Rainy,
            WeatherType::Rainy => WeatherType::LightningStorm,
            WeatherType::LightningStorm => WeatherType::Snowing,
            WeatherType::Snowing => WeatherType::Sunny,
        }
    } else {
        current
    };
    if let Some(weather) = world.resource_mut::<Weather>() {
        weather.current = next;
        weather.last_effect_turn = Some(turn);
        if next != current {
            weather.danger_zones.clear();
            weather.ambient_started_for = None;
        }
    }
    apply_per_turn_weather_effects(world, next);

    if next == current {
        return;
    }
    log(
        world,
        format!("[fg:87CEEB][b]Weather changed to {:?}![/][/fg]", next),
    );

    // Trigger visual/audio feedback for weather transitions
    if let Some(events) = world.resource_mut::<Events<verryte_core::AudioEvent>>() {
        events.send(verryte_core::AudioEvent::play("cleanse")); // Weather change blip
    }

    if let Some(vfx) = world.resource_mut::<VfxSystem>() {
        let cx = 40.0;
        let cy = 12.0; // Viewport center approximations
        match next {
            WeatherType::LightningStorm => {
                vfx.flashes.push(verryte_terminal::vfx::Flash::full_screen(
                    Color(255, 255, 220),
                    0.25,
                ));
                vfx.shakes
                    .push(verryte_terminal::vfx::ScreenShake::new(3.0, 0.3));
            }
            WeatherType::Snowing => {
                vfx.particles.extend(verryte_terminal::vfx::emit_burst(
                    cx,
                    cy,
                    25,
                    Color(200, 220, 255),
                    &['❄', '*', '·'],
                ));
            }
            WeatherType::Rainy => {
                vfx.particles.extend(verryte_terminal::vfx::emit_burst(
                    cx,
                    cy,
                    20,
                    Color(100, 160, 255),
                    &['│', '¦', '·'],
                ));
            }
            WeatherType::Sunny => {
                vfx.particles.extend(verryte_terminal::vfx::emit_burst(
                    cx,
                    cy,
                    15,
                    Color(255, 230, 120),
                    &['·', '°', '∘'],
                ));
            }
        }
    }
}

fn apply_per_turn_weather_effects(world: &mut World, weather: WeatherType) {
    match weather {
        WeatherType::LightningStorm => {
            if let Some(w) = world.resource_mut::<Weather>() {
                w.danger_zones.clear();
            }

            let map_w;
            let map_h;
            {
                let map = world
                    .resource::<TacticalMap>()
                    .expect("TacticalMap must be registered");
                map_w = map.width as i16;
                map_h = map.height as i16;
            }

            let count = {
                let rng = world.resource_mut::<Rng>().expect("Rng must be registered");
                2 + rng.next_u32(3) as usize
            };

            let mut zones = Vec::new();
            for _ in 0..count {
                let (x, y) = {
                    let rng = world.resource_mut::<Rng>().expect("Rng must be registered");
                    let x = rng.next_u32(map_w.max(1) as u32) as i16;
                    let y = rng.next_u32(map_h.max(1) as u32) as i16;
                    (x, y)
                };
                let pos = Position::new(x, y);
                let walkable = world
                    .resource::<TacticalMap>()
                    .map(|m| m.is_walkable(pos))
                    .unwrap_or(false);
                if walkable && !zones.contains(&pos) {
                    zones.push(pos);
                }
            }

            if let Some(w) = world.resource_mut::<Weather>() {
                w.danger_zones = zones.clone();
            }
        }
        WeatherType::Rainy => {
            if let Some(w) = world.resource_mut::<Weather>() {
                w.danger_zones.clear();
            }

            let mut puddle_zones = Vec::new();
            let map = world
                .resource::<TacticalMap>()
                .expect("TacticalMap must be registered");
            let map_w = map.width as i16;
            let map_h = map.height as i16;

            for ty in 0..map_h {
                for tx in 0..map_w {
                    let tile = map.tile(tx, ty);
                    if tile != Tile::Grass {
                        continue;
                    }
                    let pos = Position::new(tx, ty);
                    let mut adjacent_to_water = false;
                    for (dx, dy) in &[(0i16, -1i16), (0, 1), (-1, 0), (1, 0)] {
                        let nx = tx + dx;
                        let ny = ty + dy;
                        if nx >= 0
                            && nx < map_w
                            && ny >= 0
                            && ny < map_h
                            && map.tile(nx, ny) == Tile::Water
                        {
                            adjacent_to_water = true;
                            break;
                        }
                    }
                    if adjacent_to_water {
                        puddle_zones.push(pos);
                    }
                }
            }

            if let Some(w) = world.resource_mut::<Weather>() {
                w.danger_zones = puddle_zones;
            }
        }
        WeatherType::Snowing => {
            if let Some(w) = world.resource_mut::<Weather>() {
                w.danger_zones.clear();
            }

            let mut expanded = Vec::new();
            let map_snapshot: Vec<(i16, i16, Tile)> = {
                let map = world
                    .resource::<TacticalMap>()
                    .expect("TacticalMap must be registered");
                let map_w = map.width as i16;
                let map_h = map.height as i16;
                let mut tiles = Vec::new();
                for ty in 0..map_h {
                    for tx in 0..map_w {
                        tiles.push((tx, ty, map.tile(tx, ty)));
                    }
                }
                tiles
            };

            for (tx, ty, tile) in &map_snapshot {
                if *tile != Tile::Grass {
                    continue;
                }
                let mut adjacent_to_ice = false;
                for (dx, dy) in &[(0i16, -1i16), (0, 1), (-1, 0), (1, 0)] {
                    let nx = tx + dx;
                    let ny = ty + dy;
                    if let Some((_, _, t)) =
                        map_snapshot.iter().find(|(x, y, _)| *x == nx && *y == ny)
                    {
                        if *t == Tile::Ice {
                            adjacent_to_ice = true;
                            break;
                        }
                    }
                }
                if adjacent_to_ice {
                    let pos = Position::new(*tx, *ty);
                    expanded.push(pos);
                }
            }

            if let Some(map) = world.resource_mut::<TacticalMap>() {
                for pos in &expanded {
                    map.tiles.set(*pos, Tile::Ice);
                }
            }

            if let Some(w) = world.resource_mut::<Weather>() {
                w.danger_zones = expanded;
            }

            let mut ice_targets = Vec::new();
            for (e, class) in world.query::<CharacterClass>() {
                if let Some(status) = world.get::<crate::components::ElementalStatus>(e) {
                    if matches!(status, crate::components::ElementalStatus::Ice { .. }) {
                        ice_targets.push((e, *class));
                    }
                }
            }
            for (entity, class) in ice_targets {
                let mut log_msg = None;
                if let Some(crate::components::ElementalStatus::Ice { ref mut duration }) =
                    world.get_mut::<crate::components::ElementalStatus>(entity)
                {
                    *duration += 1;
                    let new_dur = *duration;
                    let name = Game::get_class_name(class);
                    log_msg = Some(format!(
                        "[fg:64C8FF]Snowing extends Ice on {} by 1 turn (now {}).[/fg]",
                        name, new_dur
                    ));
                }
                if let Some(msg) = log_msg {
                    log(world, msg);
                }
            }
        }
        _ => {}
    }
}

pub fn apply_weather_hazard_damage(world: &mut World, affected_team: Team) {
    let (weather, zones) = {
        let w = world
            .resource::<Weather>()
            .expect("Weather must be registered");
        (w.current, w.danger_zones.clone())
    };

    if weather != WeatherType::LightningStorm || zones.is_empty() {
        return;
    }

    for zone in &zones {
        let mut victims = Vec::new();
        for (e, p, team) in world.query2::<Position, Team>() {
            if *team == affected_team && *p == *zone {
                victims.push(e);
            }
        }
        for victim in victims {
            let victim_class = world
                .get::<CharacterClass>(victim)
                .copied()
                .unwrap_or(CharacterClass::Warrior);
            let victim_name = Game::get_class_name(victim_class);
            let mut final_hp = 0;
            let mut defeated = false;
            if let Some(stats) = world.get_mut::<Stats>(victim) {
                stats.hp -= 15;
                final_hp = stats.hp;
                if stats.hp <= 0 {
                    defeated = true;
                }
            }
            log(
                world,
                format!(
                    "[fg:FFFF64]Weather hazard: lightning struck {} at ({}, {}) for 15 damage! (HP: {})[/fg]",
                    victim_name, zone.x, zone.y, final_hp
                ),
            );
            if let Some(events) = world.resource_mut::<Events<GameEvent>>() {
                events.send(GameEvent::WeatherHazardResolved {
                    weather,
                    target: victim,
                    team: affected_team,
                    position: *zone,
                    damage: 15,
                });
            }
            let (cx, cy) = get_tile_center_pixels(world, *zone);
            if let Some(vfx) = world.resource_mut::<VfxSystem>() {
                vfx.particles
                    .extend(verryte_terminal::vfx::emit_lightning(cx, cy, cx, cy));
                vfx.floating_texts
                    .push(verryte_terminal::vfx::FloatingText::new(
                        cx,
                        cy - 2.0,
                        "-15",
                        Color(255, 255, 100),
                        true,
                    ));
            }
            if defeated {
                let name_str = victim_name.to_string();
                handle_defeat(world, victim, &name_str, victim_class, *zone);
            }
        }
    }
}

pub fn floor_modifier_system(world: &mut World) {
    let (turn, phase) = world
        .resource::<GameState>()
        .map(|state| (state.turn, state.phase))
        .unwrap_or((1, TurnPhase::Player));
    if phase != TurnPhase::Player {
        return;
    }

    let modifiers = {
        let Some(active) = world.resource_mut::<ActiveFloorModifiers>() else {
            return;
        };
        if active.modifiers.is_empty() || active.last_processed_turn == Some(turn) {
            return;
        }
        active.turns_remaining.resize(active.modifiers.len(), 1);
        active.turns_remaining.truncate(active.modifiers.len());
        active.last_processed_turn = Some(turn);
        active.clone()
    };

    for modifier in modifiers.modifiers.iter() {
        match modifier {
            FloorModifier::ElementalStorm => {
                let damage = {
                    let rng = world.resource_mut::<Rng>().expect("Rng registered");
                    5 + rng.next_u32(6) as i32
                };
                let mut targets = Vec::new();
                for (e, _pos, _team) in world.query2::<Position, Team>() {
                    targets.push(e);
                }
                for entity in targets {
                    if !world.is_alive(entity) {
                        continue;
                    }
                    let entity_class = world
                        .get::<CharacterClass>(entity)
                        .copied()
                        .unwrap_or(CharacterClass::Warrior);
                    let entity_name = Game::get_class_name(entity_class);
                    let mut final_hp = 0;
                    let mut defeated = false;
                    if let Some(stats) = world.get_mut::<Stats>(entity) {
                        stats.hp -= damage;
                        final_hp = stats.hp;
                        if stats.hp <= 0 {
                            defeated = true;
                        }
                    }
                    log(
                        world,
                        format!(
                            "[fg:FF6464]Elemental Storm deals {} damage to {}! (HP: {})[/fg]",
                            damage, entity_name, final_hp
                        ),
                    );
                    if defeated {
                        let name_str = entity_name.to_string();
                        let entity_pos = world
                            .get::<Position>(entity)
                            .copied()
                            .unwrap_or(Position::new(0, 0));
                        handle_defeat(world, entity, &name_str, entity_class, entity_pos);
                    }
                }
            }
            FloorModifier::Reversal => {
                // Reversal is applied per-turn via healing/damage checks (handled in game.rs)
            }
            _ => {}
        }
    }

    let mut expired_names = Vec::new();
    if let Some(modifiers) = world.resource_mut::<ActiveFloorModifiers>() {
        for i in 0..modifiers.turns_remaining.len() {
            if modifiers.turns_remaining[i] > 0 {
                modifiers.turns_remaining[i] -= 1;
            }
        }
        let mut to_remove = Vec::new();
        for i in (0..modifiers.modifiers.len()).rev() {
            if modifiers.turns_remaining[i] == 0 {
                expired_names.push(modifiers.modifiers[i].display_name().to_string());
                to_remove.push(i);
            }
        }
        for i in to_remove {
            modifiers.modifiers.remove(i);
            modifiers.turns_remaining.remove(i);
        }
    }
    for name in &expired_names {
        log(
            world,
            format!("[fg:808080]Floor modifier '{}' has expired.[/fg]", name),
        );
    }
}

pub fn select_floor_modifiers(world: &mut World) {
    let all = FloorModifier::all();
    let count = {
        let rng = world.resource_mut::<Rng>().expect("Rng registered");
        1 + rng.next_u32(2) as usize
    };

    let mut chosen = Vec::new();
    let mut chosen_durations = Vec::new();

    {
        let rng = world.resource_mut::<Rng>().expect("Rng registered");
        for _ in 0..count {
            let idx = rng.next_u32(all.len() as u32) as usize;
            let modifier = all[idx].clone();
            if !chosen.contains(&modifier) {
                let duration = 3 + rng.next_u32(6);
                chosen.push(modifier);
                chosen_durations.push(duration);
            }
        }
    }

    if chosen.is_empty() {
        let rng = world.resource_mut::<Rng>().expect("Rng registered");
        let idx = rng.next_u32(all.len() as u32) as usize;
        let duration = 3 + rng.next_u32(6);
        chosen.push(all[idx].clone());
        chosen_durations.push(duration);
    }

    let names: Vec<String> = chosen
        .iter()
        .map(|m| m.display_name().to_string())
        .collect();
    let desc: Vec<String> = chosen
        .iter()
        .zip(chosen_durations.iter())
        .map(|(m, d)| format!("{} ({} turns)", m.display_name(), d))
        .collect();

    if let Some(modifiers) = world.resource_mut::<ActiveFloorModifiers>() {
        modifiers.modifiers = chosen;
        modifiers.turns_remaining = chosen_durations;
        modifiers.last_processed_turn = None;
    }

    log(
        world,
        format!(
            "[fg:FF00FF][b]Floor Modifiers Active:[/] {}[/fg]",
            desc.join(", ")
        ),
    );

    for modifier_name in &names {
        match modifier_name.as_str() {
            "Darkness" => {
                log(
                    world,
                    "[fg:404040]Darkness reduces visibility radius by 2.[/fg]",
                );
            }
            "Gravity Well" => {
                log(
                    world,
                    "[fg:8B4513]Gravity Well increases all movement costs by 1.[/fg]",
                );
            }
            "Elemental Storm" => {
                log(
                    world,
                    "[fg:FF4500]Elemental Storm deals 5-10 random damage each turn to all entities.[/fg]",
                );
            }
            "Healing Surge" => {
                log(
                    world,
                    "[fg:00FF7F]Healing Surge doubles all healing amounts.[/fg]",
                );
            }
            "Frenzy" => {
                log(
                    world,
                    "[fg:FF0000]Frenzy grants +2 ATK and -1 DEF to all entities.[/fg]",
                );
                apply_frenzy_buffs(world);
            }
            "Fog of War" => {
                log(
                    world,
                    "[fg:696969]Fog of War hides enemies outside direct LOS.[/fg]",
                );
            }
            "Reversal" => {
                log(
                    world,
                    "[fg:9932CC]Reversal: healing damages and damage heals for the duration.[/fg]",
                );
            }
            _ => {}
        }
    }
}

fn apply_frenzy_buffs(world: &mut World) {
    let mut entities = Vec::new();
    for (e, _team) in world.query::<Team>() {
        entities.push(e);
    }
    for entity in entities {
        if let Some(stats) = world.get_mut::<Stats>(entity) {
            stats.atk += 2;
            stats.def = (stats.def - 1).max(0);
        }
    }
}

pub fn select_floor_modifiers_with_override(
    world: &mut World,
    chosen: Vec<FloorModifier>,
    chosen_durations: Vec<u32>,
) {
    let names: Vec<String> = chosen
        .iter()
        .map(|m| m.display_name().to_string())
        .collect();
    let desc: Vec<String> = chosen
        .iter()
        .zip(chosen_durations.iter())
        .map(|(m, d)| format!("{} ({} turns)", m.display_name(), d))
        .collect();

    if let Some(modifiers) = world.resource_mut::<ActiveFloorModifiers>() {
        modifiers.modifiers = chosen;
        modifiers.turns_remaining = chosen_durations;
        modifiers.last_processed_turn = None;
    }

    log(
        world,
        format!(
            "[fg:FF00FF][b]Floor Modifiers Active:[/] {}[/fg]",
            desc.join(", ")
        ),
    );

    for modifier_name in &names {
        if modifier_name.as_str() == "Frenzy" {
            apply_frenzy_buffs(world);
        }
    }
}

pub fn has_floor_modifier(world: &World, modifier: &FloorModifier) -> bool {
    world
        .resource::<ActiveFloorModifiers>()
        .is_some_and(|m| m.modifiers.contains(modifier))
}

pub fn floor_modifier_gravity_cost(world: &World) -> i32 {
    if has_floor_modifier(world, &FloorModifier::GravityWell) {
        1
    } else {
        0
    }
}

pub fn floor_modifier_visibility_reduction(world: &World) -> i32 {
    if has_floor_modifier(world, &FloorModifier::Darkness) {
        2
    } else {
        0
    }
}

pub fn floor_modifier_healing_multiplier(world: &World) -> f32 {
    if has_floor_modifier(world, &FloorModifier::HealingSurge) {
        2.0
    } else {
        1.0
    }
}

/// Triggers deterministic, seed-driven events on deeper floors. Events are
/// telegraphed for one player turn before they resolve so scripts, agents, and
/// interactive play can Brace, Intercept, or Embrace them on the shared path.
pub fn dynamic_floor_event_system(world: &mut World) {
    let (turn, floor, phase) = world
        .resource::<GameState>()
        .map(|state| (state.turn, state.floor, state.phase))
        .unwrap_or((1, 1, TurnPhase::Player));
    if floor < 2 || phase != TurnPhase::Player {
        return;
    }

    let pending_ready = world
        .resource::<DynamicFloorEvents>()
        .is_some_and(|events| {
            events
                .pending
                .as_ref()
                .is_some_and(|pending| turn >= pending.resolves_on_turn)
        });
    if pending_ready {
        resolve_pending_floor_event(world);
        return;
    }

    let already_pending = world
        .resource::<DynamicFloorEvents>()
        .is_some_and(|events| events.pending.is_some());
    if already_pending {
        return;
    }

    let due = world
        .resource::<DynamicFloorEvents>()
        .is_some_and(|events| turn >= events.next_event_turn);
    if !due {
        return;
    }

    let Some(kind) = choose_floor_event_kind(world, floor) else {
        advance_floor_event_schedule(world, turn);
        return;
    };
    telegraph_floor_event(world, kind);
}

fn choose_floor_event_kind(world: &mut World, floor: u32) -> Option<FloorEventKind> {
    let choose_modifier = world
        .resource_mut::<Rng>()
        .map(|rng| rng.next_u32(2) == 0)
        .unwrap_or(true);
    if choose_modifier {
        let candidates = [
            FloorModifier::Darkness,
            FloorModifier::GravityWell,
            FloorModifier::ElementalStorm,
            FloorModifier::HealingSurge,
            FloorModifier::FogOfWar,
            FloorModifier::Reversal,
        ];
        let (index, duration) = world
            .resource_mut::<Rng>()
            .map(|rng| {
                (
                    rng.next_u32(candidates.len() as u32) as usize,
                    3 + rng.next_u32(4),
                )
            })
            .unwrap_or((0, 3));
        Some(FloorEventKind::ModifierSurge {
            modifier: candidates[index].clone(),
            duration,
        })
    } else {
        let class = if floor >= 4 {
            CharacterClass::FrozenSentinel
        } else {
            CharacterClass::VoidTerror
        };
        let position = floor_event_spawn_position(world)?;
        Some(FloorEventKind::MiniBossIncursion { class, position })
    }
}

fn advance_floor_event_schedule(world: &mut World, turn: u32) {
    if let Some(events) = world.resource_mut::<DynamicFloorEvents>() {
        events.next_event_turn = turn.saturating_add(events.interval.max(1));
        events.pending = None;
    }
}

fn floor_event_spawn_position(world: &World) -> Option<Position> {
    let map = world.resource::<TacticalMap>()?;
    let occupied: std::collections::HashSet<Position> = world
        .query::<Position>()
        .into_iter()
        .map(|(_, position)| *position)
        .collect();
    let preferred = Position::new(map.width as i16 - 3, map.height as i16 / 2);

    let mut candidates = Vec::new();
    for y in 0..map.height as i16 {
        for x in 0..map.width as i16 {
            let position = Position::new(x, y);
            if map.is_walkable(position) && !occupied.contains(&position) {
                let distance = (position.x - preferred.x).abs() + (position.y - preferred.y).abs();
                candidates.push((distance, position.y, position.x, position));
            }
        }
    }
    candidates.sort_by_key(|candidate| (candidate.0, candidate.1, candidate.2));
    candidates.first().map(|candidate| candidate.3)
}

fn floor_event_warning(kind: &FloorEventKind) -> String {
    match kind {
        FloorEventKind::ModifierSurge { modifier, duration } => {
            format!(
                "{} surge incoming ({} turns)",
                modifier.display_name(),
                duration
            )
        }
        FloorEventKind::MiniBossIncursion { class, position } => {
            format!(
                "{} incursion incoming at ({}, {})",
                Game::get_class_name(*class),
                position.x,
                position.y
            )
        }
    }
}

fn telegraph_tiles_for(world: &World, kind: &FloorEventKind) -> Vec<Position> {
    match kind {
        FloorEventKind::ModifierSurge { .. } => Vec::new(),
        FloorEventKind::MiniBossIncursion { class, position } => {
            let shape = incursion_shape(*class);
            let mut tiles = shape.points_including_origin(*position);
            if let Some(map) = world.resource::<TacticalMap>() {
                tiles = map
                    .tiles
                    .clip_points(tiles)
                    .into_iter()
                    .filter(|tile| map.is_walkable(*tile))
                    .collect();
            }
            tiles
        }
    }
}

fn emit_floor_event_vfx(
    world: &mut World,
    kind: &FloorEventKind,
    tiles: &[Position],
    resolve: bool,
) {
    let color = match kind {
        FloorEventKind::ModifierSurge { .. } => Color(160, 80, 255),
        FloorEventKind::MiniBossIncursion { .. } => {
            if resolve {
                Color(255, 80, 20)
            } else {
                Color(255, 160, 40)
            }
        }
    };
    let glyphs: &[char] = if resolve {
        &['*', '✦', '!']
    } else {
        &['!', '.', '+']
    };
    let centers: Vec<(f32, f32)> = if tiles.is_empty() {
        vec![(40.0, 12.0)]
    } else {
        tiles
            .iter()
            .map(|tile| get_tile_center_pixels(world, *tile))
            .collect()
    };
    if let Some(vfx) = world.resource_mut::<VfxSystem>() {
        for (cx, cy) in centers {
            vfx.particles
                .extend(verryte_terminal::vfx::emit_shockwave(cx, cy, 12, color));
            vfx.particles
                .extend(verryte_terminal::vfx::emit_burst(cx, cy, 10, color, glyphs));
        }
        if resolve {
            vfx.flashes
                .push(verryte_terminal::vfx::Flash::full_screen(color, 0.18));
            vfx.shakes
                .push(verryte_terminal::vfx::ScreenShake::new(2.0, 0.25));
        } else {
            vfx.flashes.push(verryte_terminal::vfx::Flash::full_screen(
                Color(255, 180, 60),
                0.1,
            ));
        }
    }
}

/// Warns one turn before a deeper-floor event resolves and paints incursion tiles.
pub fn telegraph_floor_event(world: &mut World, kind: FloorEventKind) -> PendingFloorEvent {
    let turn = world
        .resource::<GameState>()
        .map(|state| state.turn)
        .unwrap_or(1);
    let tiles = telegraph_tiles_for(world, &kind);
    let description = floor_event_warning(&kind);
    let pending = PendingFloorEvent {
        kind: kind.clone(),
        telegraphed_on_turn: turn,
        resolves_on_turn: turn.saturating_add(1),
        tiles: tiles.clone(),
        response: None,
    };
    log(
        world,
        format!(
            "[fg:FF8C00][b]Floor Event Warning:[/] {} — Brace (f), Intercept (n), Embrace ([), or spend a matching item[/fg]",
            description
        ),
    );
    emit_floor_event_vfx(world, &kind, &tiles, false);
    let record = FloorEventRecord {
        turn,
        floor: world
            .resource::<GameState>()
            .map(|state| state.floor)
            .unwrap_or(1),
        kind,
        description: description.clone(),
    };
    if let Some(events) = world.resource_mut::<DynamicFloorEvents>() {
        events.pending = Some(pending.clone());
        events.next_event_turn = pending.resolves_on_turn;
    }
    if let Some(events) = world.resource_mut::<Events<GameEvent>>() {
        events.send(GameEvent::FloorEventTelegraphed(record));
    }
    pending
}

fn apply_response_to_kind(
    kind: FloorEventKind,
    response: Option<FloorEventResponse>,
) -> FloorEventKind {
    match (kind, response) {
        (FloorEventKind::ModifierSurge { modifier, duration }, Some(FloorEventResponse::Brace)) => {
            FloorEventKind::ModifierSurge {
                modifier,
                duration: duration.saturating_sub(2).max(1),
            }
        }
        (
            FloorEventKind::ModifierSurge { modifier, duration },
            Some(FloorEventResponse::Intercept),
        ) => FloorEventKind::ModifierSurge {
            modifier,
            duration: duration.saturating_sub(3).max(1),
        },
        (kind, _) => kind,
    }
}

fn apply_incursion_hp_modifier(
    world: &mut World,
    kind: &FloorEventKind,
    response: Option<FloorEventResponse>,
) {
    let FloorEventKind::MiniBossIncursion { position, .. } = kind else {
        return;
    };
    let multiplier = match response {
        Some(FloorEventResponse::Bolster) => 0.4,
        Some(FloorEventResponse::Brace) => 0.7,
        Some(FloorEventResponse::Intercept) => 0.5,
        _ => return,
    };
    let target = world
        .query2::<Position, Team>()
        .into_iter()
        .find(|(_, pos, team)| **pos == *position && **team == Team::Enemy)
        .map(|(entity, _, _)| entity);
    if let Some(entity) = target {
        if let Some(stats) = world.get_mut::<Stats>(entity) {
            let hp = ((stats.hp as f32) * multiplier).round() as i32;
            stats.hp = hp.max(1);
            stats.max_hp = stats.max_hp.max(stats.hp);
        }
    }
}

fn apply_channel_heal(world: &mut World) {
    let players: Vec<Entity> = world
        .query2::<Team, Stats>()
        .into_iter()
        .filter(|(_, team, stats)| **team == Team::Player && stats.hp > 0)
        .map(|(entity, _, _)| entity)
        .collect();
    for entity in players {
        apply_heal(world, entity, 20);
    }
    log(
        world,
        "[fg:98FB98]Channeled Healing Surge restored 20 HP to the party[/fg]".to_string(),
    );
}

fn resolve_pending_floor_event(world: &mut World) {
    let pending = world
        .resource_mut::<DynamicFloorEvents>()
        .and_then(|events| events.pending.take());
    let Some(pending) = pending else {
        return;
    };
    let kind = apply_response_to_kind(pending.kind.clone(), pending.response);
    trigger_floor_event(world, kind.clone());
    if pending.response == Some(FloorEventResponse::Intercept) {
        mark_incursion_first_attack_disrupted(world, &kind);
    }
    apply_incursion_hp_modifier(world, &kind, pending.response);
    if pending.response == Some(FloorEventResponse::Channel) {
        apply_channel_heal(world);
    }
    let turn = world
        .resource::<GameState>()
        .map(|state| state.turn)
        .unwrap_or(1);
    advance_floor_event_schedule(world, turn);
}

fn mark_incursion_first_attack_disrupted(world: &mut World, kind: &FloorEventKind) {
    let FloorEventKind::MiniBossIncursion { position, .. } = kind else {
        return;
    };
    let source = world
        .query2::<Position, IncursionMiniBoss>()
        .into_iter()
        .find(|(_, candidate, _)| **candidate == *position)
        .map(|(entity, _, _)| entity);
    if let Some(source) = source {
        world.insert(source, IncursionFirstAttackDisrupted);
    }
}

/// Applies one event through existing modifier and spawn primitives and emits
/// a structured game event for reports, replays, and agents.
pub fn trigger_floor_event(world: &mut World, kind: FloorEventKind) -> FloorEventRecord {
    let (turn, floor) = world
        .resource::<GameState>()
        .map(|state| (state.turn, state.floor))
        .unwrap_or((1, 1));
    let tiles = telegraph_tiles_for(world, &kind);

    let description = match &kind {
        FloorEventKind::ModifierSurge { modifier, duration } => {
            let mut refreshed = false;
            if let Some(active) = world.resource_mut::<ActiveFloorModifiers>() {
                if let Some(index) = active.modifiers.iter().position(|item| item == modifier) {
                    if let Some(remaining) = active.turns_remaining.get_mut(index) {
                        *remaining = (*remaining).max(*duration);
                    }
                    refreshed = true;
                } else {
                    active.modifiers.push(modifier.clone());
                    active.turns_remaining.push(*duration);
                }
            }
            format!(
                "{} {} for {} turns",
                modifier.display_name(),
                if refreshed { "intensified" } else { "surged" },
                duration
            )
        }
        FloorEventKind::MiniBossIncursion { class, position } => {
            let entity = world.spawn_character_scaled(*position, Team::Enemy, *class, floor);
            world.insert(entity, IncursionMiniBoss);
            format!(
                "{} invaded at ({}, {})",
                Game::get_class_name(*class),
                position.x,
                position.y
            )
        }
    };

    let record = FloorEventRecord {
        turn,
        floor,
        kind: kind.clone(),
        description,
    };
    log(
        world,
        format!(
            "[fg:FF8C00][b]Dynamic Floor Event:[/] {}[/fg]",
            record.description
        ),
    );
    emit_floor_event_vfx(world, &kind, &tiles, true);
    if let Some(events) = world.resource_mut::<DynamicFloorEvents>() {
        events.history.push(record.clone());
        if events.history.len() > 8 {
            events.history.remove(0);
        }
        events.pending = None;
    }
    if let Some(events) = world.resource_mut::<Events<GameEvent>>() {
        events.send(GameEvent::FloorEventTriggered(record.clone()));
    }
    record
}

/// Answers a telegraphed floor event. `spend_ap` is false for item wards.
/// `require_tiles` is false for Energy Elixir remote intercepts.
pub fn apply_floor_event_response(
    world: &mut World,
    response: FloorEventResponse,
    actor: Option<Entity>,
    spend_ap: bool,
) -> Result<String, String> {
    apply_floor_event_response_ex(world, response, actor, spend_ap, true)
}

pub fn apply_floor_event_response_ex(
    world: &mut World,
    response: FloorEventResponse,
    actor: Option<Entity>,
    spend_ap: bool,
    require_tiles: bool,
) -> Result<String, String> {
    let pending_exists = world
        .resource::<DynamicFloorEvents>()
        .is_some_and(|events| events.pending.is_some());
    if !pending_exists {
        return Err("No pending floor event".to_string());
    }
    let already_answered = world
        .resource::<DynamicFloorEvents>()
        .and_then(|events| events.pending.as_ref())
        .is_some_and(|pending| pending.response.is_some());
    if already_answered {
        return Err("Already responded to this floor event".to_string());
    }

    let kind = world
        .resource::<DynamicFloorEvents>()
        .and_then(|events| events.pending.as_ref().map(|pending| pending.kind.clone()))
        .expect("pending floor event");
    validate_response_for_kind(response, &kind)?;

    if response == FloorEventResponse::Intercept && require_tiles {
        let tiles = world
            .resource::<DynamicFloorEvents>()
            .and_then(|events| events.pending.as_ref())
            .map(|pending| pending.tiles.clone())
            .unwrap_or_default();
        if !tiles.is_empty() {
            let standing = actor
                .and_then(|entity| world.get::<Position>(entity).copied())
                .is_some_and(|pos| tiles.contains(&pos));
            if !standing {
                return Err("Intercept requires standing on a telegraphed tile".to_string());
            }
        }
    }

    if spend_ap {
        let Some(entity) = actor else {
            return Err("Select a character first".to_string());
        };
        let cost = response.ap_cost();
        let ap = world
            .get::<Stats>(entity)
            .map(|stats| stats.ap)
            .unwrap_or(0);
        if ap < cost {
            return Err(format!(
                "Not enough AP to {} (costs {} AP)",
                response.display_name().to_lowercase(),
                cost
            ));
        }
        consume_response_item(world, entity, response)?;
        if let Some(stats) = world.get_mut::<Stats>(entity) {
            stats.ap -= cost;
        }
    }

    let description = world
        .resource::<DynamicFloorEvents>()
        .and_then(|events| events.pending.as_ref())
        .map(|pending| floor_event_warning(&pending.kind))
        .unwrap_or_default();

    match response {
        FloorEventResponse::Brace => {
            commit_pending_response(world, FloorEventResponse::Brace);
            let msg = format!("Braced against {}", description);
            log(world, format!("[fg:87CEEB]{}[/fg]", msg));
            emit_response_event(world, response, msg.clone());
            Ok(msg)
        }
        FloorEventResponse::Intercept => {
            if let Some(events) = world.resource_mut::<DynamicFloorEvents>() {
                if let Some(pending) = events.pending.as_mut() {
                    pending.response = Some(FloorEventResponse::Intercept);
                    pending.resolves_on_turn = pending.resolves_on_turn.saturating_add(1);
                    events.next_event_turn = pending.resolves_on_turn;
                }
            }
            let effect = if matches!(kind, FloorEventKind::MiniBossIncursion { .. }) {
                " and disrupted its first attack"
            } else {
                ""
            };
            let msg = format!("Intercepted {} — delayed one turn{}", description, effect);
            log(world, format!("[fg:FFD700]{}[/fg]", msg));
            emit_response_event(world, response, msg.clone());
            Ok(msg)
        }
        FloorEventResponse::Embrace => {
            if let Some(state) = world.resource_mut::<GameState>() {
                state.concert_energy = (state.concert_energy + 15).min(100);
            }
            commit_pending_response(world, FloorEventResponse::Embrace);
            let msg = format!("Embraced {} — +15 Concert Energy", description);
            log(world, format!("[fg:DA70D6]{}[/fg]", msg));
            emit_response_event(world, response, msg.clone());
            resolve_pending_floor_event(world);
            Ok(msg)
        }
        FloorEventResponse::Purify => {
            if let Some(events) = world.resource_mut::<DynamicFloorEvents>() {
                events.pending = None;
            }
            let turn = world
                .resource::<GameState>()
                .map(|state| state.turn)
                .unwrap_or(1);
            advance_floor_event_schedule(world, turn);
            let msg = format!("Purified {} — event cancelled", description);
            log(world, format!("[fg:98FB98]{}[/fg]", msg));
            emit_response_event(world, response, msg.clone());
            Ok(msg)
        }
        FloorEventResponse::Bolster => {
            commit_pending_response(world, FloorEventResponse::Bolster);
            let msg = format!(
                "Bolstered against {} — incursion will spawn weakened",
                description
            );
            log(world, format!("[fg:87CEEB]{}[/fg]", msg));
            emit_response_event(world, response, msg.clone());
            Ok(msg)
        }
        FloorEventResponse::Channel => {
            commit_pending_response(world, FloorEventResponse::Channel);
            let msg = format!(
                "Channeled {} — party will heal when it resolves",
                description
            );
            log(world, format!("[fg:98FB98]{}[/fg]", msg));
            emit_response_event(world, response, msg.clone());
            Ok(msg)
        }
    }
}

fn commit_pending_response(world: &mut World, response: FloorEventResponse) {
    if let Some(events) = world.resource_mut::<DynamicFloorEvents>() {
        if let Some(pending) = events.pending.as_mut() {
            pending.response = Some(response);
        }
    }
}

fn validate_response_for_kind(
    response: FloorEventResponse,
    kind: &FloorEventKind,
) -> Result<(), String> {
    match (response, kind) {
        (FloorEventResponse::Purify, FloorEventKind::ModifierSurge { .. }) => Ok(()),
        (FloorEventResponse::Bolster, FloorEventKind::MiniBossIncursion { .. }) => Ok(()),
        (
            FloorEventResponse::Channel,
            FloorEventKind::ModifierSurge {
                modifier: FloorModifier::HealingSurge,
                ..
            },
        ) => Ok(()),
        (
            FloorEventResponse::Brace | FloorEventResponse::Intercept | FloorEventResponse::Embrace,
            _,
        ) => Ok(()),
        (FloorEventResponse::Purify, _) => {
            Err("Cleanse Remedy only purifies modifier surges".to_string())
        }
        (FloorEventResponse::Bolster, _) => {
            Err("Aegis Elixir only bolsters mini-boss incursions".to_string())
        }
        (FloorEventResponse::Channel, _) => {
            Err("Healing Potion only channels a Healing Surge".to_string())
        }
    }
}

fn consume_response_item(
    world: &mut World,
    actor: Entity,
    response: FloorEventResponse,
) -> Result<(), String> {
    let (matches, label): (fn(&ItemEffect) -> bool, &str) = match response {
        FloorEventResponse::Purify => (
            |effect| matches!(effect, ItemEffect::Cleanse | ItemEffect::CleanseAndHeal(_)),
            "a Cleanse Remedy",
        ),
        FloorEventResponse::Bolster => (
            |effect| matches!(effect, ItemEffect::RestoreShield(_, _)),
            "an Aegis Elixir",
        ),
        FloorEventResponse::Channel => (
            |effect| matches!(effect, ItemEffect::Heal(_)),
            "a Healing Potion",
        ),
        _ => return Ok(()),
    };

    let items = world
        .get::<Inventory>(actor)
        .map(|inventory| inventory.items.clone())
        .unwrap_or_default();
    let found = items.into_iter().find(|&item_ent| {
        world
            .get::<Item>(item_ent)
            .is_some_and(|item| matches(&item.effect))
    });
    let Some(item_ent) = found else {
        return Err(format!("Requires {}", label));
    };
    if let Some(inventory) = world.get_mut::<Inventory>(actor) {
        inventory.items.retain(|item| *item != item_ent);
    }
    world.despawn(item_ent);
    Ok(())
}

pub struct AppliedFloorEventItem {
    pub response_name: String,
    pub description: String,
    pub apply_normal_effect: bool,
}

/// Uses an existing inventory item as an event-specific answer. Returns `None`
/// when the item should follow its normal effect instead.
pub fn try_use_item_on_floor_event(
    world: &mut World,
    effect: &ItemEffect,
    actor: Entity,
) -> Option<Result<AppliedFloorEventItem, String>> {
    let pending = world
        .resource::<DynamicFloorEvents>()
        .and_then(|events| events.pending.as_ref())?;
    if pending.response.is_some() {
        return None;
    }
    let kind = pending.kind.clone();
    let (response, require_tiles, apply_normal_effect) = match (effect, &kind) {
        (
            ItemEffect::Cleanse | ItemEffect::CleanseAndHeal(_),
            FloorEventKind::ModifierSurge { .. },
        ) => (FloorEventResponse::Purify, true, false),
        (ItemEffect::RestoreShield(_, _), FloorEventKind::MiniBossIncursion { .. }) => {
            (FloorEventResponse::Bolster, true, true)
        }
        (ItemEffect::ReplenishAp(_), _) => (FloorEventResponse::Intercept, false, false),
        (
            ItemEffect::Heal(_),
            FloorEventKind::ModifierSurge {
                modifier: FloorModifier::HealingSurge,
                ..
            },
        ) => (FloorEventResponse::Channel, true, true),
        _ => return None,
    };

    Some(
        apply_floor_event_response_ex(world, response, Some(actor), false, require_tiles).map(
            |description| AppliedFloorEventItem {
                response_name: response.display_name().to_string(),
                description,
                apply_normal_effect,
            },
        ),
    )
}

fn emit_response_event(world: &mut World, response: FloorEventResponse, description: String) {
    if let Some(events) = world.resource_mut::<Events<GameEvent>>() {
        events.send(GameEvent::FloorEventResponded {
            response,
            description,
        });
    }
}
