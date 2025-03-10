use super::{ChannelState, ChannelStateError};
use crate::request::{CommandRequest, FromCommandRequest};
use arc_swap::ArcSwapOption;
use ron::ser::PrettyConfig;
use std::fs::OpenOptions;
use std::ops::Deref;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Semaphore;

pub trait PersistedType:
    serde::Serialize + for<'de> serde::Deserialize<'de> + Sync + Send + 'static
{
    const FILENAME: &'static str;

    // might be called multiple times!
    fn init(channel: &str) -> Self;

    fn handle_read_error(channel: &str, _error: anyhow::Error) -> Self {
        Self::init(channel)
    }

    fn handle_write_error(_channel: &str, _error: anyhow::Error) {
        // do nothing
    }
}

pub(crate) struct Persisted<T: PersistedType> {
    inner: ArcSwapOption<T>,
    lock: Semaphore,
}

impl<T: PersistedType> From<T> for Persisted<T> {
    fn from(value: T) -> Self {
        Self {
            inner: ArcSwapOption::new(Some(Arc::new(value))),
            lock: Semaphore::new(1),
        }
    }
}

impl<T: PersistedType> Persisted<T> {
    pub fn new() -> Self {
        Self {
            inner: ArcSwapOption::new(None),
            lock: Semaphore::new(1),
        }
    }

    fn for_channel<'a>(&'a self, channel: &'a str) -> PersistedChannelState<'a, T> {
        PersistedChannelState {
            inner: &self.inner,
            lock: &self.lock,
            channel,
        }
    }
}

pub struct PersistedChannelState<'a, T: PersistedType> {
    inner: &'a ArcSwapOption<T>,
    lock: &'a Semaphore,
    channel: &'a str,
}

impl<'a, 'req, T: PersistedType> FromCommandRequest<'a, 'req> for PersistedChannelState<'req, T> {
    type Error = ChannelStateError;

    fn from_command_request(request: &'a CommandRequest<'req>) -> Result<Self, Self::Error> {
        let channel_state =
            <ChannelState<Persisted<T>> as FromCommandRequest>::from_command_request(request)?;
        let channel = request.channel();
        Ok(channel_state.for_channel(channel.username()))
    }
}

impl<T: PersistedType> PersistedChannelState<'_, T> {
    pub async fn read(&self) -> Arc<T> {
        match self.inner.load().deref() {
            Some(value) => value.clone(),
            None => {
                let permit = self.lock.acquire().await.unwrap();
                if let Some(value) = self.inner.load().deref() {
                    return value.clone();
                }
                let value = read_from_disk::<T>(self.channel).await;
                let result = value.unwrap_or_else(|e| {
                    log::error!(
                        "Error loading {} for channel {} from disk: {:?}",
                        <T as PersistedType>::FILENAME,
                        self.channel,
                        e
                    );
                    Some(<T as PersistedType>::handle_read_error(self.channel, e))
                });
                let result = result.unwrap_or_else(|| <T as PersistedType>::init(self.channel));
                let result = Arc::new(result);
                self.inner.store(Some(result.clone()));
                drop(permit);
                result
            }
        }
    }

    pub async fn maybe_update<R, F>(&self, mut f: F) -> (Arc<T>, Option<Arc<T>>)
    where
        F: FnMut(&T) -> Option<R>,
        R: Into<T>,
    {
        let permit = self.lock.acquire().await.unwrap();
        log::debug!("{} - MAYBE UPDATE", <T as PersistedType>::FILENAME);
        let value = if let Some(value) = self.inner.load().deref() {
            log::debug!("{} - EXSISTS", <T as PersistedType>::FILENAME);
            value.clone()
        } else {
            log::debug!("{} - INIT", <T as PersistedType>::FILENAME);

            let value = read_from_disk::<T>(self.channel).await;
            let result = value.unwrap_or_else(|e| {
                log::error!(
                    "Error loading {} for channel {} from disk: {:?}",
                    <T as PersistedType>::FILENAME,
                    self.channel,
                    e
                );
                Some(<T as PersistedType>::handle_read_error(self.channel, e))
            });
            let result = result.unwrap_or_else(|| <T as PersistedType>::init(self.channel));
            let result = Arc::new(result);
            self.inner.store(Some(result.clone()));
            result
        };
        let optional_value = f(&value);
        if let Some(new_value) = optional_value {
            let new_value = Arc::new(new_value.into());
            let result = store_on_disk(self.channel, new_value.clone()).await;
            let old_value = self.inner.swap(Some(new_value.clone()));
            drop(permit);
            if let Err(e) = result {
                log::error!(
                    "Error saving {} for channel {} to disk: {:?}",
                    <T as PersistedType>::FILENAME,
                    self.channel,
                    e
                );
                <T as PersistedType>::handle_write_error(self.channel, e)
            }
            return (
                old_value.expect("Expected value, since it was initialized and never set to None"),
                Some(new_value),
            );
        }
        (value, None)
    }

    pub async fn update<R, F>(&self, mut f: F) -> (Arc<T>, Arc<T>)
    where
        F: FnMut(&T) -> R,
        R: Into<T>,
    {
        let (old, new) = self.maybe_update(move |value| Some((f)(value))).await;
        (old, new.unwrap())
    }
}

fn prepare_path<T: PersistedType>(channel: &str) -> anyhow::Result<PathBuf> {
    let mut path = std::env::current_dir()?;
    path.push("data");
    path.push(channel);
    path.push(T::FILENAME);
    path.set_extension("ron");
    Ok(path)
}

async fn prepare_paths<T: PersistedType>(channel: &str) -> anyhow::Result<(PathBuf, PathBuf)> {
    let mut path = std::env::current_dir()?;
    path.push("data");
    path.push(channel);
    tokio::fs::create_dir_all(&path).await?;
    path.push(T::FILENAME);
    let mut temp_path = path.clone();
    temp_path.set_extension("ron.temp");
    path.set_extension("ron");
    Ok(dbg!((temp_path, path)))
}

async fn store_on_disk<T: PersistedType>(channel: &str, store_value: Arc<T>) -> anyhow::Result<()> {
    let (temp_path, path) = prepare_paths::<T>(channel).await?;
    tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
        let file = OpenOptions::new()
            .read(false)
            .write(true)
            .append(false)
            // .create_new(true) // => could use create_new but then what happens if the file existed?
            .truncate(true)
            .create(true)
            .open(&temp_path)?;
        ron::ser::to_writer_pretty(
            &file,
            &store_value.deref(),
            <PrettyConfig as Default>::default(),
        )?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&temp_path, &path)?;
        Ok(())
    })
    .await??;
    Ok(())
}

async fn read_from_disk<T: PersistedType>(channel: &str) -> anyhow::Result<Option<T>> {
    let path = prepare_path::<T>(channel)?;
    let value = tokio::task::spawn_blocking(move || -> anyhow::Result<Option<T>> {
        let file = OpenOptions::new()
            .read(true)
            .write(false)
            .append(false)
            // .create_new(true) // => could use create_new but then what happens if the file existed?
            .truncate(false)
            .create(false)
            .open(path);
        let file = match file {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(None);
            }
            Err(e) => {
                return Err(e.into());
            }
        };
        let read_value = ron::de::from_reader(&file)?;
        drop(file);
        Ok(Some(read_value))
    })
    .await??;
    Ok(value)
}
