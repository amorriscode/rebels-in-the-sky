use crate::store::ASSETS_DIR;
use crate::types::AppResult;
use once_cell::sync::Lazy;
use rodio::{Decoder, OutputStream, Source};
use rodio::{OutputStreamHandle, Sink};
use serde::Deserialize;
use std::io::Cursor;

pub static PLAYLIST_DATA: Lazy<Option<Vec<SampleData>>> = Lazy::new(|| {
    let file = ASSETS_DIR.get_file("data/playlist_data.json")?;
    let data = file.contents_utf8()?;
    serde_json::from_str(&data).ok()
});

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SampleData {
    pub title: String,
    pub filename: String,
}

pub struct MusicPlayer {
    _stream: OutputStream,
    _stream_handle: OutputStreamHandle,
    sink: Sink,
    pub is_playing: bool,
    sources: Vec<rodio::source::Buffered<Decoder<Cursor<Vec<u8>>>>>,
    source_index: Option<usize>,
}

unsafe impl Send for MusicPlayer {}

impl MusicPlayer {
    pub fn new() -> AppResult<MusicPlayer> {
        let (_stream, _stream_handle) = OutputStream::try_default()?;
        let mut player = MusicPlayer {
            sink: Sink::try_new(&_stream_handle)?,
            _stream,
            _stream_handle,
            is_playing: true,
            sources: vec![],
            source_index: None,
        };

        for sample in PLAYLIST_DATA.as_ref().unwrap() {
            player.load_source(format!("sounds/{}", &sample.filename).as_str())?;
        }

        // Start in paused state.
        player.pause();
        player.next();

        Ok(player)
    }

    fn load_source(&mut self, file_path: &str) -> AppResult<()> {
        let data = ASSETS_DIR
            .get_file(file_path)
            .ok_or("Failed to load sound file".to_string())?;

        let file = Cursor::new(data.contents().to_vec());
        let source = Decoder::new(file)?.buffered();
        self.sources.push(source);

        Ok(())
    }

    pub fn play(&mut self) {
        self.sink.play();
        self.is_playing = true;
    }

    pub fn pause(&mut self) {
        self.sink.pause();
        self.is_playing = false;
    }

    pub fn check_if_next(&mut self) {
        if self.sink.empty() {
            self.next();
        }
    }

    pub fn next(&mut self) {
        if let Some(idx) = self.source_index {
            self.source_index = Some((idx + 1) % self.sources.len());
        } else {
            self.source_index = Some(0);
        }

        self.sink.clear();
        self.sink.append(
            self.sources[self
                .source_index
                .expect("audio.rs: source_index should have been set")]
            .clone(),
        );

        if self.is_playing {
            self.sink.play();
        }
    }

    pub fn previous(&mut self) {
        if let Some(idx) = self.source_index {
            self.source_index = Some((idx + self.sources.len() - 1) % self.sources.len());
        } else {
            self.source_index = Some(0);
        }
        self.sink.clear();
        self.sink.append(
            self.sources[self
                .source_index
                .expect("audio.rs: source_index should have been set")]
            .clone(),
        );

        if self.is_playing {
            self.sink.play();
        }
    }

    pub fn toggle(&mut self) {
        if self.is_playing {
            self.pause();
        } else {
            self.play();
        }
    }

    pub fn currently_playing(&self) -> Option<&SampleData> {
        if let Some(index) = self.source_index {
            return PLAYLIST_DATA.as_ref()?.get(index);
        }
        None
    }
}
