use crate::components::{
    AvailableCombos, BossConfig, BossPhase, CharacterClass, CharacterElement, ComboSkill,
    ComboSkillDef, EchoItem, ElementalShield, ElementalStatus, EquippedItems, GameEvent, GameState,
    Outcome, Position, Rooted, ShieldType, Stats, Team, Weather, WeatherType,
};
use crate::game::Game;

use verryte_core::{Entity, Events, MessageLog, Rng, World};
use verryte_terminal::{vfx::VfxSystem, Color};

use super::morale::{apply_ally_defeated_morale, apply_enemy_defeated_morale};
use super::movement::{get_tile_center_pixels, play_spatial_sfx};
use super::progression::award_xp;

pub fn log(world: &mut World, msg: impl Into<String>) {
    if let Some(log) = world.resource_mut::<MessageLog>() {
        log.push(msg);
    }
}

pub fn weather_damage_modifier(world: &World, base_damage: i32, attacker: Option<Entity>) -> i32 {
    let weather = match world.resource::<Weather>() {
        Some(w) => w.current,
        None => return base_damage,
    };
    let is_fire = attacker
        .and_then(|e| world.get::<CharacterElement>(e))
        .map(|ce| ce.is_fire())
        .unwrap_or(false);
    let is_ice = attacker
        .and_then(|e| world.get::<CharacterElement>(e))
        .map(|ce| ce.is_ice())
        .unwrap_or(false);
    let modifier: f32 = match (weather, is_fire, is_ice) {
        (WeatherType::Rainy, true, _) => 0.80,
        (WeatherType::Snowing, true, _) => 0.85,
        (WeatherType::Snowing, _, true) => 1.15,
        _ => 1.0,
    };
    (base_damage as f32 * modifier) as i32
}

pub fn effective_atk(world: &World, entity: Entity) -> i32 {
    let base_atk = world.get::<Stats>(entity).map(|s| s.atk).unwrap_or(0);
    let equip_bonus = world
        .get::<EquippedItems>(entity)
        .map(|e| e.total_atk_bonus())
        .unwrap_or(0);
    base_atk + equip_bonus
}

pub fn effective_def(world: &World, entity: Entity) -> i32 {
    let base_def = world.get::<Stats>(entity).map(|s| s.def).unwrap_or(0);
    let equip_bonus = world
        .get::<EquippedItems>(entity)
        .map(|e| e.total_def_bonus())
        .unwrap_or(0);
    base_def + equip_bonus
}

