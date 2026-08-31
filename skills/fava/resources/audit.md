# App-builder audit

- Do not parse or retain a private key just to obtain its public key; Fava already owns signer identity.
- Do not run signing or crypto work from an observation/read callback.
