//! Timestamps. The local database writes UTC without a zone (2026-10-22T10:00:00.123456); the cloud uses
//! ISO 8601 with a zone. Comparisons use one normalised form, so the two never disagree about which change is newer

/// The form the local database stores: six fraction digits, no zone
pub fn normalize(text: &str) -> String {
    let text = text.trim().trim_end_matches('Z').trim_end_matches("+00:00");
    let (base, fraction) = text.split_once('.').unwrap_or((text, ""));
    let base = base.replace(' ', "T");
    let fraction: String = fraction.chars().take(6).collect();
    format!("{base}.{:0<6}", fraction)
}

/// A local timestamp in the cloud's form
pub fn to_cloud(local: &str) -> String {
    format!("{}Z", normalize(local))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cloud_and_local_forms_compare_equal() {
        assert_eq!(normalize("2026-10-22T12:00:00.123456+00:00"), normalize("2026-10-22T12:00:00.123456"));
        assert_eq!(normalize("2026-10-20 09:25:26.4536"), "2026-10-20T09:25:26.453600");
        assert_eq!(normalize("2026-10-22T12:00:00Z"), "2026-10-22T12:00:00.000000");
        assert_eq!(to_cloud("2026-10-22T12:00:00.5"), "2026-10-22T12:00:00.500000Z");
    }
}
