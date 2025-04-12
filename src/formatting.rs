use chrono::{Local, TimeZone};
use num_bigint::BigUint;
use num_traits::cast::ToPrimitive;

pub fn format_bytes_to_iec(bytes: impl Into<BigUint>) -> String {
    let bytes = bytes.into();
    let kb = BigUint::from(1024u32);
    let mb = kb.clone() * &kb;
    let gb = mb.clone() * &kb;
    let tb = gb.clone() * &kb;

    // Upper thresholds - only go to the next unit at 1024 of the current unit
    let kb_threshold = &kb * BigUint::from(1024u32);
    let mb_threshold = &mb * BigUint::from(1024u32);
    let gb_threshold = &gb * BigUint::from(1024u32);

    if bytes < kb {
        format!("{} B", bytes)
    } else if bytes < kb_threshold {
        // Convert to float for formatting with decimal places
        let bytes_f64 = bytes.to_f64().unwrap();
        let kb_f64 = kb.to_f64().unwrap();
        format!("{:.1} KiB", bytes_f64 / kb_f64)
    } else if bytes < mb_threshold {
        let bytes_f64 = bytes.to_f64().unwrap();
        let mb_f64 = mb.to_f64().unwrap();
        format!("{:.1} MiB", bytes_f64 / mb_f64)
    } else if bytes < gb_threshold {
        let bytes_f64 = bytes.to_f64().unwrap();
        let gb_f64 = gb.to_f64().unwrap();
        format!("{:.1} GiB", bytes_f64 / gb_f64)
    } else {
        let bytes_f64 = bytes.to_f64().unwrap();
        let tb_f64 = tb.to_f64().unwrap();
        format!("{:.1} TiB", bytes_f64 / tb_f64)
    }
}

pub fn format_bytes_to_si(bytes: impl Into<BigUint>) -> String {
    let bytes = bytes.into();
    let kb = BigUint::from(1000u32);
    let mb = kb.clone() * &kb;
    let gb = mb.clone() * &kb;
    let tb = gb.clone() * &kb;

    // Upper thresholds - only go to the next unit when we reach 1000 of the current unit
    let kb_threshold = &kb * BigUint::from(1000u32);
    let mb_threshold = &mb * BigUint::from(1000u32);
    let gb_threshold = &gb * BigUint::from(1000u32);

    if bytes < kb {
        format!("{} B", bytes)
    } else if bytes < kb_threshold {
        // Convert to float for formatting with decimal places
        let bytes_f64 = bytes.to_f64().unwrap();
        let kb_f64 = kb.to_f64().unwrap();
        format!("{:.1} KB", bytes_f64 / kb_f64)
    } else if bytes < mb_threshold {
        let bytes_f64 = bytes.to_f64().unwrap();
        let mb_f64 = mb.to_f64().unwrap();
        format!("{:.1} MB", bytes_f64 / mb_f64)
    } else if bytes < gb_threshold {
        let bytes_f64 = bytes.to_f64().unwrap();
        let gb_f64 = gb.to_f64().unwrap();
        format!("{:.1} GB", bytes_f64 / gb_f64)
    } else {
        let bytes_f64 = bytes.to_f64().unwrap();
        let tb_f64 = tb.to_f64().unwrap();
        format!("{:.1} TB", bytes_f64 / tb_f64)
    }
}

pub fn format_datetime_to_localtime(seconds_since_epoch: i64) -> String {
    let datetime = Local.timestamp_opt(seconds_since_epoch, 0).unwrap();
    datetime.format("%a %b %d %H:%M:%S %Y").to_string()
}

pub fn fuzzy_format_bytes_to_si(bytes: impl Into<BigUint>) -> String {
    let bytes = bytes.into();
    let bytes_f64 = bytes.to_f64().unwrap();

    const BASE: f64 = 1000.0;
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    const TOLERANCE: f64 = 0.001;

    let mut val = bytes_f64;
    let mut idx = 0;

    loop {
        let next_unit_available = idx + 1 < UNITS.len();

        // Check if we should display as a whole number (within tolerance and < 999.5)
        let fractional_part = val - val.floor();
        let is_whole = fractional_part.abs() < TOLERANCE;

        if is_whole && (val < 999.5 || !next_unit_available) {
            return format!("{:.0} {}", val.floor(), UNITS[idx]);
        }

        // Check precision thresholds
        if val < 99.995 {
            return format!("{:.2} {}", val, UNITS[idx]);
        }

        if val < 999.95 || !next_unit_available {
            return format!("{:.1} {}", val, UNITS[idx]);
        }

        // Move to next unit
        val /= BASE;
        idx += 1;
    }
}

pub fn parse_size_to_bytes(size_str: &str) -> Option<u64> {
    let parts: Vec<&str> = size_str.split_whitespace().collect();
    if parts.len() != 2 {
        return None;
    }

    let number: f64 = parts[0].parse().ok()?;
    let unit = parts[1];

    let multiplier = match unit {
        "B" => 1,
        // IEC units (binary)
        "KiB" => 1024,
        "MiB" => 1024 * 1024,
        "GiB" => 1024 * 1024 * 1024,
        "TiB" => 1024_u64.pow(4),
        // SI units (decimal)
        "KB" => 1000,
        "MB" => 1000 * 1000,
        "GB" => 1000 * 1000 * 1000,
        "TB" => 1000_u64.pow(4),
        _ => return None,
    };

    Some((number * multiplier as f64) as u64)
}
