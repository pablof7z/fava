use fava_query::{PublicKey, Query, QueryError, QuerySnapshot, SingleLetterTag};
use fava_write::Kind;

use crate::ContactList;

mod sealed {
    use fava_query::PublicKey;

    pub(crate) trait Sealed {}

    impl Sealed for PublicKey {}
    impl Sealed for &PublicKey {}
    impl Sealed for Vec<PublicKey> {}
    impl Sealed for &Vec<PublicKey> {}
    impl Sealed for &[PublicKey] {}
    impl<const N: usize> Sealed for [PublicKey; N] {}
    impl<const N: usize> Sealed for &[PublicKey; N] {}
}

/// Accepted one-or-many author inputs for [`contact_list`].
///
/// This trait is sealed so the query builder retains one exact meaning for
/// every supported input shape.
#[allow(private_bounds)]
pub trait IntoContactAuthors: sealed::Sealed {
    /// Concrete public-key iterator consumed by the ordinary query builder.
    #[doc(hidden)]
    type IntoIter: Iterator<Item = PublicKey>;

    /// Convert the supported input without creating query or observation work.
    #[doc(hidden)]
    fn into_contact_authors(self) -> Self::IntoIter;
}

impl IntoContactAuthors for PublicKey {
    type IntoIter = std::iter::Once<Self>;

    fn into_contact_authors(self) -> Self::IntoIter {
        std::iter::once(self)
    }
}

impl IntoContactAuthors for &PublicKey {
    type IntoIter = std::iter::Once<PublicKey>;

    fn into_contact_authors(self) -> Self::IntoIter {
        std::iter::once(*self)
    }
}

impl IntoContactAuthors for Vec<PublicKey> {
    type IntoIter = std::vec::IntoIter<PublicKey>;

    fn into_contact_authors(self) -> Self::IntoIter {
        self.into_iter()
    }
}

impl<'a> IntoContactAuthors for &'a Vec<PublicKey> {
    type IntoIter = std::iter::Copied<std::slice::Iter<'a, PublicKey>>;

    fn into_contact_authors(self) -> Self::IntoIter {
        self.iter().copied()
    }
}

impl<'a> IntoContactAuthors for &'a [PublicKey] {
    type IntoIter = std::iter::Copied<std::slice::Iter<'a, PublicKey>>;

    fn into_contact_authors(self) -> Self::IntoIter {
        self.iter().copied()
    }
}

impl<const N: usize> IntoContactAuthors for [PublicKey; N] {
    type IntoIter = std::array::IntoIter<PublicKey, N>;

    fn into_contact_authors(self) -> Self::IntoIter {
        self.into_iter()
    }
}

impl<'a, const N: usize> IntoContactAuthors for &'a [PublicKey; N] {
    type IntoIter = std::iter::Copied<std::slice::Iter<'a, PublicKey>>;

    fn into_contact_authors(self) -> Self::IntoIter {
        self.iter().copied()
    }
}

/// Query the newest kind-3 contact list for each concrete author.
///
/// Empty author input retains a present-empty author axis and therefore
/// matches nothing. No global result limit is applied: the ordinary evaluator
/// independently selects the newest replaceable event at each author
/// coordinate.
///
/// # Errors
///
/// Returns [`QueryError`] when the neutral query owner refuses the author input.
pub fn contact_list(authors: impl IntoContactAuthors) -> Result<Query, QueryError> {
    Query::events()
        .kinds([Kind::ContactList])?
        .authors(authors.into_contact_authors())
}

/// Project valid followed public keys in snapshot order, then source-entry order.
///
/// The projection owns no mutable state. Each record is decoded through
/// [`ContactList`], so malformed and duplicate entries remain represented by the
/// decoder even though this key-only projection emits only valid follows.
#[must_use]
pub fn follows_of(snapshot: &QuerySnapshot) -> Vec<PublicKey> {
    let mut follows = Vec::new();
    for record in snapshot.events.iter() {
        let Ok(list) = ContactList::from_event(&record.event) else {
            continue;
        };
        follows.extend(list.follows().iter().map(crate::Follow::pubkey));
    }
    follows
}

/// Query kind-3 contact lists containing an exact lowercase `p` entry target.
///
/// # Errors
///
/// Returns [`QueryError`] when the neutral query owner refuses construction.
pub fn followers_of(subject: PublicKey) -> Result<Query, QueryError> {
    Query::events()
        .kinds([Kind::ContactList])?
        .tag_values(SingleLetterTag::LOWERCASE_P, [subject.to_hex()])
}
