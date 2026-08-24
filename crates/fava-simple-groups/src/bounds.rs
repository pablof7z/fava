pub(super) const MAX_SIMPLE_GROUP_HOST_INPUT_ITEMS: usize = 256;
pub(super) const MAX_SIMPLE_GROUP_ID_BYTES: usize = 4_096;
pub(super) const MAX_SIMPLE_GROUP_QUERY_RESULTS: usize = 4_096;
pub(super) const MAX_SIMPLE_GROUP_CONTEXT_INPUT_ITEMS: usize = 2_000;
pub(super) const MAX_DISCOVERY_INPUT_ITEMS: usize = 256;

pub(super) fn collect_at_most<T>(
    input: impl IntoIterator<Item = T>,
    maximum: usize,
) -> Result<Vec<T>, usize> {
    let values: Vec<_> = input.into_iter().take(maximum.saturating_add(1)).collect();
    if values.len() > maximum {
        Err(values.len())
    } else {
        Ok(values)
    }
}
