//! Verify one Nostr kind-9999 vocabulary-approval event.
//!
//! Reads one JSON event from stdin.  Exits 0 and prints the approved term name
//! on stdout if the event passes all checks.  Exits 1 and prints a diagnostic
//! on stderr otherwise.
//!
//! Checks performed (fail on first failure):
//! 1. Valid JSON that parses as a Nostr event.
//! 2. `nostr::Event::verify()` — id hash + Schnorr signature.
//! 3. `pubkey` is the owner pubkey.
//! 4. `kind` is 9999.
//! 5. Exactly one `name` tag.

use nostr::event::Event;
use std::io::Read;

const OWNER: &str = "fa984bd7dbb282f07e16e7ae87b26a2a7b9b90b7246a44771f0cf5ae58018f52";
const APPROVAL_KIND: u16 = 9999;

fn main() {
    let mut input = String::new();
    if let Err(e) = std::io::stdin().read_to_string(&mut input) {
        eprintln!("vocab-verify: cannot read stdin: {e}");
        std::process::exit(1);
    }

    let event: Event = match Event::from_json(input.trim()) {
        Ok(ev) => ev,
        Err(e) => {
            eprintln!("vocab-verify: parse failed: {e}");
            std::process::exit(1);
        }
    };

    if let Err(e) = event.verify() {
        eprintln!("vocab-verify: crypto verification failed: {e}");
        std::process::exit(1);
    }

    if event.pubkey.to_hex() != OWNER {
        eprintln!(
            "vocab-verify: pubkey is not the owner: {}",
            event.pubkey.to_hex()
        );
        std::process::exit(1);
    }

    if event.kind.as_u16() != APPROVAL_KIND {
        eprintln!(
            "vocab-verify: kind must be {APPROVAL_KIND}, got {}",
            event.kind.as_u16()
        );
        std::process::exit(1);
    }

    let name_tags: Vec<&str> = event
        .tags
        .iter()
        .filter(|t| t.kind() == "name")
        .filter_map(|t| t.content())
        .collect();

    let name = match name_tags.len() {
        1 => name_tags[0],
        0 => {
            eprintln!("vocab-verify: event has no name tag");
            std::process::exit(1);
        }
        n => {
            eprintln!("vocab-verify: event has {n} name tags (must be exactly 1)");
            std::process::exit(1);
        }
    };

    println!("{name}");
}
