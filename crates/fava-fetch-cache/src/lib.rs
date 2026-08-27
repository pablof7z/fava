//! Bounded freshness fetch cache with negative-cache and stale-result semantics.
//!
//! A `FetchCache` remembers the outcome of a keyed fetch — success, negative
//! (not found), or error — for a bounded TTL. After the TTL, a previous result
//! is still available as [`FetchOutcome::Stale`] with the age as evidence
//! rather than being silently dropped or returned as fresh.
//!
//! Service functions in [`nip05`] and [`nip11`] use this cache to implement
//! bounded-freshness NIP-05 identifier resolution and NIP-11 relay info
//! fetching with explicit stale-result evidence.

use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub mod nip05;
pub mod nip11;

/// Freshness metadata returned alongside a cached result.
#[derive(Clone, Debug)]
pub struct FetchAge {
    /// When the fetch completed.
    pub fetched_at: Instant,
    /// How much time has elapsed since the fetch.
    pub age: Duration,
    /// Whether the result is within the caller's requested TTL.
    pub is_fresh: bool,
}

impl FetchAge {
    /// Compute freshness for a result fetched at `fetched_at` against `ttl`.
    #[must_use]
    pub fn compute(fetched_at: Instant, ttl: Duration) -> Self {
        let age = fetched_at.elapsed();
        Self {
            fetched_at,
            age,
            is_fresh: age <= ttl,
        }
    }
}

/// Result of a cache lookup for one key.
#[derive(Clone, Debug)]
pub enum FetchOutcome {
    /// Found a successful prior fetch result.
    Ok {
        /// The cached response body.
        body: String,
        /// Freshness of the cached result.
        age: FetchAge,
    },
    /// The prior fetch returned "not found" (HTTP 404 or equivalent).
    NotFound {
        /// Freshness of the negative-cache entry.
        age: FetchAge,
    },
    /// The prior fetch returned an error; the error message is bounded.
    Error {
        /// Bounded error description.
        reason: String,
        /// Freshness of the error-cache entry.
        age: FetchAge,
    },
    /// No prior fetch is cached; caller must perform one.
    Absent,
}

impl FetchOutcome {
    /// Whether this is a fresh positive result within the requested TTL.
    #[must_use]
    pub fn is_fresh_ok(&self) -> bool {
        matches!(self, Self::Ok { age, .. } if age.is_fresh)
    }

    /// Whether this is a fresh negative result within the requested TTL.
    #[must_use]
    pub fn is_fresh_not_found(&self) -> bool {
        matches!(self, Self::NotFound { age } if age.is_fresh)
    }

    /// Whether the cached entry is stale (past TTL but still present).
    #[must_use]
    pub fn is_stale(&self) -> bool {
        match self {
            Self::Ok { age, .. } | Self::NotFound { age } | Self::Error { age, .. } => {
                !age.is_fresh
            }
            Self::Absent => false,
        }
    }

    /// Body of a cached successful fetch, if present.
    #[must_use]
    pub fn body(&self) -> Option<&str> {
        match self {
            Self::Ok { body, .. } => Some(body),
            _ => None,
        }
    }

    /// Freshness metadata, if any cached entry is present.
    #[must_use]
    pub fn age(&self) -> Option<&FetchAge> {
        match self {
            Self::Ok { age, .. } | Self::NotFound { age } | Self::Error { age, .. } => Some(age),
            Self::Absent => None,
        }
    }
}

/// Provider contract for a bounded, TTL-aware fetch result cache.
pub trait FetchCache: Send + Sync {
    /// Query the cache for `key` using `ttl` to compute freshness.
    fn get(&self, key: &str, ttl: Duration) -> FetchOutcome;

    /// Store a successful response for `key` at the given `fetched_at` time.
    fn set_ok(&self, key: &str, body: String, fetched_at: Instant);

    /// Store a not-found result for `key` at the given `fetched_at` time.
    fn set_not_found(&self, key: &str, fetched_at: Instant);

    /// Store an error result for `key` at the given `fetched_at` time.
    fn set_error(&self, key: &str, reason: String, fetched_at: Instant);

    /// Remove any cached entry for `key`.
    fn evict(&self, key: &str);
}

/// Raw HTTP response from a [`HttpFetcher`].
#[derive(Clone, Debug)]
pub struct HttpResponse {
    /// HTTP status code.
    pub status: u16,
    /// Response body (text).
    pub body: String,
}

/// Boxed async HTTP fetch function.
pub type FetchFuture<'a> = Pin<Box<dyn Future<Output = Result<HttpResponse, String>> + Send + 'a>>;

