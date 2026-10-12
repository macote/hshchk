use super::*;

#[test]
fn throughput_handles_empty_intervals_counter_resets_and_unit_boundaries() {
    assert_eq!(
        [BPS, KBPS, MBPS, GBPS, TBPS],
        ["B/s", "KiB/s", "MiB/s", "GiB/s", "TiB/s"]
    );
    for (current, previous, elapsed) in [(10, 0, 0), (10, 10, 1), (0, 10, 1)] {
        let speed = get_speed(current, previous, elapsed);
        assert_eq!(speed.bytes_per_interval, 0);
        assert_eq!(speed.unit, BPS);
    }
    for (bytes, expected, unit) in [
        (1_023, 1_023, BPS),
        (1_024, 1, KBPS),
        (1_048_576, 1, MBPS),
        (1_073_741_824, 1, GBPS),
        (1_099_511_627_776, 1, TBPS),
    ] {
        let speed = get_speed(bytes + 100, 100, 1_000);
        assert_eq!(speed.bytes_per_interval, expected);
        assert_eq!(speed.unit, unit);
    }
    let extreme = get_speed(u64::MAX, 0, 1);
    assert_eq!(extreme.unit, TBPS);
    assert!(extreme.bytes_per_interval > 0);
}
