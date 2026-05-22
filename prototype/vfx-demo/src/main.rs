use std::time::{Duration, Instant};

use verryte_terminal::vfx::{
    emit_burst, emit_fire, emit_heal, emit_ice, emit_lightning, emit_slash, AoeRing, Flash,
    FloatingText, ScreenShake, VfxSystem,
};
use verryte_terminal::{Cell, Color, Grid, Rect};
use verryte_tty::verryte_input::{InputEvent, Key};
use verryte_tty::{poll_event, terminal_size};

const TARGET_FPS: u32 = 30;
const FRAME_DURATION: Duration = Duration::from_millis(1000 / TARGET_FPS as u64);

// ── Sprite Loading ────────────────────────────────────────────────────────────

fn load_sprite(path: &str, target_w: u32, target_h: u32) -> Grid {
    let reader =
        image::io::Reader::open(path).unwrap_or_else(|e| panic!("failed to open {}: {}", path, e));
    let reader = reader
        .with_guessed_format()
        .unwrap_or_else(|e| panic!("failed to guess format {}: {}", path, e));
    let img = reader
        .decode()
        .unwrap_or_else(|e| panic!("failed to decode {}: {}", path, e));
    let resized = img.resize_exact(target_w, target_h, image::imageops::FilterType::Triangle);
    let mut grid = verryte_terminal::image_to_grid(&resized);

    // Chroma-key: set near-white pixels to transparent.
    // Only keep cells where at least one of fg/bg has meaningful color.
    for y in 0..grid.height() {
        for x in 0..grid.width() {
            let cell = grid.get(x, y).copied().unwrap_or(Cell::EMPTY);
            let fg_white = cell.fg.0 > 220 && cell.fg.1 > 220 && cell.fg.2 > 220;
            let bg_white = cell.bg.0 > 220 && cell.bg.1 > 220 && cell.bg.2 > 220;
            if fg_white && bg_white {
                grid.put(x, y, Cell::EMPTY);
            } else if fg_white {
                // Top pixel is background — show only bottom pixel
                grid.put(
                    x,
                    y,
                    Cell {
                        glyph: '▄',
                        fg: cell.bg,
                        bg: Color::BLACK,
                        attrs: cell.attrs,
                    },
                );
            } else if bg_white {
                // Bottom pixel is background — show only top pixel
                grid.put(
                    x,
                    y,
                    Cell {
                        glyph: '▀',
                        fg: cell.fg,
                        bg: Color::BLACK,
                        attrs: cell.attrs,
                    },
                );
            }
        }
    }

    grid
}

fn tint_grid_white(grid: &Grid) -> Grid {
    let mut out = grid.clone();
    for y in 0..out.height() {
        for x in 0..out.width() {
            let cell = out.get(x, y).copied().unwrap_or(Cell::EMPTY);
            if !cell.is_transparent() {
                out.put(
                    x,
                    y,
                    Cell {
                        glyph: cell.glyph,
                        fg: Color(255, 255, 255),
                        bg: Color(255, 255, 255),
                        attrs: cell.attrs,
                    },
                );
            }
        }
    }
    out
}

// ── Scene ─────────────────────────────────────────────────────────────────────

struct Scene {
    vfx: VfxSystem,
    frame_count: u64,
    time: f32,
    kael_hp: i32,
    kael_max_hp: i32,
    mira_hp: i32,
    mira_max_hp: i32,
    enemy_hp: i32,
    enemy_max_hp: i32,
    kael_flash: u32,
    mira_flash: u32,
    enemy_flash: u32,
    log_lines: Vec<String>,
    combo_counter: u32,
    sprite_kael: Grid,
    sprite_mira: Grid,
    sprite_blight: Grid,
    sprite_kael_white: Grid,
    sprite_mira_white: Grid,
    sprite_blight_white: Grid,
}

