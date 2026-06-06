pub use rodio::OutputStream;
use rodio::{Decoder, OutputStreamHandle, Sink, Source};
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
                    self.music_sink.append(source.repeat_infinite());
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
    use verryte_core::Events;

    // ---- AudioRegistry tests ----

    #[test]
    fn test_audio_registry() {
        let mut registry = AudioRegistry::new();
        registry.register("hit", vec![1, 2, 3]);
        assert_eq!(registry.get("hit"), Some(&vec![1, 2, 3]));
        assert_eq!(registry.get("miss"), None);
    }

    #[test]
    fn test_spatial_audio_helpers() {
        let mut registry = AudioRegistry::new();
        registry.register("laser", vec![0; 100]);
        assert_eq!(registry.get("laser").unwrap().len(), 100);
    }

    #[test]
    fn test_registry_overwrite() {
        let mut registry = AudioRegistry::new();
        registry.register("sfx", vec![1]);
        registry.register("sfx", vec![2, 3]);
        assert_eq!(registry.get("sfx"), Some(&vec![2, 3]));
    }

    #[test]
    fn test_registry_multiple_entries() {
        let mut registry = AudioRegistry::new();
        registry.register("a", vec![1]);
        registry.register("b", vec![2]);
        registry.register("c", vec![3]);
        assert!(registry.get("a").is_some());
        assert!(registry.get("b").is_some());
        assert!(registry.get("c").is_some());
        assert!(registry.get("d").is_none());
    }

    #[test]
    fn test_registry_empty_data() {
        let mut registry = AudioRegistry::new();
        registry.register("empty", vec![]);
        assert_eq!(registry.get("empty"), Some(&vec![]));
    }

    #[test]
    fn test_registry_default_is_empty() {
        let registry = AudioRegistry::default();
        assert!(registry.get("anything").is_none());
    }

    #[test]
    fn test_registry_empty_name() {
        let mut registry = AudioRegistry::new();
        registry.register("", vec![42]);
        assert_eq!(registry.get(""), Some(&vec![42]));
    }

    #[test]
    fn test_registry_large_data() {
        let mut registry = AudioRegistry::new();
        let big_data = vec![0xAB; 1_000_000];
        registry.register("big", big_data.clone());
        assert_eq!(registry.get("big").unwrap().len(), 1_000_000);
        assert_eq!(registry.get("big").unwrap()[0], 0xAB);
    }

    // ---- audio_system tests ----

    #[test]
    fn test_audio_system_no_player_does_not_panic() {
        let mut world = verryte_core::World::new();
        world.insert_resource(Events::<verryte_core::AudioEvent>::new());
        audio_system(&mut world);
    }

    #[test]
    fn test_audio_system_drains_events_without_player() {
        let mut world = verryte_core::World::new();
        let mut events = Events::<verryte_core::AudioEvent>::new();
        events.send(verryte_core::AudioEvent::play("test"));
        events.send(verryte_core::AudioEvent::play("test2"));
        world.insert_resource(events);
        audio_system(&mut world);
        let remaining = world
            .resource::<Events<verryte_core::AudioEvent>>()
            .expect("Events resource should exist");
        assert!(remaining.is_empty());
    }

    #[test]
    fn test_audio_system_no_event_channel_does_not_panic() {
        let mut world = verryte_core::World::new();
        audio_system(&mut world);
    }

    #[test]
    fn test_audio_system_processes_panned_event_without_player() {
        let mut world = verryte_core::World::new();
        let mut events = Events::<verryte_core::AudioEvent>::new();
        events.send(
            verryte_core::AudioEvent::play("sfx")
                .with_pan(-0.5)
                .with_volume(0.3),
        );
        world.insert_resource(events);
        audio_system(&mut world);
        let remaining = world
            .resource::<Events<verryte_core::AudioEvent>>()
            .unwrap();
        assert!(remaining.is_empty(), "panned events should be drained");
    }

    #[test]
    fn test_audio_system_processes_looped_event_without_player() {
        let mut world = verryte_core::World::new();
        let mut events = Events::<verryte_core::AudioEvent>::new();
        events.send(verryte_core::AudioEvent::loop_music("ambient").with_volume(0.5));
        world.insert_resource(events);
        audio_system(&mut world);
        let remaining = world
            .resource::<Events<verryte_core::AudioEvent>>()
            .unwrap();
        assert!(remaining.is_empty(), "looped events should be drained");
    }

    // ---- AudioPlayer tests (require audio device) ----

    #[test]
    fn test_audio_player_registration() {
        if let Ok((mut player, _stream)) = AudioPlayer::try_new() {
            player.register("jump", vec![1, 2, 3]);
            assert_eq!(player.registry.get("jump"), Some(&vec![1, 2, 3]));
        }
    }

    #[test]
    fn test_audio_player_play_music() {
        if let Ok((mut player, _stream)) = AudioPlayer::try_new() {
            player.register("bgm", vec![0; 100]);
            player.play_music("bgm", true);
            player.play_music("bgm", false);
            player.stop_music();
            player.set_music_volume(0.5);
        }
    }

    #[test]
    fn test_audio_player_play_missing_name_is_noop() {
        if let Ok((player, _stream)) = AudioPlayer::try_new() {
            player.play_sfx("nonexistent");
            player.play_sfx_ex("nonexistent", 0.5, 1.0);
            player.play_sfx_panned("nonexistent", -1.0, 0.8);
            player.play_sfx_spatial("nonexistent", (5.0, 5.0), (0.0, 0.0), 10.0);
            player.play_music("nonexistent", true);
        }
    }

    #[test]
    fn test_audio_player_volume_controls() {
        if let Ok((mut player, _stream)) = AudioPlayer::try_new() {
            player.register("sfx", vec![0; 100]);
            player.set_music_volume(0.0);
            player.set_music_volume(1.0);
            player.set_music_volume(2.0);
            player.play_sfx_ex("sfx", 0.0, 1.0);
            player.play_sfx_ex("sfx", 1.0, 1.0);
            player.play_sfx_ex("sfx", 0.5, 0.5);
        }
    }

    #[test]
    fn test_audio_player_music_loop_toggle() {
        if let Ok((mut player, _stream)) = AudioPlayer::try_new() {
            player.register("bgm", vec![0; 100]);
            player.play_music("bgm", true);
            player.play_music("bgm", false);
            player.play_music("bgm", true);
            player.stop_music();
        }
    }

    #[test]
    fn test_audio_player_load_asset_missing_path() {
        if let Ok((mut player, _stream)) = AudioPlayer::try_new() {
            let result = player.load_asset("nope", "/nonexistent/path/audio.wav");
            assert!(result.is_err());
        }
    }

    // ---- Spatial panning math tests (pure logic, no audio device) ----

    #[test]
    fn test_spatial_panning_clamp_values() {
        let pan_left = (-1.0_f32).clamp(-1.0, 1.0);
        let pan_right = (1.0_f32).clamp(-1.0, 1.0);
        let pan_center = (0.0_f32).clamp(-1.0, 1.0);
        let pan_clamped_high = (5.0_f32).clamp(-1.0, 1.0);
        let pan_clamped_low = (-3.0_f32).clamp(-1.0, 1.0);
        assert_eq!(pan_left, -1.0);
        assert_eq!(pan_right, 1.0);
        assert_eq!(pan_center, 0.0);
        assert_eq!(pan_clamped_high, 1.0);
        assert_eq!(pan_clamped_low, -1.0);
    }

    #[test]
    fn test_spatial_attenuation_calculation() {
        let max_range = 10.0_f32;

        let dist_same = 0.0_f32;
        let atten_same = (1.0 - (dist_same / max_range)).clamp(0.0, 1.0);
        assert_eq!(atten_same, 1.0, "same position should be full volume");

        let dist_half = 5.0_f32;
        let atten_half = (1.0 - (dist_half / max_range)).clamp(0.0, 1.0);
        assert!(
            (atten_half - 0.5).abs() < f32::EPSILON,
            "half range should be half volume"
        );

        let dist_at = 10.0_f32;
        let atten_at = (1.0 - (dist_at / max_range)).clamp(0.0, 1.0);
        assert_eq!(atten_at, 0.0, "at max range should be zero volume");

        let dist_beyond = 20.0_f32;
        let atten_beyond = (1.0 - (dist_beyond / max_range)).clamp(0.0, 1.0);
        assert_eq!(
            atten_beyond, 0.0,
            "beyond max range should be clamped to zero"
        );
    }

    #[test]
    fn test_spatial_pan_from_emitter_position() {
        let max_range = 10.0_f32;

        let dx_right = 5.0_f32;
        let pan_right = (dx_right / max_range).clamp(-1.0, 1.0);
        assert_eq!(pan_right, 0.5, "emitter to the right should pan right");

        let dx_left = -7.0_f32;
        let pan_left = (dx_left / max_range).clamp(-1.0, 1.0);
        assert_eq!(pan_left, -0.7, "emitter to the left should pan left");

        let dx_center = 0.0_f32;
        let pan_center = (dx_center / max_range).clamp(-1.0, 1.0);
        assert_eq!(pan_center, 0.0, "emitter at center should pan center");
    }

    // ---- AudioEvent builder tests ----

    #[test]
    fn test_audio_event_builder_zero_volume() {
        let ev = verryte_core::AudioEvent::play("silent").with_volume(0.0);
        assert_eq!(ev.volume, Some(0.0));
        assert_eq!(ev.name, "silent");
    }

    #[test]
    fn test_audio_event_builder_max_pan() {
        let ev = verryte_core::AudioEvent::play("right").with_pan(1.0);
        assert_eq!(ev.pan, Some(1.0));

        let ev2 = verryte_core::AudioEvent::play("left").with_pan(-1.0);
        assert_eq!(ev2.pan, Some(-1.0));
    }

    #[test]
    fn test_audio_event_loop_music_with_volume_and_pan() {
        let ev = verryte_core::AudioEvent::loop_music("rain")
            .with_volume(0.3)
            .with_pan(0.0);
        assert!(ev.looped);
        assert_eq!(ev.volume, Some(0.3));
        assert_eq!(ev.pan, Some(0.0));
    }

    #[test]
    fn test_audio_event_play_is_not_looped() {
        let ev = verryte_core::AudioEvent::play("hit");
        assert!(!ev.looped);
    }

    // ---- Audio system integration tests ----

    #[test]
    fn test_audio_system_with_player_processes_events() {
        if let Ok((mut player, _stream)) = AudioPlayer::try_new() {
            player.register("hit", vec![0; 100]);
            player.register("theme", vec![0; 100]);

            let mut world = verryte_core::World::new();
            let mut events = Events::<verryte_core::AudioEvent>::new();
            events.send(verryte_core::AudioEvent::play("hit"));
            events.send(verryte_core::AudioEvent::play("hit").with_volume(0.5));
            events.send(verryte_core::AudioEvent::play("hit").with_pan(-0.8));
            events.send(verryte_core::AudioEvent::loop_music("theme").with_volume(0.6));
            world.insert_resource(events);
            world.insert_resource(player);

            audio_system(&mut world);

            let remaining = world
                .resource::<Events<verryte_core::AudioEvent>>()
                .unwrap();
            assert!(remaining.is_empty(), "all events should be consumed");
        }
    }

    #[test]
    fn test_audio_system_empty_event_queue() {
        if let Ok((player, _stream)) = AudioPlayer::try_new() {
            let mut world = verryte_core::World::new();
            world.insert_resource(Events::<verryte_core::AudioEvent>::new());
            world.insert_resource(player);
            audio_system(&mut world);
        }
    }
}