/// Provider contract for executing actual HTTP GET requests.
pub trait HttpFetcher: Send + Sync {
    /// Perform a GET request to `url`. Returns [`HttpResponse`] or a bounded
    /// error string. TLS is handled by the implementation.
    fn get<'a>(&'a self, url: &'a str) -> FetchFuture<'a>;
}

#[derive(Clone, Debug)]
enum StoredOutcome {
    Ok { body: String, fetched_at: Instant },
    NotFound { fetched_at: Instant },
    Error { reason: String, fetched_at: Instant },
}

impl StoredOutcome {
    fn to_fetch_outcome(&self, ttl: Duration) -> FetchOutcome {
        match self {
            Self::Ok { body, fetched_at } => FetchOutcome::Ok {
                body: body.clone(),
                age: FetchAge::compute(*fetched_at, ttl),
            },
            Self::NotFound { fetched_at } => FetchOutcome::NotFound {
                age: FetchAge::compute(*fetched_at, ttl),
            },
            Self::Error { reason, fetched_at } => FetchOutcome::Error {
                reason: reason.clone(),
                age: FetchAge::compute(*fetched_at, ttl),
            },
        }
    }
}

/// Thread-safe in-memory [`FetchCache`] with bounded per-key retention.
pub struct MemoryFetchCache {
    inner: Mutex<BTreeMap<String, StoredOutcome>>,
}

impl Default for MemoryFetchCache {
    fn default() -> Self {
        Self::new()
    }
}

impl MemoryFetchCache {
    /// Create an empty in-memory cache.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(BTreeMap::new()),
        }
    }
}

impl FetchCache for MemoryFetchCache {
    fn get(&self, key: &str, ttl: Duration) -> FetchOutcome {
        let inner = self.inner.lock().expect("fetch cache lock poisoned");
        inner
            .get(key)
            .map_or(FetchOutcome::Absent, |stored| stored.to_fetch_outcome(ttl))
    }

    fn set_ok(&self, key: &str, body: String, fetched_at: Instant) {
        let mut inner = self.inner.lock().expect("fetch cache lock poisoned");
        inner.insert(key.to_owned(), StoredOutcome::Ok { body, fetched_at });
    }

    fn set_not_found(&self, key: &str, fetched_at: Instant) {
        let mut inner = self.inner.lock().expect("fetch cache lock poisoned");
        inner.insert(key.to_owned(), StoredOutcome::NotFound { fetched_at });
    }

    fn set_error(&self, key: &str, reason: String, fetched_at: Instant) {
        let mut inner = self.inner.lock().expect("fetch cache lock poisoned");
        inner.insert(key.to_owned(), StoredOutcome::Error { reason, fetched_at });
    }

    fn evict(&self, key: &str) {
        let mut inner = self.inner.lock().expect("fetch cache lock poisoned");
        inner.remove(key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn absent_when_nothing_cached() {
        let cache = MemoryFetchCache::new();
        assert!(matches!(
            cache.get("http://example.com/foo", Duration::from_secs(60)),
            FetchOutcome::Absent
        ));
    }

    #[test]
    fn fresh_ok_within_ttl() {
        let cache = MemoryFetchCache::new();
        cache.set_ok("key", "body".to_owned(), Instant::now());
        let outcome = cache.get("key", Duration::from_secs(60));
        assert!(outcome.is_fresh_ok(), "should be fresh ok");
        assert_eq!(outcome.body(), Some("body"));
    }

    #[test]
    fn stale_ok_after_ttl() {
        let cache = MemoryFetchCache::new();
        let past = Instant::now() - Duration::from_secs(100);
        cache.set_ok("key", "body".to_owned(), past);
        let outcome = cache.get("key", Duration::from_secs(60));
        assert!(outcome.is_stale(), "should be stale");
        assert!(!outcome.is_fresh_ok(), "should not be fresh");
        assert!(
            outcome
                .age()
                .is_some_and(|a| a.age >= Duration::from_secs(99)),
            "age must be reflected"
        );
    }

    #[test]
    fn negative_cache_fresh_within_ttl() {
        let cache = MemoryFetchCache::new();
        cache.set_not_found("key", Instant::now());
        let outcome = cache.get("key", Duration::from_secs(60));
        assert!(outcome.is_fresh_not_found());
    }

    #[test]
    fn evict_removes_entry() {
        let cache = MemoryFetchCache::new();
        cache.set_ok("key", "body".to_owned(), Instant::now());
        cache.evict("key");
        assert!(matches!(
            cache.get("key", Duration::from_secs(60)),
            FetchOutcome::Absent
        ));
    }
}