impl Scene {
    fn new() -> Self {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let asset_dir = format!("{}/../wuthering-terminal/assets", manifest_dir);
        let sprite_kael = load_sprite(&format!("{}/kael.png", asset_dir), 12, 12);
        let sprite_mira = load_sprite(&format!("{}/mira.png", asset_dir), 12, 12);
        let sprite_blight = load_sprite(&format!("{}/blight-sovereign.png", asset_dir), 16, 16);
        let sprite_kael_white = tint_grid_white(&sprite_kael);
        let sprite_mira_white = tint_grid_white(&sprite_mira);
        let sprite_blight_white = tint_grid_white(&sprite_blight);

        Self {
            vfx: VfxSystem::new(),
            frame_count: 0,
            time: 0.0,
            kael_hp: 120,
            kael_max_hp: 120,
            mira_hp: 90,
            mira_max_hp: 90,
            enemy_hp: 250,
            enemy_max_hp: 250,
            kael_flash: 0,
            mira_flash: 0,
            enemy_flash: 0,
            log_lines: vec![
                "VFX Demo - Tactical RPG".to_string(),
                "".to_string(),
                "Keys:".to_string(),
                "  1 - Fire burst (particles)".to_string(),
                "  2 - Ice explosion".to_string(),
                "  3 - Lightning bolt".to_string(),
                "  4 - Sword slash".to_string(),
                "  5 - Heal effect".to_string(),
                "  6 - Screen shake".to_string(),
                "  7 - Full-screen flash".to_string(),
                "  8 - AoE ring".to_string(),
                "  9 - Combo attack".to_string(),
                "  0 - Ultimate (all effects)".to_string(),
                "  q - Quit".to_string(),
            ],
            combo_counter: 0,
            sprite_kael,
            sprite_mira,
            sprite_blight,
            sprite_kael_white,
            sprite_mira_white,
            sprite_blight_white,
        }
    }

    fn add_log(&mut self, msg: String) {
        self.log_lines.push(msg);
        if self.log_lines.len() > 20 {
            self.log_lines.remove(0);
        }
    }

    fn trigger_fire(&mut self, w: u16, h: u16) {
        let cx = w as f32 * 0.7;
        let cy = h as f32 * 0.35;
        self.vfx.particles.extend(emit_fire(cx, cy, 25));
        self.vfx.flashes.push(Flash::region(
            Color(255, 120, 30),
            0.15,
            Rect::new(
                (cx as u16).saturating_sub(6),
                (cy as u16).saturating_sub(4),
                12,
                8,
            ),
        ));
        self.enemy_flash = 6;
        let dmg = 15 + (self.frame_count % 10) as i32;
        self.enemy_hp = (self.enemy_hp - dmg).max(0);
        self.vfx.floating_texts.push(FloatingText {
            x: cx - 1.0,
            y: cy - 2.0,
            text: format!("-{}", dmg),
            fg: Color(255, 80, 30),
            vy: -1.5,
            lifetime: 1.5,
            max_lifetime: 1.5,
            bold: true,
        });
        self.add_log(format!("Fire burst! {} damage", dmg));
    }

    fn trigger_ice(&mut self, w: u16, h: u16) {
        let cx = w as f32 * 0.7;
        let cy = h as f32 * 0.35;
        self.vfx.particles.extend(emit_ice(cx, cy, 30));
        self.vfx.flashes.push(Flash::region(
            Color(100, 180, 255),
            0.2,
            Rect::new(
                (cx as u16).saturating_sub(6),
                (cy as u16).saturating_sub(4),
                12,
                8,
            ),
        ));
        self.vfx.shakes.push(ScreenShake::new(1.5, 0.2));
        self.enemy_flash = 6;
        let dmg = 20 + (self.frame_count % 8) as i32;
        self.enemy_hp = (self.enemy_hp - dmg).max(0);
        self.vfx.floating_texts.push(FloatingText {
            x: cx - 1.0,
            y: cy - 2.0,
            text: format!("-{}", dmg),
            fg: Color(150, 200, 255),
            vy: -1.5,
            lifetime: 1.5,
            max_lifetime: 1.5,
            bold: true,
        });
        self.add_log(format!("Ice explosion! {} damage", dmg));
    }

    fn trigger_lightning(&mut self, w: u16, h: u16) {
        let sx = w as f32 * 0.25;
        let sy = h as f32 * 0.25;
        let tx = w as f32 * 0.7;
        let ty = h as f32 * 0.35;
        self.vfx.particles.extend(emit_lightning(sx, sy, tx, ty));
        self.vfx
            .flashes
            .push(Flash::full_screen(Color(255, 255, 200), 0.1));
        self.vfx.shakes.push(ScreenShake::new(3.0, 0.3));
        self.enemy_flash = 8;
        let dmg = 35 + (self.frame_count % 15) as i32;
        self.enemy_hp = (self.enemy_hp - dmg).max(0);
        self.vfx.floating_texts.push(FloatingText {
            x: tx - 1.0,
            y: ty - 3.0,
            text: format!("-{} ⚡", dmg),
            fg: Color(255, 255, 100),
            vy: -2.0,
            lifetime: 1.8,
            max_lifetime: 1.8,
            bold: true,
        });
        self.add_log(format!("Lightning strike! {} damage", dmg));
    }

