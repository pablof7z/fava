//! NIP-11 relay info document fetching with stale-result evidence semantics.
//!
//! Fetches a relay's info document (`GET /` with `Accept: application/nostr+json`)
//! and caches the raw JSON body. On re-access after TTL expiry the cached body
//! is returned as [`Nip11Result::Stale`] with explicit freshness evidence so
//! the caller can distinguish a stale relay info from a fresh one.

use std::time::{Duration, Instant};

use crate::{FetchCache, FetchOutcome, HttpFetcher};

/// The relay's NIP-11 document when one is available, and whether it is within
/// its TTL, past it, or the fetch failed.
#[derive(Clone, Debug)]
pub enum Nip11Result {
    /// Fresh relay info document (within TTL).
    Fresh {
        /// Raw JSON body of the NIP-11 document.
        info_json: String,
    },
    /// Cached relay info document, but the TTL has elapsed.
    /// The caller should re-fetch soon, but can use `info_json` now.
    Stale {
        /// Cached raw JSON body.
        info_json: String,
        /// Freshness evidence: how old the cached entry is.
        age: crate::FetchAge,
    },
    /// HTTP fetch failed. The error is bounded.
    Error(String),
}

impl Nip11Result {
    /// Whether this result is a stale cached document past TTL.
    #[must_use]
    pub fn is_stale(&self) -> bool {
        matches!(self, Self::Stale { .. })
    }

    /// The relay's NIP-11 document, whether within its TTL or past it.
    #[must_use]
    pub fn info_json(&self) -> Option<&str> {
        match self {
            Self::Fresh { info_json } | Self::Stale { info_json, .. } => Some(info_json),
            Self::Error(_) => None,
        }
    }
}

/// Fetch a relay's NIP-11 info document via the cache.
///
/// The relay WebSocket URL (ws:// or wss://) is converted to the
/// corresponding HTTP URL. The response is cached under the relay URL string.
/// A stale cache hit returns [`Nip11Result::Stale`] with explicit age evidence.
///
/// # Errors
///
/// Does not return `Err`; all outcomes including network failures are encoded
/// in [`Nip11Result`] so the caller has full evidence of what happened.
pub async fn fetch(
    relay_url: &str,
    cache: &dyn FetchCache,
    http: &dyn HttpFetcher,
    ttl: Duration,
) -> Nip11Result {
    let http_url = relay_url_to_http(relay_url);
    let cache_key = relay_url;

    match cache.get(cache_key, ttl) {
        FetchOutcome::Ok { body, age } if age.is_fresh => {
            return Nip11Result::Fresh { info_json: body };
        }
        FetchOutcome::Ok { body, age } => {
            return Nip11Result::Stale {
                info_json: body,
                age,
            };
        }
        FetchOutcome::Error { age, .. } if age.is_fresh => {
            return Nip11Result::Error("NIP-11 fetch previously failed (cached)".to_owned());
        }
        FetchOutcome::Absent | FetchOutcome::NotFound { .. } | FetchOutcome::Error { .. } => {}
    }

    let fetched_at = Instant::now();
    match http.get(&http_url).await {
        Err(e) => {
            cache.set_error(cache_key, e.clone(), fetched_at);
            Nip11Result::Error(e)
        }
        Ok(resp) if resp.status == 200 => {
            cache.set_ok(cache_key, resp.body.clone(), fetched_at);
            Nip11Result::Fresh {
                info_json: resp.body,
            }
        }
        Ok(resp) => {
            let reason = format!("NIP-11 HTTP {}", resp.status);
            cache.set_error(cache_key, reason.clone(), fetched_at);
            Nip11Result::Error(reason)
        }
    }
}

fn relay_url_to_http(relay_url: &str) -> String {
    if let Some(rest) = relay_url.strip_prefix("wss://") {
        format!("https://{rest}")
    } else if let Some(rest) = relay_url.strip_prefix("ws://") {
        format!("http://{rest}")
    } else {
        relay_url.to_owned()
    }
}
