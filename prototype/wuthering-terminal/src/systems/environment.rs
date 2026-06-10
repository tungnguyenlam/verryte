use crate::components::{
    ActiveFloorModifiers, CharacterClass, FloorModifier, GameState, Position, Stats, Team,
    TurnPhase, Weather, WeatherType,
};
use crate::game::Game;
use crate::map::{TacticalMap, Tile};

use verryte_core::{Events, Rng, World};
use verryte_terminal::{vfx::VfxSystem, Color};

use super::combat::{handle_defeat, log};
use super::movement::get_tile_center_pixels;

pub fn weather_ambient_system(world: &mut World) {
    let current = world
        .resource::<Weather>()
        .map(|w| w.current)
        .unwrap_or(WeatherType::Sunny);

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
    }
}

pub const WEATHER_CYCLE_TURNS: u32 = 3;

pub fn weather_cycle_system(world: &mut World) {
    let current = world
        .resource::<Weather>()
        .map(|w| w.current)
        .unwrap_or(WeatherType::Sunny);

    apply_per_turn_weather_effects(world, current);

    let turn = world.resource::<GameState>().map(|s| s.turn).unwrap_or(1);
    if turn == 1 || turn % WEATHER_CYCLE_TURNS != 1 {
        return;
    }
    let next = match current {
        WeatherType::Sunny => WeatherType::Rainy,
        WeatherType::Rainy => WeatherType::LightningStorm,
        WeatherType::LightningStorm => WeatherType::Snowing,
        WeatherType::Snowing => WeatherType::Sunny,
    };
    if let Some(weather) = world.resource_mut::<Weather>() {
        weather.current = next;
        weather.danger_zones.clear();
    }
    log(
        world,
        format!("[fg:87CEEB][b]Weather changed to {:?}![/][/fg]", next),
    );
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
                if walkable {
                    zones.push(pos);
                }
            }

            if let Some(w) = world.resource_mut::<Weather>() {
                w.danger_zones = zones.clone();
            }

            for zone in &zones {
                let mut victims = Vec::new();
                for (e, p, team) in world.query2::<Position, Team>() {
                    if *p == *zone {
                        victims.push((e, *team));
                    }
                }
                for (victim, _team) in victims {
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
                            "[fg:FFFF64]Lightning struck {} at ({}, {}) for 15 damage! (HP: {})[/fg]",
                            victim_name, zone.x, zone.y, final_hp
                        ),
                    );
                    if defeated {
                        let name_str = victim_name.to_string();
                        handle_defeat(world, victim, &name_str, victim_class, *zone);
                    }
                }
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

pub fn apply_weather_hazard_damage(world: &mut World) {
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
        for (e, p, _team) in world.query2::<Position, Team>() {
            if *p == *zone {
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
    let modifiers = world
        .resource::<ActiveFloorModifiers>()
        .cloned()
        .unwrap_or_default();
    if modifiers.modifiers.is_empty() {
        return;
    }

    let phase = world
        .resource::<GameState>()
        .map(|s| s.phase)
        .unwrap_or(TurnPhase::Player);

    for modifier in modifiers.modifiers.iter() {
        match modifier {
            FloorModifier::ElementalStorm if phase == TurnPhase::Player => {
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
