//! Small cross-platform helpers.

pub fn human_size(bytes: u64) -> String {
    const GB: f64 = 1024.0 * 1024.0 * 1024.0;
    const MB: f64 = 1024.0 * 1024.0;
    let b = bytes as f64;
    if b >= GB {
        format!("{:.1} GB", b / GB)
    } else {
        format!("{:.0} MB", (b / MB).max(1.0))
    }
}

/// "26.04.1" -> [26, 4, 1] for numeric ordering. Non-numeric parts count as 0.
pub fn version_key(v: &str) -> Vec<u32> {
    v.trim_start_matches('v')
        .split(['.', '-'])
        .map(|p| p.parse().unwrap_or(0))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_order_numerically() {
        assert!(version_key("26.10") > version_key("26.04.1"));
        assert!(version_key("v0.10.0") > version_key("0.9.9"));
        assert!(version_key("1.0.0") == version_key("v1.0.0"));
    }

    #[test]
    fn sizes_are_human() {
        assert_eq!(human_size(6_482_409_472), "6.0 GB");
        assert_eq!(human_size(311_796), "1 MB");
    }
}
