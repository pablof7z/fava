//! External compile-surface proof for the NIP-02 capability crate.

use std::sync::Arc;

use fava_query::{Query, QuerySnapshot};
use fava_state::RelayUrl;
use fava_write::{
    EventBuilder, EventValue, Kind, PublicKey, ReplaceableEventEdit, ReplaceableEventMaterializer,
    Tag, Timestamp, WriteIntentError,
};

use fava_nip02::{
    ContactList, ContactListError, ContactListRowEvidence, Follow, IntoContactAuthors,
};

type EditResult = Result<ReplaceableEventEdit, WriteIntentError>;
type PublicKeyEdit = fn(PublicKey) -> EditResult;
type Selection = fn() -> Arc<dyn ReplaceableEventMaterializer>;

const FOLLOW: PublicKeyEdit = fava_nip02::follow;
const UNFOLLOW: PublicKeyEdit = fava_nip02::unfollow;
const MATERIALIZER: Selection = fava_nip02::materializer;
const FOLLOWS_OF: fn(&QuerySnapshot) -> Vec<PublicKey> = fava_nip02::follows_of;
const FOLLOWERS_OF: fn(PublicKey) -> Query = fava_nip02::followers_of;

fn contact_lists<A: IntoContactAuthors>(authors: A) -> Query {
    fava_nip02::contact_list(authors)
}

fn inspect_follow(follow: &Follow) -> (usize, PublicKey, Option<&RelayUrl>, Option<&str>) {
    (
        follow.source_index(),
        follow.pubkey(),
        follow.relay(),
        follow.petname(),
    )
}

fn inspect_row(row: &ContactListRowEvidence) -> (usize, &[String]) {
    (row.source_index(), row.raw_row())
}

#[test]
fn external_surface_uses_only_approved_functions_and_types() {
    let approved_functions: [PublicKeyEdit; 2] = [FOLLOW, UNFOLLOW];
    assert_eq!(approved_functions.len(), 2);
    assert_eq!(MATERIALIZER().kind(), Kind::ContactList);

    let author =
        PublicKey::from_hex("79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798")
            .expect("generator public key");
    let event = EventBuilder::new(author, Kind::ContactList)
        .created_at(Timestamp::from(7))
        .tag(Tag::parse(["p", &author.to_hex()]).expect("valid follow row"))
        .tag(Tag::parse(["p"]).expect("short evidence row"))
        .build()
        .expect("bounded contact list");
    let list: ContactList =
        ContactList::from_event(&EventValue::Unsigned(event)).expect("external decoder surface");
    let follows: &[Follow] = list.follows();
    let evidence: &[ContactListRowEvidence] = list.evidence();
    assert_eq!(inspect_follow(&follows[0]), (0, author, None, None));
    assert_eq!(inspect_row(&evidence[0]), (1, &["p".to_owned()][..]));
    let decode: fn(&EventValue) -> Result<ContactList, ContactListError> = ContactList::from_event;
    assert!(decode as usize != 0);

    let one = contact_lists(author);
    let many = contact_lists([author]);
    let borrowed_authors = vec![author];
    let borrowed = contact_lists::<&Vec<PublicKey>>(&borrowed_authors);
    assert_eq!(one, many);
    assert_eq!(many, borrowed);
    assert_eq!(FOLLOWERS_OF(author).selection().authors, None);
    assert!(FOLLOWS_OF(&QuerySnapshot::evaluated(Vec::new(), &[])).is_empty());
}
