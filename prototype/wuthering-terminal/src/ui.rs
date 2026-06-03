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

    let turn_label = format!("FLOOR: {} | TURN: {:02} | ", state.floor, state.turn);
    let phase_label = format!("PHASE: {:<12}", phase_str);

    let combo_label = if state.combo_count > 0 {
        format!(
            " | COMBO: x{} (+{}%)",
            state.combo_count,
            state.combo_count * 5
        )
    } else {
        "".to_string()
    };
    let combo_color = Color(255, 120, 50);

    let ce_pct = (state.concert_energy as f32 / 100.0).clamp(0.0, 1.0);
    let ce_color = if state.concert_energy >= 100 {
        Color(255, 215, 0)
    } else {
        Color(100, 200, 255)
    };
    let ce_label = format!(" | CONCERT: {:>3}/100 ", state.concert_energy);
    let bar_w: u16 = 10;
    let ce_total = ce_label.len() as u16 + bar_w + 1;
    let ce_x = term_w.saturating_sub(ce_total + 2);

    let turn_x = 2u16;
    let phase_x = turn_x + turn_label.len() as u16;
    let combo_x = phase_x + phase_label.len() as u16;
    let sel_x = combo_x + combo_label.len() as u16 + 1;

    if turn_x < ce_x {
        grid.write_str(turn_x, hud_y + 1, &turn_label, Color::WHITE, hud_bg);
    }
    if phase_x < ce_x {
        grid.write_str(phase_x, hud_y + 1, &phase_label, phase_color, hud_bg);
    }
    if combo_x < ce_x && !combo_label.is_empty() {
        grid.write_str(combo_x, hud_y + 1, &combo_label, combo_color, hud_bg);
    }
    if sel_x < ce_x {
        let max_sel = (ce_x.saturating_sub(sel_x + 1)) as usize;
        let sel_truncated = if selection_str.len() > max_sel && max_sel > 3 {
            format!("{}...", &selection_str[..max_sel - 3])
        } else {
            selection_str.clone()
        };
        grid.write_str(sel_x, hud_y + 1, &sel_truncated, Color::WHITE, hud_bg);
    }

    if ce_x > 2 {
        grid.write_str(ce_x, hud_y + 1, &ce_label, ce_color, hud_bg);
        let bar_x = ce_x + ce_label.len() as u16;
        if bar_x + bar_w <= term_w {
            verryte_terminal::ProgressBar::new(verryte_terminal::Rect::new(
                bar_x,
                hud_y + 1,
                bar_w,
                1,
            ))
            .with_value(ce_pct)
            .with_colors(ce_color, Color(40, 40, 50))
            .render(grid);
        }
    }

    let hovered_tile = map.tile(state.cursor.x, state.cursor.y);
    let tile_type_str = match hovered_tile {
        Tile::Grass => "Grass",
        Tile::Wall => "Wall",
        Tile::Water => "Water",
        Tile::Lava => "Lava",
        Tile::Ice => "Ice",
        Tile::Stairs => "Stairs",
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
    let cursor_label = format!("CURSOR: ({:02}, {:02}) | ", state.cursor.x, state.cursor.y);
    grid.write_str(2, hud_y + 2, &cursor_label, Color::CYAN, hud_bg);

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
    let echo_label = format!(" | ECHOES: {}", echo_str);
    let echo_x = term_w.saturating_sub(echo_label.len() as u16 + 2);

    let entity_x = cursor_label.len() as u16 + 2;
    let max_entity = echo_x.saturating_sub(entity_x + 1) as usize;
    let truncated_hovered = if hovered_str.len() > max_entity && max_entity > 3 {
        format!("{}...", &hovered_str[..max_entity - 3])
    } else {
        hovered_str.clone()
    };
    grid.write_str(
        entity_x,
        hud_y + 2,
        &truncated_hovered,
        Color::WHITE,
        hud_bg,
    );

    if echo_x > entity_x + truncated_hovered.len() as u16 {
        grid.write_str(echo_x, hud_y + 2, &echo_label, Color::YELLOW, hud_bg);
    }

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
                        crate::components::ItemEffect::RestoreShield(st, v) => {
                            format!("(Shield +{} {:?})", v, st)
                        }
                        crate::components::ItemEffect::Combined(heal, ap) => {
                            format!("(+{} HP, +{} AP)", heal, ap)
                        }
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

pub fn render_minimap(grid: &mut Grid, world: &World, board_h: u16) {
    let map = world.resource::<TacticalMap>().unwrap();
    let state = world.resource::<GameState>().unwrap();
    let mm_w = map.width + 2;
    let mm_h = map.height + 2;
    let mm_x = grid.width().saturating_sub(mm_w + 1);
    let mm_y = 1u16;

    if mm_x == 0 || mm_y + mm_h > board_h {
        return;
    }

    let mm_bg = Color(5, 5, 10);
    for dy in 0..mm_h {
        for dx in 0..mm_w {
            grid.put(mm_x + dx, mm_y + dy, Cell::new(' ').with_bg(mm_bg));
        }
    }

    grid.draw_border_styled(
        verryte_terminal::Rect::new(mm_x, mm_y, mm_w, mm_h),
        verryte_terminal::BorderStyle::Rounded,
        Color(60, 80, 100),
        mm_bg,
    );

    let inner_x = mm_x + 1;
    let inner_y = mm_y + 1;

    let clock = world.resource::<verryte_core::GameClock>().unwrap();
    let ticks = clock.elapsed_ticks();

    for ty in 0..map.height {
        for tx in 0..map.width {
            let pt = crate::Position::new(tx as i16, ty as i16);
            let tile = map.tile(pt.x, pt.y);
            let (ch, fg) = match tile {
                Tile::Grass => ('·', Color(30, 60, 30)),
                Tile::Wall => ('#', Color(80, 80, 80)),
                Tile::Water => {
                    let phase = ((ticks + (tx as u64) * 3 + (ty as u64) * 7) / 10) % 3;
                    let glyph = match phase {
                        0 => '~',
                        1 => '≈',
                        _ => '∽',
                    };
                    (glyph, Color(40, 40, 120))
                }
                Tile::Lava => {
                    let phase = ((ticks + (tx as u64) * 3 + (ty as u64) * 7) / 8) % 3;
                    let glyph = match phase {
                        0 => '^',
                        1 => 'v',
                        _ => '*',
                    };
                    (glyph, Color(180, 40, 20))
                }
                Tile::Ice => ('-', Color(150, 220, 255)),
                Tile::Stairs => ('>', Color(255, 215, 0)),
            };
            grid.put(
                inner_x + tx,
                inner_y + ty,
                Cell::new(ch).with_fg(fg).with_bg(mm_bg),
            );
        }
    }

    for (_e, pos, team) in world.query2::<crate::Position, Team>() {
        let (ch, fg) = match team {
            Team::Player => ('P', Color::GREEN),
            Team::Enemy => ('E', Color::RED),
        };
        let px = inner_x + pos.x as u16;
        let py = inner_y + pos.y as u16;
        if px < grid.width() && py < grid.height() {
            grid.put(
                px,
                py,
                Cell::new(ch)
                    .with_fg(fg)
                    .with_bg(mm_bg)
                    .with_attrs(verryte_terminal::CellAttrs::NONE.bold()),
            );
        }
    }

    let cx = inner_x + state.cursor.x as u16;
    let cy = inner_y + state.cursor.y as u16;
    if cx < grid.width() && cy < grid.height() {
        grid.put(
            cx,
            cy,
            Cell::new('X')
                .with_fg(Color::YELLOW)
                .with_bg(mm_bg)
                .with_attrs(verryte_terminal::CellAttrs::NONE.bold()),
        );
    }
}