    fn trigger_slash(&mut self, w: u16, h: u16) {
        let sx = w as f32 * 0.35;
        let sy = h as f32 * 0.4;
        self.vfx.particles.extend(emit_slash(sx, sy, 1.0));
        self.vfx.flashes.push(Flash::region(
            Color(255, 255, 255),
            0.1,
            Rect::new(
                (sx as u16).saturating_sub(2),
                (sy as u16).saturating_sub(3),
                16,
                6,
            ),
        ));
        self.vfx.shakes.push(ScreenShake::new(2.0, 0.15));
        self.enemy_flash = 5;
        let dmg = 25 + (self.frame_count % 12) as i32;
        self.enemy_hp = (self.enemy_hp - dmg).max(0);
        self.vfx.floating_texts.push(FloatingText {
            x: w as f32 * 0.65,
            y: h as f32 * 0.3,
            text: format!("-{}", dmg),
            fg: Color(255, 255, 255),
            vy: -2.0,
            lifetime: 1.2,
            max_lifetime: 1.2,
            bold: true,
        });
        self.add_log(format!("Sword slash! {} damage", dmg));
    }

    fn trigger_heal(&mut self, w: u16, h: u16) {
        let cx = w as f32 * 0.15;
        let cy = h as f32 * 0.4;
        self.vfx.particles.extend(emit_heal(cx, cy, 20));
        self.vfx.flashes.push(Flash::region(
            Color(80, 255, 120),
            0.2,
            Rect::new(
                (cx as u16).saturating_sub(4),
                (cy as u16).saturating_sub(2),
                8,
                6,
            ),
        ));
        let heal = 20;
        self.mira_hp = (self.mira_hp + heal).min(self.mira_max_hp);
        self.mira_flash = 6;
        self.vfx.floating_texts.push(FloatingText {
            x: cx - 1.0,
            y: cy - 3.0,
            text: format!("+{} ♥", heal),
            fg: Color(100, 255, 150),
            vy: -1.0,
            lifetime: 1.5,
            max_lifetime: 1.5,
            bold: true,
        });
        self.add_log(format!("Mira heals! +{} HP", heal));
    }

    fn trigger_shake(&mut self) {
        self.vfx.shakes.push(ScreenShake::new(5.0, 0.5));
        self.add_log("Screen shake!".to_string());
    }

    fn trigger_flash(&mut self, _w: u16, _h: u16) {
        self.vfx
            .flashes
            .push(Flash::full_screen(Color(255, 255, 255), 0.3));
        self.add_log("Flash!".to_string());
    }

    fn trigger_aoe(&mut self, w: u16, h: u16) {
        let cx = (w as f32 * 0.7) as i32;
        let cy = (h as f32 * 0.4) as i32;
        self.vfx.aoe_rings.push(AoeRing {
            cx,
            cy,
            max_radius: 10.0,
            current_radius: 1.0,
            expand_speed: 15.0,
            color: Color(255, 80, 80),
            lifetime: 0.8,
            max_lifetime: 0.8,
        });
        self.vfx.flashes.push(Flash::region(
            Color(255, 80, 80),
            0.15,
            Rect::new(
                (cx as u16).saturating_sub(10),
                (cy as u16).saturating_sub(5),
                20,
                10,
            ),
        ));
        self.vfx.shakes.push(ScreenShake::new(2.5, 0.3));
        self.enemy_flash = 8;
        let dmg = 30 + (self.frame_count % 10) as i32;
        self.enemy_hp = (self.enemy_hp - dmg).max(0);
        self.vfx.floating_texts.push(FloatingText {
            x: cx as f32 - 2.0,
            y: cy as f32 - 4.0,
            text: format!("-{} AoE!", dmg),
            fg: Color(255, 120, 80),
            vy: -2.0,
            lifetime: 2.0,
            max_lifetime: 2.0,
            bold: true,
        });
        self.add_log(format!("AoE blast! {} damage", dmg));
    }

