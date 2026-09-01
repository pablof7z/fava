//! Standalone NIP-01/NIP-42 wire helper for live harnesses.
//!
//! Python has no bundled BIP-340 schnorr implementation, so a harness that
//! must verify a relay-delivered event's signature, or sign a fixture-side
//! NIP-42 `AUTH` response on behalf of an inspection connection that is not
//! the application under test, shells out to this small binary instead of
//! trusting an unverified event or skipping the check.
//!
//! ```text
//! nip01-wire-helper verify   < event.json          # exit 0 "ok", or exit 1 "invalid: <reason>"
//! nip01-wire-helper sign-auth < request.json        # prints the signed kind-22242 event JSON
//! nip01-wire-helper pubkey <secret-key-hex>          # prints the exact derived public key hex
//! ```
//!
//! `request.json` for `sign-auth` is `{"secret_key": "<hex>", "relay": "<url>", "challenge": "<text>"}`.

use std::io::{Read, Write, stdin, stdout};
use std::process::ExitCode;

use nostr::event::{Event, Kind, SignEvent, Tag, UnsignedEvent};
use nostr::key::{Keys, SecretKey};
use nostr::types::{RelayUrl, Timestamp};
use serde::Deserialize;
use serde_json::Value;

fn main() -> ExitCode {
    let mode = std::env::args().nth(1);
    match mode.as_deref() {
        Some("verify") => verify(),
        Some("sign-auth") => sign_auth(),
        Some("pubkey") => pubkey(),
        _ => {
            eprintln!("usage: nip01-wire-helper <verify|sign-auth|pubkey>");
            ExitCode::from(2)
        }
    }
}

fn pubkey() -> ExitCode {
    let Some(secret_key) = std::env::args().nth(2) else {
        eprintln!("usage: nip01-wire-helper pubkey <secret-key-hex>");
        return ExitCode::from(2);
    };
    let secret_key = match SecretKey::from_hex(&secret_key) {
        Ok(key) => key,
        Err(error) => {
            eprintln!("invalid secret_key: {error}");
            return ExitCode::from(2);
        }
    };
    println!("{}", Keys::new(secret_key).public_key());
    ExitCode::SUCCESS
}

fn verify() -> ExitCode {
    let mut input = String::new();
    if stdin().read_to_string(&mut input).is_err() {
        eprintln!("invalid: could not read stdin");
        return ExitCode::from(2);
    }
    let event: Event = match serde_json::from_str(&input) {
        Ok(event) => event,
        Err(error) => {
            eprintln!("invalid: {error}");
            return ExitCode::FAILURE;
        }
    };
    match event.verify() {
        Ok(()) => {
            println!("ok");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("invalid: {error}");
            ExitCode::FAILURE
        }
    }
}

#[derive(Deserialize)]
struct SignAuthRequest {
    secret_key: String,
    relay: String,
    challenge: String,
}

fn sign_auth() -> ExitCode {
    let mut input = String::new();
    if stdin().read_to_string(&mut input).is_err() {
        eprintln!("could not read stdin");
        return ExitCode::from(2);
    }
    let request: SignAuthRequest = match serde_json::from_str(&input) {
        Ok(request) => request,
        Err(error) => {
            eprintln!("malformed request: {error}");
            return ExitCode::from(2);
        }
    };
    let secret_key = match SecretKey::from_hex(&request.secret_key) {
        Ok(key) => key,
        Err(error) => {
            eprintln!("invalid secret_key: {error}");
            return ExitCode::from(2);
        }
    };
    let relay = match RelayUrl::parse(&request.relay) {
        Ok(relay) => relay,
        Err(error) => {
            eprintln!("invalid relay: {error}");
            return ExitCode::from(2);
        }
    };
    let keys = Keys::new(secret_key);
    let unsigned = UnsignedEvent::new(
        keys.public_key(),
        Timestamp::now(),
        Kind::from_u16(22242),
        [
            Tag::parse(["relay", relay.as_str()]).expect("relay tag builds"),
            Tag::parse(["challenge", request.challenge.as_str()]).expect("challenge tag builds"),
        ],
        "",
    );
    let signed = match keys.sign_event(unsigned) {
        Ok(event) => event,
        Err(error) => {
            eprintln!("could not sign: {error}");
            return ExitCode::FAILURE;
        }
    };
    let value: Value = serde_json::to_value(&signed).expect("a signed event serializes");
    let mut output = stdout().lock();
    if writeln!(output, "{value}").is_err() {
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