pub fn resolve_combat_hit(
    world: &mut World,
    attacker: Option<Entity>,
    target: Entity,
    base_damage: i32,
    attacker_name: &str,
    target_name: &str,
    pos: Position,
) -> (i32, bool) {
    let sfx_name = match attacker.and_then(|e| world.get::<CharacterClass>(e)) {
        Some(CharacterClass::Warrior) => "warrior_attack",
        Some(CharacterClass::Mage) => "mage_attack",
        Some(CharacterClass::Healer) => "healer_attack",
        Some(CharacterClass::Boss) => "boss_attack",
        _ => "enemy_attack",
    };
    play_spatial_sfx(world, sfx_name, pos);

    let base_damage = weather_damage_modifier(world, base_damage, attacker);

    let (is_crit, is_block, damage) = {
        let rng = world.resource_mut::<Rng>().expect("Rng must be registered");
        let roll = rng.next_u32(100);
        if roll < 20 {
            (true, false, (base_damage as f32 * 1.5) as i32)
        } else if roll < 35 {
            (false, true, (base_damage / 2).max(1))
        } else {
            (false, false, base_damage)
        }
    };

    let weather = world
        .resource::<Weather>()
        .map(|w| w.current)
        .unwrap_or(WeatherType::Sunny);
    let mut lightning_bonus = 0;
    if weather == WeatherType::LightningStorm {
        let bonus_roll = {
            let rng = world.resource_mut::<Rng>().expect("Rng must be registered");
            rng.next_u32(100)
        };
        if bonus_roll < 25 {
            lightning_bonus = 15;
            log(
                world,
                "[fg:FFFF64]Lightning strikes from the storm! Bonus 15 damage![/fg]",
            );
        }
    }
    let damage = damage + lightning_bonus;

    let mut shield_absorbed = 0;
    let mut shield_broke = false;
    let mut shield_type_opt = None;
    let mut shield_remaining = 0;
    if let Some(shield) = world.get_mut::<ElementalShield>(target) {
        shield_type_opt = Some(shield.shield_type);
        if shield.amount >= damage {
            shield.amount -= damage;
            shield_absorbed = damage;
            shield_remaining = shield.amount;
        } else {
            shield_absorbed = shield.amount;
            shield.amount = 0;
            shield_broke = true;
        }
    }
    if shield_broke {
        world.remove::<ElementalShield>(target);
    }

    let actual_damage = damage - shield_absorbed;
    let mut defeated = false;
    let mut final_hp = 0;
    if let Some(stats) = world.get_mut::<Stats>(target) {
        stats.hp -= actual_damage;
        final_hp = stats.hp;
        if stats.hp <= 0 {
            defeated = true;
        }
    }

    if let Some(attacker) = attacker {
        let equipment_lifesteal_percent = world
            .get::<EquippedItems>(attacker)
            .map(|equipped| equipped.total_lifesteal_percent())
            .unwrap_or(0);
        let equipment_lifesteal = actual_damage * equipment_lifesteal_percent as i32 / 100;
        if equipment_lifesteal > 0 {
            let mut healed = 0;
            if let Some(stats) = world.get_mut::<Stats>(attacker) {
                let before = stats.hp;
                stats.hp = (stats.hp + equipment_lifesteal).min(stats.max_hp);
                healed = stats.hp - before;
            }
            if healed > 0 {
                log(
                    world,
                    format!("{}'s equipment restored {} HP!", attacker_name, healed),
                );
            }
        }
    }

    let log_msg = if is_crit {
        format!(
            "[fg:FF8080]{} attacked {} for [b]{} damage![/] [fg:E6E600][b](CRITICAL HIT!)[/] (Target HP: {})[/fg]",
            attacker_name, target_name, damage, final_hp
        )
    } else if is_block {
        format!(
            "[fg:CCCCCC]{} attacked {} for {} damage! (BLOCKED!) (Target HP: {})[/fg]",
            attacker_name, target_name, damage, final_hp
        )
    } else {
        format!(
            "{} attacked {} for [fg:FF3333]{} damage![/] (Target HP: {})",
            attacker_name, target_name, damage, final_hp
        )
    };
    log(world, log_msg);

    if shield_absorbed > 0 {
        let shield_name =
            match shield_type_opt.expect("shield_type must be set when shield_absorbed > 0") {
                ShieldType::Ice => "[fg:80D0FF]Ice Shield[/fg]",
                ShieldType::Lightning => "[fg:FFD700]Lightning Shield[/fg]",
                ShieldType::Nature => "[fg:50DC64]Nature Shield[/fg]",
                ShieldType::Physical => "[fg:CCCCCC]Physical Shield[/fg]",
            };
        if shield_broke {
            log(
                world,
                format!(
                    "{}'s {} broke! It absorbed {} damage.",
                    target_name, shield_name, shield_absorbed
                ),
            );
        } else {
            log(
                world,
                format!(
                    "{}'s {} absorbed {} damage! (Shield HP: {})",
                    target_name, shield_name, shield_absorbed, shield_remaining
                ),
            );
        }
    }

    let (tcx, tcy) = get_tile_center_pixels(world, pos);
    let float_text = if is_crit {
        format!("CRIT! -{}", actual_damage)
    } else if is_block {
        format!("BLOCK! -{}", actual_damage)
    } else {
        format!("-{}", actual_damage)
    };

    let float_color = if is_crit {
        Color(255, 215, 0) // Gold
    } else if is_block {
        Color(160, 160, 160) // Grey
    } else {
        Color(255, 50, 50) // Red
    };

    {
        if let Some(events) = world.resource_mut::<Events<verryte_core::AudioEvent>>() {
            let pan = ((pos.x as f32 - 12.0) / 12.0).clamp(-1.0, 1.0);
            if is_crit {
                events.send(verryte_core::AudioEvent::play("crit").with_pan(pan));
            } else {
                events.send(verryte_core::AudioEvent::play("hit").with_pan(pan));
            }
        }

        let vfx = world
            .resource_mut::<VfxSystem>()
            .expect("VfxSystem must be registered");
        vfx.floating_texts
            .push(verryte_terminal::vfx::FloatingText::new_eased(
                tcx,
                tcy - 2.0,
                &float_text,
                float_color,
                is_crit,
                verryte_terminal::vfx::EasingMode::QuadOut,
            ));

        if shield_absorbed > 0 {
            let shield_color =
                match shield_type_opt.expect("shield_type must be set when shield_absorbed > 0") {
                    ShieldType::Ice => Color(100, 200, 255),
                    ShieldType::Lightning => Color(255, 230, 50),
                    ShieldType::Nature => Color(50, 220, 100),
                    ShieldType::Physical => Color(160, 160, 160),
                };
            vfx.floating_texts
                .push(verryte_terminal::vfx::FloatingText::new_eased(
                    tcx + 2.0,
                    tcy - 1.0,
                    &format!("SHIELD -{}", shield_absorbed),
                    shield_color,
                    false,
                    verryte_terminal::vfx::EasingMode::QuadOut,
                ));

            // Custom spiral particle trajectory for shield hits!
            let mut shield_particles = Vec::new();
            for i in 0..10 {
                let angle = (i as f32 / 10.0) * std::f32::consts::TAU;
                shield_particles.push(verryte_terminal::vfx::Particle {
                    x: tcx,
                    y: tcy,
                    vx: angle.cos() * 1.5,
                    vy: angle.sin() * 0.7,
                    glyph: '✦',
                    fg: shield_color,
                    bg: Color::BLACK,
                    lifetime: 0.5,
                    max_lifetime: 0.5,
                    attrs: verryte_terminal::CellAttrs::NONE.bold(),
                    trajectory: verryte_terminal::vfx::Trajectory::Spiral {
                        speed: 2.0,
                        radius: 1.5,
                    },
                });
            }
            vfx.particles.extend(shield_particles);
        }

        let flash_color = if is_crit {
            Color(255, 215, 0)
        } else {
            Color(255, 100, 100)
        };
        let flash_duration = if is_crit { 0.15 } else { 0.1 };
        vfx.flashes
            .push(verryte_terminal::vfx::Flash::full_screen_eased(
                flash_color,
                flash_duration,
                verryte_terminal::EasingMode::QuadOut,
            ));

        vfx.particles.extend(verryte_terminal::vfx::emit_slash(
            tcx,
            tcy,
            if is_crit { 2.0 } else { 1.0 },
        ));

        let shake_intensity = if is_crit { 3.5 } else { 1.5 };
        let shake_duration = if is_crit { 0.4 } else { 0.25 };
        let shake_easing = if is_crit {
            verryte_terminal::vfx::EasingMode::ExpoOut
        } else {
            verryte_terminal::vfx::EasingMode::QuadOut
        };
        vfx.shakes
            .push(verryte_terminal::vfx::ScreenShake::new_eased(
                shake_intensity,
                shake_duration,
                shake_easing,
            ));
    }

    (actual_damage, defeated)
}

