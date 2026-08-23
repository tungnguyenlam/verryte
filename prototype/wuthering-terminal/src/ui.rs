use crate::components::{
    BattleStats, CharacterClass, ElementalStatus, GameState, Outcome, Rooted, Stats, Team,
    TurnPhase, Weather, WeatherType,
};
use crate::game::Game;
use crate::map::{TacticalMap, Tile};
use verryte_core::{MessageLog, World};
use verryte_terminal::{Camera, Cell, CellAttrs, Color, Grid};

pub fn get_tile_animated_cell(tile: Tile, x: u16, y: u16, ticks: u64) -> Option<Cell> {
    match tile {
        Tile::Water => {
            let phase = ((ticks + (x as u64) * 5 + (y as u64) * 11) / 6) % 4;
            let (glyph, fg) = match phase {
                0 => ('~', Color(60, 80, 180)),
                1 => ('≈', Color(80, 100, 200)),
                2 => ('∽', Color(50, 70, 160)),
                _ => ('~', Color(70, 90, 190)),
            };
            Some(
                Cell::new(glyph)
                    .with_fg(fg)
                    .with_bg(Color(20, 30, 80))
                    .with_attrs(CellAttrs::NONE),
            )
        }
        Tile::Lava => {
            let pulse = ((ticks + (x as u64) * 3 + (y as u64) * 7) / 5) % 6;
            let (glyph, fg, bg) = match pulse {
                0 => ('^', Color(255, 80, 20), Color(180, 30, 10)),
                1 => ('*', Color(255, 120, 30), Color(200, 50, 10)),
                2 => ('v', Color(255, 60, 10), Color(160, 20, 5)),
                3 => ('^', Color(255, 140, 40), Color(220, 60, 15)),
                4 => ('*', Color(255, 100, 20), Color(190, 40, 10)),
                _ => ('v', Color(255, 70, 15), Color(170, 25, 8)),
            };
            Some(
                Cell::new(glyph)
                    .with_fg(fg)
                    .with_bg(bg)
                    .with_attrs(CellAttrs::NONE.bold()),
            )
        }
        Tile::Ice => {
            let sparkle = ((ticks + (x as u64) * 13 + (y as u64) * 17) / 15) % 20;
            if sparkle == 0 {
                Some(
                    Cell::new('✦')
                        .with_fg(Color(255, 255, 255))
                        .with_bg(Color(100, 160, 220))
                        .with_attrs(CellAttrs::NONE.bold()),
                )
            } else if sparkle == 10 {
                Some(
                    Cell::new('·')
                        .with_fg(Color(220, 240, 255))
                        .with_bg(Color(90, 150, 210))
                        .with_attrs(CellAttrs::NONE),
                )
            } else {
                Some(
                    Cell::new('-')
                        .with_fg(Color(150, 220, 255))
                        .with_bg(Color(80, 140, 200))
                        .with_attrs(CellAttrs::NONE),
                )
            }
        }
        Tile::Mud => {
            let cycle = ((ticks + (x as u64) * 7 + (y as u64) * 3) / 12) % 4;
            let bg = match cycle {
                0 => Color(60, 40, 20),
                1 => Color(70, 48, 25),
                2 => Color(55, 38, 18),
                _ => Color(65, 44, 22),
            };
            Some(
                Cell::new('=')
                    .with_fg(Color(100, 70, 40))
                    .with_bg(bg)
                    .with_attrs(CellAttrs::NONE.dim()),
            )
        }
        _ => None,
    }
}

