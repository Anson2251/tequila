//! Registry cache implementation
//! 
//! This module provides caching functionality for Wine registries to improve performance
//! when working with large registry files.

use crate::prefix::error::{Result, PrefixError};
use crate::prefix::regeditor::traits::RegistryCache;
use crate::prefix::regeditor::registry::WineRegistry;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Default cache TTL (time to live) in seconds
const DEFAULT_CACHE_TTL: Duration = Duration::from_secs(300); // 5 minutes

/// Cache entry for a registry
#[derive(Debug)]
struct CacheEntry {
    /// The cached registry
    registry: WineRegistry,
    /// When the cache entry was created
    created_at: Instant,
    /// Time to live for this entry
    ttl: Duration,
}

impl CacheEntry {
    /// Create a new cache entry
    fn new(registry: WineRegistry, ttl: Duration) -> Self {
        Self {
            registry,
            created_at: Instant::now(),
            ttl,
        }
    }

    /// Check if the cache entry has expired
    fn is_expired(&self) -> bool {
        self.created_at.elapsed() > self.ttl
    }
}

/// In-memory registry cache implementation
#[derive(Debug)]
pub struct InMemoryRegistryCache {
    /// The cache storage
    cache: Arc<RwLock<HashMap<PathBuf, CacheEntry>>>,
    /// Default TTL for new entries
    default_ttl: Duration,
}

impl InMemoryRegistryCache {
    /// Create a new in-memory cache
    /// 
    /// # Arguments
    /// * `default_ttl` - Default time to live for cache entries
    /// 
    /// # Returns
    /// A new cache instance
    pub fn new(default_ttl: Duration) -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            default_ttl,
        }
    }

    /// Create a new cache with default TTL
    pub fn with_default_ttl() -> Self {
        Self::new(DEFAULT_CACHE_TTL)
    }

    /// Clean up expired entries from the cache
    pub async fn cleanup_expired(&self) -> Result<()> {
        let cache = self.cache.clone();
        
        tokio::spawn(async move {
            let mut cache_guard = cache.write().await;
            let now = Instant::now();
            
            // Remove expired entries
            cache_guard.retain(|_, entry| {
                now.duration_since(entry.created_at) < entry.ttl
            });
        });
        
        Ok(())
    }

    /// Get cache statistics
    pub async fn stats(&self) -> CacheStats {
        let cache = self.cache.read().await;
        let total_entries = cache.len();
        let mut expired_entries = 0;
        
        let now = Instant::now();
        for entry in cache.values() {
            if now.duration_since(entry.created_at) > entry.ttl {
                expired_entries += 1;
            }
        }
        
        CacheStats {
            total_entries,
            expired_entries,
            valid_entries: total_entries - expired_entries,
        }
    }
}

#[async_trait::async_trait]
impl RegistryCache for InMemoryRegistryCache {
    /// Get cached registry for a prefix
    async fn get_cached_registry(&self, prefix_path: &PathBuf) -> Result<Option<WineRegistry>> {
        let cache = self.cache.read().await;
        
        if let Some(entry) = cache.get(prefix_path) {
            if !entry.is_expired() {
                return Ok(Some(entry.registry.clone()));
            }
        }
        
        Ok(None)
    }

    /// Cache registry for a prefix
    async fn cache_registry(&self, prefix_path: &PathBuf, registry: WineRegistry) -> Result<()> {
        let mut cache = self.cache.write().await;
        let entry = CacheEntry::new(registry, self.default_ttl);
        cache.insert(prefix_path.clone(), entry);
        Ok(())
    }

    /// Invalidate cache for a prefix
    async fn invalidate_cache(&self, prefix_path: &PathBuf) -> Result<()> {
        let mut cache = self.cache.write().await;
        cache.remove(prefix_path);
        Ok(())
    }

    /// Clear all cached registries
    async fn clear_all_cache(&self) -> Result<()> {
        let mut cache = self.cache.write().await;
        cache.clear();
        Ok(())
    }
}

/// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    /// Total number of entries in cache
    pub total_entries: usize,
    /// Number of expired entries
    pub expired_entries: usize,
    /// Number of valid entries
    pub valid_entries: usize,
}

impl Default for InMemoryRegistryCache {
    fn default() -> Self {
        Self::with_default_ttl()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prefix::regeditor::keys::*;
    use std::path::Path;

    #[tokio::test]
    async fn test_cache_basic_operations() {
        let cache = InMemoryRegistryCache::with_default_ttl();
        let path = PathBuf::from("/test/prefix");
        
        // Initially no cached registry
        assert!(cache.get_cached_registry(&path).await.unwrap().is_none());
        
        // Cache a registry
        let registry = WineRegistry::new();
        cache.cache_registry(&path, registry).await.unwrap();
        
        // Now we should have a cached registry
        assert!(cache.get_cached_registry(&path).await.unwrap().is_some());
        
        // Invalidate cache
        cache.invalidate_cache(&path).await.unwrap();
        
        // Should be gone now
        assert!(cache.get_cached_registry(&path).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_cache_stats() {
        let cache = InMemoryRegistryCache::with_default_ttl();
        let path1 = PathBuf::from("/test/prefix1");
        let path2 = PathBuf::from("/test/prefix2");
        
        // Add some entries
        cache.cache_registry(&path1, WineRegistry::new()).await.unwrap();
        cache.cache_registry(&path2, WineRegistry::new()).await.unwrap();
        
        let stats = cache.stats().await;
        assert_eq!(stats.total_entries, 2);
        assert_eq!(stats.valid_entries, 2);
        assert_eq!(stats.expired_entries, 0);
        
        // Clear all
        cache.clear_all_cache().await.unwrap();
        
        let stats = cache.stats().await;
        assert_eq!(stats.total_entries, 0);
    }
}