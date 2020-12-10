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
    if elapsed_millis == 0 {
        return Speed {
            bytes_per_interval: 0,
            unit: BPS,
        };
    }

    let speed = (current_bytes - previous_bytes) as u128 * 1_000 / elapsed_millis;
    if speed < 1_024 {
        return Speed {
            bytes_per_interval: speed.try_into().unwrap(),
            unit: BPS,
        };
    } else if speed < 1_048_576 {
        return Speed {
            bytes_per_interval: (speed / 1_024).try_into().unwrap(),
            unit: KBPS,
        };
    } else if speed < 1_073_741_824 {
        return Speed {
            bytes_per_interval: (speed / 1_048_576).try_into().unwrap(),
            unit: MBPS,
        };
    } else if speed < 1_099_511_627_776 {
        return Speed {
            bytes_per_interval: (speed / 1_048_576).try_into().unwrap(),
            unit: GBPS,
        };
    }

    Speed {
        bytes_per_interval: (speed / 1_099_511_627_776).try_into().unwrap(),
        unit: TBPS,
    }
}