pub fn render_tile_overlays(
    grid: &mut Grid,
    map: &TacticalMap,
    ticks: u64,
    offset_x: u16,
    offset_y: u16,
    tile_w: u16,
    tile_h: u16,
) {
    for ty in 0..map.height {
        for tx in 0..map.width {
            let tile = map.tile(tx as i16, ty as i16);
            if let Some(anim_cell) = get_tile_animated_cell(tile, tx, ty, ticks) {
                let base_x = offset_x + tx * tile_w;
                let base_y = offset_y + ty * tile_h;
                for dy in 0..tile_h {
                    for dx in 0..tile_w {
                        let px = base_x + dx;
                        let py = base_y + dy;
                        if px < grid.width() && py < grid.height() {
                            if let Some(existing) = grid.get(px, py) {
                                if existing.glyph == ' ' || existing.bg == Color::BLACK {
                                    grid.put(px, py, anim_cell);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

pub fn render_hud(grid: &mut Grid, world: &World, term_w: u16, term_h: u16) {
    let event_label = world
        .resource::<crate::components::DynamicFloorEvents>()
        .and_then(|events| {
            events.pending.as_ref().map(|pending| {
                let answered = pending
                    .response
                    .map(|response| format!(" {}", response.display_name().to_uppercase()))
                    .unwrap_or_default();
                let pattern = pending
                    .kind
                    .telegraph_pattern_name()
                    .map(|name| format!(" {}", name.to_uppercase()))
                    .unwrap_or_default();
                format!(
                    " | EVENT T{}{}{}",
                    pending.resolves_on_turn, pattern, answered
                )
            })
        })
        .unwrap_or_default();
    let event_color = Color(255, 140, 0);
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
                ElementalStatus::Poison { duration } => {
                    format!(" [POISON:{}]", duration)
                }
                ElementalStatus::Regen { duration } => {
                    format!(" [REGEN:{}]", duration)
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

    let zoom = world.resource::<Camera>().map(|c| c.zoom).unwrap_or(1.0);
    let turn_label = format!(
        "FLOOR: {} | TURN: {:02} | ZOOM: {:.1}x | ",
        state.floor, state.turn, zoom
    );
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
    let event_x = combo_x + combo_label.len() as u16;
    let sel_x = event_x + event_label.len() as u16 + 1;

    if turn_x < ce_x {
        grid.write_str(turn_x, hud_y + 1, &turn_label, Color::WHITE, hud_bg);
    }
    if phase_x < ce_x {
        grid.write_str(phase_x, hud_y + 1, &phase_label, phase_color, hud_bg);
    }
    if combo_x < ce_x && !combo_label.is_empty() {
        grid.write_str(combo_x, hud_y + 1, &combo_label, combo_color, hud_bg);
    }
    if event_x < ce_x && !event_label.is_empty() {
        grid.write_str(event_x, hud_y + 1, &event_label, event_color, hud_bg);
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
        Tile::Mud => "Mud",
        Tile::SpikeTrap => "Spike Trap",
        Tile::PoisonCloud => "Poison Cloud",
        Tile::HealingSpring => "Healing Spring",
        Tile::CrackedFloor => "Cracked Floor",
        Tile::PressurePlate => "Pressure Plate",
        Tile::ThornBush => "Thorn Bush",
        Tile::SteamVent => "Steam Vent",
        Tile::ExplodingBarrel => "Exploding Barrel",
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
            ElementalStatus::Poison { duration } => {
                format!(" [POISON:{}]", duration)
            }
            ElementalStatus::Regen { duration } => {
                format!(" [REGEN:{}]", duration)
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

    // Render Overlay if in Help state
    if state.ui_state == crate::components::UIState::Help {
        render_help(grid, term_w, term_h);
    }

    // Render Overlay if in Bestiary state
    if state.ui_state == crate::components::UIState::Bestiary {
        render_bestiary(grid, world, term_w, term_h);
    }

    // Render Overlay if in SkillTree state
    if state.ui_state == crate::components::UIState::SkillTree {
        render_skill_tree(grid, world, term_w, term_h);
    }

    // Render Overlay if in CombatLog state
    if state.ui_state == crate::components::UIState::CombatLog {
        render_combat_log(grid, world, term_w, term_h);
    }

    // Render Overlay if in Console state
    if state.ui_state == crate::components::UIState::Console {
        render_console(grid, world, term_w, term_h);
    }

    // Render Overlay if in SaveLoadMenu state
    if state.ui_state == crate::components::UIState::SaveLoadMenu {
        render_save_load_menu(grid, world, term_w, term_h);
    }

    // Render Overlay if in InspectCharacter state
    if state.ui_state == crate::components::UIState::InspectCharacter {
        render_inspect_character(grid, world, term_w, term_h);
    }
}

fn event_item_hint(
    effect: &crate::components::ItemEffect,
    kind: &crate::components::FloorEventKind,
) -> Option<&'static str> {
    use crate::components::{FloorEventKind, FloorModifier, ItemEffect};
    match (effect, kind) {
        (
            ItemEffect::Cleanse | ItemEffect::CleanseAndHeal(_),
            FloorEventKind::ModifierSurge { .. },
        ) => Some("(Purify surge)"),
        (ItemEffect::RestoreShield(_, _), FloorEventKind::MiniBossIncursion { .. }) => {
            Some("(Bolster incursion)")
        }
        (ItemEffect::ReplenishAp(_), _) => Some("(Remote intercept)"),
        (
            ItemEffect::Heal(_),
            FloorEventKind::ModifierSurge {
                modifier: FloorModifier::HealingSurge,
                ..
            },
        ) => Some("(Channel surge)"),
        (ItemEffect::EventWard, _) => Some("(Brace event)"),
        _ => None,
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

    let pending_kind = world
        .resource::<crate::components::DynamicFloorEvents>()
        .and_then(|events| events.pending.as_ref())
        .filter(|pending| pending.response.is_none())
        .map(|pending| pending.kind.clone());

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

                    let mut effect_str = match item.effect {
                        crate::components::ItemEffect::Heal(v) => format!("(Heal {})", v),
                        crate::components::ItemEffect::ReplenishAp(v) => format!("(AP +{})", v),
                        crate::components::ItemEffect::Cleanse => "(Cleanse)".to_string(),
                        crate::components::ItemEffect::RestoreShield(st, v) => {
                            format!("(Shield +{} {:?})", v, st)
                        }
                        crate::components::ItemEffect::Combined(heal, ap) => {
                            format!("(+{} HP, +{} AP)", heal, ap)
                        }
                        crate::components::ItemEffect::CleanseAndHeal(v) => {
                            format!("(Cleanse, +{} HP)", v)
                        }
                        crate::components::ItemEffect::UpgradeKit => "(Upgrade Kit)".to_string(),
                        crate::components::ItemEffect::EventWard => "(Brace event)".to_string(),
                    };
                    if let Some(kind) = pending_kind.as_ref() {
                        if let Some(hint) = event_item_hint(&item.effect, kind) {
                            effect_str = format!("{} {}", effect_str, hint);
                        }
                    }
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

pub fn render_help(grid: &mut Grid, term_w: u16, term_h: u16) {
    let layout = verryte_terminal::Layout::vertical()
        .add_percent(5)
        .add_percent(90)
        .add_percent(5)
        .split(verryte_terminal::Rect::new(0, 0, term_w, term_h));

    let main_rect = layout[1];
    let sub_layout = verryte_terminal::Layout::horizontal()
        .add_percent(15)
        .add_percent(70)
        .add_percent(15)
        .split(main_rect);

    let panel_rect = sub_layout[1];
    let panel_bg = Color(15, 15, 25);
    grid.fill_rect(panel_rect, Cell::new(' ').with_bg(panel_bg));
    grid.draw_rounded_panel(
        panel_rect,
        " HELP / CONTROLS ",
        Color::CYAN,
        panel_bg,
        Color::WHITE,
    );

    let left_col_x = panel_rect.x + 2;
    let right_col_x = panel_rect.x + (panel_rect.width / 2) + 1;
    let mut y = panel_rect.y + 2;
    let bottom = panel_rect.bottom() - 2;

    let controls: &[(&str, &str)] = &[
        ("WASD / Arrows", "Move cursor"),
        ("Enter", "Confirm / Select"),
        ("Esc", "Cancel / Back"),
        ("E", "End Turn"),
        ("Tab", "Cycle characters"),
        ("1, 2, 3", "Skills (or QTE Swap)"),
        ("4, 5, 6", "Direct swap to char"),
        ("I", "Open inventory"),
        ("1-9 (in inv)", "Use inventory item"),
        ("F5", "Save game"),
        ("F9", "Load game"),
        ("B", "Auto battle toggle"),
        ("R", "Step to safety"),
        ("F", "Brace floor event"),
        ("N / [", "Intercept / Embrace event"),
        ("M", "Toggle minimap"),
        ("U / Y", "Undo / Redo"),
        ("F3", "Performance overlay"),
        (">", "Descend stairs"),
        ("F10", "Toggle recording"),
        ("F11 / F12 / P", "Replay controls"),
        ("?", "This help overlay"),
        ("Q", "Quit game"),
    ];

    #[allow(clippy::explicit_counter_loop)]
    for &(key, desc) in controls {
        if y >= bottom {
            break;
        }
        grid.write_str(left_col_x, y, key, Color::YELLOW, panel_bg);
        grid.write_str(left_col_x + 18, y, desc, Color::WHITE, panel_bg);
        y += 1;
    }

    let mut ry = panel_rect.y + 2;
    grid.write_str(right_col_x, ry, "MECHANICS", Color(255, 215, 0), panel_bg);
    ry += 2;

    let mechanics: &[&str] = &[
        "Move to an enemy tile to attack.",
        "Skills cost AP to use.",
        "",
        "QTE Swap: Costs 100 Concert",
        "Energy. Each character has an",
        "intro skill on swap-in.",
        "",
        "Telegraphed attacks shown in",
        "RED tiles. Attack the boss to",
        "Parry and stun it.",
        "",
        "Floor events warn one turn",
        "ahead in ORANGE. Brace, Intercept,",
        "or Embrace before they resolve.",
        "Ward Charm auto-braces.",
        "",
        "Defeated enemies may drop",
        "Echoes. Move a character to",
        "the tile to absorb and gain",
        "new abilities.",
        "",
        "Elemental reactions:",
        "  Ice + Lightning = Shatter",
        "  Lightning + Nature = Overgrow",
        "  Nature + Ice = Bloom",
        "",
        "Terrain costs: Water=2 AP,",
        "  Mud=3 AP, Ice is slippery.",
    ];

    for &line in mechanics {
        if ry >= bottom {
            break;
        }
        if line.is_empty() {
            ry += 1;
            continue;
        }
        let color = if line.starts_with("  ") {
            Color(200, 200, 200)
        } else {
            Color::WHITE
        };
        grid.write_str(right_col_x, ry, line, color, panel_bg);
        ry += 1;
    }

    grid.write_str(
        panel_rect.x + 2,
        bottom,
        "Press [?] or [Esc] to close.",
        Color::CYAN,
        panel_bg,
    );
}

pub fn render_battle_summary(
    grid: &mut Grid,
    world: &World,
    outcome: Outcome,
    term_w: u16,
    term_h: u16,
) {
    let panel_w: u16 = 48.min(term_w.saturating_sub(4));
    let panel_h: u16 = 20.min(term_h.saturating_sub(2));
    let panel_x = (term_w.saturating_sub(panel_w)) / 2;
    let panel_y = (term_h.saturating_sub(panel_h)) / 2;
    let panel_bg = Color(15, 15, 25);

    let rect = verryte_terminal::Rect::new(panel_x, panel_y, panel_w, panel_h);
    grid.fill_rect(rect, Cell::new(' ').with_bg(panel_bg));
    grid.draw_rounded_panel(
        rect,
        " BATTLE SUMMARY ",
        Color::CYAN,
        panel_bg,
        Color::WHITE,
    );

    let title = match outcome {
        Outcome::Victory => "VICTORY!",
        Outcome::Defeat => "DEFEAT",
        _ => "BATTLE ENDED",
    };
    let title_color = match outcome {
        Outcome::Victory => Color(255, 215, 0),
        Outcome::Defeat => Color(255, 50, 50),
        _ => Color::WHITE,
    };
    let title_x = panel_x + (panel_w.saturating_sub(title.len() as u16)) / 2;
    grid.write_str(title_x, panel_y + 2, title, title_color, panel_bg);

    let state = world.resource::<GameState>().unwrap();
    let stats = world.resource::<BattleStats>().cloned().unwrap_or_default();

    let echoes = world
        .resource::<crate::components::EquippedEchoes>()
        .map(|e| {
            e.abilities
                .iter()
                .map(|a| format!("{:?}", a))
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_else(|| "None".to_string());

    let left_x = panel_x + 3;
    let mut row = panel_y + 4;
    let label_color = Color(160, 180, 200);
    let _value_color = Color::WHITE;
    let line_gap = 1u16;

    let rows: Vec<(&str, String, Color)> = vec![
        ("Floor", format!("{}", state.floor), Color(200, 200, 200)),
        (
            "Turns",
            format!("{}", stats.total_turns),
            Color(200, 200, 200),
        ),
        (
            "Damage Dealt",
            format!("{}", stats.total_damage_dealt),
            Color(255, 120, 80),
        ),
        (
            "Damage Taken",
            format!("{}", stats.total_damage_taken),
            Color(255, 80, 80),
        ),
        (
            "Healing Done",
            format!("{}", stats.total_healing_done),
            Color(80, 255, 80),
        ),
        (
            "Enemies Killed",
            format!("{}", stats.total_kills),
            Color(255, 200, 50),
        ),
        (
            "Max Combo",
            format!("x{}", stats.max_combo_reached),
            Color(255, 160, 50),
        ),
        (
            "Team Swaps",
            format!("{}", stats.total_swaps),
            Color(153, 51, 255),
        ),
        (
            "Concert Energy",
            format!("{}/100", state.concert_energy),
            Color(100, 200, 255),
        ),
        ("Equipped Echoes", echoes, Color(255, 215, 0)),
    ];

    let max_label_w = rows.iter().map(|(l, _, _)| l.len()).max().unwrap_or(0) as u16;

    for (label, value, val_color) in &rows {
        if row >= panel_y + panel_h - 3 {
            break;
        }
        grid.write_str(left_x, row, label, label_color, panel_bg);
        let vx = left_x + max_label_w + 2;
        if vx < panel_x + panel_w - 2 {
            let max_val = (panel_x + panel_w - 2 - vx) as usize;
            let truncated = if value.len() > max_val && max_val > 3 {
                format!("{}...", &value[..max_val - 3])
            } else {
                value.clone()
            };
            grid.write_str(vx, row, &truncated, *val_color, panel_bg);
        }
        row += line_gap;
    }

    let hint = "Press any key to exit...";
    let hint_x = panel_x + (panel_w.saturating_sub(hint.len() as u16)) / 2;
    if panel_y + panel_h >= 2 {
        grid.write_str(
            hint_x,
            panel_y + panel_h - 2,
            hint,
            Color(100, 100, 120),
            panel_bg,
        );
    }
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
                Tile::Mud => ('=', Color(100, 70, 40)),
                Tile::SpikeTrap => ('^', Color(200, 50, 50)),
                Tile::PoisonCloud => ('~', Color(100, 200, 50)),
                Tile::HealingSpring => ('+', Color(50, 200, 50)),
                Tile::CrackedFloor => ('%', Color(120, 100, 80)),
                Tile::PressurePlate => ('_', Color(180, 180, 50)),
                Tile::ThornBush => ('*', Color(80, 120, 40)),
                Tile::SteamVent => ('v', Color(200, 130, 80)),
                Tile::ExplodingBarrel => ('o', Color(255, 128, 0)),
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

pub fn render_weather_danger_zones(
    grid: &mut Grid,
    world: &World,
    viewport: &verryte_terminal::TileViewport,
    tile_w: u16,
    tile_h: u16,
    ticks: u64,
) {
    let weather = match world.resource::<Weather>() {
        Some(w) => w,
        None => return,
    };
    for pos in &weather.danger_zones {
        let (sx, sy) = viewport.world_to_screen(pos.x as f32, pos.y as f32);
        for dy in 0..tile_h {
            for dx in 0..tile_w {
                let tx = sx + dx as i32;
                let ty = sy + dy as i32;
                if !viewport.rect.contains(tx as u16, ty as u16) {
                    continue;
                }
                if let Some(cell) = grid.get_mut(tx as u16, ty as u16) {
                    match weather.current {
                        WeatherType::LightningStorm => {
                            let flash = (ticks / 8).is_multiple_of(2);
                            cell.bg = verryte_terminal::vfx::blend_color(
                                cell.bg,
                                if flash {
                                    Color(255, 255, 100)
                                } else {
                                    Color(200, 50, 50)
                                },
                                0.5,
                            );
                            if dx == 0 && dy == 0 {
                                cell.fg = if flash {
                                    Color(200, 50, 50)
                                } else {
                                    Color(255, 255, 100)
                                };
                                cell.glyph = '!';
                            }
                        }
                        WeatherType::Rainy => {
                            cell.bg = verryte_terminal::vfx::blend_color(
                                cell.bg,
                                Color(30, 60, 150),
                                0.3,
                            );
                            if dx == 0 && dy == 0 {
                                cell.fg = Color(180, 200, 255);
                                cell.glyph = '~';
                            }
                        }
                        WeatherType::Snowing => {
                            cell.bg = verryte_terminal::vfx::blend_color(
                                cell.bg,
                                Color(180, 200, 220),
                                0.25,
                            );
                            if dx == 0 && dy == 0 {
                                let sparkle = (ticks.wrapping_add((pos.x + pos.y * 7) as u64) / 12)
                                    .is_multiple_of(2);
                                if sparkle {
                                    cell.fg = Color(220, 230, 255);
                                    cell.glyph = '*';
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}

pub fn render_skill_tree(grid: &mut Grid, world: &World, term_w: u16, term_h: u16) {
    let state = world.resource::<GameState>().unwrap();
    let Some(selected) = state.selected_entity else {
        return;
    };
    let Some(skill_tree) = world.get::<crate::components::SkillTree>(selected) else {
        return;
    };
    let Some(class) = world.get::<CharacterClass>(selected) else {
        return;
    };
    let char_name = Game::get_class_name(*class);

    let layout = verryte_terminal::Layout::vertical()
        .add_percent(15)
        .add_percent(70)
        .add_percent(15)
        .split(verryte_terminal::Rect::new(0, 0, term_w, term_h));

    let main_rect = layout[1];
    let sub_layout = verryte_terminal::Layout::horizontal()
        .add_percent(15)
        .add_percent(70)
        .add_percent(15)
        .split(main_rect);

    let panel_rect = sub_layout[1];
    let panel_bg = Color(15, 25, 35);
    grid.fill_rect(panel_rect, Cell::new(' ').with_bg(panel_bg));
    grid.draw_rounded_panel(
        panel_rect,
        &format!(
            " SKILL TREE: {} (SP: {}) ",
            char_name.to_uppercase(),
            skill_tree.skill_points
        ),
        Color(0, 191, 255), // Deep sky blue
        panel_bg,
        Color::WHITE,
    );

    use crate::components::SkillSlot;
    let slots = [
        (SkillSlot::Skill1, "Skill 1 (Skill1)"),
        (SkillSlot::Skill2, "Skill 2 (Skill2)"),
        (SkillSlot::Skill3, "Skill 3 (Skill3)"),
        (SkillSlot::Passive, "Passive (Passive)"),
    ];

    let start_x = panel_rect.x + 2;
    let mut current_y = panel_rect.y + 2;

    for (slot, slot_name) in slots {
        if current_y + 4 >= panel_rect.y + panel_rect.height {
            break;
        }

        let _ = grid.write_rich(
            start_x,
            current_y,
            &format!("[fg:00BFFF][b]{}[/fg][/b]", slot_name),
        );
        current_y += 1;

        let mut slot_upgrades: Vec<_> = skill_tree
            .upgrades
            .iter()
            .filter(|u| u.skill_slot == slot)
            .collect();
        slot_upgrades.sort_by_key(|u| u.tier);

        for u in slot_upgrades {
            let status = if u.unlocked {
                "[fg:50FF50]UNLOCKED[/fg]"
            } else if skill_tree.can_unlock(&u.upgrade_id) {
                &format!("[fg:FFFF50]AVAILABLE (Cost: {})[/fg]", u.cost)
            } else {
                &format!("[fg:888888]LOCKED (Cost: {})[/fg]", u.cost)
            };

            let line = format!(
                "  Tier {}: {} - {} - {}",
                u.tier, u.name, u.description, status
            );
            let _ = grid.write_rich(start_x, current_y, &line);
            current_y += 1;
        }
        current_y += 1; // spacer
    }

    let help_y = panel_rect.y + panel_rect.height - 2;
    let _ = grid.write_rich(
        start_x,
        help_y,
        "[fg:FFFFFF]Use script command: `upgrade:<s1|s2|s3|passive>_<tier>` | Press T/ESC to close[/fg]",
    );
}

pub fn render_bestiary(grid: &mut Grid, world: &World, term_w: u16, term_h: u16) {
    let bestiary = world.resource::<crate::components::Bestiary>();
    let lore_journal = world.resource::<crate::components::LoreJournal>();

    let layout = verryte_terminal::Layout::vertical()
        .add_percent(10)
        .add_percent(80)
        .add_percent(10)
        .split(verryte_terminal::Rect::new(0, 0, term_w, term_h));

    let main_rect = layout[1];
    let panel_bg = Color(20, 15, 25);
    grid.fill_rect(main_rect, Cell::new(' ').with_bg(panel_bg));
    grid.draw_rounded_panel(
        main_rect,
        " BESTIARY & LORE JOURNAL ",
        Color::MAGENTA,
        panel_bg,
        Color::WHITE,
    );

    let columns = verryte_terminal::Layout::horizontal()
        .add_percent(50)
        .add_percent(50)
        .split(verryte_terminal::Rect::new(
            main_rect.x + 1,
            main_rect.y + 1,
            main_rect.width - 2,
            main_rect.height - 2,
        ));

    let bestiary_rect = columns[0];
    let lore_rect = columns[1];

    let mut by = bestiary_rect.y + 1;
    let bx = bestiary_rect.x + 2;
    let _ = grid.write_rich(bx, by, "[fg:FF00FF][b]— ENEMY ENCYCLOPEDIA —[/fg][/b]");
    by += 2;

    if let Some(b) = bestiary {
        for entry in &b.entries {
            if by + 4 >= bestiary_rect.y + bestiary_rect.height {
                break;
            }
            if !entry.encountered {
                let _ = grid.write_rich(bx, by, "[fg:888888]??? (Not encountered yet)[/fg]");
                by += 2;
                continue;
            }

            let name_color = match entry.class {
                CharacterClass::Boss => "FF0000",
                CharacterClass::GlacialGolem | CharacterClass::CursedSentinel => "00FFFF",
                _ => "FFFFFF",
            };
            let _ = grid.write_rich(
                bx,
                by,
                &format!(
                    "[fg:{}][b]{}[/fg][/b] (Defeated: {}, Killed Players: {})",
                    name_color, entry.name, entry.defeated_count, entry.times_killed_by
                ),
            );
            by += 1;

            let weakness = entry.known_weakness.as_deref().unwrap_or("Unknown");
            let resistance = entry.known_resistance.as_deref().unwrap_or("Unknown");
            let _ = grid.write_rich(
                bx + 2,
                by,
                &format!(
                    "[fg:FF8888]Weakness:[/] {} | [fg:8888FF]Resistance:[/] {}",
                    weakness, resistance
                ),
            );
            by += 1;

            if !entry.drop_table.is_empty() {
                let drops = entry.drop_table.join(", ");
                let _ = grid.write_rich(
                    bx + 2,
                    by,
                    &format!("[fg:FFFF80]Notable Drops:[/] [fg:FFFFC0]{}[/fg]", drops),
                );
                by += 1;
            }
            by += 1; // spacer
        }
    } else {
        let _ = grid.write_rich(bx, by, "[fg:888888]No bestiary data loaded[/fg]");
    }

    let mut ly = lore_rect.y + 1;
    let lx = lore_rect.x + 2;
    let _ = grid.write_rich(lx, ly, "[fg:FFD700][b]— ARCHIVED KNOWLEDGE —[/fg][/b]");
    ly += 2;

    if let Some(journal) = lore_journal {
        for entry in &journal.entries {
            if ly + 3 >= lore_rect.y + lore_rect.height {
                break;
            }
            if !entry.discovered {
                let _ =
                    grid.write_rich(lx, ly, "[fg:888888]Locked Lore Entry (Play to unlock)[/fg]");
                ly += 2;
                continue;
            }

            let _ = grid.write_rich(
                lx,
                ly,
                &format!(
                    "[fg:FFD700][b]{}[/fg][/b] (Turn {})",
                    entry.title, entry.turn_discovered
                ),
            );
            ly += 1;

            let text = &entry.text;
            let max_w = (lore_rect.width - 6) as usize;
            let words = text.split_whitespace();
            let mut line = String::new();
            for word in words {
                if line.len() + word.len() + 1 > max_w {
                    let _ = grid.write_rich(lx + 2, ly, &format!("[fg:CCCCCC]{}[/fg]", line));
                    ly += 1;
                    line.clear();
                }
                if !line.is_empty() {
                    line.push(' ');
                }
                line.push_str(word);
            }
            if !line.is_empty() {
                let _ = grid.write_rich(lx + 2, ly, &format!("[fg:CCCCCC]{}[/fg]", line));
                ly += 1;
            }
            ly += 2; // spacer
        }
    } else {
        let _ = grid.write_rich(lx, ly, "[fg:888888]No lore journal data loaded[/fg]");
    }
}

pub fn render_combat_log(grid: &mut Grid, world: &World, term_w: u16, term_h: u16) {
    let state = world.resource::<GameState>().unwrap();
    let Some(log) = world.resource::<MessageLog>() else {
        return;
    };

    let main_rect = verryte_terminal::Rect::new(0, 0, term_w, term_h);
    let layout = verryte_terminal::Layout::horizontal()
        .add_percent(10)
        .add_percent(80)
        .add_percent(10)
        .split(main_rect);

    let sub_layout = verryte_terminal::Layout::vertical()
        .add_percent(10)
        .add_percent(80)
        .add_percent(10)
        .split(layout[1]);

    let panel_rect = sub_layout[1];
    let panel_bg = Color(15, 10, 20);
    grid.fill_rect(panel_rect, Cell::new(' ').with_bg(panel_bg));
    grid.draw_rounded_panel(
        panel_rect,
        " COMBAT HISTORY LOG (Up/Down to scroll, Esc/G to close) ",
        Color::YELLOW,
        panel_bg,
        Color::WHITE,
    );

    let messages = log.messages();
    let max_visible = panel_rect.height.saturating_sub(4) as usize;
    let len = messages.len();
    let scroll = state.log_scroll_offset.min(len.saturating_sub(max_visible));
    let end_idx = len.saturating_sub(scroll);
    let start_idx = end_idx.saturating_sub(max_visible);
    let visible_slice = &messages[start_idx..end_idx];

    for (y, msg) in (panel_rect.y + 2..).zip(visible_slice.iter()) {
        grid.write_str(panel_rect.x + 2, y, msg, Color::WHITE, panel_bg);
    }
}

pub fn render_console(grid: &mut Grid, world: &World, term_w: u16, term_h: u16) {
    let Some(input) = world.resource::<verryte_input::TextInput>() else {
        return;
    };

    let panel_h = 3;
    let panel_y = term_h.saturating_sub(panel_h);
    let panel_rect = verryte_terminal::Rect::new(
        2,
        panel_y.saturating_sub(1),
        term_w.saturating_sub(4),
        panel_h,
    );
    let panel_bg = Color(10, 20, 30);
    grid.fill_rect(panel_rect, Cell::new(' ').with_bg(panel_bg));
    grid.draw_rounded_panel(
        panel_rect,
        " CHEAT CONSOLE ",
        Color::CYAN,
        panel_bg,
        Color::WHITE,
    );

    grid.write_str(
        panel_rect.x + 2,
        panel_rect.y + 1,
        "> ",
        Color::YELLOW,
        panel_bg,
    );
    let text = input.text();
    grid.write_str(
        panel_rect.x + 4,
        panel_rect.y + 1,
        text,
        Color::WHITE,
        panel_bg,
    );

    // Draw cursor
    let cursor_pos = input.cursor();
    let cursor_x = panel_rect.x + 4 + cursor_pos as u16;
    if cursor_x < panel_rect.right() {
        if let Some(cell) = grid.get_mut(cursor_x, panel_rect.y + 1) {
            cell.bg = Color(255, 255, 255);
            cell.fg = Color(0, 0, 0);
        }
    }
}

pub fn render_save_load_menu(grid: &mut Grid, world: &World, term_w: u16, term_h: u16) {
    let layout = verryte_terminal::Layout::vertical()
        .add_percent(15)
        .add_percent(70)
        .add_percent(15)
        .split(verryte_terminal::Rect::new(0, 0, term_w, term_h));

    let main_rect = layout[1];
    let sub_layout = verryte_terminal::Layout::horizontal()
        .add_percent(25)
        .add_percent(50)
        .add_percent(25)
        .split(main_rect);

    let panel_rect = sub_layout[1];
    let panel_bg = Color(20, 20, 35);
    grid.fill_rect(panel_rect, Cell::new(' ').with_bg(panel_bg));
    grid.draw_rounded_panel(
        panel_rect,
        " SAVE / LOAD GAME ",
        Color::CYAN,
        panel_bg,
        Color::WHITE,
    );

    let Some(state) = world.resource::<GameState>() else {
        return;
    };

    let mut y = panel_rect.y + 3;
    for slot in 0..3 {
        let is_selected = state.selected_save_slot == slot;
        let slot_color = if is_selected {
            Color::YELLOW
        } else {
            Color::GREY
        };
        let prefix = if is_selected { "=> " } else { "   " };

        let info = get_slot_info(slot);

        let row_str = format!("{}{}", prefix, info);
        grid.write_str(panel_rect.x + 2, y, &row_str, slot_color, panel_bg);
        y += 2;
    }

    y = panel_rect.bottom() - 3;
    grid.write_str(panel_rect.x + 2, y, "Controls:", Color::CYAN, panel_bg);
    grid.write_str(
        panel_rect.x + 2,
        y + 1,
        "[↑/↓] Select Slot | [F5/S] Save | [F9/Enter] Load",
        Color::WHITE,
        panel_bg,
    );
}

fn get_slot_info(slot: usize) -> String {
    let base_path = crate::game::saves_dir();
    let filename = format!("save_slot_{}.json", slot);
    let path = format!("{}/{}", base_path, filename);
    if let Ok(content) = std::fs::read_to_string(&path) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&content) {
            let turn = v["world"]["resources"]["GameState"]["turn"]
                .as_u64()
                .unwrap_or(0);
            let floor = v["world"]["resources"]["GameState"]["floor"]
                .as_u64()
                .unwrap_or(0);
            let timestamp_str = v["timestamp"].as_str().unwrap_or("unknown");

            let time_desc = if let Ok(ts) = timestamp_str.parse::<u64>() {
                let formatted_time = format_unix_timestamp(ts);
                format!(
                    "Slot {}: Floor {}, Turn {} ({})",
                    slot + 1,
                    floor,
                    turn,
                    formatted_time
                )
            } else {
                format!("Slot {}: Floor {}, Turn {}", slot + 1, floor, turn)
            };
            return time_desc;
        }
    }
    format!("Slot {}: <Empty Slot>", slot + 1)
}

fn format_unix_timestamp(ts: u64) -> String {
    let hours = (ts / 3600) % 24;
    let minutes = (ts / 60) % 60;
    let seconds = ts % 60;
    format!("{:02}:{:02}:{:02} UTC", hours, minutes, seconds)
}

pub fn render_inspect_character(grid: &mut Grid, world: &World, term_w: u16, term_h: u16) {
    let layout = verryte_terminal::Layout::vertical()
        .add_percent(5)
        .add_percent(90)
        .add_percent(5)
        .split(verryte_terminal::Rect::new(0, 0, term_w, term_h));

    let main_rect = layout[1];
    let sub_layout = verryte_terminal::Layout::horizontal()
        .add_percent(15)
        .add_percent(70)
        .add_percent(15)
        .split(main_rect);

    let panel_rect = sub_layout[1];
    let panel_bg = Color(15, 20, 25);
    grid.fill_rect(panel_rect, Cell::new(' ').with_bg(panel_bg));
    grid.draw_rounded_panel(
        panel_rect,
        " CHARACTER SHEET ",
        Color::CYAN,
        panel_bg,
        Color::WHITE,
    );

    let Some(state) = world.resource::<GameState>() else {
        return;
    };

    let target_ent = if let Some(sel) = state.selected_entity {
        Some(sel)
    } else {
        world
            .query2::<Team, CharacterClass>()
            .iter()
            .find(|(_, team, _)| **team == Team::Player)
            .map(|(e, _, _)| *e)
    };

    let Some(ent) = target_ent else {
        grid.write_str(
            panel_rect.x + 4,
            panel_rect.y + 4,
            "No character selected.",
            Color::YELLOW,
            panel_bg,
        );
        return;
    };

    let class = world
        .get::<CharacterClass>(ent)
        .copied()
        .unwrap_or(CharacterClass::Warrior);
    let name = Game::get_class_name(class);

    let stats = world.get::<Stats>(ent).cloned().unwrap_or(Stats {
        hp: 0,
        max_hp: 0,
        atk: 0,
        def: 0,
        spd: 0,
        ap: 0,
        max_ap: 0,
        level: 1,
        xp: 0,
    });

    let header = format!("{}  --  Level {}", name, stats.level);
    grid.write_str(
        panel_rect.x + 4,
        panel_rect.y + 2,
        &header,
        Color::CYAN,
        panel_bg,
    );

    let mut y = panel_rect.y + 4;
    let xp_needed = stats.level * 100;

    grid.write_str(
        panel_rect.x + 4,
        y,
        &format!("HP:  {}/{}", stats.hp, stats.max_hp),
        Color::WHITE,
        panel_bg,
    );
    grid.write_str(
        panel_rect.x + 4,
        y + 1,
        &format!("AP:  {}/{}", stats.ap, stats.max_ap),
        Color::WHITE,
        panel_bg,
    );
    grid.write_str(
        panel_rect.x + 4,
        y + 2,
        &format!("XP:  {}/{}", stats.xp, xp_needed),
        Color::WHITE,
        panel_bg,
    );

    y += 4;
    grid.write_str(
        panel_rect.x + 4,
        y,
        &format!("Base ATK:  {}", stats.atk),
        Color::WHITE,
        panel_bg,
    );
    grid.write_str(
        panel_rect.x + 4,
        y + 1,
        &format!("Base DEF:  {}", stats.def),
        Color::WHITE,
        panel_bg,
    );
    grid.write_str(
        panel_rect.x + 4,
        y + 2,
        &format!("Base SPD:  {}", stats.spd),
        Color::WHITE,
        panel_bg,
    );

    y += 4;
    grid.write_str(
        panel_rect.x + 4,
        y,
        "--- TRAIT ---",
        Color::YELLOW,
        panel_bg,
    );
    if let Some(tr) = world.get::<crate::components::CharacterTrait>(ent) {
        let (trait_name, desc) = match tr.trait_type {
            crate::components::HeroTrait::SwiftFoot => ("Swift Foot", "+1 bonus AP on turn start"),
            crate::components::HeroTrait::StormChaser => {
                ("Storm Chaser", "+10 damage to Lightning reactions")
            }
            crate::components::HeroTrait::PurifyingTouch => {
                ("Purifying Touch", "50% chance to cleanse on healing")
            }
            crate::components::HeroTrait::IceWalker => {
                ("Ice Walker", "prevents sliding on Ice terrain")
            }
            crate::components::HeroTrait::ShadowStrike => {
                ("Shadow Strike", "+5 flanking attack damage")
            }
        };
        grid.write_str(
            panel_rect.x + 4,
            y + 1,
            &format!("{}: {}", trait_name, desc),
            Color::WHITE,
            panel_bg,
        );
    } else {
        grid.write_str(panel_rect.x + 4, y + 1, "None", Color::GREY, panel_bg);
    }

    let right_col = panel_rect.x + (panel_rect.width / 2) + 2;
    let mut ey = panel_rect.y + 4;
    grid.write_str(
        right_col,
        ey,
        "--- EQUIPPED GEAR ---",
        Color::YELLOW,
        panel_bg,
    );
    ey += 2;

    if let Some(equipped) = world.get::<crate::components::EquippedItems>(ent) {
        let slots = [
            ("Weapon", &equipped.weapon),
            ("Armor", &equipped.armor),
            ("Accessory", &equipped.accessory),
        ];

        for (slot_name, item_opt) in slots {
            grid.write_str(
                right_col,
                ey,
                &format!("{}:", slot_name),
                Color::CYAN,
                panel_bg,
            );
            if let Some(item) = item_opt {
                grid.write_str(right_col + 12, ey, &item.name, Color::WHITE, panel_bg);

                let mut bonuses = Vec::new();
                if item.atk_bonus > 0 {
                    bonuses.push(format!("+{} ATK", item.atk_bonus));
                }
                if item.def_bonus > 0 {
                    bonuses.push(format!("+{} DEF", item.def_bonus));
                }
                if item.hp_bonus > 0 {
                    bonuses.push(format!("+{} HP", item.hp_bonus));
                }
                if item.spd_bonus > 0 {
                    bonuses.push(format!("+{} SPD", item.spd_bonus));
                }

                let bonus_str = if bonuses.is_empty() {
                    "".to_string()
                } else {
                    format!(" ({})", bonuses.join(", "))
                };
                grid.write_str(right_col + 12, ey + 1, &bonus_str, Color::GREEN, panel_bg);
            } else {
                grid.write_str(right_col + 12, ey, "<Empty Slot>", Color::GREY, panel_bg);
            }
            ey += 3;
        }
    } else {
        grid.write_str(
            right_col,
            ey,
            "No equipment component found.",
            Color::GREY,
            panel_bg,
        );
    }

    grid.write_str(
        panel_rect.x + 4,
        panel_rect.bottom() - 2,
        "Press [Esc] or [C] to return",
        Color::GREY,
        panel_bg,
    );
}
