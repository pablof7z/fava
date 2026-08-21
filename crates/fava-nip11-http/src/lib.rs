//! Bounded HTTP acquisition of NIP-11 relay information documents.
//!
//! This crate owns one bounded request/response exchange and nothing else. It
//! keeps no document, makes no freshness or staleness claim, performs no
//! single-flight coalescing, and uses no service cache. Those semantics belong
//! to the relay-information service profile and are not promised here.

use std::sync::Arc;
use std::time::Duration;

use fava_nip11::{
    MAX_DOCUMENT_BYTES, RelayInformation, RelayInformationError, RelayInformationFetcher,
    parse_relay_information,
};
use fava_state::RelayUrl;
use rustls::ClientConfig;
use rustls::pki_types::ServerName;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;

/// Longest one complete acquisition may take.
pub const DEFAULT_DEADLINE: Duration = Duration::from_secs(10);

/// Largest response Fava will read, headers included.
const MAX_RESPONSE_BYTES: usize = MAX_DOCUMENT_BYTES + 16_384;

/// Largest number of response headers Fava will parse.
const MAX_HEADERS: usize = 64;

/// One bounded HTTP fetcher for relay information documents.
pub struct HttpRelayInformationFetcher {
    deadline: Duration,
    tls: Arc<ClientConfig>,
}

impl Default for HttpRelayInformationFetcher {
    fn default() -> Self {
        Self::with_deadline(DEFAULT_DEADLINE)
    }
}

impl HttpRelayInformationFetcher {
    /// Construct a fetcher with one exact acquisition deadline.
    #[must_use]
    pub fn with_deadline(deadline: Duration) -> Self {
        let roots = rustls::RootCertStore {
            roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
        };
        Self {
            deadline,
            tls: Arc::new(
                ClientConfig::builder()
                    .with_root_certificates(roots)
                    .with_no_client_auth(),
            ),
        }
    }

    async fn acquire(&self, relay: &RelayUrl) -> Result<RelayInformation, RelayInformationError> {
        let target = Target::parse(relay)?;
        let request = format!(
            "GET {} HTTP/1.1\r\nHost: {}\r\nAccept: application/nostr+json\r\nUser-Agent: fava\r\nConnection: close\r\n\r\n",
            target.path, target.authority
        );
        let stream = TcpStream::connect((target.host.as_str(), target.port))
            .await
            .map_err(|error| RelayInformationError::Unreachable(error.to_string()))?;
        let body = if target.secure {
            let name = ServerName::try_from(target.host.clone()).map_err(|error| {
                RelayInformationError::Unreachable(format!("invalid relay host name: {error}"))
            })?;
            let stream = TlsConnector::from(Arc::clone(&self.tls))
                .connect(name, stream)
                .await
                .map_err(|error| RelayInformationError::Unreachable(error.to_string()))?;
            exchange(stream, &request).await?
        } else {
            exchange(stream, &request).await?
        };
        parse_relay_information(&body)
    }
}

