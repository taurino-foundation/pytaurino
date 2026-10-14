// a modified version of:
// https://github.com/denoland/deno/blob/0ae83847f498a2886ae32172e50fd5bdbab2f524/core/resources.rs#L220

use {
    anyhow::{Context, Result, anyhow},
    std::{
        any::{Any, TypeId, type_name},
        borrow::Cow,
        collections::BTreeMap,
        sync::Arc,
    },
};

/// Resources are Rust objects that are stored in [`ResourceTable`] and managed by Tauri.
///
/// They are identified in JS by a numeric ID (the resource ID, or rid).
/// Resources can be created in commands. Resources can also be retrieved in commands by
/// their rid. Resources are thread-safe.
///
/// Resources are reference counted in Rust. This means that they can be
/// cloned and passed around. When the last reference is dropped, the resource
/// is automatically closed. As long as the resource exists in the resource
/// table, the reference count is at least 1.
pub trait Resource: Any + 'static + Send + Sync {
    /// Returns a string representation of the resource.
    ///
    /// The default implementation returns the Rust type name, but specific
    /// resource types may override this trait method.
    fn name(&self) -> Cow<'_, str> { type_name::<Self>().into() }

    /// Resources may implement the `close()` trait method if they need to do
    /// resource-specific clean-ups, such as cancelling pending futures, after a
    /// resource has been removed from the resource table.
    fn close(self: Arc<Self>) {}
}

impl dyn Resource {
    #[inline(always)]
    fn is<T: Resource>(&self) -> bool { self.type_id() == TypeId::of::<T>() }

    #[inline(always)]
    pub(crate) fn downcast_arc<T: Resource>(self: &Arc<Self>) -> Option<&Arc<T>> {
        if self.is::<T>() {
            // A resource is stored as `Arc<T>` in a BTreeMap
            // and is safe to cast to `Arc<T>` because of the runtime
            // check done in `self.is::<T>()`.
            let ptr = self as *const Arc<_> as *const Arc<T>;

            Some(unsafe { &*ptr })
        } else {
            None
        }
    }
}

/// A [`ResourceId`] is an integer value referencing a resource.
///
/// It could be considered to be the Tauri equivalent of a file descriptor
/// in POSIX-like operating systems.
pub type ResourceId = u32;

/// Map-like data structure storing Tauri's resources
/// (equivalent to file descriptors).
///
/// Provides basic methods for element access. A resource can be of any type.
/// Different types of resources can be stored in the same map and provided
/// with a name for description.
///
/// Each resource is identified through a _resource ID (rid)_, which acts as
/// the key in the map.
#[derive(Default)]
pub struct ResourceTable {
    index: BTreeMap<ResourceId, Arc<dyn Resource>>,
}

impl ResourceTable {
    /// Generates a random resource ID.
    fn new_random_rid() -> Result<ResourceId> {
        let mut bytes = [0_u8; 4];

        getrandom::fill(&mut bytes).context("failed to generate a random resource ID")?;

        Ok(u32::from_ne_bytes(bytes))
    }

    /// Inserts a resource into the resource table, which takes ownership of it.
    ///
    /// The resource type is erased at runtime and must be statically known when
    /// retrieving it through [`ResourceTable::get`].
    ///
    /// Returns a unique resource ID, which acts as a key for this resource.
    pub fn add<T: Resource>(&mut self, resource: T) -> Result<ResourceId> { self.add_arc(Arc::new(resource)) }

    /// Inserts an `Arc`-wrapped resource into the resource table.
    ///
    /// The resource type is erased at runtime and must be statically known when
    /// retrieving it through [`ResourceTable::get`].
    ///
    /// Returns a unique resource ID, which acts as a key for this resource.
    pub fn add_arc<T: Resource>(&mut self, resource: Arc<T>) -> Result<ResourceId> {
        let resource = resource as Arc<dyn Resource>;

        self.add_arc_dyn(resource)
    }

    /// Inserts an `Arc`-wrapped dynamically typed resource into the resource table.
    ///
    /// Returns a unique resource ID, which acts as a key for this resource.
    pub fn add_arc_dyn(&mut self, resource: Arc<dyn Resource>) -> Result<ResourceId> {
        let mut rid = Self::new_random_rid()?;

        while self.index.contains_key(&rid) {
            rid = Self::new_random_rid()?;
        }

        let removed_resource = self.index.insert(rid, resource);

        debug_assert!(removed_resource.is_none());

        Ok(rid)
    }

