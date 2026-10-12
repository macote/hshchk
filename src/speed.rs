use std::fmt;

pub static BPS: &str = "B/s";
pub static KBPS: &str = "KiB/s";
pub static MBPS: &str = "MiB/s";
pub static GBPS: &str = "GiB/s";
pub static TBPS: &str = "TiB/s";

pub struct Speed {
    pub bytes_per_interval: f64,
    pub unit: &'static str,
}

impl fmt::Display for Speed {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:.2} {}", self.bytes_per_interval, self.unit)
    }
}

#[cfg(test)]
#[path = "tests/speed.rs"]
mod tests;

pub fn get_speed(current_bytes: u64, previous_bytes: u64, elapsed_millis: u128) -> Speed {
    let speed = if elapsed_millis == 0 || previous_bytes >= current_bytes {
        0.0
    } else {
        (current_bytes - previous_bytes) as f64 * 1_000.0 / elapsed_millis as f64
    };

    match speed {
        speed if speed < 1_024.0 => Speed {
            bytes_per_interval: speed,
            unit: BPS,
        },
        speed if speed < 1_048_576.0 => Speed {
            bytes_per_interval: speed / 1_024.0,
            unit: KBPS,
        },
        speed if speed < 1_073_741_824.0 => Speed {
            bytes_per_interval: speed / 1_048_576.0,
            unit: MBPS,
        },
        speed if speed < 1_099_511_627_776.0 => Speed {
            bytes_per_interval: speed / 1_073_741_824.0,
            unit: GBPS,
        },
        _ => Speed {
            bytes_per_interval: speed / 1_099_511_627_776.0,
            unit: TBPS,
        },
    }
}
