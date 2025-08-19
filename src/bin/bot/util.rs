//! This module contains utilities usable in various contexts.

use image::{ImageBuffer, Luma};
use poise::serenity_prelude::{ComponentInteraction, GuildId};
use rusttype::{Font, Scale, point};
use std::fmt::Display;
use std::{mem, slice};
use std::{fmt::Debug, process, str::FromStr};
use std::{io::Cursor, sync::LazyLock};
use tracing::{error, warn};

use crate::models::net::KateContext;

pub trait ParseUnwrapAll<T>: IntoIterator
where
    Self::Item: AsRef<str>,
    T: FromStr,
    T::Err: Debug,
{
    /// Parses every item in `Self` and collects them.
    ///
    /// # Termination
    /// Process will exit on a failing parse.
    fn parse_unwrap_all(self) -> Vec<T>;
}

impl<I, T> ParseUnwrapAll<T> for I
where
    I: IntoIterator,
    I::Item: AsRef<str>,
    T: FromStr,
    T::Err: Debug,
{
    fn parse_unwrap_all(self) -> Vec<T> {
        self.into_iter()
            .map(|value| {
                value.as_ref().parse().unwrap_or_else(|err| {
                    error!(value = value.as_ref(), error = ?err, "Failed to parse value");
                    process::exit(2)
                })
            })
            .collect()
    }
}

pub trait LobbyId {
    /// Gets the lobby_id. If the source is from a guild, it is the guild_id,
    /// otherwise it's the user_id.
    fn lobby_id(&self) -> u64;
}

impl LobbyId for KateContext<'_> {
    fn lobby_id(&self) -> u64 {
        self.guild_id()
            .map(GuildId::get)
            .unwrap_or(self.author().id.get())
    }
}

impl LobbyId for ComponentInteraction {
    fn lobby_id(&self) -> u64 {
        self.guild_id
            .map(GuildId::get)
            .unwrap_or(self.user.id.get())
    }
}

pub trait GameId {
    /// Gets game_id which identifies which game Self is intended for.
    fn game_id(&self) -> &str;
}

impl GameId for ComponentInteraction {
    /// Extracts the first CSV field from data.custom_id.
    fn game_id(&self) -> &str {
        self.data
            .custom_id
            .split_once(",")
            .map(|(left, _)| left)
            .unwrap_or(&self.data.custom_id)
    }
}

pub trait Logging {
    /// Logs with `message` at the WARN level if Err. Returns the original Self.
    fn on_err_warn(self, message: &str) -> Self
    where
        Self: Sized;

    /// Logs with "Send failed" at the WARN level if Err. Returns the original Self.
    fn on_err_warn_send_failed(self) -> Self
    where
        Self: Sized;
}

impl<T, E> Logging for Result<T, E>
where
    E: Display,
{
    fn on_err_warn(self, message: &str) -> Self
    where
        Self: Sized,
    {
        if let Err(ref error) = self {
            warn!(%error, message);
        }
        self
    }

    fn on_err_warn_send_failed(self) -> Self
    where
        Self: Sized,
    {
        if let Err(ref error) = self {
            warn!(%error, "Send failed");
        }
        self
    }
}

/// A warpper around [`Vec`] that lets it be used as if its a map.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct IndexMap<K, V>(Vec<(K, V)>);

impl<K, V> Default for IndexMap<K, V> {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<K: Eq, V> IndexMap<K, V> {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self::default()
    }

    /// Finds a value in the map using `key`.
    pub fn get(&self, key: &K) -> Option<&V> {
        self.0
            .iter()
            .find(|entry| &entry.0 == key)
            .map(|entry| &entry.1)
    }

    /// Checks if the key exists in the map.
    pub fn contains_key(&self, key: &K) -> bool {
        self.0.iter().any(|entry| &entry.0 == key)
    }

    /// Removes a value in the map using `key` and returns it.
    #[allow(dead_code)]
    pub fn remove(&mut self, key: &K) -> Option<V> {
        self.0
            .iter()
            .position(|entry| entry.0 == *key)
            .map(|idx| self.0.swap_remove(idx).1)
    }

