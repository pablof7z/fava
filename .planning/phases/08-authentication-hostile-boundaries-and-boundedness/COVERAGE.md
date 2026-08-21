# Phase 08 API Coverage Declaration

The deterministic API detector is intentionally satisfied by a reasoned non-application-API disposition.

Phase 08 exercises Nostr relay protocols through existing Rust contracts, real WebSocket/HTTP sockets, and checked-in relay/process fixtures. It does not add or integrate an external application API or SDK. NIP-42, NIP-11, relay EVENT/REQ/OK/NOTICE traffic, Khatru, and `nostr-rs-relay` are protocol/process conformance surfaces, not application API capabilities.

Therefore the phase must prove protocol behavior through owner/provider tests, public Fava capstones, real sockets, separate processes, and independent witnesses. It must not fabricate endpoint, SDK-client, authentication-token, pagination, webhook, rate-limit, or external API capability rows merely to satisfy a detector.