pub fn handle_defeat(
    world: &mut World,
    entity: Entity,
    name: &str,
    class: CharacterClass,
    pos: Position,
) {
    if class == CharacterClass::Boss {
        let phase = world
            .resource::<GameState>()
            .expect("GameState resource must be registered")
            .boss_phase;
        if phase == BossPhase::Phase1 {
            let config = world.resource::<BossConfig>().cloned().unwrap_or_default();
            if let Some(stats) = world.get_mut::<Stats>(entity) {
                stats.max_hp = config.phase2_max_hp;
                stats.hp = config.phase2_max_hp;
                stats.atk += config.phase2_atk_bonus;
                stats.def += config.phase2_def_bonus;
                stats.spd += config.phase2_spd_bonus;
                stats.max_ap = config.phase2_max_ap;
                stats.ap = config.phase2_max_ap;
            }

            if let Some(state) = world.resource_mut::<GameState>() {
                state.boss_phase = BossPhase::Phase2;
            }

            log(world, "Blight Sovereign enters Phase 2! Its power intensifies, and Celestial Ruin is unleashed!");

            let (bx, by) = get_tile_center_pixels(world, pos);

            let vfx = world
                .resource_mut::<VfxSystem>()
                .expect("VfxSystem must be registered");
            vfx.particles.extend(verryte_terminal::vfx::emit_burst(
                bx,
                by,
                50,
                Color(255, 0, 0),
                &['✦', '*', '░', '▓', '¤'],
            ));
            vfx.shakes
                .push(verryte_terminal::vfx::ScreenShake::new_eased(
                    5.0,
                    1.0,
                    verryte_terminal::vfx::EasingMode::ExpoOut,
                ));
            vfx.flashes
                .push(verryte_terminal::vfx::Flash::full_screen_eased(
                    Color(255, 0, 0),
                    0.5,
                    verryte_terminal::EasingMode::ExpoOut,
                ));
            return;
        }
    }

    log(world, format!("{} was defeated!", name));
    if let Some(events) = world.resource_mut::<Events<verryte_core::AudioEvent>>() {
        events.send(verryte_core::AudioEvent::play("defeat"));
    }
    if let Some(events) = world.resource_mut::<Events<GameEvent>>() {
        events.send(GameEvent::Defeated { entity });
    }

    let defeated_team = world.get::<Team>(entity).copied();
    if defeated_team == Some(Team::Player) {
        apply_ally_defeated_morale(world, entity);
    } else if defeated_team == Some(Team::Enemy) {
        apply_enemy_defeated_morale(world);
    }

    world.despawn(entity);

    // Award XP if it was an enemy
    if class != CharacterClass::Boss
        || (class == CharacterClass::Boss
            && world
                .resource::<GameState>()
                .expect("GameState must be registered")
                .boss_phase
                == BossPhase::Phase2)
    {
        let xp_amount = match class {
            CharacterClass::Boss => 1000,
            CharacterClass::ShadowStalker => 50,
            CharacterClass::CursedSentinel => 40,
            CharacterClass::PlagueWraith => 35,
            CharacterClass::GlacialGolem => 60,
            _ => 20,
        };
        award_xp(world, xp_amount);
    }

    if class == CharacterClass::Boss {
        log(
            world,
            "Blight Sovereign dropped an Echo! Move a character to its tile to absorb it.",
        );
        world.builder().with(pos).with(EchoItem { class }).build();
        award_equipment_set_reward(world, class, name);

        // Visual boss death burst
        let (cx, cy) = get_tile_center_pixels(world, pos);
        let vfx = world
            .resource_mut::<VfxSystem>()
            .expect("VfxSystem must be registered");
        vfx.particles.extend(verryte_terminal::vfx::emit_burst(
            cx,
            cy,
            40,
            Color(180, 50, 255),
            &['✦', '✧', '░', '▓', '¤'],
        ));
        vfx.shakes
            .push(verryte_terminal::vfx::ScreenShake::new_eased(
                4.0,
                0.8,
                verryte_terminal::vfx::EasingMode::ExpoOut,
            ));
        vfx.flashes
            .push(verryte_terminal::vfx::Flash::full_screen_eased(
                Color(255, 255, 255),
                0.4,
                verryte_terminal::EasingMode::ExpoOut,
            ));
    } else if class == CharacterClass::CursedSentinel || class == CharacterClass::GlacialGolem {
        let kit = world
            .builder()
            .with(crate::components::Item {
                name: "Upgrade Kit".to_string(),
                effect: crate::components::ItemEffect::UpgradeKit,
                consumed: false,
            })
            .build();
        log(world, format!("{} dropped an Upgrade Kit!", name));
        if let Some(player) = world
            .query2::<Team, crate::components::Inventory>()
            .iter()
            .find(|(_, team, _)| **team == Team::Player)
            .map(|(e, _, _)| *e)
        {
            if let Some(inventory) = world.get_mut::<crate::components::Inventory>(player) {
                inventory.items.push(kit);
            }
        }
    }
    if class != CharacterClass::Boss {
        award_equipment_set_reward(world, class, name);
    }

    let enemy_exists = world
        .query::<Team>()
        .into_iter()
        .any(|(_, team)| *team == Team::Enemy);
    let echo_exists = world.query::<EchoItem>().into_iter().next().is_some();
    if !enemy_exists && !echo_exists {
        world
            .resource_mut::<GameState>()
            .expect("GameState resource must be registered")
            .outcome = Outcome::Victory;
        log(
            world,
            "[fg:32CD32][b]Victory![/] All enemies defeated.[/fg]",
        );
    }
}

