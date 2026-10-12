use super::*;

#[test]
fn throughput_handles_empty_intervals_counter_resets_and_unit_boundaries() {
    assert_eq!(
        [BPS, KBPS, MBPS, GBPS, TBPS],
        ["B/s", "KiB/s", "MiB/s", "GiB/s", "TiB/s"]
    );
    for (current, previous, elapsed) in [(10, 0, 0), (10, 10, 1), (0, 10, 1)] {
        let speed = get_speed(current, previous, elapsed);
        assert_eq!(speed.bytes_per_interval, 0.0);
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
        assert_eq!(speed.bytes_per_interval, f64::from(expected));
        assert_eq!(speed.unit, unit);
    }
    let extreme = get_speed(u64::MAX, 0, 1);
    assert_eq!(extreme.unit, TBPS);
    assert!(extreme.bytes_per_interval > 0.0);
}

#[test]
fn throughput_preserves_fractional_binary_units_and_formats_two_decimals() {
    assert_eq!(get_speed(1, 0, 2_000).to_string(), "0.50 B/s");
    assert_eq!(get_speed(1_536, 0, 1_000).to_string(), "1.50 KiB/s");
    assert_eq!(get_speed(1_572_864, 0, 1_000).to_string(), "1.50 MiB/s");
    assert_eq!(get_speed(1_116_691_497, 0, 1_000).to_string(), "1.04 GiB/s");
    assert_eq!(
        get_speed(248_729_809_985, 0, 175_065).to_string(),
        "1.32 GiB/s"
    );
    assert_eq!(
        get_speed(25_373_442_048, 0, 18_780).to_string(),
        "1.26 GiB/s"
    );
}
