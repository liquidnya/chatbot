use super::persisted_state::{Persisted, PersistedType};
use core::borrow::Borrow;
use core::fmt;
use core::fmt::Display;
use core::hash::Hash;
use derive_more::{Deref, From};
use state::TypeMap;
use std::collections::hash_map::Entry;
use std::rc::Rc;
use std::sync::Arc;
use std::{collections::HashMap, unreachable};
use tokio::sync::{RwLock, RwLockReadGuard};

pub(crate) struct CachedChannelContainer<'a> {
    #[allow(clippy::redundant_allocation)] // TODO: maybe there is a way to make all of this better
    cache: HashMap<String, Rc<Arc<TypeMap![Send + Sync]>>>, // the Rc is just used, such that the Arc (within the Rc) can be copied without having to synchronise the atomic reference in the arc
    container: &'a ChannelContainer,
}

impl CachedChannelContainer<'_> {
    pub async fn get<T>(&'_ mut self, channel: &T) -> Rc<Arc<TypeMap![Send + Sync]>>
    where
        String: Borrow<T>,
        T: ?Sized + Eq + Hash + ToOwned<Owned = String>,
    {
        match self.cache.get(channel) {
            Some(channel) => channel.clone(),
            None => match self.cache.entry(channel.to_owned()) {
                Entry::Occupied(_) => unreachable!(),
                Entry::Vacant(vacant) => vacant
                    .insert(Rc::new(self.container.get_arc(channel).await))
                    .clone(),
            },
        }
    }
}

pub struct ContainerBuilder {
    inner: TypeMap![Send + Sync],
}

impl ContainerBuilder {
    fn new() -> Self {
        ContainerBuilder {
            inner: <TypeMap![Send + Sync]>::new(),
        }
    }

    fn into_inner(self) -> TypeMap![Send + Sync] {
        self.inner
    }

    pub fn set<T: Send + Sync + 'static>(&self, value: T) {
        self.inner.set(value);
    }

    pub fn register_persisted_type<T: PersistedType>(&self) {
        self.inner.set(Persisted::<T>::new());
    }

    pub fn register_persisted_value<T: PersistedType>(&self, value: T) {
        self.inner.set(Persisted::<T>::from(value));
    }
}

pub type ChannelContainerTemplate = Box<dyn Fn(&str, &ContainerBuilder) + Send + Sync>;

pub struct ChannelContainer {
    container: RwLock<HashMap<String, Arc<TypeMap![Send + Sync]>>>,
    template: ChannelContainerTemplate,
}

#[derive(From)]
pub struct ChannelContainerGuard<'a>(RwLockReadGuard<'a, TypeMap![Send + Sync]>);

impl core::ops::Deref for ChannelContainerGuard<'_> {
    type Target = TypeMap![Send + Sync];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl ChannelContainer {
    pub fn new(f: ChannelContainerTemplate) -> Self {
        Self {
            container: RwLock::new(HashMap::new()),
            template: f,
        }
    }

    pub(crate) fn create_local_cache(&self) -> CachedChannelContainer {
        CachedChannelContainer {
            cache: Default::default(),
            container: self,
        }
    }

    pub async fn get_arc<T>(&self, channel: &T) -> Arc<TypeMap![Send + Sync]>
    where
        String: Borrow<T>,
        T: ?Sized + Eq + Hash + ToOwned<Owned = String>,
    {
        fn get_channel_container<K>(
            map: RwLockReadGuard<'_, HashMap<String, Arc<TypeMap![Send + Sync]>>>,
            channel: &K,
        ) -> Option<Arc<TypeMap![Send + Sync]>>
        where
            String: Borrow<K>,
            K: ?Sized + Eq + Hash,
        {
            tokio::sync::RwLockReadGuard::<'_, HashMap<String, Arc<TypeMap![Send + Sync]>>>::try_map(
                map,
                |map| map.get(channel),
            )
            .ok()
            .as_deref()
            .cloned()
        }
        {
            // unlock reading and getting the channel state if available
            let map = self.container.read().await;
            if let Some(container) = get_channel_container(map, channel) {
                return container;
            }
            // unlocked
        }
        // insert new channel container
        let mut map = self.container.write().await;
        let key = channel.to_owned();
        let value = ContainerBuilder::new();
        (self.template)(&key, &value);
        let mut value = value.into_inner();
        value.freeze();
        let container = Arc::new(value);
        map.insert(key, container.clone());
        container
    }

    pub async fn get<T>(&self, channel: &T) -> ChannelContainerGuard<'_>
    where
        String: Borrow<T>,
        T: ?Sized + Eq + Hash + ToOwned<Owned = String>,
    {
        fn get_channel_guard<'a, K>(
            map: RwLockReadGuard<'a, HashMap<String, Arc<TypeMap![Send + Sync]>>>,
            channel: &K,
        ) -> Option<ChannelContainerGuard<'a>>
        where
            String: Borrow<K>,
            K: ?Sized + Eq + Hash,
        {
            tokio::sync::RwLockReadGuard::<'_, HashMap<String, Arc<TypeMap![Send + Sync]>>>::try_map(
                map,
                |map| map.get(channel),
            )
            .ok()
            .map(|guard| {
                tokio::sync::RwLockReadGuard::<'_, Arc<TypeMap![Send + Sync]>>::map(guard, |container| {
                    container as &TypeMap![Send + Sync]
                })
            })
            .map(ChannelContainerGuard::from)
        }
        {
            // unlock reading and getting the channel state if available
            let map = self.container.read().await;
            if let Some(container) = get_channel_guard(map, channel) {
                return container;
            }
            // unlocked
        }
        // insert new channel container
        let mut map = self.container.write().await;
        let key = channel.to_owned();
        let value = ContainerBuilder::new();
        (self.template)(&key, &value);
        let mut value = value.into_inner();
        value.freeze();
        map.insert(key, Arc::new(value));
        let map = map.downgrade(); // TODO: create issue for downgrade with included mapping https://github.com/tokio-rs/tokio/issues
        get_channel_guard(map, channel)
            .expect("Expected value be in HashMap after inserting while holding the lock.")
    }
}

#[derive(Debug, Clone, Deref, From)]
pub struct ChannelState<'a, T: Send + Sync + 'static>(&'a T);

#[derive(Debug)]
pub enum ChannelStateError {
    NoContext,
    NoChannelContainer,
    NoValue(&'static str),
}

impl Display for ChannelStateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        match self {
            ChannelStateError::NoContext => write!(f, "CommandRequest is missing context"),
            ChannelStateError::NoChannelContainer => write!(f, "No ChannelContainer was setup"),
            ChannelStateError::NoValue(type_name) => write!(
                f,
                "No value set for type {} in {}",
                type_name,
                std::any::type_name::<ChannelContainer>()
            ),
        }
    }
}

impl std::error::Error for ChannelStateError {}

/*
impl From<StateError> for ChannelStateError {
    fn from(error: StateError) -> Self {
        match error {
            StateError::NoContext => ChannelStateError::NoContext,
            StateError::NoValue(_) => ChannelStateError::NoChannelContainer,
        }
    }
}
*/
