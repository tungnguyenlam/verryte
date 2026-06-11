use crate::components::{CharacterClass, Position, PrestigeClass, PrestigeProgress, Stats, Team};
use crate::game::Game;

use verryte_core::{Entity, Events, World};
use verryte_terminal::{vfx::VfxSystem, Color};

use super::combat::log;
use super::movement::{get_tile_center_pixels, play_spatial_sfx};

pub fn award_xp(world: &mut World, amount: u32) {
    let players: Vec<Entity> = world
        .query::<Team>()
        .iter()
        .filter(|(_, team)| **team == Team::Player)
        .map(|(e, _)| *e)
        .collect();

    let floor = world
        .resource::<crate::components::GameState>()
        .map(|s| s.floor)
        .unwrap_or(1);

    let scaled_amount = amount + (amount * floor.saturating_sub(1) * 15) / 100;

    for e in &players {
        let pos = world.get::<Position>(*e).copied();
        if let Some(p) = pos {
            let (cx, cy) = get_tile_center_pixels(world, p);
            if let Some(vfx) = world.resource_mut::<VfxSystem>() {
                vfx.floating_texts
                    .push(verryte_terminal::vfx::FloatingText::new(
                        cx - 2.0,
                        cy - 4.0,
                        &format!("+{}XP", scaled_amount),
                        Color(100, 200, 255),
                        false,
                    ));
            }
        }
    }

    let mut level_ups = Vec::new();
    for e in players {
        let class = *world
            .get::<CharacterClass>(e)
            .expect("player must have CharacterClass");
        if let Some(stats) = world.get_mut::<Stats>(e) {
            stats.xp += scaled_amount;
            let needed = stats.level * 100;
            if stats.xp >= needed {
                stats.xp -= needed;
                stats.level += 1;
                stats.max_hp += 10;
                stats.hp = stats.max_hp;
                stats.atk += 2;
                stats.def += 1;
                level_ups.push((e, class, stats.level));

                if let Some(tree) = world.get_mut::<crate::components::SkillTree>(e) {
                    tree.skill_points += 1;
                }

                let char_pos = world.get::<Position>(e).copied();
                if let Some(pos) = char_pos {
                    play_spatial_sfx(world, "level_up", pos);
                } else if let Some(events) =
                    world.resource_mut::<Events<verryte_core::AudioEvent>>()
                {
                    events.send(verryte_core::AudioEvent::play("level_up"));
                }
            }
        }
    }

    for (entity, class, level) in level_ups {
        log(
            world,
            format!(
                "[fg:FFD700][b]LEVEL UP![/] {} reached level {}![/fg]",
                Game::get_class_name(class),
                level
            ),
        );

        let pos = world
            .get::<Position>(entity)
            .copied()
            .unwrap_or(Position::new(0, 0));
        let (cx, cy) = get_tile_center_pixels(world, pos);

        if let Some(vfx) = world.resource_mut::<VfxSystem>() {
            vfx.particles.extend(verryte_terminal::vfx::emit_burst(
                cx,
                cy,
                30,
                Color(255, 215, 0),
                &['✦', '✧', '*', '★'],
            ));
            vfx.shakes
                .push(verryte_terminal::vfx::ScreenShake::new_eased(
                    3.0,
                    0.5,
                    verryte_terminal::vfx::EasingMode::ExpoOut,
                ));
            vfx.flashes
                .push(verryte_terminal::vfx::Flash::full_screen_eased(
                    Color(255, 215, 0),
                    0.3,
                    verryte_terminal::EasingMode::ExpoOut,
                ));
            vfx.floating_texts
                .push(verryte_terminal::vfx::FloatingText::new(
                    cx - 3.0,
                    cy - 6.0,
                    &format!("LVL {}!", level),
                    Color(255, 215, 0),
                    true,
                ));
        }
    }
}

pub fn prestige_system(world: &mut World) {
    let mut promotions: Vec<(Entity, CharacterClass, PrestigeClass)> = Vec::new();

    for (e, class, progress) in world.query2::<CharacterClass, PrestigeProgress>() {
        if progress.promoted {
            continue;
        }
        let team = world.get::<Team>(e).copied().unwrap_or(Team::Enemy);
        if team != Team::Player {
            continue;
        }

        let target_prestige = match class {
            CharacterClass::Warrior if progress.kill_count >= 10 => {
                Some(PrestigeClass::BladeMaster)
            }
            CharacterClass::Mage if progress.total_damage_dealt >= 500 => {
                Some(PrestigeClass::Archmage)
            }
            CharacterClass::Healer if progress.total_healing_done >= 300 => {
                Some(PrestigeClass::DivineHealer)
            }
            _ => None,
        };

        if let Some(prestige) = target_prestige {
            promotions.push((e, *class, prestige));
        }
    }

    for (entity, class, prestige) in promotions {
        if let Some(progress) = world.get_mut::<PrestigeProgress>(entity) {
            progress.class = prestige;
            progress.promoted = true;
        }

        if let Some(stats) = world.get_mut::<Stats>(entity) {
            stats.atk += 5;
            stats.def += 3;
            stats.max_hp += 20;
            stats.hp += 20;
        }

        let char_name = Game::get_class_name(class);
        let prestige_name = prestige.display_name();
        log(
            world,
            format!(
                "[fg:FFD700]{} has ascended to {}![/fg]",
                char_name, prestige_name
            ),
        );

        let pos = world
            .get::<Position>(entity)
            .copied()
            .unwrap_or(Position::new(0, 0));
        let (cx, cy) = get_tile_center_pixels(world, pos);

        if let Some(vfx) = world.resource_mut::<VfxSystem>() {
            vfx.particles.extend(verryte_terminal::vfx::emit_burst(
                cx,
                cy,
                40,
                Color(255, 215, 0),
                &['✦', '✧', '*', '★'],
            ));
            vfx.shakes
                .push(verryte_terminal::vfx::ScreenShake::new_eased(
                    4.0,
                    0.8,
                    verryte_terminal::vfx::EasingMode::ExpoOut,
                ));
            vfx.flashes
                .push(verryte_terminal::vfx::Flash::full_screen_eased(
                    Color(255, 215, 0),
                    0.4,
                    verryte_terminal::EasingMode::ExpoOut,
                ));
        }

        play_spatial_sfx(world, "level_up", pos);
    }
}
