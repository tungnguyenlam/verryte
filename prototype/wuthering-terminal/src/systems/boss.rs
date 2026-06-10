use crate::components::{
    CharacterClass, EchoAbility, EquippedEchoes, GameEvent, GameState, Position, Stats, Team,
    TurnPhase,
};
use crate::game::Game;

use verryte_core::{Events, World};
use verryte_terminal::{vfx::VfxSystem, Color};

use super::ai::enemy_ai_system;
use super::combat::{handle_defeat, log};
use super::movement::get_tile_center_pixels;

pub fn end_player_turn_system(world: &mut World) {
    // Execute any telegraphed attacks first!
    let (telegraph_tiles, telegraph_damage) = {
        let telegraph_zone = world
            .resource_mut::<crate::components::TelegraphZone>()
            .expect("TelegraphZone must be registered");
        let tiles = telegraph_zone.tiles.clone();
        let damage = telegraph_zone.damage;
        telegraph_zone.tiles.clear();
        (tiles, damage)
    };

    if !telegraph_tiles.is_empty() {
        log(
            world,
            "[fg:E6E600][b]Blight Sovereign[/] releases [fg:9933FF][b]Dark Annihilation![/][/fg]",
        );

        // VFX feedback!
        {
            let vfx = world
                .resource_mut::<VfxSystem>()
                .expect("VfxSystem must be registered");
            vfx.flashes
                .push(verryte_terminal::vfx::Flash::full_screen_eased(
                    Color(120, 0, 180),
                    0.3,
                    verryte_terminal::EasingMode::ExpoOut,
                ));
            vfx.shakes
                .push(verryte_terminal::vfx::ScreenShake::new_eased(
                    4.5,
                    0.6,
                    verryte_terminal::vfx::EasingMode::ExpoOut,
                ));
        }

        let mut hit_count = 0;
        // Check all player characters standing in telegraph tiles
        let mut players_in_zone = Vec::new();
        for (e, p, team) in world.query2::<Position, Team>() {
            if *team == Team::Player && telegraph_tiles.contains(p) {
                players_in_zone.push(e);
            }
        }

        for pe in players_in_zone {
            let target_class = *world
                .get::<CharacterClass>(pe)
                .expect("player must have CharacterClass");
            let target_pos = *world
                .get::<Position>(pe)
                .expect("player must have Position");
            let target_name = Game::get_class_name(target_class);
            let mut final_hp = 0;
            if let Some(stats) = world.get_mut::<Stats>(pe) {
                stats.hp -= telegraph_damage;
                final_hp = stats.hp;
            }
            log(
                world,
                format!(
                    "Dark Annihilation hit {} for {} damage! (HP: {})",
                    target_name, telegraph_damage, final_hp
                ),
            );

            // Thorns Echo: reflect 10% damage back to boss
            if let Some(echoes) = world.resource::<EquippedEchoes>() {
                if echoes.abilities.contains(&EchoAbility::Thorns) {
                    let reflect_damage = (telegraph_damage as f32 * 0.1) as i32;
                    let mut boss_ent = None;
                    for (e, class) in world.query::<CharacterClass>() {
                        if *class == CharacterClass::Boss {
                            boss_ent = Some(e);
                            break;
                        }
                    }
                    if let Some(be) = boss_ent {
                        let mut boss_hp = 0;
                        if let Some(stats) = world.get_mut::<Stats>(be) {
                            stats.hp -= reflect_damage;
                            boss_hp = stats.hp;
                        }
                        log(
                            world,
                            format!(
                                "Thorns reflected {} damage back to Blight Sovereign! (Boss HP: {})",
                                reflect_damage, boss_hp
                            ),
                        );
                    }
                }
            }

            hit_count += 1;

            let (cx, cy) = get_tile_center_pixels(world, target_pos);
            {
                let vfx = world
                    .resource_mut::<VfxSystem>()
                    .expect("VfxSystem must be registered");
                vfx.floating_texts
                    .push(verryte_terminal::vfx::FloatingText::new_eased(
                        cx,
                        cy - 2.0,
                        "-50",
                        Color(255, 20, 20),
                        true,
                        verryte_terminal::vfx::EasingMode::QuadOut,
                    ));
                vfx.particles
                    .extend(verryte_terminal::vfx::emit_fire(cx, cy, 15));
            }

            if final_hp <= 0 {
                let name_str = target_name.to_string();
                handle_defeat(world, pe, &name_str, target_class, target_pos);
            }
        }

        if hit_count == 0 {
            log(world, "Dark Annihilation missed everyone!");
        }
    }

    {
        let state = world
            .resource_mut::<GameState>()
            .expect("GameState must be registered");
        state.phase = TurnPhase::Enemy;
        state.selected_entity = None;
    }
    log(world, "Enemy Phase starts!");
    if let Some(events) = world.resource_mut::<Events<GameEvent>>() {
        events.send(GameEvent::PhaseChanged(TurnPhase::Enemy));
        events.send(GameEvent::TurnEnded);
    }

    // Run enemy AI
    enemy_ai_system(world);
}
