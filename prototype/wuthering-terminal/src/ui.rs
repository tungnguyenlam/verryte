use crate::components::{
    CharacterClass, ElementalStatus, GameState, Rooted, Stats, Team, TurnPhase,
};
use crate::game::Game;
use crate::map::{TacticalMap, Tile};
use verryte_core::{MessageLog, World};
use verryte_terminal::{Cell, Color, Grid};

pub fn render_hud(grid: &mut Grid, world: &World, term_w: u16, term_h: u16) {
    let state = world.resource::<GameState>().unwrap();
    let map = world.resource::<TacticalMap>().unwrap();

    // Draw HUD stationary at the bottom
    let hud_y = term_h.saturating_sub(6);
    let hud_w = term_w;
    let hud_h = 6u16;

    let hud_bg = Color(10, 10, 15);
    for dy in 0..hud_h {
        for dx in 0..hud_w {
            grid.put(dx, hud_y + dy, Cell::new(' ').with_bg(hud_bg));
        }
    }

    // Draw a rounded border around the HUD panel
    let hud_rect = verryte_terminal::Rect::new(0, hud_y, hud_w, hud_h);
    grid.draw_border_styled(
        hud_rect,
        verryte_terminal::BorderStyle::Rounded,
        Color(80, 120, 160),
        hud_bg,
    );
    // Drop shadow for visual depth
    grid.draw_shadow(hud_rect);

    let phase_str = match state.phase {
        TurnPhase::Player => "PLAYER PHASE",
        TurnPhase::Enemy => "ENEMY PHASE",
    };
    let phase_color = match state.phase {
        TurnPhase::Player => Color::GREEN,
        TurnPhase::Enemy => Color::RED,
    };

    let mut selection_str = "Selected: None".to_string();
    if let Some(sel_entity) = state.selected_entity {
        if let (Some(class), Some(stats)) = (
            world.get::<CharacterClass>(sel_entity),
            world.get::<Stats>(sel_entity),
        ) {
            let name = Game::get_class_name(*class);
            let elemental = world
                .get::<ElementalStatus>(sel_entity)
                .copied()
                .unwrap_or(ElementalStatus::None);
            let status_badge = match elemental {
                ElementalStatus::Ice { duration } => {
                    format!(" [ICE:{}]", duration)
                }
                ElementalStatus::Lightning { duration } => {
                    format!(" [LIGHTNING:{}]", duration)
                }
                ElementalStatus::Nature { duration } => {
                    format!(" [NATURE:{}]", duration)
                }
                _ => "".to_string(),
            };
            let is_rooted = world.get::<Rooted>(sel_entity).is_some();
            let is_stunned = world
                .get::<crate::components::Stunned>(sel_entity)
                .is_some();
            let root_badge = if is_stunned {
                " [STUNNED]"
            } else if is_rooted {
                " [ROOTED]"
            } else {
                ""
            };
            let shield_badge =
                if let Some(shield) = world.get::<crate::components::ElementalShield>(sel_entity) {
                    format!(
                        " [SHIELD: {:?} ({}/{})]",
                        shield.shield_type, shield.amount, shield.max_amount
                    )
                } else {
                    "".to_string()
                };
            selection_str = format!(
                "Selected: {} (Lvl {} | HP: {}/{}, AP: {}/{}){}{}{}",
                name,
                stats.level,
                stats.hp,
                stats.max_hp,
                stats.ap,
                stats.max_ap,
                status_badge,
                root_badge,
                shield_badge
            );
        }
    }

    // Draw HUD line 1 components
    grid.write_str(
        2,
        hud_y + 1,
        &format!("TURN: {:02} | ", state.turn),
        Color::WHITE,
        hud_bg,
    );
    grid.write_str(
        13,
        hud_y + 1,
        &format!("PHASE: {:<12}", phase_str),
        phase_color,
        hud_bg,
    );
    grid.write_str(
        31,
        hud_y + 1,
        &format!(" | {}", selection_str),
        Color::WHITE,
        hud_bg,
    );

    let ce_pct = (state.concert_energy as f32 / 100.0).clamp(0.0, 1.0);
    let ce_color = if state.concert_energy >= 100 {
        Color(255, 215, 0) // Gold
    } else {
        Color(100, 200, 255) // Cyan-ish
    };

    grid.write_str(
        90,
        hud_y + 1,
        &format!(" | CONCERT: {:>3}/100 ", state.concert_energy),
        ce_color,
        hud_bg,
    );

    verryte_terminal::ProgressBar::new(verryte_terminal::Rect::new(110, hud_y + 1, 10, 1))
        .with_value(ce_pct)
        .with_colors(ce_color, Color(40, 40, 50))
        .render(grid);

    let hovered_tile = map.tile(state.cursor.x, state.cursor.y);
    let tile_type_str = match hovered_tile {
        Tile::Grass => "Grass",
        Tile::Wall => "Wall",
        Tile::Water => "Water",
    };

    let hovered_str = if let Some((target_entity, target_team, target_stats, target_class)) =
        get_entity_at(world, state.cursor)
    {
        let name = Game::get_class_name(target_class);
        let team_str = match target_team {
            Team::Player => "Player",
            Team::Enemy => "Enemy",
        };
        let elemental = world
            .query::<ElementalStatus>()
            .into_iter()
            .find(|(e, _)| {
                world
                    .get::<crate::Position>(*e)
                    .map(|p| *p == state.cursor)
                    .unwrap_or(false)
            })
            .map(|(_, s)| *s)
            .unwrap_or(ElementalStatus::None);

        let status_badge = match elemental {
            ElementalStatus::Ice { duration } => {
                format!(" [ICE:{}]", duration)
            }
            ElementalStatus::Lightning { duration } => {
                format!(" [LIGHTNING:{}]", duration)
            }
            ElementalStatus::Nature { duration } => {
                format!(" [NATURE:{}]", duration)
            }
            _ => "".to_string(),
        };
        let is_rooted = world.query::<Rooted>().into_iter().any(|(e, _)| {
            world
                .get::<crate::Position>(e)
                .map(|p| *p == state.cursor)
                .unwrap_or(false)
        });
        let is_stunned = world
            .query::<crate::components::Stunned>()
            .into_iter()
            .any(|(e, _)| {
                world
                    .get::<crate::Position>(e)
                    .map(|p| *p == state.cursor)
                    .unwrap_or(false)
            });
        let root_badge = if is_stunned {
            " [STUNNED]"
        } else if is_rooted {
            " [ROOTED]"
        } else {
            ""
        };
        let shield_badge =
            if let Some(shield) = world.get::<crate::components::ElementalShield>(target_entity) {
                format!(
                    " [SHIELD: {:?} ({}/{})]",
                    shield.shield_type, shield.amount, shield.max_amount
                )
            } else {
                "".to_string()
            };
        format!(
            "Tile: {} | Entity: {} (HP: {}/{}, AP: {}/{}, Team: {}){}{}{}",
            tile_type_str,
            name,
            target_stats.hp,
            target_stats.max_hp,
            target_stats.ap,
            target_stats.max_ap,
            team_str,
            status_badge,
            root_badge,
            shield_badge
        )
    } else {
        format!("Tile: {} | Entity: None", tile_type_str)
    };

    // Draw HUD line 2 components
    grid.write_str(
        2,
        hud_y + 2,
        &format!("CURSOR: ({:02}, {:02}) | ", state.cursor.x, state.cursor.y),
        Color::CYAN,
        hud_bg,
    );
    grid.write_str(20, hud_y + 2, &hovered_str, Color::WHITE, hud_bg);

    let mut echo_str = "None".to_string();
    if let Some(echoes) = world.resource::<crate::components::EquippedEchoes>() {
        if !echoes.abilities.is_empty() {
            echo_str = echoes
                .abilities
                .iter()
                .map(|a| format!("{:?}", a))
                .collect::<Vec<_>>()
                .join(", ");
        }
    }
    grid.write_str(
        80,
        hud_y + 2,
        &format!(" | ECHOES: {}", echo_str),
        Color::YELLOW,
        hud_bg,
    );

    // AI / Recording indicators
    if state.auto_battle {
        grid.write_str(
            term_w.saturating_sub(8),
            hud_y + 1,
            " AUTO ",
            Color::BLACK,
            Color::YELLOW,
        );
    }
    if state.is_recording {
        grid.write_str(
            term_w.saturating_sub(16),
            hud_y + 1,
            " REC ",
            Color::WHITE,
            Color::RED,
        );
    }
    if let Some(replay) = world.resource::<crate::components::ReplayState>() {
        if replay.active {
            grid.write_str(
                term_w.saturating_sub(26),
                hud_y + 1,
                " REPLAY ",
                Color::BLACK,
                Color::CYAN,
            );
        }
    }

    if let Some(log) = world.resource::<MessageLog>() {
        let view = verryte_terminal::MessageLogView::new(verryte_terminal::Rect::new(
            0,
            hud_y + 3,
            term_w,
            3,
        ))
        .with_colors(Color::YELLOW, hud_bg);
        view.render(grid, log.messages());
    }

    // Render Overlay if in Inventory state
    if state.ui_state == crate::components::UIState::Inventory {
        render_inventory(grid, world, term_w, term_h);
    }
}