impl RelayInformationFetcher for HttpRelayInformationFetcher {
    fn get(
        &self,
        relay: RelayUrl,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<Output = Result<RelayInformation, RelayInformationError>>
                + Send
                + '_,
        >,
    > {
        Box::pin(async move {
            match tokio::time::timeout(self.deadline, self.acquire(&relay)).await {
                Ok(result) => result,
                Err(_) => Err(RelayInformationError::Unreachable(format!(
                    "relay information did not arrive within {:?}",
                    self.deadline
                ))),
            }
        })
    }
}

/// Exact HTTP destination derived from one relay URL.
struct Target {
    host: String,
    port: u16,
    path: String,
    authority: String,
    secure: bool,
}

impl Target {
    fn parse(relay: &RelayUrl) -> Result<Self, RelayInformationError> {
        let url = relay.as_str();
        let (secure, rest) = if let Some(rest) = url.strip_prefix("wss://") {
            (true, rest)
        } else if let Some(rest) = url.strip_prefix("ws://") {
            (false, rest)
        } else {
            return Err(RelayInformationError::Unreachable(format!(
                "relay URL is not a WebSocket URL: {url}"
            )));
        };
        let (authority, path) = rest
            .split_once('/')
            .map_or((rest, "/"), |(authority, path)| {
                (
                    authority,
                    if path.is_empty() {
                        "/"
                    } else {
                        rest.split_at(rest.len() - path.len() - 1).1
                    },
                )
            });
        let (host, port) = authority.rsplit_once(':').map_or_else(
            || (authority.to_owned(), if secure { 443 } else { 80 }),
            |(host, port)| {
                port.parse::<u16>().map_or_else(
                    |_| (authority.to_owned(), if secure { 443 } else { 80 }),
                    |port| (host.to_owned(), port),
                )
            },
        );
        if host.is_empty() {
            return Err(RelayInformationError::Unreachable(format!(
                "relay URL has no host: {url}"
            )));
        }
        Ok(Self {
            host,
            port,
            path: path.to_owned(),
            authority: authority.to_owned(),
            secure,
        })
    }
}

async fn exchange<S>(mut stream: S, request: &str) -> Result<Vec<u8>, RelayInformationError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    stream
        .write_all(request.as_bytes())
        .await
        .map_err(|error| RelayInformationError::Unreachable(error.to_string()))?;
    stream
        .flush()
        .await
        .map_err(|error| RelayInformationError::Unreachable(error.to_string()))?;

    let mut response = Vec::new();
    let mut chunk = [0_u8; 8_192];
    loop {
        let read = stream
            .read(&mut chunk)
            .await
            .map_err(|error| RelayInformationError::Unreachable(error.to_string()))?;
        if read == 0 {
            break;
        }
        if response.len() + read > MAX_RESPONSE_BYTES {
            return Err(RelayInformationError::TooLarge {
                bytes: response.len() + read,
                maximum: MAX_RESPONSE_BYTES,
            });
        }
        response.extend_from_slice(&chunk[..read]);
    }
    body_of(&response)
}

fn body_of(response: &[u8]) -> Result<Vec<u8>, RelayInformationError> {
    let mut headers = [httparse::EMPTY_HEADER; MAX_HEADERS];
    let mut parsed = httparse::Response::new(&mut headers);
    let offset = match parsed.parse(response) {
        Ok(httparse::Status::Complete(offset)) => offset,
        Ok(httparse::Status::Partial) => {
            return Err(RelayInformationError::Malformed(
                "relay closed before a complete HTTP response".to_owned(),
            ));
        }
        Err(error) => return Err(RelayInformationError::Malformed(error.to_string())),
    };
    match parsed.code {
        Some(200) => {}
        Some(code) => {
            return Err(RelayInformationError::Refused(format!(
                "relay answered the information request with HTTP {code}"
            )));
        }
        None => {
            return Err(RelayInformationError::Malformed(
                "HTTP response has no status code".to_owned(),
            ));
        }
    }
    Ok(response[offset..].to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_loopback_relay_url_becomes_a_plain_http_target() {
        let relay = RelayUrl::parse("ws://127.0.0.1:8899").expect("relay URL");
        let target = Target::parse(&relay).expect("target parses");
        assert_eq!(target.host, "127.0.0.1");
        assert_eq!(target.port, 8899);
        assert_eq!(target.path, "/");
        assert!(!target.secure);
    }

    #[test]
    fn a_secure_relay_url_defaults_to_port_443_and_its_path() {
        let relay = RelayUrl::parse("wss://relay.example/nostr").expect("relay URL");
        let target = Target::parse(&relay).expect("target parses");
        assert_eq!(target.host, "relay.example");
        assert_eq!(target.port, 443);
        assert_eq!(target.path, "/nostr");
        assert!(target.secure);
    }

    #[test]
    fn a_non_success_status_is_a_refusal_rather_than_an_empty_document() {
        let response = b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n";
        assert!(matches!(
            body_of(response),
            Err(RelayInformationError::Refused(_))
        ));
    }

    #[test]
    fn a_success_response_yields_exactly_its_body() {
        let response =
            b"HTTP/1.1 200 OK\r\nContent-Type: application/nostr+json\r\n\r\n{\"name\":\"r\"}";
        assert_eq!(body_of(response).expect("body"), b"{\"name\":\"r\"}");
    }
}
