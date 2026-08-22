use std::time::{Duration, Instant};
use verryte_input::{InputEvent, Key};
use verryte_map::Point;
use verryte_terminal::{Cell, Color, Grid};
use verryte_tty::{init, poll_event, render, render_diff};
use wuthering_terminal::map::{TacticalMap, Tile};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut map = TacticalMap::tactical();
    let mut cursor = Point::new(0, 0);
    let mut selected_tile = Tile::Wall;
    let mut prev_frame: Option<Grid> = None;

    let _guard = init()?;

    let mut last_tick = Instant::now();
    let tick_rate = Duration::from_millis(33);

    loop {
        let (w, h) = verryte_tty::terminal_size();

        while let Some(event) = poll_event() {
            if let InputEvent::Resize { .. } = event {
                prev_frame = None;
                continue;
            }
            if let InputEvent::Key {
                key,
                kind: verryte_input::KeyEventKind::Press,
            } = event
            {
                match key {
                    Key::Esc | Key::Char('q') => return Ok(()),
                    Key::Up | Key::Char('w') => cursor.y = (cursor.y - 1).max(0),
                    Key::Down | Key::Char('s') => {
                        cursor.y = (cursor.y + 1).min(map.height as i16 - 1)
                    }
                    Key::Left | Key::Char('a') => cursor.x = (cursor.x - 1).max(0),
                    Key::Right | Key::Char('d') => {
                        cursor.x = (cursor.x + 1).min(map.width as i16 - 1)
                    }
                    Key::Char('1') => selected_tile = Tile::Wall,
                    Key::Char('2') => selected_tile = Tile::Grass,
                    Key::Char('3') => selected_tile = Tile::Water,
                    Key::Char('4') => selected_tile = Tile::Lava,
                    Key::Char('5') => selected_tile = Tile::Mud,
                    Key::Char('6') => selected_tile = Tile::Ice,
                    Key::Char('7') => selected_tile = Tile::Stairs,
                    Key::Char('8') => selected_tile = Tile::SpikeTrap,
                    Key::Char('9') => selected_tile = Tile::ExplodingBarrel,
                    Key::Char('0') => selected_tile = Tile::PoisonCloud,
                    Key::Char('p') | Key::Char('P') => selected_tile = Tile::HealingSpring,
                    Key::Char('c') | Key::Char('C') => selected_tile = Tile::CrackedFloor,
                    Key::Char('t') | Key::Char('T') => selected_tile = Tile::PressurePlate,
                    Key::Char('b') | Key::Char('B') => selected_tile = Tile::ThornBush,
                    Key::Char('g') | Key::Char('G') => selected_tile = Tile::SteamVent,
                    Key::Char('v') | Key::Char('V') => {
                        if let Ok(s) = serde_json::to_string_pretty(&map) {
                            let _ = std::fs::write("custom_map.json", s);
                        }
                    }
                    Key::Char('l') | Key::Char('L') => {
                        if let Ok(content) = std::fs::read_to_string("custom_map.json") {
                            if let Ok(loaded) = serde_json::from_str::<TacticalMap>(&content) {
                                map = loaded;
                            }
                        }
                    }
                    Key::Enter | Key::Char(' ') => {
                        map.tiles.set(cursor, selected_tile);
                    }
                    _ => {}
                }
            }
        }

        // Render Map
        let mut grid = Grid::new(w, h);
        let cell_w = 4;
        let cell_h = 2;
        let offset_x = (w.saturating_sub(map.width * cell_w) / 2) as i32;
        let offset_y = (h.saturating_sub(map.height * cell_h) / 2) as i32;

        for y in 0..map.height {
            for x in 0..map.width {
                let tile = map.tile(x as i16, y as i16);
                let (ch, fg, bg) = match tile {
                    Tile::Wall => ('#', Color(100, 100, 100), Color(50, 50, 50)),
                    Tile::Grass => ('.', Color(50, 200, 50), Color(20, 50, 20)),
                    Tile::Water => ('~', Color(50, 50, 200), Color(20, 20, 100)),
                    Tile::Lava => ('^', Color(255, 50, 50), Color(100, 20, 20)),
                    Tile::Ice => ('-', Color(200, 200, 255), Color(50, 50, 150)),
                    Tile::Stairs => ('>', Color(255, 255, 0), Color(50, 50, 0)),
                    Tile::Mud => ('=', Color(150, 100, 50), Color(50, 30, 10)),
                    Tile::SpikeTrap => ('!', Color(200, 200, 200), Color(50, 50, 50)),
                    Tile::ExplodingBarrel => ('o', Color(255, 128, 0), Color(100, 50, 0)),
                    Tile::PoisonCloud => ('p', Color(128, 0, 128), Color(50, 20, 50)),
                    Tile::HealingSpring => ('+', Color(0, 255, 128), Color(10, 50, 30)),
                    Tile::CrackedFloor => ('%', Color(140, 120, 100), Color(40, 30, 20)),
                    Tile::PressurePlate => ('T', Color(255, 128, 128), Color(50, 30, 30)),
                    Tile::ThornBush => ('*', Color(100, 150, 50), Color(30, 40, 20)),
                    Tile::SteamVent => ('v', Color(200, 200, 200), Color(80, 80, 80)),
                };

                let px = offset_x + (x * cell_w) as i32;
                let py = offset_y + (y * cell_h) as i32;

                for dy in 0..cell_h {
                    for dx in 0..cell_w {
                        grid.put(
                            (px + dx as i32) as u16,
                            (py + dy as i32) as u16,
                            Cell::new(ch).with_fg(fg).with_bg(bg),
                        );
                    }
                }
            }
        }

        // Draw cursor
        let cx = offset_x + (cursor.x * cell_w as i16) as i32;
        let cy = offset_y + (cursor.y * cell_h as i16) as i32;
        for dy in 0..cell_h {
            for dx in 0..cell_w {
                if let Some(cell) = grid.get_mut((cx + dx as i32) as u16, (cy + dy as i32) as u16) {
                    cell.bg = Color(255, 255, 255);
                    if cell.fg == Color(255, 255, 255) {
                        cell.fg = Color(0, 0, 0); // ensure contrast
                    }
                }
            }
        }

        // Draw UI
        grid.write_str(
            0,
            0,
            &format!(
                "Level Editor | Tile: {:?} | (1-9/0/p/c/t/b/g to change, Space to paint, V to Save, L to Load, Q to quit)",
                selected_tile
            ),
            Color(255, 255, 255),
            Color(0, 0, 0),
        );

        if let Some(prev) = prev_frame {
            render_diff(&prev, &grid);
        } else {
            render(&grid);
        }
        prev_frame = Some(grid);

        let now = Instant::now();
        let elapsed = now.duration_since(last_tick);
        if elapsed < tick_rate {
            std::thread::sleep(tick_rate - elapsed);
        }
        last_tick = Instant::now();
    }
}
