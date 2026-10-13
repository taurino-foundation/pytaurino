use {
    crate::{ArcMut, arc_mut, paths::SafePathBuf},
    anyhow::Result,
    parking_lot::RwLock,
    std::{
        collections::HashMap,
        fs,
        path::{Component, Path, PathBuf},
        sync::{Arc, Mutex, MutexGuard},
    },
    tao::event::DeviceId,
};

// ============================================================
// UUID String Serialization
// ============================================================
/// Python's Pydantic model_dump(mode="json") encodes
/// UUID values as strings.
///
/// Explicitly use the same representation in MessagePack.
pub mod uuid_as_string {
    use {
        serde::{Deserialize, Deserializer, Serializer},
        uuid::Uuid,
    };
    pub fn serialize<S>(value: &Uuid, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&value.to_string())
    }
    pub fn deserialize<'de, D>(deserializer: D) -> Result<Uuid, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Uuid::parse_str(&value).map_err(serde::de::Error::custom)
    }
}

pub fn default_true() -> bool { true }

// Converts PoisonError into a custom error without taking over its guard.
// Locks window state and converts mutex poisoning into an engine error.
pub fn lock_state<'a, T>(mutex: &'a Mutex<T>, name: &str) -> anyhow::Result<MutexGuard<'a, T>> {
    mutex.lock().map_err(|_| anyhow::anyhow!("Window {name} mutex is poisoned"))
}

#[macro_export]
macro_rules! log_if_err {
    ($result:expr) => {{
        if let Err(error) = $result {
            log::info!("{error}");
        }
    }};
}

pub fn prepare_data_directory(window_label: &str, data_directory: Option<PathBuf>) -> Option<PathBuf> {
    let data_directory = data_directory?;

    // Validate window label.
    let label_path = Path::new(window_label);
    let mut components = label_path.components();

    if !matches!(components.next(), Some(Component::Normal(_))) || components.next().is_some() {
        eprintln!("Invalid window label `{window_label}`: expected a single directory name");
        return None;
    }

    // Validate configured directory.
    let safe_path = match SafePathBuf::new(data_directory.clone()) {
        Ok(path) => path,
        Err(error) => {
            eprintln!("Unsafe data_directory `{}`: {error:#}", data_directory.display());
            return None;
        }
    };

    let path: &Path = safe_path.as_ref();

    // Reject absolute paths and unsafe components.
    if !path.is_relative()
        || path.components().any(|component| !matches!(component, Component::Normal(_) | Component::CurDir))
    {
        eprintln!("data_directory `{}` must be a safe relative path", path.display());
        return None;
    }

    // Resolve the platform-specific local data directory.
    let local_dir = dirs::data_local_dir().or_else(|| {
        eprintln!("Failed to resolve local data directory");
        None
    })?;

    let resolved_path = local_dir.join(window_label).join(path);

    // Create directory and missing parents if necessary.
    if let Err(error) = fs::create_dir_all(&resolved_path) {
        eprintln!("Failed to create data directory `{}`: {error}", resolved_path.display());
        return None;
    }

    Some(resolved_path)
}

struct DeviceRegistryInner {
    next_id: u32,
    by_device: HashMap<DeviceId, u32>,
    by_id: HashMap<u32, DeviceId>,
}

#[derive(Clone)]
pub struct DeviceRegistry {
    inner: Arc<RwLock<DeviceRegistryInner>>,
}

impl DeviceRegistry {
    pub fn new() -> Result<ArcMut<Self>> {
        Ok(arc_mut(Self {
            inner: Arc::new(RwLock::new(DeviceRegistryInner {
                next_id: 1,
                by_device: HashMap::new(),
                by_id: HashMap::new(),
            })),
        }))
    }

    fn next_available_id(inner: &mut DeviceRegistryInner) -> u32 {
        loop {
            let id = inner.next_id;

            inner.next_id = inner.next_id.checked_add(1).unwrap_or(1);

            if !inner.by_id.contains_key(&id) {
                return id;
            }
        }
    }

    /// Returns the existing runtime id for this device.
    /// If the device is not registered yet, creates a new id first.
    pub fn get_or_create_id(&self, device_id: DeviceId) -> u32 {
        let mut inner = self.inner.write();

        if let Some(id) = inner.by_device.get(&device_id) {
            return *id;
        }

        let id = Self::next_available_id(&mut inner);

        inner.by_device.insert(device_id, id);
        inner.by_id.insert(id, device_id);

        id
    }

    pub fn get_id(&self, device_id: &DeviceId) -> Option<u32> { self.inner.read().by_device.get(device_id).copied() }

    pub fn get_device(&self, id: u32) -> Option<DeviceId> { self.inner.read().by_id.get(&id).copied() }

    pub fn remove_by_device(&self, device_id: &DeviceId) -> Option<u32> {
        let mut inner = self.inner.write();

        let id = inner.by_device.remove(device_id)?;
        inner.by_id.remove(&id);

        Some(id)
    }

    pub fn remove_by_id(&self, id: u32) -> Option<DeviceId> {
        let mut inner = self.inner.write();

        let device_id = inner.by_id.remove(&id)?;
        inner.by_device.remove(&device_id);

        Some(device_id)
    }

    pub fn clear(&self) {
        let mut inner = self.inner.write();

        inner.by_device.clear();
        inner.by_id.clear();
        inner.next_id = 1;
    }
}