pub fn render_inventory(grid: &mut Grid, world: &World, term_w: u16, term_h: u16) {
    let state = world.resource::<GameState>().unwrap();
    let Some(selected) = state.selected_entity else {
        return;
    };
    let Some(inventory) = world.get::<crate::components::Inventory>(selected) else {
        return;
    };

    let layout = verryte_terminal::Layout::vertical()
        .add_percent(20)
        .add_percent(60)
        .add_percent(20)
        .split(verryte_terminal::Rect::new(0, 0, term_w, term_h));

    let main_rect = layout[1];
    let sub_layout = verryte_terminal::Layout::horizontal()
        .add_percent(25)
        .add_percent(50)
        .add_percent(25)
        .split(main_rect);

    let panel_rect = sub_layout[1];
    let panel_bg = Color(20, 20, 30);
    grid.fill_rect(panel_rect, Cell::new(' ').with_bg(panel_bg));
    grid.draw_rounded_panel(
        panel_rect,
        " INVENTORY ",
        Color::CYAN,
        panel_bg,
        Color::WHITE,
    );

    if inventory.items.is_empty() {
        grid.write_str(
            panel_rect.x + 2,
            panel_rect.y + 2,
            "Empty.",
            Color::GREY,
            panel_bg,
        );
    } else {
        for (i, item_entity) in inventory.items.iter().enumerate() {
            if let Some(item) = world.get::<crate::components::Item>(*item_entity) {
                let y = panel_rect.y + 2 + i as u16;
                if y < panel_rect.bottom() - 1 {
                    let shortcut = format!("[{}]", i + 1);
                    grid.write_str(panel_rect.x + 2, y, &shortcut, Color::YELLOW, panel_bg);
                    grid.write_str(panel_rect.x + 6, y, &item.name, Color::WHITE, panel_bg);

                    let effect_str = match item.effect {
                        crate::components::ItemEffect::Heal(v) => format!("(Heal {})", v),
                        crate::components::ItemEffect::ReplenishAp(v) => format!("(AP +{})", v),
                        crate::components::ItemEffect::Cleanse => "(Cleanse)".to_string(),
                    };
                    grid.write_str(panel_rect.x + 25, y, &effect_str, Color::GREY, panel_bg);
                }
            }
        }
    }

    grid.write_str(
        panel_rect.x + 2,
        panel_rect.bottom() - 2,
        "Press [1-9] to use, [i] to close.",
        Color::CYAN,
        panel_bg,
    );
}

fn get_entity_at(
    world: &World,
    pos: crate::Position,
) -> Option<(verryte_core::Entity, Team, Stats, CharacterClass)> {
    for (e, p, team) in world.query2::<crate::Position, Team>() {
        if *p == pos {
            let stats = world.get::<Stats>(e)?.clone();
            let class = *world.get::<CharacterClass>(e)?;
            return Some((e, *team, stats, class));
        }
    }
    None
}