fn award_equipment_set_reward(
    world: &mut World,
    defeated_class: CharacterClass,
    defeated_name: &str,
) {
    let Some((hero_class, equipment)) =
        crate::equipment::set_reward_for_defeated_class(defeated_class)
    else {
        return;
    };
    let item_name = equipment.name.clone();
    let Some(hero) = world
        .query2::<Team, CharacterClass>()
        .iter()
        .find(|(_, team, class)| **team == Team::Player && **class == hero_class)
        .map(|(entity, _, _)| *entity)
    else {
        return;
    };
    if let Some(equipped) = world.get_mut::<EquippedItems>(hero) {
        equipped.equip(equipment);
        log(
            world,
            format!(
                "{} equipped {} from {}.",
                Game::get_class_name(hero_class),
                item_name,
                defeated_name
            ),
        );
    }
}

pub fn apply_spread_status(world: &mut World, target: Entity, new_status: ElementalStatus) {
    if !world.is_alive(target) {
        return;
    }
    let old_status = world
        .get::<ElementalStatus>(target)
        .copied()
        .unwrap_or(ElementalStatus::None);
    if old_status == new_status {
        let old_dur = match old_status {
            ElementalStatus::Ice { duration } => duration,
            ElementalStatus::Lightning { duration } => duration,
            ElementalStatus::Nature { duration } => duration,
            ElementalStatus::Poison { duration } => duration,
            ElementalStatus::Regen { duration } => duration,
            _ => 0,
        };
        let new_dur = match new_status {
            ElementalStatus::Ice { duration } => duration,
            ElementalStatus::Lightning { duration } => duration,
            ElementalStatus::Nature { duration } => duration,
            ElementalStatus::Poison { duration } => duration,
            ElementalStatus::Regen { duration } => duration,
            _ => 0,
        };
        if new_dur > old_dur {
            world.insert(target, new_status);
        }
        return;
    }

    let target_class = world
        .get::<CharacterClass>(target)
        .copied()
        .unwrap_or(CharacterClass::Warrior);
    let target_name = Game::get_class_name(target_class);
    let target_pos = world
        .get::<Position>(target)
        .copied()
        .unwrap_or(Position::new(0, 0));

    match (old_status, new_status) {
        // Shatter: Ice + Lightning
        (ElementalStatus::Ice { .. }, ElementalStatus::Lightning { .. })
        | (ElementalStatus::Lightning { .. }, ElementalStatus::Ice { .. }) => {
            log(
                world,
                format!(
                    "[fg:64C8FF][b]Elemental Reaction: SHATTER[/] on {} via spread![/fg]",
                    target_name
                ),
            );
            let bonus_damage = 30;
            let mut defeated = false;
            if let Some(stats) = world.get_mut::<Stats>(target) {
                stats.hp -= bonus_damage;
                if stats.hp <= 0 {
                    defeated = true;
                }
            }
            let (tx, ty) = get_tile_center_pixels(world, target_pos);
            if let Some(vfx) = world.resource_mut::<VfxSystem>() {
                vfx.floating_texts
                    .push(verryte_terminal::vfx::FloatingText::new(
                        tx,
                        ty - 1.0,
                        &format!("SHATTER! -{}", bonus_damage),
                        Color(100, 200, 255),
                        true,
                    ));
                vfx.particles
                    .extend(verryte_terminal::vfx::emit_shatter(tx, ty, 20));
            }
            if let Some(events) = world.resource_mut::<Events<GameEvent>>() {
                events.send(GameEvent::ReactionTriggered {
                    entity: target,
                    reaction: "Shatter".to_owned(),
                    damage: bonus_damage,
                    healing: 0,
                });
            }
            world.insert(target, ElementalStatus::None);
            if defeated {
                handle_defeat(world, target, target_name, target_class, target_pos);
            }
        }
        // Overgrowth: Lightning + Nature
        (ElementalStatus::Lightning { .. }, ElementalStatus::Nature { .. })
        | (ElementalStatus::Nature { .. }, ElementalStatus::Lightning { .. }) => {
            log(
                world,
                format!(
                    "[fg:32DC64][b]Elemental Reaction: OVERGROWTH[/] on {} via spread![/fg]",
                    target_name
                ),
            );
            let bonus_damage = 10;
            let mut defeated = false;
            if let Some(stats) = world.get_mut::<Stats>(target) {
                stats.hp -= bonus_damage;
                if stats.hp <= 0 {
                    defeated = true;
                }
            }
            world.insert(target, Rooted { duration: 1 });
            let (tx, ty) = get_tile_center_pixels(world, target_pos);
            if let Some(vfx) = world.resource_mut::<VfxSystem>() {
                vfx.floating_texts
                    .push(verryte_terminal::vfx::FloatingText::new(
                        tx,
                        ty - 1.0,
                        &format!("OVERGROWTH! -{} [ROOTED]", bonus_damage),
                        Color(50, 220, 100),
                        true,
                    ));
                vfx.particles
                    .extend(verryte_terminal::vfx::emit_bloom(tx, ty, 10));
            }
            if let Some(events) = world.resource_mut::<Events<GameEvent>>() {
                events.send(GameEvent::ReactionTriggered {
                    entity: target,
                    reaction: "Overgrowth".to_owned(),
                    damage: bonus_damage,
                    healing: 0,
                });
            }
            world.insert(target, ElementalStatus::None);
            if defeated {
                handle_defeat(world, target, target_name, target_class, target_pos);
            }
        }
        // Bloom: Nature + Ice
        (ElementalStatus::Nature { .. }, ElementalStatus::Ice { .. })
        | (ElementalStatus::Ice { .. }, ElementalStatus::Nature { .. }) => {
            log(
                world,
                format!(
                    "[fg:FFD700][b]Elemental Reaction: BLOOM[/] on {} via spread![/fg]",
                    target_name
                ),
            );
            let healing_amount = 20;
            let mut allies = Vec::new();
            let target_team = world.get::<Team>(target).copied().unwrap_or(Team::Player);
            for (e, p, team) in world.query2::<Position, Team>() {
                if *team == target_team {
                    let dist = (p.x - target_pos.x).abs() + (p.y - target_pos.y).abs();
                    if dist <= 1 {
                        allies.push(e);
                    }
                }
            }
            for ally in allies {
                let a_class = world
                    .get::<CharacterClass>(ally)
                    .copied()
                    .unwrap_or(CharacterClass::Warrior);
                let a_pos = world
                    .get::<Position>(ally)
                    .copied()
                    .unwrap_or(Position::new(0, 0));
                let mut final_hp = 0;
                if let Some(stats) = world.get_mut::<Stats>(ally) {
                    stats.hp = std::cmp::min(stats.max_hp, stats.hp + healing_amount);
                    final_hp = stats.hp;
                }
                log(
                    world,
                    format!(
                        "[fg:32FF32]Bloom healed {} for [b]{} HP![/] (HP: {})[/fg]",
                        Game::get_class_name(a_class),
                        healing_amount,
                        final_hp
                    ),
                );
                let (ax, ay) = get_tile_center_pixels(world, a_pos);
                if let Some(vfx) = world.resource_mut::<VfxSystem>() {
                    vfx.floating_texts
                        .push(verryte_terminal::vfx::FloatingText::new(
                            ax,
                            ay - 2.0,
                            &format!("+{} (Bloom)", healing_amount),
                            Color(50, 255, 50),
                            true,
                        ));
                    vfx.particles
                        .extend(verryte_terminal::vfx::emit_bloom(ax, ay, 8));
                }
            }
            if let Some(events) = world.resource_mut::<Events<GameEvent>>() {
                events.send(GameEvent::ReactionTriggered {
                    entity: target,
                    reaction: "Bloom".to_owned(),
                    damage: 0,
                    healing: healing_amount,
                });
            }
            world.insert(target, ElementalStatus::None);
        }
        _ => {
            world.insert(target, new_status);
            let badge = match new_status {
                ElementalStatus::Ice { .. } => "Ice",
                ElementalStatus::Lightning { .. } => "Lightning",
                ElementalStatus::Nature { .. } => "Nature",
                ElementalStatus::Poison { .. } => "Poison",
                ElementalStatus::Regen { .. } => "Regen",
                _ => "None",
            };
            let color_hex = match badge {
                "Ice" => "64C8FF",
                "Lightning" => "FFFF64",
                "Nature" => "32DC64",
                "Poison" => "A020F0",
                "Regen" => "32CD32",
                _ => "FFFFFF",
            };
            log(
                world,
                format!(
                    "Applied [fg:{}][b]{}[/] element to {} via spread.",
                    color_hex, badge, target_name
                ),
            );
            if let Some(events) = world.resource_mut::<Events<GameEvent>>() {
                events.send(GameEvent::ElementalApplied {
                    entity: target,
                    status: new_status,
                });
            }
        }
    }
}

