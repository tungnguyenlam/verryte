use crate::components::{
    ActiveHazards, Destructible, ElementalStatus, HazardEffect, HazardType, Stats,
};
use crate::map::{TacticalMap, Tile};
use verryte_map::Point;

pub type Position = Point;

pub struct HazardSystem;

#[derive(Clone, Debug)]
pub struct HazardTriggerResult {
    pub hazard_type: HazardType,
    pub damage: i32,
    pub healing: i32,
    pub status_effect: Option<ElementalStatus>,
    pub message: String,
    pub should_destroy_tile: bool,
}

fn tile_to_hazard(tile: Tile) -> Option<(HazardType, i32, i32, Option<ElementalStatus>, i32)> {
    match tile {
        Tile::SpikeTrap => Some((HazardType::SpikeTrap, 15, 0, None, 1)),
        Tile::PoisonCloud => Some((
            HazardType::PoisonCloud,
            5,
            0,
            Some(ElementalStatus::Poison { duration: 3 }),
            -1,
        )),
        Tile::HealingSpring => Some((HazardType::HealingSpring, 0, 25, None, -1)),
        Tile::CrackedFloor => Some((HazardType::CrackedFloor, 0, 0, None, 1)),
        Tile::PressurePlate => Some((HazardType::PressurePlate, 0, 0, None, -1)),
        Tile::ThornBush => Some((HazardType::ThornBush, 10, 0, None, -1)),
        Tile::Lava => Some((HazardType::FireTile, 20, 0, None, -1)),
        Tile::Ice => Some((
            HazardType::IceTile,
            0,
            0,
            Some(ElementalStatus::Ice { duration: 2 }),
            -1,
        )),
        _ => None,
    }
}

impl HazardSystem {
    pub fn initialize_hazards(map: &TacticalMap) -> ActiveHazards {
        let mut hazards = Vec::new();
        for y in 0..map.height {
            for x in 0..map.width {
                let pos = Position::new(x as i16, y as i16);
                let tile = map.tile(pos.x, pos.y);
                if let Some((htype, damage, healing, status, trigger_count)) = tile_to_hazard(tile)
                {
                    hazards.push((
                        pos,
                        HazardEffect {
                            hazard_type: htype,
                            damage,
                            healing,
                            status,
                            duration: 0,
                            trigger_count,
                        },
                    ));
                }
            }
        }
        ActiveHazards { hazards }
    }

    pub fn trigger_hazard(
        hazards: &ActiveHazards,
        pos: Position,
        _stats: &Stats,
    ) -> Option<HazardTriggerResult> {
        for (hazard_pos, effect) in &hazards.hazards {
            if *hazard_pos == pos && effect.trigger_count != 0 {
                let message = match effect.hazard_type {
                    HazardType::SpikeTrap => "Stepped on a spike trap!".to_string(),
                    HazardType::PoisonCloud => "Enveloped in poison cloud!".to_string(),
                    HazardType::HealingSpring => "Refreshed by a healing spring!".to_string(),
                    HazardType::CrackedFloor => "The floor cracks beneath your feet!".to_string(),
                    HazardType::PressurePlate => "A pressure plate clicks!".to_string(),
                    HazardType::ThornBush => "Thorns tear at your skin!".to_string(),
                    HazardType::FireTile => "Searing heat from lava!".to_string(),
                    HazardType::IceTile => "The ice chills you to the bone!".to_string(),
                };
                return Some(HazardTriggerResult {
                    hazard_type: effect.hazard_type,
                    damage: effect.damage,
                    healing: effect.healing,
                    status_effect: effect.status,
                    message,
                    should_destroy_tile: effect.hazard_type == HazardType::CrackedFloor,
                });
            }
        }
        None
    }

    pub fn process_cracked_floor(map: &mut TacticalMap, pos: Position) -> bool {
        if map.tile(pos.x, pos.y) == Tile::CrackedFloor {
            map.tiles.set(pos, Tile::Wall);
            return true;
        }
        false
    }

    pub fn update_destructibles(
        destructibles: &mut [(Position, Destructible)],
        map: &mut TacticalMap,
    ) {
        for (pos, d) in destructibles.iter_mut() {
            if d.hp <= 0 && !d.destroyed {
                d.destroyed = true;
                map.tiles.set(*pos, d.replacement_tile);
            }
        }
    }

    pub fn populate_hazards(map: &mut TacticalMap, floor: u32, seed: u64) -> ActiveHazards {
        let mut state = seed | 1;
        let mut rng = || -> u64 {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };

        let count = 4 + (floor as usize * 2);
        let hazard_tiles = [
            Tile::SpikeTrap,
            Tile::PoisonCloud,
            Tile::HealingSpring,
            Tile::CrackedFloor,
            Tile::PressurePlate,
            Tile::ThornBush,
        ];

        let mut placed = 0;
        let mut attempts = 0;
        while placed < count && attempts < count * 10 {
            attempts += 1;
            let x = (rng() % (map.width.saturating_sub(2)) as u64) as i16 + 1;
            let y = (rng() % (map.height.saturating_sub(2)) as u64) as i16 + 1;
            let pos = Position::new(x, y);
            if map.tile(x, y) == Tile::Grass {
                let tile_idx = (rng() % hazard_tiles.len() as u64) as usize;
                map.tiles.set(pos, hazard_tiles[tile_idx]);
                placed += 1;
            }
        }

        Self::initialize_hazards(map)
    }

    pub fn has_hazard(hazards: &ActiveHazards, pos: Position, htype: HazardType) -> bool {
        hazards
            .hazards
            .iter()
            .any(|(p, e)| *p == pos && e.hazard_type == htype)
    }

    pub fn cleanup_hazards(hazards: &mut ActiveHazards) {
        hazards
            .hazards
            .retain(|(_, e)| e.trigger_count > 0 || e.trigger_count == -1);
    }

    pub fn decrement_trigger(hazards: &mut ActiveHazards, pos: Position) {
        for (hazard_pos, effect) in &mut hazards.hazards {
            if *hazard_pos == pos && effect.trigger_count > 0 {
                effect.trigger_count -= 1;
                return;
            }
        }
    }

    pub fn apply_hazard_to_entity(
        stats: &mut Stats,
        map: &mut TacticalMap,
        hazards: &mut ActiveHazards,
        result: &HazardTriggerResult,
        pos: Position,
    ) {
        stats.hp = (stats.hp - result.damage + result.healing).clamp(0, stats.max_hp);
        if result.should_destroy_tile {
            Self::process_cracked_floor(map, pos);
        }
        Self::decrement_trigger(hazards, pos);
    }
}