    fn trigger_combo(&mut self, w: u16, h: u16) {
        self.combo_counter += 1;
        let cx = w as f32 * 0.7;
        let cy = h as f32 * 0.35;

        // Slash
        self.vfx.particles.extend(emit_slash(cx - 8.0, cy, 1.0));
        // Burst
        self.vfx.particles.extend(emit_burst(
            cx,
            cy,
            15,
            Color(255, 200, 50),
            &['*', '✦', '·'],
        ));
        self.vfx.shakes.push(ScreenShake::new(2.0, 0.2));
        self.vfx.flashes.push(Flash::region(
            Color(255, 200, 50),
            0.1,
            Rect::new(
                (cx as u16).saturating_sub(5),
                (cy as u16).saturating_sub(3),
                10,
                6,
            ),
        ));
        self.enemy_flash = 6;
        let dmg = 10 + self.combo_counter as i32 * 5;
        self.enemy_hp = (self.enemy_hp - dmg).max(0);
        self.vfx.floating_texts.push(FloatingText {
            x: cx - 2.0,
            y: cy - 3.0,
            text: format!("{}x COMBO -{}", self.combo_counter, dmg),
            fg: Color(255, 220, 80),
            vy: -2.0,
            lifetime: 1.5,
            max_lifetime: 1.5,
            bold: true,
        });
        self.add_log(format!("Combo x{}! {} damage", self.combo_counter, dmg));
    }

    fn trigger_ultimate(&mut self, w: u16, h: u16) {
        // Phase 1: Screen darkens + shake
        self.vfx.shakes.push(ScreenShake::new(6.0, 0.8));
        self.vfx
            .flashes
            .push(Flash::full_screen(Color(255, 255, 255), 0.15));

        // Phase 2: Particles from both characters
        let wx = w as f32 * 0.15;
        let wy = h as f32 * 0.4;
        let mx = w as f32 * 0.25;
        let my = h as f32 * 0.35;
        let ex = w as f32 * 0.7;
        let ey = h as f32 * 0.35;

        self.vfx.particles.extend(emit_fire(wx, wy, 15));
        self.vfx.particles.extend(emit_ice(mx, my, 15));
        self.vfx.particles.extend(emit_lightning(wx, wy, ex, ey));
        self.vfx.particles.extend(emit_burst(
            ex,
            ey,
            25,
            Color(255, 100, 200),
            &['✦', '*', '◇', '·', '°'],
        ));

        // Phase 3: AoE
        self.vfx.aoe_rings.push(AoeRing {
            cx: ex as i32,
            cy: ey as i32,
            max_radius: 15.0,
            current_radius: 1.0,
            expand_speed: 20.0,
            color: Color(255, 100, 200),
            lifetime: 1.0,
            max_lifetime: 1.0,
        });

        // Phase 4: Massive damage
        self.enemy_flash = 15;
        let dmg = 80 + (self.frame_count % 20) as i32;
        self.enemy_hp = (self.enemy_hp - dmg).max(0);
        self.vfx.floating_texts.push(FloatingText {
            x: ex - 3.0,
            y: ey - 5.0,
            text: format!("ULTIMATE -{}", dmg),
            fg: Color(255, 100, 255),
            vy: -1.5,
            lifetime: 2.5,
            max_lifetime: 2.5,
            bold: true,
        });
        self.add_log(format!("ULTIMATE! {} damage!", dmg));
    }

    fn update(&mut self, dt: f32) {
        self.time += dt;
        self.frame_count += 1;
        self.vfx.update(dt);

        if self.kael_flash > 0 {
            self.kael_flash -= 1;
        }
        if self.mira_flash > 0 {
            self.mira_flash -= 1;
        }
        if self.enemy_flash > 0 {
            self.enemy_flash -= 1;
        }

        if self.enemy_hp <= 0 {
            self.enemy_hp = self.enemy_max_hp;
            self.add_log("Enemy defeated! Respawning...".to_string());
            self.vfx.particles.extend(emit_burst(
                56.0,
                10.0,
                40,
                Color(255, 200, 50),
                &['✦', '*', '◇', '·', '°', '†', '‡'],
            ));
            self.vfx
                .flashes
                .push(Flash::full_screen(Color(255, 255, 200), 0.2));
            self.vfx.shakes.push(ScreenShake::new(4.0, 0.4));
        }
    }

