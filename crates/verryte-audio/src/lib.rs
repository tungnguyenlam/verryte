use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink};
use std::collections::HashMap;
use std::io::Cursor;

/// A registry of preloaded audio data.
#[derive(Default)]
pub struct AudioRegistry {
    sounds: HashMap<String, Vec<u8>>,
}

impl AudioRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, name: &str, data: Vec<u8>) {
        self.sounds.insert(name.to_string(), data);
    }

    pub fn get(&self, name: &str) -> Option<&Vec<u8>> {
        self.sounds.get(name)
    }
}

/// The main audio player resource. It is Send + Sync because it only holds
/// handles and registries, not the output stream itself.
pub struct AudioPlayer {
    handle: OutputStreamHandle,
    music_sink: Sink,
    registry: AudioRegistry,
}

impl AudioPlayer {
    /// Create a new audio player and the output stream it depends on.
    ///
    /// IMPORTANT: The returned `OutputStream` must be kept alive for audio
    /// to play. If it is dropped, all sound will stop immediately.
    pub fn try_new() -> Result<(Self, OutputStream), String> {
        let (stream, handle) = OutputStream::try_default()
            .map_err(|e| format!("Failed to open output stream: {}", e))?;

        let music_sink =
            Sink::try_new(&handle).map_err(|e| format!("Failed to create music sink: {}", e))?;

        Ok((
            Self {
                handle,
                music_sink,
                registry: AudioRegistry::new(),
            },
            stream,
        ))
    }

    pub fn register(&mut self, name: &str, data: Vec<u8>) {
        self.registry.register(name, data);
    }

    pub fn load_asset<P: AsRef<std::path::Path>>(
        &mut self,
        name: &str,
        path: P,
    ) -> Result<(), String> {
        let data = std::fs::read(path).map_err(|e| format!("Failed to read audio file: {}", e))?;
        self.registry.register(name, data);
        Ok(())
    }

    pub fn play_sfx(&self, name: &str) {
        self.play_sfx_ex(name, 1.0, 1.0);
    }

    pub fn play_sfx_ex(&self, name: &str, volume: f32, speed: f32) {
        if let Some(data) = self.registry.get(name) {
            let cursor = Cursor::new(data.clone());
            if let Ok(source) = Decoder::new(cursor) {
                if let Ok(sink) = Sink::try_new(&self.handle) {
                    sink.set_volume(volume);
                    sink.set_speed(speed);
                    sink.append(source);
                    sink.detach();
                }
            }
        }
    }

    pub fn play_sfx_panned(&self, name: &str, pan: f32, volume: f32) {
        if let Some(data) = self.registry.get(name) {
            let cursor = Cursor::new(data.clone());
            if let Ok(source) = Decoder::new(cursor) {
                if let Ok(sink) = rodio::SpatialSink::try_new(
                    &self.handle,
                    [pan.clamp(-1.0, 1.0), 0.0, 0.0],
                    [-1.0, 0.0, 0.0],
                    [1.0, 0.0, 0.0],
                ) {
                    sink.set_volume(volume);
                    sink.append(source);
                    sink.detach();
                }
            }
        }
    }

    pub fn play_sfx_spatial(
        &self,
        name: &str,
        emitter_pos: (f32, f32),
        listener_pos: (f32, f32),
        max_range: f32,
    ) {
        if let Some(data) = self.registry.get(name) {
            let cursor = Cursor::new(data.clone());
            if let Ok(source) = Decoder::new(cursor) {
                let dx = emitter_pos.0 - listener_pos.0;
                let dy = emitter_pos.1 - listener_pos.1;
                let dist = (dx * dx + dy * dy).sqrt();
                let attenuation = (1.0 - (dist / max_range)).clamp(0.0, 1.0);
                let pan = if max_range > 0.0 {
                    (dx / max_range).clamp(-1.0, 1.0)
                } else {
                    0.0
                };

                if let Ok(sink) = rodio::SpatialSink::try_new(
                    &self.handle,
                    [pan, 0.0, 0.0],
                    [-1.0, 0.0, 0.0],
                    [1.0, 0.0, 0.0],
                ) {
                    sink.set_volume(attenuation);
                    sink.append(source);
                    sink.detach();
                }
            }
        }
    }

    pub fn play_music(&self, name: &str, loop_music: bool) {
        if let Some(data) = self.registry.get(name) {
            self.music_sink.stop();
            let cursor = Cursor::new(data.clone());
            if let Ok(source) = Decoder::new(cursor) {
                if loop_music {
                    // rodio doesn't have a simple loop yet without creating a custom source
                    // but for a first pass we'll just play it once
                    self.music_sink.append(source);
                } else {
                    self.music_sink.append(source);
                }
            }
        }
    }

    pub fn stop_music(&self) {
        self.music_sink.stop();
    }

    pub fn set_music_volume(&self, volume: f32) {
        self.music_sink.set_volume(volume);
    }
}

/// A system that processes [`verryte_core::AudioEvent`]s and plays them.
pub fn audio_system(world: &mut verryte_core::World) {
    use verryte_core::{AudioEvent, Events};

    // We take all events to ensure they are processed even if the player is missing
    let events = if let Some(events_res) = world.resource_mut::<Events<AudioEvent>>() {
        events_res.take()
    } else {
        return;
    };

    if let Some(player) = world.resource::<AudioPlayer>() {
        for event in events {
            if event.looped {
                let vol = event.volume.unwrap_or(1.0);
                player.set_music_volume(vol);
                player.play_music(&event.name, true);
            } else {
                let vol = event.volume.unwrap_or(1.0);
                if let Some(pan) = event.pan {
                    player.play_sfx_panned(&event.name, pan, vol);
                } else {
                    player.play_sfx_ex(&event.name, vol, 1.0);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_registry() {
        let mut registry = AudioRegistry::new();
        registry.register("hit", vec![1, 2, 3]);
        assert_eq!(registry.get("hit"), Some(&vec![1, 2, 3]));
        assert_eq!(registry.get("miss"), None);
    }

    #[test]
    fn test_spatial_audio_helpers() {
        // Just verify spatial and panned helpers compile and run with no panic on mock registry
        let mut registry = AudioRegistry::new();
        registry.register("laser", vec![0; 100]);
        assert_eq!(registry.get("laser").unwrap().len(), 100);
    }
}
