use crate::WineRegistry;
use crate::traits::RegistryCache;
use base::error::Result;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

const DEFAULT_CACHE_TTL: Duration = Duration::from_secs(300);

#[derive(Debug)]
struct CacheEntry {
    registry: WineRegistry,
    created_at: Instant,
    ttl: Duration,
}

impl CacheEntry {
    fn new(registry: WineRegistry, ttl: Duration) -> Self {
        Self {
            registry,
            created_at: Instant::now(),
            ttl,
        }
    }

    fn is_expired(&self) -> bool {
        self.created_at.elapsed() > self.ttl
    }
}

#[derive(Debug)]
pub struct InMemoryRegistryCache {
    cache: Arc<RwLock<HashMap<PathBuf, CacheEntry>>>,
    default_ttl: Duration,
}

impl InMemoryRegistryCache {
    pub fn new(default_ttl: Duration) -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            default_ttl,
        }
    }

    pub fn with_default_ttl() -> Self {
        Self::new(DEFAULT_CACHE_TTL)
    }

    pub async fn cleanup_expired(&self) -> Result<()> {
        let cache = self.cache.clone();
        tokio::spawn(async move {
            let mut cache_guard = cache.write().await;
            cache_guard
                .retain(|_, entry| Instant::now().duration_since(entry.created_at) < entry.ttl);
        });
        Ok(())
    }

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
    async fn get_cached_registry(&self, prefix_path: &PathBuf) -> Result<Option<WineRegistry>> {
        let cache = self.cache.read().await;
        if let Some(entry) = cache.get(prefix_path) {
            if !entry.is_expired() {
                return Ok(Some(entry.registry.clone()));
            }
        }
        Ok(None)
    }

    async fn cache_registry(&self, prefix_path: &PathBuf, registry: WineRegistry) -> Result<()> {
        let mut cache = self.cache.write().await;
        cache.insert(
            prefix_path.clone(),
            CacheEntry::new(registry, self.default_ttl),
        );
        Ok(())
    }

    async fn invalidate_cache(&self, prefix_path: &PathBuf) -> Result<()> {
        let mut cache = self.cache.write().await;
        cache.remove(prefix_path);
        Ok(())
    }

    async fn clear_all_cache(&self) -> Result<()> {
        let mut cache = self.cache.write().await;
        cache.clear();
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct CacheStats {
    pub total_entries: usize,
    pub expired_entries: usize,
    pub valid_entries: usize,
}

impl Default for InMemoryRegistryCache {
    fn default() -> Self {
        Self::with_default_ttl()
    }
}
