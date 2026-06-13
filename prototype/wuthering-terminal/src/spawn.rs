use crate::components::{
    CharacterClass, ElementalStatus, EquippedItems, Fatigue, Inventory, Item, ItemEffect, Morale,
    Position, Stats, Team,
};
use verryte_core::{Entity, World};

pub trait Spawner {
    fn spawn_character(&mut self, pos: Position, team: Team, class: CharacterClass) -> Entity;
    fn spawn_item(&mut self, name: &str, effect: ItemEffect) -> Entity;
    fn spawn_barrel(&mut self, pos: Position) -> Entity;
    fn spawn_character_scaled(
        &mut self,
        pos: Position,
        team: Team,
        class: CharacterClass,
        floor: u32,
    ) -> Entity;
}

pub fn scale_stats_by_floor(base: &Stats, floor: u32) -> Stats {
    if floor <= 1 {
        return base.clone();
    }
    let scale = 1.0 + 0.15 * (floor as f32 - 1.0);
    Stats {
        hp: (base.hp as f32 * scale) as i32,
        max_hp: (base.max_hp as f32 * scale) as i32,
        atk: (base.atk as f32 * scale) as i32,
        def: (base.def as f32 * scale) as i32,
        spd: base.spd,
        ap: base.ap,
        max_ap: base.max_ap,
        level: base.level + floor.saturating_sub(1),
        xp: 0,
    }
}

impl Spawner for World {
    fn spawn_character(&mut self, pos: Position, team: Team, class: CharacterClass) -> Entity {
        Self::spawn_character_scaled(self, pos, team, class, 1)
    }

    fn spawn_character_scaled(
        &mut self,
        pos: Position,
        team: Team,
        class: CharacterClass,
        floor: u32,
    ) -> Entity {
        let base = base_stats(class);
        let stats = if team == Team::Enemy && floor > 1 {
            scale_stats_by_floor(&base, floor)
        } else {
            base
        };

        let trait_opt = match class {
            CharacterClass::Warrior => Some(crate::components::CharacterTrait {
                trait_type: crate::components::HeroTrait::SwiftFoot,
            }),
            CharacterClass::Rogue => Some(crate::components::CharacterTrait {
                trait_type: crate::components::HeroTrait::ShadowStrike,
            }),
            CharacterClass::Mage => Some(crate::components::CharacterTrait {
                trait_type: crate::components::HeroTrait::StormChaser,
            }),
            CharacterClass::Healer => Some(crate::components::CharacterTrait {
                trait_type: crate::components::HeroTrait::PurifyingTouch,
            }),
            CharacterClass::GlacialGolem => Some(crate::components::CharacterTrait {
                trait_type: crate::components::HeroTrait::IceWalker,
            }),
            CharacterClass::DestructibleObject => None,
            _ => None,
        };

        let element = match class {
            CharacterClass::Warrior => crate::components::CharacterElement::fire(),
            CharacterClass::Mage => crate::components::CharacterElement::lightning(),
            CharacterClass::Healer => crate::components::CharacterElement::nature(),
            CharacterClass::GlacialGolem => crate::components::CharacterElement::ice(),
            CharacterClass::Boss | CharacterClass::VoidTerror => {
                crate::components::CharacterElement::fire()
            }
            CharacterClass::Berserker | CharacterClass::EliteBerserker => {
                crate::components::CharacterElement::fire()
            }
            CharacterClass::Tactician | CharacterClass::EliteTactician => {
                crate::components::CharacterElement::lightning()
            }
            CharacterClass::Summoner | CharacterClass::EliteSummoner => {
                crate::components::CharacterElement::nature()
            }
            CharacterClass::Assassin | CharacterClass::EliteAssassin => {
                crate::components::CharacterElement::ice()
            }
            CharacterClass::DestructibleObject => crate::components::CharacterElement::physical(),
            _ => crate::components::CharacterElement::physical(),
        };

        let mut builder = self
            .builder()
            .with(pos)
            .with(team)
            .with(class)
            .with(stats)
            .with(ElementalStatus::None)
            .with(element)
            .with(Inventory::default());

        if let Some(t) = trait_opt {
            builder = builder.with(t);
        }

        if team == Team::Player {
            let starter_gear = crate::equipment::equipment_for_class(class);
            let mut equipped = EquippedItems::default();
            for item in starter_gear {
                equipped.equip(item);
            }
            builder = builder.with(equipped);
            builder = builder.with(crate::components::Threat { value: 0 });
            builder = builder.with(crate::components::PrestigeProgress::default());
            builder = builder.with(Morale {
                value: 70,
                max: 100,
            });
            builder = builder.with(Fatigue::default());
            builder = builder.with(crate::components::SkillTree::for_class(class));
        } else {
            let archetype = match class {
                CharacterClass::CorruptedSpore | CharacterClass::ShadowStalker => {
                    crate::components::AIArchetype::Coward
                }
                CharacterClass::EnemyCleric => crate::components::AIArchetype::Cleric,
                CharacterClass::Berserker | CharacterClass::EliteBerserker => {
                    crate::components::AIArchetype::Berserker
                }
                CharacterClass::Tactician | CharacterClass::EliteTactician => {
                    crate::components::AIArchetype::Tactician
                }
                CharacterClass::Summoner | CharacterClass::EliteSummoner => {
                    crate::components::AIArchetype::Summoner
                }
                CharacterClass::Assassin | CharacterClass::EliteAssassin => {
                    crate::components::AIArchetype::Assassin
                }
                CharacterClass::DestructibleObject => crate::components::AIArchetype::Chaser,
                _ => crate::components::AIArchetype::Chaser,
            };
            builder = builder.with(archetype);
            let morale_value = if class == CharacterClass::Boss {
                100
            } else {
                (50 + (floor as i32 - 1) * 5).min(100)
            };
            builder = builder.with(Morale {
                value: morale_value,
                max: 100,
            });
            builder = builder.with(Fatigue::default());
        }

        builder.build()
    }