pub fn process_team_status_effects(world: &mut World, team: Team) {
    let mut targets = Vec::new();
    for (e, pos, t, class) in world.query3::<Position, Team, CharacterClass>() {
        if *t == team {
            if let Some(status) = world.get::<ElementalStatus>(e).copied() {
                if status != ElementalStatus::None {
                    targets.push((e, status, *pos, *class));
                }
            }
        }
    }

    for (entity, original_status, pos, class) in targets {
        if !world.is_alive(entity) {
            continue;
        }

        let mut current_hp = world.get::<Stats>(entity).map(|s| s.hp).unwrap_or(0);
        if current_hp <= 0 {
            continue;
        }

        let target_name = Game::get_class_name(class);
        let mut defeated = false;
        let mut next_status = ElementalStatus::None;

        match original_status {
            ElementalStatus::Poison { duration } => {
                if let Some(stats) = world.get_mut::<Stats>(entity) {
                    stats.hp = (stats.hp - 10).max(0);
                    current_hp = stats.hp;
                    if stats.hp <= 0 {
                        defeated = true;
                    }
                }
                log(
                    world,
                    format!(
                        "{} suffered [fg:A020F0][b]10 Poison damage![/] (HP: {})",
                        target_name, current_hp
                    ),
                );
                let (tx, ty) = get_tile_center_pixels(world, pos);
                if let Some(vfx) = world.resource_mut::<VfxSystem>() {
                    vfx.floating_texts
                        .push(verryte_terminal::vfx::FloatingText::new(
                            tx,
                            ty - 1.0,
                            "-10 (Poison)",
                            Color(160, 32, 240),
                            true,
                        ));
                    vfx.particles.extend(verryte_terminal::vfx::emit_burst(
                        tx,
                        ty,
                        8,
                        Color(160, 32, 240),
                        &['*'],
                    ));
                }
                next_status = if duration > 1 && !defeated {
                    ElementalStatus::Poison {
                        duration: duration - 1,
                    }
                } else {
                    ElementalStatus::None
                };
            }
            ElementalStatus::Regen { duration } => {
                if let Some(stats) = world.get_mut::<Stats>(entity) {
                    stats.hp = (stats.hp + 10).min(stats.max_hp);
                    current_hp = stats.hp;
                }
                log(
                    world,
                    format!(
                        "{} healed for [fg:32CD32][b]10 HP[/] via Regen. (HP: {})",
                        target_name, current_hp
                    ),
                );
                let (tx, ty) = get_tile_center_pixels(world, pos);
                if let Some(vfx) = world.resource_mut::<VfxSystem>() {
                    vfx.floating_texts
                        .push(verryte_terminal::vfx::FloatingText::new(
                            tx,
                            ty - 1.0,
                            "+10 (Regen)",
                            Color(50, 205, 50),
                            true,
                        ));
                    vfx.particles
                        .extend(verryte_terminal::vfx::emit_bloom(tx, ty, 8));
                }
                next_status = if duration > 1 {
                    ElementalStatus::Regen {
                        duration: duration - 1,
                    }
                } else {
                    ElementalStatus::None
                };
            }
            ElementalStatus::Ice { duration } => {
                next_status = if duration > 1 {
                    ElementalStatus::Ice {
                        duration: duration - 1,
                    }
                } else {
                    ElementalStatus::None
                };
            }
            ElementalStatus::Lightning { duration } => {
                next_status = if duration > 1 {
                    ElementalStatus::Lightning {
                        duration: duration - 1,
                    }
                } else {
                    ElementalStatus::None
                };
            }
            ElementalStatus::Nature { duration } => {
                next_status = if duration > 1 {
                    ElementalStatus::Nature {
                        duration: duration - 1,
                    }
                } else {
                    ElementalStatus::None
                };
            }
            ElementalStatus::None => {}
        }

        if !defeated {
            if let Some(status) = world.get_mut::<ElementalStatus>(entity) {
                *status = next_status;
            }
        } else {
            handle_defeat(world, entity, target_name, class, pos);
            continue;
        }

        let spread_status_opt = match original_status {
            ElementalStatus::Poison { duration } => Some(ElementalStatus::Poison {
                duration: duration.saturating_sub(1).max(1),
            }),
            ElementalStatus::Ice { duration } => Some(ElementalStatus::Ice {
                duration: duration.saturating_sub(1).max(1),
            }),
            ElementalStatus::Lightning { duration } => Some(ElementalStatus::Lightning {
                duration: duration.saturating_sub(1).max(1),
            }),
            ElementalStatus::Nature { duration } => Some(ElementalStatus::Nature {
                duration: duration.saturating_sub(1).max(1),
            }),
            _ => None,
        };

        if let Some(spread_status) = spread_status_opt {
            let mut adj_entities = Vec::new();
            for (other_e, other_pos) in world.query::<Position>() {
                if other_e != entity {
                    let dist = (pos.x - other_pos.x).abs() + (pos.y - other_pos.y).abs();
                    if dist == 1 && world.get::<Team>(other_e).is_some() {
                        adj_entities.push(other_e);
                    }
                }
            }

            for adj_e in adj_entities {
                apply_spread_status(world, adj_e, spread_status);
            }
        }
    }
}

