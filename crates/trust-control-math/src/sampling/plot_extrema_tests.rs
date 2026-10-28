use super::extrema_indices;

#[test]
fn a_set_within_its_limit_keeps_every_point() {
    let values = [1.0, 2.0, 3.0];
    assert_eq!(extrema_indices(3, &[&values], 1, 3), vec![0, 1, 2]);
}

#[test]
fn a_long_set_keeps_the_first_and_last_point_and_each_groups_extremes() {
    // Ten points in two groups of five: the first group's lowest is at 1 and its highest at 3, the second group's lowest
    // at 9 and its highest at 6.
    let values = [5.0, 1.0, 5.0, 9.0, 5.0, 5.0, 8.0, 5.0, 5.0, 2.0];
    assert_eq!(extrema_indices(10, &[&values], 2, 4), vec![0, 1, 3, 6, 9]);
}

#[test]
fn a_single_spike_in_a_long_flat_series_is_kept() {
    let mut values = vec![35.0; 100_000];
    values[54_321] = 38.9;
    let kept = extrema_indices(values.len(), &[&values], 1_000, 6_000);
    assert!(kept.contains(&54_321), "the spike is kept");
    assert!(kept.len() <= 2 * 1_000 + 2, "{} points", kept.len());
}

#[test]
fn every_series_keeps_its_own_extremes_on_the_shared_time_base() {
    let rising: Vec<f64> = (0..1_000).map(f64::from).collect();
    let mut command = vec![75.0; 1_000];
    command[500] = 100.0;
    let kept = extrema_indices(1_000, &[&rising, &command], 10, 50);
    assert!(kept.contains(&500), "the command's peak is kept");
    assert!(
        kept.windows(2).all(|pair| pair[0] < pair[1]),
        "ascending and unique: {kept:?}"
    );
    assert!(kept.len() <= 2 * 2 * 10 + 2);
}

#[test]
fn a_nan_is_never_a_lowest_or_a_highest_value() {
    let values = [3.0, f64::NAN, 1.0, 2.0, 7.0, f64::NAN];
    assert_eq!(extrema_indices(6, &[&values], 1, 2), vec![0, 2, 4, 5]);
}
