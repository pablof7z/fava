pub(super) const MAX_GROUP_HOST_INPUT_ITEMS: usize = 256;
pub(super) const MAX_GROUP_ID_BYTES: usize = 4_096;

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