pub fn is_flanking_position(world: &World, attacker_pos: Position, target_pos: Position) -> bool {
    let dist = (attacker_pos.x - target_pos.x).abs() + (attacker_pos.y - target_pos.y).abs();
    if dist != 1 {
        return false;
    }
    let dx = attacker_pos.x - target_pos.x;
    let dy = attacker_pos.y - target_pos.y;
    for (_other_e, other_pos, team) in world.query2::<Position, Team>() {
        if *team == Team::Enemy {
            let other_dist =
                (other_pos.x - target_pos.x).abs() + (other_pos.y - target_pos.y).abs();
            if other_dist == 1 {
                let odx = other_pos.x - target_pos.x;
                let ody = other_pos.y - target_pos.y;
                if (dx != 0 && dy == 0 && odx == 0 && ody != 0)
                    || (dx == 0 && dy != 0 && odx != 0 && ody == 0)
                {
                    return true;
                }
            }
        }
    }
    false
}

pub fn combo_detection_system(world: &mut World) {
    let mut players: Vec<(Entity, Position, CharacterClass)> = Vec::new();
    for (e, pos, team, class) in world.query3::<Position, Team, CharacterClass>() {
        if *team == Team::Player {
            players.push((e, *pos, *class));
        }
    }

    let warrior = players
        .iter()
        .find(|(_, _, c)| *c == CharacterClass::Warrior);
    let mage = players.iter().find(|(_, _, c)| *c == CharacterClass::Mage);
    let healer = players
        .iter()
        .find(|(_, _, c)| *c == CharacterClass::Healer);

    let adjacent = |a: &Position, b: &Position| (a.x - b.x).abs() + (a.y - b.y).abs() == 1;

    let mut combos: Vec<(ComboSkill, Vec<Entity>)> = Vec::new();

    let all_three = warrior.is_some() && mage.is_some() && healer.is_some();
    if all_three {
        let w = warrior.unwrap();
        let m = mage.unwrap();
        let h = healer.unwrap();
        let w_m = (w.1.x - m.1.x).abs() + (w.1.y - m.1.y).abs();
        let m_h = (m.1.x - h.1.x).abs() + (m.1.y - h.1.y).abs();
        let w_h = (w.1.x - h.1.x).abs() + (w.1.y - h.1.y).abs();
        let pairs_adjacent = (if w_m == 1 { 1 } else { 0 })
            + (if m_h == 1 { 1 } else { 0 })
            + (if w_h == 1 { 1 } else { 0 });
        if pairs_adjacent >= 2 || (w_m <= 2 && m_h <= 2 && w_h <= 2) {
            combos.push((ComboSkill::TrinityStrike, vec![w.0, m.0, h.0]));
        }
    }

    if combos.is_empty() {
        if let (Some(w), Some(m)) = (warrior, mage) {
            if adjacent(&w.1, &m.1) {
                combos.push((ComboSkill::BladeStorm, vec![w.0, m.0]));
            }
        }
        if let (Some(w), Some(h)) = (warrior, healer) {
            if adjacent(&w.1, &h.1) {
                combos.push((ComboSkill::HolySmite, vec![w.0, h.0]));
            }
        }
        if let (Some(m), Some(h)) = (mage, healer) {
            if adjacent(&m.1, &h.1) {
                combos.push((ComboSkill::ArcaneSanctuary, vec![m.0, h.0]));
            }
        }
    }

    if !combos.is_empty() {
        let names: Vec<String> = combos
            .iter()
            .map(|(s, _)| ComboSkillDef::for_skill(s).name.clone() + " available!")
            .collect();
        log(
            world,
            format!("[fg:FFD700][b]Combo Skills:[/] {}[/fg]", names.join(", ")),
        );
    }

    if let Some(resource) = world.resource_mut::<AvailableCombos>() {
        *resource = AvailableCombos { combos };
    } else {
        world.insert_resource(AvailableCombos { combos });
    }
}