    /// Returns `true` if any resource with the given `rid` exists.
    pub fn has(&self, rid: ResourceId) -> bool { self.index.contains_key(&rid) }

    /// Returns a reference-counted pointer to the resource of type `T`
    /// with the given `rid`.
    ///
    /// Returns an error if the resource does not exist or if the resource
    /// exists but its type does not match `T`.
    pub fn get<T: Resource>(&self, rid: ResourceId) -> Result<Arc<T>> {
        let resource = self.index.get(&rid).ok_or_else(|| anyhow!("resource with id {rid} was not found"))?;

        resource.downcast_arc::<T>().cloned().ok_or_else(|| {
            anyhow!("resource with id {rid} has type `{}`, expected `{}`", resource.name(), type_name::<T>(),)
        })
    }

    /// Returns a reference-counted pointer to the resource with the given `rid`.
    ///
    /// Returns an error if the resource does not exist.
    pub fn get_any(&self, rid: ResourceId) -> Result<Arc<dyn Resource>> {
        self.index.get(&rid).cloned().ok_or_else(|| anyhow!("resource with id {rid} was not found"))
    }

    /// Replaces a resource with a new resource.
    ///
    /// # Panics
    ///
    /// Panics if the resource does not exist.
    pub fn replace<T: Resource>(&mut self, rid: ResourceId, resource: T) {
        let result = self.index.insert(rid, Arc::new(resource) as Arc<dyn Resource>);

        assert!(result.is_some(), "resource with id {rid} does not exist",);
    }

    /// Removes a resource of type `T` from the resource table and returns it.
    ///
    /// If a resource with the given `rid` exists but its type does not match
    /// `T`, it is not removed from the resource table.
    ///
    /// The resource's [`Resource::close`] method is **not** called.
    ///
    /// Also note that there might be a case where the returned `Arc<T>` is
    /// referenced by other variables. That is, we cannot assume that
    /// `Arc::strong_count(&returned_arc)` is always equal to `1` on success.
    ///
    /// In particular, be careful when extracting the inner value of type `T`
    /// from `Arc<T>`.
    pub fn take<T: Resource>(&mut self, rid: ResourceId) -> Result<Arc<T>> {
        let resource = self.get::<T>(rid)?;

        self.index.remove(&rid);

        Ok(resource)
    }

    /// Removes a resource from the resource table and returns it.
    ///
    /// The resource's [`Resource::close`] method is **not** called.
    ///
    /// Also note that there might be a case where the returned resource is
    /// referenced by other variables. We therefore cannot assume that the
    /// returned `Arc` has a strong count of `1`.
    pub fn take_any(&mut self, rid: ResourceId) -> Result<Arc<dyn Resource>> {
        self.index.remove(&rid).ok_or_else(|| anyhow!("resource with id {rid} was not found"))
    }

    /// Returns an iterator that yields an `(id, name)` pair for every resource
    /// currently in the resource table.
    ///
    /// This can be used for debugging purposes.
    ///
    /// The order in which items appear is not guaranteed.
    pub fn names(&self) -> impl Iterator<Item = (ResourceId, Cow<'_, str>)> {
        self.index.iter().map(|(&id, resource)| (id, resource.name()))
    }

    /// Removes the resource with the given `rid` from the resource table.
    ///
    /// If the only reference to this resource existed in the resource table,
    /// this causes the resource to be dropped.
    ///
    /// Since resources are reference counted, pending operations are not
    /// automatically cancelled. A resource may implement [`Resource::close`]
    /// to perform clean-up such as cancelling pending operations.
    pub fn close(&mut self, rid: ResourceId) -> Result<()> {
        let resource = self.index.remove(&rid).ok_or_else(|| anyhow!("resource with id {rid} was not found"))?;

        resource.close();

        Ok(())
    }

    /// Removes and frees all resources stored in the resource table.
    ///
    /// The resources' [`Resource::close`] methods are **not** called.
    pub fn clear(&mut self) { self.index.clear(); }
}