    fn render(&self, w: u16, h: u16) -> Grid {
        let mut grid = Grid::new(w, h);
        grid.clear(Cell::new(' ').with_bg(Color(12, 12, 18)));

        // ── Background pattern ────────────────────────────────────────────
        for y in 0..h {
            for x in 0..w {
                if (x + y) % 8 == 0 {
                    grid.put(
                        x,
                        y,
                        Cell::new('·')
                            .with_fg(Color(25, 25, 35))
                            .with_bg(Color(12, 12, 18)),
                    );
                }
            }
        }

        // ── Ground line ───────────────────────────────────────────────────
        let ground_y = (h as f32 * 0.6) as u16;
        for x in 0..w {
            grid.put(
                x,
                ground_y,
                Cell::new('─')
                    .with_fg(Color(40, 40, 55))
                    .with_bg(Color(12, 12, 18)),
            );
        }

        // ── Characters ────────────────────────────────────────────────────
        let rover_x = (w as f32 * 0.08) as i32;
        let rover_y = (h as f32 * 0.22) as i32;
        if self.kael_flash > 0 {
            grid.blit(&self.sprite_kael_white, rover_x, rover_y);
        } else {
            grid.blit(&self.sprite_kael, rover_x, rover_y);
        }

        let baizhi_x = (w as f32 * 0.22) as i32;
        let baizhi_y = (h as f32 * 0.22) as i32;
        if self.mira_flash > 0 {
            grid.blit(&self.sprite_mira_white, baizhi_x, baizhi_y);
        } else {
            grid.blit(&self.sprite_mira, baizhi_x, baizhi_y);
        }

        let enemy_x = (w as f32 * 0.62) as i32;
        let enemy_y = (h as f32 * 0.18) as i32;
        if self.enemy_flash > 0 {
            grid.blit(&self.sprite_blight_white, enemy_x, enemy_y);
        } else {
            grid.blit(&self.sprite_blight, enemy_x, enemy_y);
        }

        // ── HP Bars ───────────────────────────────────────────────────────
        let bar_y = (h as f32 * 0.62) as u16;
        let rover_bar_x = rover_x as u16;
        // Rover HP
        grid.write_str(
            rover_bar_x,
            bar_y,
            "Kael",
            Color(100, 200, 255),
            Color::BLACK,
        );
        let hp_ratio = self.kael_hp as f32 / self.kael_max_hp as f32;
        grid.draw_progress_bar(
            rover_bar_x,
            bar_y + 1,
            10,
            hp_ratio,
            Cell::new('█').with_fg(Color(80, 180, 240)),
            Cell::new('░').with_fg(Color(40, 40, 40)),
        );
        grid.write_str(
            rover_bar_x + 11,
            bar_y + 1,
            &format!("{}/{}", self.kael_hp, self.kael_max_hp),
            Color(180, 180, 180),
            Color::BLACK,
        );

        // Baizhi HP
        let baizhi_bar_x = baizhi_x as u16;
        grid.write_str(
            baizhi_bar_x,
            bar_y,
            "Mira",
            Color(150, 220, 255),
            Color::BLACK,
        );
        let hp_ratio = self.mira_hp as f32 / self.mira_max_hp as f32;
        grid.draw_progress_bar(
            baizhi_bar_x,
            bar_y + 1,
            10,
            hp_ratio,
            Cell::new('█').with_fg(Color(120, 200, 240)),
            Cell::new('░').with_fg(Color(40, 40, 40)),
        );
        grid.write_str(
            baizhi_bar_x + 11,
            bar_y + 1,
            &format!("{}/{}", self.mira_hp, self.mira_max_hp),
            Color(180, 180, 180),
            Color::BLACK,
        );

        // Crownless HP
        let enemy_bar_x = (w as f32 * 0.55) as u16;
        grid.write_str(
            enemy_bar_x,
            bar_y,
            "Blight Sovereign",
            Color(220, 80, 200),
            Color::BLACK,
        );
        let hp_ratio = self.enemy_hp as f32 / self.enemy_max_hp as f32;
        grid.draw_progress_bar(
            enemy_bar_x,
            bar_y + 1,
            14,
            hp_ratio,
            Cell::new('█').with_fg(Color(200, 60, 60)),
            Cell::new('░').with_fg(Color(40, 40, 40)),
        );
        grid.write_str(
            enemy_bar_x + 15,
            bar_y + 1,
            &format!("{}/{}", self.enemy_hp, self.enemy_max_hp),
            Color(180, 180, 180),
            Color::BLACK,
        );

        // ── Log panel ─────────────────────────────────────────────────────
        let log_x = 0u16;
        let log_y = (h as f32 * 0.72) as u16;
        let log_h = h.saturating_sub(log_y + 1);
        grid.write_str(
            log_x,
            log_y,
            "── Battle Log ──",
            Color(100, 100, 120),
            Color::BLACK,
        );
        let start = if self.log_lines.len() > log_h as usize {
            self.log_lines.len() - log_h as usize
        } else {
            0
        };
        for (i, line) in self.log_lines[start..].iter().enumerate() {
            let ly = log_y + 1 + i as u16;
            if ly >= h {
                break;
            }
            let fg = if line.contains("ULTIMATE") {
                Color(255, 100, 255)
            } else if line.contains("damage") || line.contains("damage!") {
                Color(255, 180, 80)
            } else if line.contains("Heal") || line.contains("HP") {
                Color(100, 255, 150)
            } else if line.contains("Combo") {
                Color(255, 220, 80)
            } else {
                Color(160, 160, 180)
            };
            grid.write_str(log_x, ly, line, fg, Color::BLACK);
        }

        // ── VFX overlay ──────────────────────────────────────────────────
        self.vfx.render(&mut grid, w, h);
        self.vfx.render_flash(&mut grid, w, h);

        // ── Title bar ─────────────────────────────────────────────────────
        let title = "⚔ Tactical RPG VFX ⚔";
        let title_x = w.saturating_sub(title.len() as u16) / 2;
        grid.write_str(title_x, 0, title, Color(255, 200, 80), Color::BLACK);

        // Frame counter
        let fps_str = format!("Frame:{}", self.frame_count);
        grid.write_str(
            w.saturating_sub(fps_str.len() as u16 + 1),
            1,
            &fps_str,
            Color(80, 80, 100),
            Color::BLACK,
        );

        grid
    }
}