    fn spawn_item(&mut self, name: &str, effect: ItemEffect) -> Entity {
        self.builder()
            .with(Item {
                name: name.to_string(),
                effect,
                consumed: false,
            })
            .build()
    }

    fn spawn_barrel(&mut self, pos: Position) -> Entity {
        self.builder()
            .with(pos)
            .with(Team::Enemy) // Targeted by players
            .with(CharacterClass::DestructibleObject)
            .with(Stats {
                hp: 20,
                max_hp: 20,
                atk: 0,
                def: 0,
                spd: 0,
                ap: 0,
                max_ap: 0,
                level: 1,
                xp: 0,
            })
            .with(crate::components::Destructible {
                hp: 20,
                max_hp: 20,
                destroyed: false,
                replacement_tile: crate::map::Tile::Grass,
            })
            .with(ElementalStatus::None)
            .build()
    }
}

pub fn base_stats(class: CharacterClass) -> Stats {
    match class {
        CharacterClass::Warrior => Stats {
            hp: 100,
            max_hp: 100,
            atk: 20,
            def: 10,
            spd: 5,
            ap: 3,
            max_ap: 3,
            level: 1,
            xp: 0,
        },
        CharacterClass::Rogue => Stats {
            hp: 75,
            max_hp: 75,
            atk: 25,
            def: 8,
            spd: 8,
            ap: 3,
            max_ap: 3,
            level: 1,
            xp: 0,
        },
        CharacterClass::Mage => Stats {
            hp: 60,
            max_hp: 60,
            atk: 35,
            def: 5,
            spd: 4,
            ap: 3,
            max_ap: 3,
            level: 1,
            xp: 0,
        },
        CharacterClass::Healer => Stats {
            hp: 70,
            max_hp: 70,
            atk: 10,
            def: 8,
            spd: 6,
            ap: 3,
            max_ap: 3,
            level: 1,
            xp: 0,
        },
        CharacterClass::Boss => Stats {
            hp: 500,
            max_hp: 500,
            atk: 40,
            def: 20,
            spd: 3,
            ap: 0,
            max_ap: 4,
            level: 10,
            xp: 0,
        },
        CharacterClass::ShadowStalker => Stats {
            hp: 80,
            max_hp: 80,
            atk: 25,
            def: 5,
            spd: 8,
            ap: 4,
            max_ap: 4,
            level: 1,
            xp: 0,
        },
        CharacterClass::CorruptedSpore => Stats {
            hp: 40,
            max_hp: 40,
            atk: 15,
            def: 0,
            spd: 10,
            ap: 2,
            max_ap: 2,
            level: 1,
            xp: 0,
        },
        CharacterClass::CursedSentinel => Stats {
            hp: 60,
            max_hp: 60,
            atk: 30,
            def: 15,
            spd: 3,
            ap: 2,
            max_ap: 2,
            level: 3,
            xp: 0,
        },
        CharacterClass::PlagueWraith => Stats {
            hp: 50,
            max_hp: 50,
            atk: 20,
            def: 5,
            spd: 7,
            ap: 3,
            max_ap: 3,
            level: 2,
            xp: 0,
        },
        CharacterClass::GlacialGolem => Stats {
            hp: 120,
            max_hp: 120,
            atk: 25,
            def: 20,
            spd: 2,
            ap: 2,
            max_ap: 2,
            level: 4,
            xp: 0,
        },
        CharacterClass::EnemyCleric => Stats {
            hp: 55,
            max_hp: 55,
            atk: 12,
            def: 6,
            spd: 5,
            ap: 3,
            max_ap: 3,
            level: 2,
            xp: 0,
        },
        CharacterClass::VoidTerror => Stats {
            hp: 120,
            max_hp: 120,
            atk: 50,
            def: 10,
            spd: 6,
            ap: 3,
            max_ap: 3,
            level: 5,
            xp: 0,
        },
        CharacterClass::Berserker => Stats {
            hp: 90,
            max_hp: 90,
            atk: 40,
            def: 3,
            spd: 7,
            ap: 3,
            max_ap: 3,
            level: 3,
            xp: 0,
        },
        CharacterClass::Tactician => Stats {
            hp: 70,
            max_hp: 70,
            atk: 25,
            def: 10,
            spd: 6,
            ap: 3,
            max_ap: 3,
            level: 3,
            xp: 0,
        },
        CharacterClass::Summoner => Stats {
            hp: 60,
            max_hp: 60,
            atk: 15,
            def: 8,
            spd: 5,
            ap: 3,
            max_ap: 3,
            level: 3,
            xp: 0,
        },
        CharacterClass::Assassin => Stats {
            hp: 55,
            max_hp: 55,
            atk: 35,
            def: 4,
            spd: 9,
            ap: 4,
            max_ap: 4,
            level: 4,
            xp: 0,
        },
        CharacterClass::EliteBerserker => Stats {
            hp: 140,
            max_hp: 140,
            atk: 55,
            def: 8,
            spd: 8,
            ap: 4,
            max_ap: 4,
            level: 6,
            xp: 0,
        },
        CharacterClass::EliteTactician => Stats {
            hp: 100,
            max_hp: 100,
            atk: 35,
            def: 15,
            spd: 7,
            ap: 4,
            max_ap: 4,
            level: 6,
            xp: 0,
        },
        CharacterClass::EliteSummoner => Stats {
            hp: 85,
            max_hp: 85,
            atk: 20,
            def: 12,
            spd: 6,
            ap: 4,
            max_ap: 4,
            level: 6,
            xp: 0,
        },
        CharacterClass::EliteAssassin => Stats {
            hp: 75,
            max_hp: 75,
            atk: 50,
            def: 6,
            spd: 11,
            ap: 5,
            max_ap: 5,
            level: 7,
            xp: 0,
        },
        CharacterClass::DestructibleObject => Stats {
            hp: 20,
            max_hp: 20,
            atk: 0,
            def: 0,
            spd: 0,
            ap: 0,
            max_ap: 0,
            level: 1,
            xp: 0,
        },
    }
}
