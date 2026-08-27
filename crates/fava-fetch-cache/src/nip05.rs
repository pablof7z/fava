//! NIP-05 identifier resolution with negative-cache and stale-result semantics.
//!
//! Resolves `<name>@<domain>` identifiers to Nostr public keys.
//! Negative results (404 or name absent from JSON) are cached to prevent
//! repeated queries for unknown identifiers within the TTL.

use std::time::{Duration, Instant};

use crate::{FetchCache, FetchOutcome, HttpFetcher};

/// Whether a NIP-05 identifier resolved, was absent, or was answered from cache,
/// and how stale that answer is.
#[derive(Clone, Debug)]
pub enum Nip05Result {
    /// Public key resolved fresh within the requested TTL.
    Fresh {
        /// Hex-encoded 32-byte public key.
        pubkey_hex: String,
    },
    /// The identifier was not found (404 or name absent). Fresh within TTL.
    NotFound,
    /// A previous resolve returned not-found, still within the negative-cache TTL.
    NegativeCached {
        /// Age of the negative-cache entry.
        age: crate::FetchAge,
    },
    /// A previous resolve returned a key, but the cache entry is past TTL.
    /// The caller may use `pubkey_hex` but should treat it as potentially stale.
    Stale {
        /// Cached public key hex.
        pubkey_hex: String,
        /// Freshness evidence: how old the cached entry is.
        age: crate::FetchAge,
    },
    /// HTTP fetch failed. The error is bounded.
    Error(String),
}

impl Nip05Result {
    /// Whether this result came from the negative cache (no network).
    #[must_use]
    pub fn is_negative_cached(&self) -> bool {
        matches!(self, Self::NegativeCached { .. })
    }

    /// Whether this result is stale.
    #[must_use]
    pub fn is_stale(&self) -> bool {
        matches!(self, Self::Stale { .. })
    }
}

/// Resolve a NIP-05 identifier using the provided cache and HTTP fetcher.
///
/// Cache key is the full `.well-known/nostr.json` URL. A `200` response is
/// parsed for the `names` map. A `404` is stored as negative. Any other
/// status or parse failure is stored as an error.
///
/// # Errors
///
/// Does not return `Err`; all outcomes including network failures are encoded
/// in [`Nip05Result`] so the caller has full evidence of what happened.
pub async fn resolve(
    identifier: &str,
    cache: &dyn FetchCache,
    http: &dyn HttpFetcher,
    ttl: Duration,
) -> Nip05Result {
    let Some((name, domain)) = identifier.split_once('@') else {
        return Nip05Result::Error(format!("invalid NIP-05 identifier: {identifier}"));
    };
    // Avoid path traversal / injection in the identifier components.
    if name.is_empty() || domain.is_empty() {
        return Nip05Result::Error("NIP-05 identifier has empty name or domain".to_owned());
    }
    // Use http:// for local addresses, https:// otherwise.
    let scheme = if domain.starts_with("127.") || domain == "localhost" {
        "http"
    } else {
        "https"
    };
    let url = format!("{scheme}://{domain}/.well-known/nostr.json?name={name}");
    let cache_key = &url;

    // Check cache first.
    match cache.get(cache_key, ttl) {
        FetchOutcome::Ok { body, age } if age.is_fresh => {
            return parse_names_for(&body, name)
                .map(|hex| Nip05Result::Fresh { pubkey_hex: hex })
                .unwrap_or(Nip05Result::NotFound);
        }
        FetchOutcome::Ok { body, age } => {
            // Stale positive result.
            if let Some(hex) = parse_names_for(&body, name) {
                return Nip05Result::Stale {
                    pubkey_hex: hex,
                    age,
                };
            }
            // Name was absent in a now-stale positive fetch.
            return Nip05Result::Stale {
                pubkey_hex: String::new(),
                age,
            };
        }
        FetchOutcome::NotFound { age } if age.is_fresh => {
            return Nip05Result::NegativeCached { age };
        }
        FetchOutcome::NotFound { age } if !age.is_fresh => {
            // Stale negative — fall through to re-fetch.
            let _ = age;
        }
        FetchOutcome::Error { age, .. } if age.is_fresh => {
            // Fresh cached error — return it without re-fetching.
            return Nip05Result::Error("NIP-05 fetch previously failed (cached)".to_owned());
        }
        FetchOutcome::Absent | FetchOutcome::NotFound { .. } | FetchOutcome::Error { .. } => {}
    }

    // Execute HTTP fetch.
    let fetched_at = Instant::now();
    match http.get(&url).await {
        Err(e) => {
            cache.set_error(cache_key, e.clone(), fetched_at);
            Nip05Result::Error(e)
        }
        Ok(resp) if resp.status == 404 => {
            cache.set_not_found(cache_key, fetched_at);
            Nip05Result::NotFound
        }
        Ok(resp) if resp.status == 200 => {
            let body = resp.body;
            let result = parse_names_for(&body, name)
                .map(|hex| Nip05Result::Fresh { pubkey_hex: hex })
                .unwrap_or(Nip05Result::NotFound);
            match &result {
                Nip05Result::Fresh { .. } => {
                    cache.set_ok(cache_key, body, fetched_at);
                }
                _ => {
                    cache.set_not_found(cache_key, fetched_at);
                }
            }
            result
        }
        Ok(resp) => {
            let reason = format!("NIP-05 HTTP {}", resp.status);
            cache.set_error(cache_key, reason.clone(), fetched_at);
            Nip05Result::Error(reason)
        }
    }
}

fn parse_names_for(body: &str, name: &str) -> Option<String> {
    let json: serde_json::Value = serde_json::from_str(body).ok()?;
    json.get("names")?
        .get(name)?
        .as_str()
        .map(ToString::to_string)
}
