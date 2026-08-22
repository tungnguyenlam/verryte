use super::Game;
use crate::components::{BattleStats, CharacterClass, GameState, Position, Stats, Team};
use crate::map::{TacticalMap, Tile};
use verryte_core::{Events, GameClock};
use verryte_terminal::{Cell, Color, Grid, VisualRegistry};

impl Game {
    /// Render using the current terminal size.
    pub fn render(&self) -> Grid {
        let (term_w, term_h) = verryte_tty::terminal_size();
        self.render_sized(term_w, term_h)
    }

    /// Render into a grid of explicit `(cols, rows)` dimensions.
    ///
    /// This decouples rendering from having a real terminal attached,
    /// allowing headless runners and agent tools to specify the size.
    pub fn render_sized(&self, term_w: u16, term_h: u16) -> Grid {
        let map = self.world.resource::<TacticalMap>().unwrap();
        let state = self.world.resource::<GameState>().unwrap();
        let registry = self.world.resource::<VisualRegistry>().unwrap();
        let visibility = self.world.resource::<verryte_map::VisibilityMap>().unwrap();
        let clock = self.world.resource::<GameClock>().unwrap();

        // Determine resolution tier and tile dimensions dynamically
        let tier = verryte_terminal::ResolutionTier::from_size(term_w, term_h);
        let (tile_w, tile_h) = tier.tile_dimensions();

        // Screen layout
        let hud_h = 6;
        let board_h = term_h.saturating_sub(hud_h);
        let mut screen = Grid::new(term_w, term_h);

        let mut viewport = verryte_terminal::TileViewport::new(
            verryte_terminal::Rect::new(0, 0, term_w, board_h),
            tile_w,
            tile_h,
        );
        viewport.camera = self.camera.clone();
        viewport.camera.clamp_to_bounds(
            0.0,
            0.0,
            map.width as f32 * tile_w as f32,
            map.height as f32 * tile_h as f32,
            viewport.rect.width,
            viewport.rect.height,
        );

        // 1. Render Tiles (culled by visibility)
        let (start_x, start_y, end_x, end_y) = viewport.visible_tiles(map.width, map.height);

        for ty in start_y..end_y {
            for tx in start_x..end_x {
                let pos = Position::new(tx, ty);
                let vis = visibility.get(pos);
                if matches!(vis, verryte_map::Visibility::Hidden) {
                    continue;
                }

                let tile = map.tile(tx, ty);
                let mut color = match tile {
                    Tile::Grass => Color(30, 80, 30),
                    Tile::Wall => Color(60, 60, 60),
                    Tile::Water => Color(30, 30, 100),
                    Tile::Lava => Color(120, 20, 10),
                    Tile::Ice => Color(100, 180, 200),
                    Tile::Stairs => Color(160, 120, 40),
                    Tile::Mud => Color(80, 50, 30),
                    Tile::SpikeTrap => Color(180, 30, 30),
                    Tile::PoisonCloud => Color(80, 30, 120),
                    Tile::HealingSpring => Color(30, 160, 80),
                    Tile::CrackedFloor => Color(90, 70, 50),
                    Tile::PressurePlate => Color(140, 140, 60),
                    Tile::ThornBush => Color(50, 100, 20),
                    Tile::SteamVent => Color(200, 100, 50),
                    Tile::ExplodingBarrel => Color(255, 128, 0),
                };

                if matches!(vis, verryte_map::Visibility::Explored) {
                    color = verryte_terminal::vfx::blend_color(color, Color::BLACK, 0.6);
                }

                let (sx, sy) = viewport.world_to_screen(tx as f32, ty as f32);
                let ticks = clock.elapsed_ticks();

                // Draw tile background/border
                for dy in 0..tile_h {
                    for dx in 0..tile_w {
                        let tx_abs = sx + dx as i32;
                        let ty_abs = sy + dy as i32;

                        if viewport.rect.contains(tx_abs as u16, ty_abs as u16) {
                            let glyph = match tile {
                                Tile::Water => {
                                    let phase =
                                        ((ticks + (tx as u64) * 3 + (ty as u64) * 7) / 10) % 3;
                                    match phase {
                                        0 => '~',
                                        1 => '≈',
                                        _ => '∽',
                                    }
                                }
                                Tile::Lava => {
                                    let phase =
                                        ((ticks + (tx as u64) * 3 + (ty as u64) * 7) / 8) % 3;
                                    match phase {
                                        0 => '^',
                                        1 => 'v',
                                        _ => '*',
                                    }
                                }
                                _ => {
                                    let is_ice = matches!(tile, Tile::Ice);
                                    let is_stairs = matches!(tile, Tile::Stairs);
                                    let is_mud = matches!(tile, Tile::Mud);
                                    if dx == 0 || dy == 0 {
                                        if is_ice {
                                            '-'
                                        } else if is_stairs {
                                            '>'
                                        } else if is_mud {
                                            '='
                                        } else {
                                            '·'
                                        }
                                    } else {
                                        ' '
                                    }
                                }
                            };
                            let mut fg = match tile {
                                Tile::Water => Color(80, 80, 180),
                                Tile::Lava => Color(240, 100, 20),
                                Tile::Ice => Color(200, 240, 255),
                                Tile::Stairs => Color(255, 215, 0),
                                Tile::Mud => Color(140, 90, 50),
                                _ => Color(40, 40, 40),
                            };
                            if matches!(vis, verryte_map::Visibility::Explored) {
                                fg = verryte_terminal::vfx::blend_color(fg, Color::BLACK, 0.6);
                            }
                            screen.put(
                                tx_abs as u16,
                                ty_abs as u16,
                                Cell::new(glyph).with_fg(fg).with_bg(color),
                            );
                        }
                    }
                }
            }
        }

        // 1.5. Threat Range Overlay
        if state.show_threat_map {
            let mut threat_tiles = std::collections::HashSet::new();
            for (_e, ep, enemy_class, enemy_stats, enemy_team) in
                self.world.query4::<Position, CharacterClass, Stats, Team>()
            {
                if *enemy_team == Team::Enemy {
                    let move_range = enemy_stats.ap;
                    let atk_range = match enemy_class {
                        CharacterClass::Warrior => 1,
                        CharacterClass::Mage => 3,
                        CharacterClass::Healer => 2,
                        CharacterClass::Boss => 2,
                        CharacterClass::CorruptedSpore => 1,
                        CharacterClass::CursedSentinel => 3,
                        CharacterClass::PlagueWraith => 2,
                        CharacterClass::EnemyCleric => 2,
                        _ => 1,
                    };
                    let total_range = move_range as i32 + atk_range;

                    for dy in -total_range..=total_range {
                        for dx in -total_range..=total_range {
                            if dy.abs() + dx.abs() <= total_range {
                                let tx = ep.x + dx as i16;
                                let ty = ep.y + dy as i16;
                                if tx >= 0
                                    && tx < map.width as i16
                                    && ty >= 0
                                    && ty < map.height as i16
                                {
                                    threat_tiles.insert(Position::new(tx, ty));
                                }
                            }
                        }
                    }
                }
            }

            for pos in threat_tiles {
                let vis = visibility.get(pos);
                if matches!(vis, verryte_map::Visibility::Hidden) {
                    continue;
                }
                let (sx, sy) = viewport.world_to_screen(pos.x as f32, pos.y as f32);
                for dy in 0..tile_h {
                    for dx in 0..tile_w {
                        let tx = sx + dx as i32;
                        let ty = sy + dy as i32;
                        if viewport.rect.contains(tx as u16, ty as u16) {
                            let cell = screen.get_mut(tx as u16, ty as u16).unwrap();
                            cell.bg =
                                verryte_terminal::vfx::blend_color(cell.bg, Color(150, 0, 0), 0.25);
                        }
                    }
                }
            }
        }

        // 2. Overlays (Range, Path, Telegraphs)
        if state.targeting == crate::components::TargetingMode::None {
            if let Some(sel_entity) = state.selected_entity {
                let reachable = self.get_reachable_tiles(sel_entity);
                for pos in &reachable {
                    let (sx, sy) = viewport.world_to_screen(pos.x as f32, pos.y as f32);
                    for dy in 0..tile_h {
                        for dx in 0..tile_w {
                            let tx = sx + dx as i32;
                            let ty = sy + dy as i32;
                            if viewport.rect.contains(tx as u16, ty as u16) {
                                let cell = screen.get_mut(tx as u16, ty as u16).unwrap();
                                cell.bg = verryte_terminal::vfx::blend_color(
                                    cell.bg,
                                    Color(0, 100, 150),
                                    0.35,
                                );
                            }
                        }
                    }
                }

                // Draw path preview
                if reachable.contains(&state.cursor) {
                    if let Some(path) = self.get_path_to(sel_entity, state.cursor) {
                        let mut total_cost = 0;
                        let gravity_bonus =
                            crate::systems::floor_modifier_gravity_cost(&self.world);
                        let weather = self
                            .world
                            .resource::<crate::components::Weather>()
                            .map(|w| w.current)
                            .unwrap_or(crate::components::WeatherType::Sunny);

                        for pos in &path {
                            let tile = map.tile(pos.x, pos.y);
                            let step_cost = match (tile, weather) {
                                (Tile::Water, crate::components::WeatherType::Rainy) => 1,
                                (Tile::Ice, crate::components::WeatherType::Snowing) => 0,
                                (Tile::Water | Tile::Lava, _) => 2,
                                (Tile::Mud, _) => 3,
                                _ => 1,
                            };
                            total_cost += step_cost + gravity_bonus;
                        }

                        for pos in &path {
                            let (sx, sy) = viewport.world_to_screen(pos.x as f32, pos.y as f32);
                            for dy in 0..tile_h {
                                for dx in 0..tile_w {
                                    let tx = sx + dx as i32;
                                    let ty = sy + dy as i32;
                                    if viewport.rect.contains(tx as u16, ty as u16) {
                                        let cell = screen.get_mut(tx as u16, ty as u16).unwrap();
                                        cell.bg = verryte_terminal::vfx::blend_color(
                                            cell.bg,
                                            Color(0, 150, 220),
                                            0.4,
                                        );
                                        // Waypoint dot marker
                                        if dx == tile_w / 2 && dy == tile_h / 2 {
                                            cell.glyph = '·';
                                            cell.fg = Color(255, 255, 255);
                                        }
                                    }
                                }
                            }
                        }

                        // Display AP cost badge next to the cursor
                        let (cx, cy) =
                            viewport.world_to_screen(state.cursor.x as f32, state.cursor.y as f32);
                        let ap_str = format!("{} AP", total_cost);
                        let ap_x = cx + tile_w as i32;
                        let ap_y = cy + (tile_h as i32 / 2);
                        for (i, ch) in ap_str.chars().enumerate() {
                            let tx = ap_x + i as i32;
                            if viewport.rect.contains(tx as u16, ap_y as u16) {
                                if let Some(cell) = screen.get_mut(tx as u16, ap_y as u16) {
                                    cell.glyph = ch;
                                    cell.fg = Color(255, 255, 0); // Yellow text
                                    cell.bg = Color(10, 10, 15);
                                }
                            }
                        }
                    }
                }
            }
        } else {
            // Targeting mode range
            if let Some(sel_entity) = state.selected_entity {
                if let (Some(caster_pos), Some(class)) = (
                    self.world.get::<Position>(sel_entity),
                    self.world.get::<CharacterClass>(sel_entity),
                ) {
                    let (_name, range, _ap, _aoe, _power) = Self::get_skill_info(
                        *class,
                        state.targeting,
                    )
                    .unwrap_or(("Unknown".to_string(), 0, 0, false, 0));

                    for ty in 0..map.height {
                        for tx in 0..map.width {
                            let target = Position::new(tx as i16, ty as i16);
                            let dist =
                                (caster_pos.x - target.x).abs() + (caster_pos.y - target.y).abs();
                            if dist <= range {
                                let (sx, sy) = viewport.world_to_screen(tx as f32, ty as f32);
                                for dy in 0..tile_h {
                                    for dx in 0..tile_w {
                                        let tx_abs = sx + dx as i32;
                                        let ty_abs = sy + dy as i32;
                                        if viewport.rect.contains(tx_abs as u16, ty_abs as u16) {
                                            let cell = screen
                                                .get_mut(tx_abs as u16, ty_abs as u16)
                                                .unwrap();
                                            cell.bg = verryte_terminal::vfx::blend_color(
                                                cell.bg,
                                                Color(50, 150, 50),
                                                0.35,
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // AoE Preview under cursor
                    let dist = (caster_pos.x - state.cursor.x).abs()
                        + (caster_pos.y - state.cursor.y).abs();
                    if dist <= range {
                        let aoe_tiles = Self::get_skill_aoe(*class, state.targeting, state.cursor);
                        for pos in aoe_tiles {
                            let (sx, sy) = viewport.world_to_screen(pos.x as f32, pos.y as f32);
                            for dy in 0..tile_h {
                                for dx in 0..tile_w {
                                    let tx = sx + dx as i32;
                                    let ty = sy + dy as i32;
                                    if viewport.rect.contains(tx as u16, ty as u16) {
                                        if let Some(cell) = screen.get_mut(tx as u16, ty as u16) {
                                            cell.bg = verryte_terminal::vfx::blend_color(
                                                cell.bg,
                                                Color(200, 200, 50),
                                                0.5,
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // 3. Telegraph Zones
        let telegraph_zone = self
            .world
            .resource::<crate::components::TelegraphZone>()
            .unwrap();
        for pos in &telegraph_zone.tiles {
            let (sx, sy) = viewport.world_to_screen(pos.x as f32, pos.y as f32);
            for dy in 0..tile_h {
                for dx in 0..tile_w {
                    let tx = sx + dx as i32;
                    let ty = sy + dy as i32;
                    if viewport.rect.contains(tx as u16, ty as u16) {
                        let cell = screen.get_mut(tx as u16, ty as u16).unwrap();
                        cell.bg =
                            verryte_terminal::vfx::blend_color(cell.bg, Color(150, 0, 75), 0.4);
                    }
                }
            }
        }

        // 3b. Weather Danger Zones
        crate::ui::render_weather_danger_zones(
            &mut screen,
            &self.world,
            &viewport,
            tile_w,
            tile_h,
            clock.elapsed_ticks(),
        );

        // 4. Cursor
        let pulse = ((clock.elapsed_ticks() as f32 * 0.1).sin() * 0.5 + 0.5) * 0.6 + 0.2; // 0.2 to 0.8
        let (csx, csy) = viewport.world_to_screen(state.cursor.x as f32, state.cursor.y as f32);
        for dy in 0..tile_h {
            for dx in 0..tile_w {
                let tx = csx + dx as i32;
                let ty = csy + dy as i32;
                if viewport.rect.contains(tx as u16, ty as u16) {
                    let cell = screen.get_mut(tx as u16, ty as u16).unwrap();
                    let is_border = dx == 0 || dy == 0 || dx == tile_w - 1 || dy == tile_h - 1;
                    if is_border {
                        cell.bg =
                            verryte_terminal::vfx::blend_color(cell.bg, Color(255, 255, 0), pulse);
                    } else {
                        cell.bg = verryte_terminal::vfx::blend_color(
                            cell.bg,
                            Color(150, 150, 0),
                            pulse * 0.5,
                        );
                    }
                }
            }
        }

        // 5. Render Entities
        for (entity, pos, team, class) in self.world.query3::<Position, Team, CharacterClass>() {
            // Check visibility
            let vis = visibility.get(*pos);
            if *team != Team::Player && !matches!(vis, verryte_map::Visibility::Visible) {
                continue;
            }

            let key = match class {
                CharacterClass::Warrior => "kael",
                CharacterClass::Rogue => "kael",
                CharacterClass::Mage => "lyra",
                CharacterClass::Healer => "mira",
                CharacterClass::Boss => "blight-sovereign",
                CharacterClass::ShadowStalker => "lyra",
                CharacterClass::CorruptedSpore => "blight-sovereign",
                CharacterClass::CursedSentinel => "kael",
                CharacterClass::PlagueWraith => "lyra",
                CharacterClass::GlacialGolem => "blight-sovereign",
                CharacterClass::EnemyCleric => "mira",
                CharacterClass::VoidTerror => "blight-sovereign",
                CharacterClass::Berserker => "kael",
                CharacterClass::Tactician => "lyra",
                CharacterClass::Summoner => "blight-sovereign",
                CharacterClass::Assassin => "kael",
                CharacterClass::EliteBerserker => "kael",
                CharacterClass::EliteTactician => "lyra",
                CharacterClass::EliteSummoner => "blight-sovereign",
                CharacterClass::EliteAssassin => "kael",
                CharacterClass::FrozenSentinel => "blight-sovereign",
                CharacterClass::DestructibleObject => "barrel",
            };

            if let Some(asset) = registry.get(key) {
                let sprite_grid = asset.render(tier);
                viewport.blit_sprite(&mut screen, pos.x as f32, pos.y as f32, sprite_grid);

                // HP Bar
                if let Some(stats) = self.world.get::<Stats>(entity) {
                    let (sx, sy) = viewport.world_to_screen(pos.x as f32, pos.y as f32);
                    let bar_w = tile_w.min(10);
                    let hp_ratio = stats.hp as f32 / stats.max_hp as f32;
                    let fill_w = (bar_w as f32 * hp_ratio).round() as u16;

                    let bar_x = sx + (tile_w as i32 - bar_w as i32) / 2;
                    let bar_y = sy + tile_h as i32 - 1;

                    for i in 0..bar_w {
                        let tx = viewport.rect.x as i32 + bar_x + i as i32;
                        let ty = viewport.rect.y as i32 + bar_y;
                        if viewport.rect.contains(tx as u16, ty as u16) {
                            let color = if i < fill_w {
                                Color(0, 255, 0)
                            } else {
                                Color(100, 0, 0)
                            };
                            screen.put(tx as u16, ty as u16, Cell::new('=').with_fg(color));
                        }
                    }

                    // Render Shield Bar right above HP Bar
                    if let Some(shield) =
                        self.world.get::<crate::components::ElementalShield>(entity)
                    {
                        if shield.amount > 0 {
                            let sh_ratio = shield.amount as f32 / shield.max_amount as f32;
                            let sh_fill_w = (bar_w as f32 * sh_ratio).round() as u16;
                            let sh_color = match shield.shield_type {
                                crate::components::ShieldType::Ice => Color(100, 200, 255),
                                crate::components::ShieldType::Lightning => Color(255, 255, 0),
                                crate::components::ShieldType::Nature => Color(0, 255, 100),
                                crate::components::ShieldType::Physical => Color(200, 200, 200),
                            };
                            let sh_y = sy + tile_h as i32 - 2;
                            for i in 0..bar_w {
                                let tx = viewport.rect.x as i32 + bar_x + i as i32;
                                let ty = viewport.rect.y as i32 + sh_y;
                                if viewport.rect.contains(tx as u16, ty as u16) {
                                    let color = if i < sh_fill_w {
                                        sh_color
                                    } else {
                                        Color(50, 50, 50)
                                    };
                                    screen.put(tx as u16, ty as u16, Cell::new('-').with_fg(color));
                                }
                            }
                        }
                    }
                }
            }
        }

        // Render Echo items
        for (_e, pos, _echo) in self.world.query2::<Position, crate::components::EchoItem>() {
            let vis = visibility.get(*pos);
            if matches!(vis, verryte_map::Visibility::Hidden) {
                continue;
            }
            let (sx, sy) = viewport.world_to_screen(pos.x as f32, pos.y as f32);
            let tx = sx + tile_w as i32 / 2;
            let ty = sy + tile_h as i32 / 2;
            if viewport.rect.contains(tx as u16, ty as u16) {
                screen.put(
                    tx as u16,
                    ty as u16,
                    Cell::new('Ω')
                        .with_fg(Color(180, 50, 255))
                        .with_bg(Color::BLACK)
                        .with_attrs(verryte_terminal::CellAttrs::NONE.bold()),
                );
            }
        }

        // 6. VFX
        self.vfx().render_world(&mut screen, &viewport);

        // 7. Dialogue
        if let Some(dialogue) = self.world.resource::<verryte_terminal::DialogueState>() {
            if !dialogue.text.is_empty() {
                let dialog_w = term_w.min(60);
                let dialog_h = 8;
                let dialog_x = (term_w - dialog_w) / 2;
                let dialog_y = (term_h - dialog_h) / 2;

                let theme = match dialogue.title.as_str() {
                    "Blight Sovereign" => verryte_terminal::DialogueTheme::Blood,
                    "Kael" => verryte_terminal::DialogueTheme::Frost,
                    "Jax" => verryte_terminal::DialogueTheme::Shadow,
                    "Lyra" => verryte_terminal::DialogueTheme::Arcane,
                    "Mira" => verryte_terminal::DialogueTheme::Forest,
                    _ => verryte_terminal::DialogueTheme::Dungeon,
                };
                let box_widget = verryte_terminal::DialogueBox::new(verryte_terminal::Rect::new(
                    dialog_x, dialog_y, dialog_w, dialog_h,
                ))
                .with_theme(theme);

                box_widget.render(
                    &mut screen,
                    &dialogue.title,
                    &dialogue.text,
                    dialogue.visible_chars as usize,
                    None,
                    &dialogue.choices,
                    if dialogue.choices.is_empty() {
                        None
                    } else {
                        Some(dialogue.selected_choice)
                    },
                );
            }
        }

        // 8. Minimap
        if state.show_minimap {
            crate::ui::render_minimap(&mut screen, &self.world, board_h);
        }

        // 9. HUD
        crate::ui::render_hud(&mut screen, &self.world, term_w, term_h);

        // 9. Screen Flash
        self.vfx().render_flash(&mut screen, term_w, term_h);

        // 10. Performance Overlay
        if state.show_perf {
            if let Some(diagnostics) = self
                .world
                .resource::<verryte_core::diagnostics::Diagnostics>()
            {
                let perf_widget = verryte_terminal::widgets::PerformanceOverlay::new(
                    verryte_terminal::Rect::new(term_w.saturating_sub(30), 0, 30, 10),
                );
                perf_widget.render(&mut screen, diagnostics);
            }
        }

        screen
    }

    pub fn snapshot(&self) -> crate::snapshot::Snapshot {
        let state = self.world.resource::<GameState>().unwrap();

        let mut player_team = crate::snapshot::TeamSummary {
            count: 0,
            total_hp: 0,
            max_hp: 0,
        };
        let mut enemy_team = crate::snapshot::TeamSummary {
            count: 0,
            total_hp: 0,
            max_hp: 0,
        };

        for (_, team, stats) in self.world.query2::<Team, Stats>() {
            let summary = match team {
                Team::Player => &mut player_team,
                Team::Enemy => &mut enemy_team,
            };
            summary.count += 1;
            summary.total_hp += stats.hp;
            summary.max_hp += stats.max_hp;
        }

        let (reachable_tiles, targetable_tiles, selected_can_act) =
            if let Some(sel) = state.selected_entity {
                let reachable = self.get_reachable_tiles(sel);
                let attack_range = self
                    .world
                    .get::<CharacterClass>(sel)
                    .map(|c| match c {
                        CharacterClass::Warrior => 1,
                        CharacterClass::Mage => 3,
                        CharacterClass::Healer => 2,
                        _ => 1,
                    })
                    .unwrap_or(1);
                let can_act = self
                    .world
                    .get::<Stats>(sel)
                    .map(|s| s.ap > 0)
                    .unwrap_or(false);
                let mut targets: Vec<Position> = Vec::new();
                for (_, team, pos) in self.world.query2::<Team, Position>() {
                    if team == &Team::Enemy {
                        let dist = (pos.x - state.cursor.x).abs() + (pos.y - state.cursor.y).abs();
                        if dist <= attack_range {
                            targets.push(*pos);
                        }
                    }
                }
                (reachable, targets, can_act)
            } else {
                (Vec::new(), Vec::new(), false)
            };

        let battle_stats = self
            .world
            .resource::<BattleStats>()
            .cloned()
            .unwrap_or_default();

        crate::snapshot::Snapshot {
            turn: state.turn,
            phase: state.phase,
            outcome: state.outcome,
            cursor: state.cursor,
            player_team,
            enemy_team,
            reachable_tiles,
            targetable_tiles,
            selected_can_act,
            combo_count: state.combo_count,
            battle_stats,
            floor: state.floor,
            weather: self
                .world
                .resource::<crate::components::Weather>()
                .map(|w| w.current)
                .unwrap_or(crate::components::WeatherType::Sunny),
            turn_order: crate::battle_preview::BattlePreview::calculate_turn_order(
                &self.world,
                state.selected_entity,
            )
            .entries
            .iter()
            .map(|e| {
                format!(
                    "{}[{}]{}",
                    e.name,
                    e.spd,
                    if e.is_current { "*" } else { "" }
                )
            })
            .collect(),
            enemy_intents: {
                let default_map = crate::map::TacticalMap::new(24, 16);
                let default_telegraph = crate::components::TelegraphZone::default();
                let map_ref = self
                    .world
                    .resource::<crate::map::TacticalMap>()
                    .unwrap_or(&default_map);
                let telegraph_ref = self
                    .world
                    .resource::<crate::components::TelegraphZone>()
                    .unwrap_or(&default_telegraph);
                crate::battle_preview::BattlePreview::predict_enemy_intents(
                    &self.world,
                    map_ref,
                    telegraph_ref,
                )
                .intents
                .iter()
                .map(|i| i.description.clone())
                .collect()
            },
            damage_preview: state.selected_entity.and_then(|sel| {
                let cursor = state.cursor;
                self.get_entity_at(cursor)
                    .and_then(|(target, team, _stats, _class)| {
                        if team == Team::Enemy {
                            if let (Some(atk_stats), Some(tgt_stats)) = (
                                self.world.get::<Stats>(sel),
                                self.world.get::<Stats>(target),
                            ) {
                                let mut preview =
                                    crate::battle_preview::BattlePreview::calculate_damage_preview(
                                        atk_stats.atk,
                                        atk_stats.level,
                                        tgt_stats.def,
                                        tgt_stats.level,
                                        1.0,
                                        20,
                                    );
                                preview.can_kill = preview.max_damage >= tgt_stats.hp;
                                Some(preview)
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    })
            }),
            aoe_preview: {
                if state.targeting != crate::components::TargetingMode::None {
                    if let Some(sel) = state.selected_entity {
                        if let Some(class) = self.world.get::<CharacterClass>(sel) {
                            let mut aoe =
                                Self::get_skill_aoe(*class, state.targeting, state.cursor);
                            let is_archmage_aoe = *class == CharacterClass::Mage
                                && self
                                    .world
                                    .get::<crate::components::PrestigeProgress>(sel)
                                    .is_some_and(|p| {
                                        p.class == crate::components::PrestigeClass::Archmage
                                            && p.promoted
                                    });
                            if is_archmage_aoe {
                                let extra: Vec<Position> = aoe
                                    .iter()
                                    .flat_map(|p| p.neighbors4())
                                    .filter(|p| !aoe.contains(p))
                                    .collect();
                                aoe.extend(extra);
                            }
                            let mut extra_aoe = 0;
                            if let Some(skill_slot) = match state.targeting {
                                crate::components::TargetingMode::Skill1 => {
                                    Some(crate::components::SkillSlot::Skill1)
                                }
                                crate::components::TargetingMode::Skill2 => {
                                    Some(crate::components::SkillSlot::Skill2)
                                }
                                crate::components::TargetingMode::Skill3 => {
                                    Some(crate::components::SkillSlot::Skill3)
                                }
                                _ => None,
                            } {
                                if let Some(tree) =
                                    self.world.get::<crate::components::SkillTree>(sel)
                                {
                                    extra_aoe = tree.total_aoe_bonus(skill_slot);
                                }
                            }
                            if extra_aoe > 0 {
                                let mut expanded = aoe.clone();
                                for _ in 0..extra_aoe {
                                    let neighbors: Vec<Position> = expanded
                                        .iter()
                                        .flat_map(|p| p.neighbors4())
                                        .filter(|p| !expanded.contains(p))
                                        .collect();
                                    expanded.extend(neighbors);
                                }
                                aoe = expanded;
                            }
                            aoe
                        } else {
                            Vec::new()
                        }
                    } else {
                        Vec::new()
                    }
                } else {
                    Vec::new()
                }
            },
            available_combos: self
                .world
                .resource::<crate::components::AvailableCombos>()
                .map(|ac| {
                    ac.combos
                        .iter()
                        .map(|(s, _)| crate::components::ComboSkillDef::for_skill(s).name)
                        .collect()
                })
                .unwrap_or_default(),
            bestiary_discovered: self
                .world
                .resource::<crate::components::Bestiary>()
                .map(|b| b.entries.iter().filter(|e| e.encountered).count() as u32)
                .unwrap_or(0),
            bestiary_total: self
                .world
                .resource::<crate::components::Bestiary>()
                .map(|b| b.entries.len() as u32)
                .unwrap_or(0),
            lore_discovered: self
                .world
                .resource::<crate::components::LoreJournal>()
                .map(|j| j.entries.iter().filter(|e| e.discovered).count() as u32)
                .unwrap_or(0),
            lore_total: self
                .world
                .resource::<crate::components::LoreJournal>()
                .map(|j| j.entries.len() as u32)
                .unwrap_or(0),
            active_modifiers: self
                .world
                .resource::<crate::components::ActiveFloorModifiers>()
                .map(|m| {
                    m.modifiers
                        .iter()
                        .map(|fm| fm.display_name().to_string())
                        .collect()
                })
                .unwrap_or_default(),
            active_modifier_durations: self
                .world
                .resource::<crate::components::ActiveFloorModifiers>()
                .map(|m| m.turns_remaining.clone())
                .unwrap_or_default(),
            weather_danger_zones: self
                .world
                .resource::<crate::components::Weather>()
                .map(|w| w.danger_zones.clone())
                .unwrap_or_default(),
            next_floor_event_turn: self
                .world
                .resource::<crate::components::DynamicFloorEvents>()
                .map(|events| events.next_event_turn)
                .unwrap_or_default(),
            recent_floor_events: self
                .world
                .resource::<crate::components::DynamicFloorEvents>()
                .map(|events| {
                    events
                        .history
                        .iter()
                        .map(|event| event.description.clone())
                        .collect()
                })
                .unwrap_or_default(),
            active_set_bonuses: self
                .world
                .query2::<Team, crate::components::EquippedItems>()
                .into_iter()
                .filter(|(_, t, _)| **t == Team::Player)
                .filter_map(|(_, _, eq)| {
                    let stats = crate::equipment::check_set_bonuses(eq);
                    if stats.active_sets.is_empty() {
                        None
                    } else {
                        Some(
                            stats
                                .active_sets
                                .iter()
                                .map(|s| s.display_name().to_string())
                                .collect::<Vec<_>>()
                                .join(", "),
                        )
                    }
                })
                .collect(),
        }
    }

    pub fn update_weather_ambient(&mut self, weather: crate::components::WeatherType) {
        use crate::components::WeatherType;

        // Emit ambient audio events
        if let Some(events) = self
            .world
            .resource_mut::<Events<verryte_core::AudioEvent>>()
        {
            let (ambient_name, volume) = match weather {
                WeatherType::Rainy => ("ambient_rain", 0.4),
                WeatherType::LightningStorm => ("ambient_thunder", 0.5),
                WeatherType::Sunny => ("ambient_birds", 0.3),
                WeatherType::Snowing => ("ambient_wind", 0.35),
            };
            events.send(verryte_core::AudioEvent::loop_music(ambient_name).with_volume(volume));
        }

        // VFX effects
        match weather {
            WeatherType::Rainy => {
                let (tw, th) = self.get_tile_dimensions();
                let cols = 24u16;
                let rows = 16u16;
                let screen_w = cols * tw;
                let screen_h = rows * th;
                let mut rain = Vec::new();
                for _ in 0..30 {
                    let rx = (screen_w as f32) * 0.5;
                    let ry = (screen_h as f32) * 0.5;
                    rain.extend(verryte_terminal::vfx::emit_burst(
                        rx,
                        ry,
                        1,
                        Color(100, 140, 220),
                        &['│', '┃', '¦'],
                    ));
                }
                self.vfx_mut().particles.extend(rain);
            }
            WeatherType::LightningStorm => {
                self.vfx_mut()
                    .flashes
                    .push(verryte_terminal::vfx::Flash::full_screen(
                        Color(255, 255, 200),
                        0.15,
                    ));
                self.vfx_mut()
                    .shakes
                    .push(verryte_terminal::vfx::ScreenShake::new(2.0, 0.2));
            }
            WeatherType::Sunny => {
                let (tw, th) = self.get_tile_dimensions();
                let cols = 24u16;
                let rows = 16u16;
                let screen_w = cols * tw;
                let screen_h = rows * th;
                let cx = screen_w as f32 * 0.5;
                let cy = screen_h as f32 * 0.5;
                self.vfx_mut()
                    .particles
                    .extend(verryte_terminal::vfx::emit_burst(
                        cx,
                        cy,
                        12,
                        Color(255, 230, 100),
                        &['·', '°', '∘'],
                    ));
            }
            WeatherType::Snowing => {
                let (tw, th) = self.get_tile_dimensions();
                let cols = 24u16;
                let rows = 16u16;
                let screen_w = cols * tw;
                let screen_h = rows * th;
                let cx = screen_w as f32 * 0.5;
                let cy = screen_h as f32 * 0.5;
                self.vfx_mut()
                    .particles
                    .extend(verryte_terminal::vfx::emit_burst(
                        cx,
                        cy,
                        20,
                        Color(220, 230, 255),
                        &['*', '·', '❄'],
                    ));
            }
        }
    }
}
