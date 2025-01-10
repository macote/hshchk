use std::convert::TryInto;

pub static BPS: &str = "B/s";
pub static KBPS: &str = "KB/s";
pub static MBPS: &str = "MB/s";
pub static GBPS: &str = "GB/s";
pub static TBPS: &str = "TB/s";

pub struct Speed {
    pub bytes_per_interval: u64,
    pub unit: &'static str,
}

pub fn get_speed(current_bytes: u64, previous_bytes: u64, elapsed_millis: u128) -> Speed {
    let speed = if elapsed_millis == 0 || previous_bytes >= current_bytes {
        0 as u128
    } else {
        (current_bytes - previous_bytes) as u128 * 1_000 / elapsed_millis
    };

    match speed {
        speed if speed < 1_024 => Speed {
            bytes_per_interval: speed.try_into().unwrap(),
            unit: BPS,
        },
        speed if speed < 1_048_576 => Speed {
            bytes_per_interval: (speed / 1_024).try_into().unwrap(),
            unit: KBPS,
        },
        speed if speed < 1_073_741_824 => Speed {
            bytes_per_interval: (speed / 1_048_576).try_into().unwrap(),
            unit: MBPS,
        },
        speed if speed < 1_099_511_627_776 => Speed {
            bytes_per_interval: (speed / 1_073_741_824).try_into().unwrap(),
            unit: GBPS,
        },
        _ => Speed {
            bytes_per_interval: (speed / 1_099_511_627_776).try_into().unwrap(),
            unit: TBPS,
        },
    }
}
