//! Collection types for ArceOS standard library.
//!
//! This module re-exports all types from `alloc::collections` and adds `HashMap`
//! using `hashbrown` for no_std compatibility.

#[cfg(feature = "alloc")]
pub use alloc::collections::*;

/// A hash map implemented with quadratic probing and SIMD lookup.
///
/// This is a wrapper around `hashbrown::HashMap` that provides a `new()` method
/// compatible with the standard library's `HashMap` interface.
#[cfg(feature = "alloc")]
pub struct HashMap<K, V> {
    inner: hashbrown::HashMap<K, V, hashbrown::DefaultHashBuilder>,
}

#[cfg(feature = "alloc")]
impl<K, V> HashMap<K, V> {
    /// Creates an empty `HashMap`.
    ///
    /// The hash map is initially created with a capacity of 0, so it will not allocate until it
    /// is first inserted into.
    ///
    /// # Examples
    ///
    /// ```
    /// use axstd::collections::HashMap;
    /// let mut map: HashMap<&str, i32> = HashMap::new();
    /// ```
    #[inline]
    pub fn new() -> Self {
        Self {
            inner: hashbrown::HashMap::with_hasher(hashbrown::DefaultHashBuilder::default()),
        }
    }

    /// Creates an empty `HashMap` with the specified capacity.
    ///
    /// The hash map will be able to hold at least `capacity` elements without
    /// reallocating. If `capacity` is 0, the hash map will not allocate.
    ///
    /// # Examples
    ///
    /// ```
    /// use axstd::collections::HashMap;
    /// let mut map: HashMap<&str, i32> = HashMap::with_capacity(10);
    /// ```
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            inner: hashbrown::HashMap::with_capacity_and_hasher(
                capacity,
                hashbrown::DefaultHashBuilder::default(),
            ),
        }
    }

    /// Inserts a key-value pair into the map.
    ///
    /// If the map did not have this key present, [`None`] is returned.
    ///
    /// If the map did have this key present, the value is updated, and the old
    /// value is returned. The key is not updated, though; this matters for
    /// types that can be `==` without being identical.
    #[inline]
    pub fn insert(&mut self, k: K, v: V) -> Option<V>
    where
        K: core::hash::Hash + core::cmp::Eq,
    {
        self.inner.insert(k, v)
    }

    /// Returns a reference to the value corresponding to the key.
    #[inline]
    pub fn get<Q: ?Sized>(&self, k: &Q) -> Option<&V>
    where
        K: core::borrow::Borrow<Q> + core::hash::Hash + core::cmp::Eq,
        Q: core::hash::Hash + core::cmp::Eq,
    {
        self.inner.get(k)
    }

    /// Returns a mutable reference to the value corresponding to the key.
    #[inline]
    pub fn get_mut<Q: ?Sized>(&mut self, k: &Q) -> Option<&mut V>
    where
        K: core::borrow::Borrow<Q> + core::hash::Hash + core::cmp::Eq,
        Q: core::hash::Hash + core::cmp::Eq,
    {
        self.inner.get_mut(k)
    }

    /// Removes a key from the map, returning the value at the key if the key
    /// was previously in the map.
    #[inline]
    pub fn remove<Q: ?Sized>(&mut self, k: &Q) -> Option<V>
    where
        K: core::borrow::Borrow<Q> + core::hash::Hash + core::cmp::Eq,
        Q: core::hash::Hash + core::cmp::Eq,
    {
        self.inner.remove(k)
    }

    /// An iterator visiting all key-value pairs in arbitrary order.
    /// The iterator element type is `(&'a K, &'a V)`.
    #[inline]
    pub fn iter(&self) -> hashbrown::hash_map::Iter<'_, K, V> {
        self.inner.iter()
    }

    /// An iterator visiting all key-value pairs in arbitrary order,
    /// with mutable references to the values.
    /// The iterator element type is `(&'a K, &'a mut V)`.
    #[inline]
    pub fn iter_mut(&mut self) -> hashbrown::hash_map::IterMut<'_, K, V> {
        self.inner.iter_mut()
    }

    /// Returns the number of elements in the map.
    #[inline]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Returns `true` if the map contains no elements.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

#[cfg(feature = "alloc")]
impl<K, V> Default for HashMap<K, V> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "alloc")]
impl<K, V> core::ops::Index<&K> for HashMap<K, V>
where
    K: core::hash::Hash + core::cmp::Eq,
{
    type Output = V;

    #[inline]
    fn index(&self, key: &K) -> &Self::Output {
        &self.inner[key]
    }
}
