//! Whether one already-sent wire filter can serve a later logical demand.
//!
//! Containment is an *attach* question, not a merge question, and the two have
//! different costs. Merging rewrites a live REQ; attaching costs nothing —
//! a demand whose events the relay is already sending needs no subscription of
//! its own. Keeping subsumption out of the merge predicate is deliberate: the
//! merge rule refuses `None`-versus-`Some` because unioning an unconstrained
//! operand into a constrained one narrows, while *here* an unconstrained wide
//! axis is exactly what makes coverage hold.

use nostr::filter::Filter;

/// Whether `wide` is already asking the relay for every event `narrow` wants.
///
/// This is a containment test over each filter axis, never a merge. A `true`
/// answer means `narrow` needs no wire subscription of its own: the events it
/// wants are already arriving, and the local per-demand re-match keeps the
/// surplus out of its results.
///
/// Two boundaries are refused outright, both because a result count is not a
/// set axis:
///
/// * a `wide` that carries a `limit` never covers anything but itself — its
///   truncation boundary cannot be reconstructed for a second owner;
/// * a `narrow` that carries a `limit` attaches to nothing but a byte-identical
///   filter — the relay's own choice of which `limit` rows to return is not
///   reproducible from a wider stream.
///
/// Over-fetch is free here; the surplus is discarded locally. Under-fetch is
/// silent data loss, so no residual is ever subtracted: a demand that is not
/// fully covered executes its **whole** filter, never the difference.
#[must_use]
pub fn filter_covers(wide: &Filter, narrow: &Filter) -> bool {
    if wide == narrow {
        return true;
    }
    if wide.limit.is_some() || narrow.limit.is_some() {
        return false;
    }
    if !remainder_matches(wide, narrow) {
        return false;
    }
    covers_set(wide.ids.as_ref(), narrow.ids.as_ref())
        && covers_set(wide.authors.as_ref(), narrow.authors.as_ref())
        && covers_set(wide.kinds.as_ref(), narrow.kinds.as_ref())
        && covers_window(wide, narrow)
        && covers_tags(wide, narrow)
}

/// Whether every field outside the coverage axes is identical.
///
/// Written as a destructure so that a new `nostr::Filter` field fails the build
/// rather than being silently treated as covered.
fn remainder_matches(wide: &Filter, narrow: &Filter) -> bool {
    let Filter {
        ids: _,
        authors: _,
        kinds: _,
        search: wide_search,
        since: _,
        until: _,
        limit: _,
        generic_tags: _,
    } = wide;
    let Filter {
        ids: _,
        authors: _,
        kinds: _,
        search: narrow_search,
        since: _,
        until: _,
        limit: _,
        generic_tags: _,
    } = narrow;
    // A NIP-50 search term is not a set axis: a filter searching for one phrase
    // does not contain one searching for another, nor one searching for nothing.
    wide_search == narrow_search
}

/// Whether a wide value-set axis contains a narrow one.
///
/// `None` and `Some(empty)` are both *unconstrained* on `ids`, `authors`, and
/// `kinds`: `nostr`'s `match_event` ignores an empty set exactly as it ignores
/// an absent one. A constrained wide axis therefore never covers an
/// unconstrained narrow one.
fn covers_set<T: Ord>(
    wide: Option<&std::collections::BTreeSet<T>>,
    narrow: Option<&std::collections::BTreeSet<T>>,
) -> bool {
    let wide = wide.filter(|values| !values.is_empty());
    let narrow = narrow.filter(|values| !values.is_empty());
    match (wide, narrow) {
        (None, _) => true,
        (Some(_), None) => false,
        (Some(wide), Some(narrow)) => narrow.is_subset(wide),
    }
}

/// Whether a wide time window contains a narrow one.
fn covers_window(wide: &Filter, narrow: &Filter) -> bool {
    let since = match (wide.since, narrow.since) {
        (None, _) => true,
        (Some(_), None) => false,
        (Some(wide), Some(narrow)) => wide <= narrow,
    };
    let until = match (wide.until, narrow.until) {
        (None, _) => true,
        (Some(_), None) => false,
        (Some(wide), Some(narrow)) => wide >= narrow,
    };
    since && until
}

/// Whether a wide tag constraint contains a narrow one.
///
/// The polarity is the inverse of `ids`/`authors`/`kinds` and getting it
/// backwards is the most dangerous mistake available on this axis. A filter
/// that never mentions `#t` matches every event, tagged or not, so an *absent*
/// name on the wide side covers anything. A *present* name with an empty value
/// set matches nothing — `match_event` evaluates `any()` over an empty set —
/// so it covers only a narrow side that also asks for nothing on that name.
/// Tag names are conjunctive across names, so a name the narrow side adds only
/// makes it narrower and is always covered.
fn covers_tags(wide: &Filter, narrow: &Filter) -> bool {
    wide.generic_tags.iter().all(|(name, wide_values)| {
        narrow
            .generic_tags
            .get(name)
            .is_some_and(|narrow_values| narrow_values.is_subset(wide_values))
    })
}