    pub fn iter(&self) -> slice::Iter<'_, (K, V)> {
        self.0.iter()
    }

    /// Gets the number of entries in the map.
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Inserts a key value pair into the map. Replaces value if key already
    /// exists. Returns the original value.
    #[allow(dead_code)]
    pub fn insert(&mut self, (key, value): (K, V)) -> Option<V> {
        match self.0.iter_mut().find(|entry| entry.0 == key) {
            Some(entry) => {
                let prev = mem::replace(&mut entry.1, value);
                Some(prev)
            }
            None => {
                self.0.push((key, value));
                None
            },
        }
    }

    /// Finds a value in the map by the given key, or inserts it if it doesn't exist
    #[allow(dead_code)]
    pub fn get_or_insert(&mut self, key: K, value: V) -> &mut V {
        match self.0.iter().position(|entry| entry.0 == key) {
            Some(i) => &mut self.0[i].1,
            None => {
                self.0.push((key, value));
                &mut self.0.last_mut().unwrap().1
            }
        }
    }

    /// Finds a value in the map by the given key, or inserts it if it doesn't exist
    pub fn get_or_insert_with(&mut self, k: K, v: impl FnOnce() -> V) -> &mut V {
        match self.0.iter().position(|entry| entry.0 == k) {
            Some(i) => &mut self.0[i].1,
            None => {
                self.0.push((k, v()));
                &mut self.0.last_mut().unwrap().1
            }
        }
    }
}

impl<K, V> IntoIterator for IndexMap<K, V> {
    type Item = (K, V);
    type IntoIter = std::vec::IntoIter<(K, V)>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

/// Converts `text` into a rasterized PNG image in bytes.
pub fn text_to_image(text: &str) -> Vec<u8> {
    static FONT: LazyLock<Font<'static>> = LazyLock::new(|| {
        static FONT_DATA: &[u8; 5728064] = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/fonts/NotoSansJPBold.ttf"
        ));

        Font::try_from_bytes(FONT_DATA).unwrap()
    });

    let scale = Scale::uniform(72.0);
    let v_metrics = FONT.v_metrics(scale);

    const PADDING: f32 = 60.0;

    let glyphs: Vec<_> = FONT
        .layout(text, scale, point(PADDING, PADDING + v_metrics.ascent))
        .collect();

    let glyphs_height = (v_metrics.ascent - v_metrics.descent).ceil() as u32;
    let glyphs_width = {
        let min_x = glyphs
            .first()
            .map(|g| g.pixel_bounding_box().unwrap().min.x)
            .unwrap();
        let max_x = glyphs
            .last()
            .map(|g| g.pixel_bounding_box().unwrap().max.x)
            .unwrap();
        (max_x - min_x) as u32
    };

    let mut image = ImageBuffer::<Luma<u8>, Vec<u8>>::from_pixel(
        glyphs_width + (PADDING * 2.0) as u32,
        glyphs_height + (PADDING * 2.0) as u32,
        Luma([255]),
    );

    for glyph in glyphs {
        if let Some(bounding_box) = glyph.pixel_bounding_box() {
            glyph.draw(|x, y, v| {
                image.put_pixel(
                    // Offset the position by the glyph bounding box
                    x + bounding_box.min.x as u32,
                    y + bounding_box.min.y as u32,
                    Luma([255 - (v * 255.0) as u8]),
                )
            });
        }
    }

    let mut buf = Cursor::new(Vec::new());
    image.write_to(&mut buf, image::ImageFormat::Png).unwrap();

    buf.into_inner()
}

pub enum RetryResult {
    /// Action was successful.
    Success,
    /// Action failed but there are retries left.
    Fail,
    /// Action failed and there are no more retries left.
    Terminal,
}

/// Can be used with functions that return a success flag.
pub struct Retry {
    tries: u32,
}

impl Retry {
    pub fn new() -> Self {
        Retry { tries: 0 }
    }

    /// Try a function, returns [`RetryResult::Terminal`] after
    /// three consecutive fails.
    pub async fn try_async<F, Fut>(&mut self, f: F) -> RetryResult
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = bool>,
    {
        const MAX_TRIES: u32 = 3;
        if self.tries >= MAX_TRIES {
            return RetryResult::Terminal;
        }

        match f().await {
            true => {
                self.tries = 0;
                RetryResult::Success
            }
            false => {
                self.tries += 1;
                RetryResult::Fail
            }
        }
    }
}
