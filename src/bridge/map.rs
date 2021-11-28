use std::alloc::{alloc, Layout};
use std::collections::{BTreeMap, HashMap};

use crate::bridge::{Data, Method, Scheme};
use crate::sys;

/// Data wrapper used to convert to/from envoy_map.
/// Should not be used directly when possible.
#[derive(Debug)]
pub struct Map(Vec<(Data, Data)>);

pub type Headers = Map;
pub type StatsTags = Map;

impl Map {
    pub fn new_request_headers(
        method: Method,
        scheme: Scheme,
        authority: impl AsRef<str>,
        path: impl AsRef<str>,
    ) -> Self {
        Map::from(vec![
            (":method", method.into_envoy_method()),
            (":scheme", scheme.into_envoy_scheme()),
            (":authority", authority.as_ref()),
            (":path", path.as_ref()),
        ])
    }

    pub unsafe fn from_envoy_map(envoy_map: sys::envoy_map) -> Self {
        let length = envoy_map.length.try_into().unwrap();
        let mut entries = Vec::with_capacity(length);
        for i in 0..length {
            let entry = &*envoy_map.entries.add(i);
            entries.push((
                Data::from_envoy_data_no_release(&entry.key),
                Data::from_envoy_data_no_release(&entry.value),
            ));
        }
        sys::release_envoy_map(envoy_map);
        Self(entries)
    }

    pub fn into_envoy_map(self) -> sys::envoy_map {
        let length = self.0.len();
        let layout = Layout::array::<sys::envoy_map_entry>(length)
            .expect("failed to construct layout for envoy_map_entry block");
        let memory;
        unsafe {
            memory = alloc(layout) as *mut sys::envoy_map_entry;
        }

        for (i, entry) in self.0.into_iter().enumerate() {
            let envoy_entry = sys::envoy_map_entry {
                key: entry.0.into_envoy_data(),
                value: entry.1.into_envoy_data(),
            };
            unsafe {
                (*memory.add(i)) = envoy_entry;
            }
        }

        sys::envoy_map {
            length: length.try_into().unwrap(),
            entries: memory,
        }
    }
}

impl<T, U> From<T> for Map
where
    T: IntoIterator<Item = (U, U)>,
    U: Into<Data>,
{
    fn from(source: T) -> Self {
        let mut entries = Vec::new();
        for (key, value) in source.into_iter() {
            entries.push((key.into(), value.into()));
        }
        Map(entries)
    }
}

impl<T, U> TryFrom<Map> for HashMap<T, T>
where
    T: TryFrom<Data, Error = U> + Eq + std::hash::Hash,
{
    type Error = U;

    fn try_from(map: Map) -> Result<Self, Self::Error> {
        let mut entries = Vec::with_capacity(map.0.len());
        for (key, value) in map.0.into_iter() {
            entries.push((T::try_from(key)?, T::try_from(value)?));
        }
        Ok(Self::from_iter(entries.into_iter()))
    }
}

impl<T, U> TryFrom<Map> for BTreeMap<T, T>
where
    T: TryFrom<Data, Error = U> + Ord,
{
    type Error = U;

    fn try_from(map: Map) -> Result<Self, Self::Error> {
        let mut entries = Vec::with_capacity(map.0.len());
        for (key, value) in map.0.into_iter() {
            entries.push((T::try_from(key)?, T::try_from(value)?));
        }
        Ok(Self::from_iter(entries.into_iter()))
    }
}