fn main() {
    eprintln!("Loading sprites...");
    let scene = Scene::new();
    eprintln!("Sprites loaded. Initializing terminal...");

    let _guard = verryte_tty::init().expect("failed to init terminal");
    let mut scene = scene;
    let mut last_time = Instant::now();
    let mut prev_grid = Grid::new(1, 1);

    loop {
        let now = Instant::now();
        let dt = now.duration_since(last_time);
        last_time = now;
        let dt_secs = dt.as_secs_f32();

        // Input
        while let Some(event) = poll_event() {
            match event {
                InputEvent::Key(Key::Char('q')) | InputEvent::Key(Key::Esc) => {
                    return;
                }
                InputEvent::Key(Key::Char('1')) => {
                    let (w, h) = terminal_size();
                    scene.trigger_fire(w, h);
                }
                InputEvent::Key(Key::Char('2')) => {
                    let (w, h) = terminal_size();
                    scene.trigger_ice(w, h);
                }
                InputEvent::Key(Key::Char('3')) => {
                    let (w, h) = terminal_size();
                    scene.trigger_lightning(w, h);
                }
                InputEvent::Key(Key::Char('4')) => {
                    let (w, h) = terminal_size();
                    scene.trigger_slash(w, h);
                }
                InputEvent::Key(Key::Char('5')) => {
                    let (w, h) = terminal_size();
                    scene.trigger_heal(w, h);
                }
                InputEvent::Key(Key::Char('6')) => {
                    scene.trigger_shake();
                }
                InputEvent::Key(Key::Char('7')) => {
                    let (w, h) = terminal_size();
                    scene.trigger_flash(w, h);
                }
                InputEvent::Key(Key::Char('8')) => {
                    let (w, h) = terminal_size();
                    scene.trigger_aoe(w, h);
                }
                InputEvent::Key(Key::Char('9')) => {
                    let (w, h) = terminal_size();
                    scene.trigger_combo(w, h);
                }
                InputEvent::Key(Key::Char('0')) => {
                    let (w, h) = terminal_size();
                    scene.trigger_ultimate(w, h);
                }
                _ => {}
            }
        }

        // Update
        scene.update(dt_secs);

        // Render
        let (w, h) = terminal_size();
        let mut frame = scene.render(w, h);

        // Apply screen shake offset (shift the entire frame)
        let (ox, oy) = scene.vfx.shake_offset();
        if ox != 0 || oy != 0 {
            let mut shaken = Grid::new(w, h);
            shaken.clear(Cell::new(' ').with_bg(Color(12, 12, 18)));
            for y in 0..h {
                for x in 0..w {
                    let sx = (x as i16 - ox).max(0) as u16;
                    let sy = (y as i16 - oy).max(0) as u16;
                    if sx < w && sy < h {
                        if let Some(cell) = frame.get(sx, sy).copied() {
                            shaken.put(x, y, cell);
                        }
                    }
                }
            }
            frame = shaken;
        }

        verryte_tty::render_diff(&prev_grid, &frame);
        prev_grid = frame;

        // FPS cap
        let elapsed = now.elapsed();
        if elapsed < FRAME_DURATION {
            std::thread::sleep(FRAME_DURATION - elapsed);
        }
    }
}
