use crate::components::{
    CharacterClass, ElementalStatus, Inventory, Item, ItemEffect, Position, Stats, Team,
};
use verryte_core::{Entity, World};

pub trait Spawner {
    fn spawn_character(&mut self, pos: Position, team: Team, class: CharacterClass) -> Entity;
    fn spawn_item(&mut self, name: &str, effect: ItemEffect) -> Entity;
}

impl Spawner for World {
    fn spawn_character(&mut self, pos: Position, team: Team, class: CharacterClass) -> Entity {
        let stats = match class {
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
        };

        self.builder()
            .with(pos)
            .with(team)
            .with(class)
            .with(stats)
            .with(ElementalStatus::None)
            .with(Inventory::default())
            .build()
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
}
