//! The points a plot keeps of a long set of series that share one time base (specification 35, B4.5): the points are split
//! into consecutive groups of equal size, and each group keeps the points where each series takes its lowest and its highest
//! value in that group. The first and the last point are always kept, so no peak, step or limit is lost from the plot. A set
//! no longer than its limit keeps every point.

#[cfg(test)]
#[path = "plot_extrema_tests.rs"]
mod tests;

/// The indices to keep of `count` points whose series are `series` (each holding `count` values): every index when `count`
/// is at most `limit` or `groups` is zero; otherwise, for each of `groups` consecutive groups of equal size, the index of
/// each series' lowest and highest value in the group (the first such index on a tie), and the first and the last index.
/// The indices are ascending and unique, at most `2 · series.len() · groups + 2` of them. A `NaN` value is never a lowest
/// or a highest one.
///
/// # Panics
/// A series holds fewer than `count` values.
#[must_use]
pub fn extrema_indices(count: usize, series: &[&[f64]], groups: usize, limit: usize) -> Vec<usize> {
    if count <= limit || groups == 0 {
        return (0..count).collect();
    }
    let mut kept = Vec::with_capacity(2 * series.len() * groups + 2);
    kept.push(0);
    for group in 0..groups {
        let start = group * count / groups;
        let end = (group + 1) * count / groups;
        for values in series {
            if let Some((low, high)) = extremes(&values[start..end]) {
                kept.push(start + low);
                kept.push(start + high);
            }
        }
    }
    kept.push(count - 1);
    kept.sort_unstable();
    kept.dedup();
    kept
}

/// The positions of the lowest and the highest finite value of `values`, the first on a tie; `None` when no value is finite
/// or there are none.
fn extremes(values: &[f64]) -> Option<(usize, usize)> {
    let mut finite = values
        .iter()
        .enumerate()
        .filter(|(_, value)| value.is_finite());
    let (first, _) = finite.next()?;
    let (mut low, mut high) = (first, first);
    for (index, value) in finite {
        if *value < values[low] {
            low = index;
        }
        if *value > values[high] {
            high = index;
        }
    }
    Some((low, high))
}
