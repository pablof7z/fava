//! External compile-surface proof for the NIP-02 capability crate.

use std::sync::Arc;

use fava_write::{
    Kind, PublicKey, ReplaceableEventEdit, ReplaceableEventMaterializer, WriteIntentError,
};

type EditResult = Result<ReplaceableEventEdit, WriteIntentError>;
type PublicKeyEdit = fn(PublicKey) -> EditResult;
type Selection = fn() -> Arc<dyn ReplaceableEventMaterializer>;

const FOLLOW: PublicKeyEdit = fava_nip02::follow;
const UNFOLLOW: PublicKeyEdit = fava_nip02::unfollow;
const MATERIALIZER: Selection = fava_nip02::materializer;

#[test]
fn external_surface_uses_only_approved_functions_and_types() {
    let approved_functions: [PublicKeyEdit; 2] = [FOLLOW, UNFOLLOW];
    assert_eq!(approved_functions.len(), 2);
    assert_eq!(MATERIALIZER().kind(), Kind::ContactList);
}
