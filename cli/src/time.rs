use crate::value_contracts::valid_date;
pub(crate) fn parse_utc_seconds(v: &str) -> Option<i64> {
    if !v.is_ascii()
        || v.len() != 20
        || !v.ends_with('Z')
        || &v[4..5] != "-"
        || &v[7..8] != "-"
        || &v[10..11] != "T"
        || &v[13..14] != ":"
        || &v[16..17] != ":"
        || !valid_date(&v[..10])
    {
        return None;
    }
    if !v[..4]
        .bytes()
        .chain(v[5..7].bytes())
        .chain(v[8..10].bytes())
        .chain(v[11..13].bytes())
        .chain(v[14..16].bytes())
        .chain(v[17..19].bytes())
        .all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    let n = |a, b| v[a..b].parse::<i64>().ok();
    let (y, m, d, h, mi, s) = (
        n(0, 4)?,
        n(5, 7)?,
        n(8, 10)?,
        n(11, 13)?,
        n(14, 16)?,
        n(17, 19)?,
    );
    if h > 23 || mi > 59 || s > 59 {
        return None;
    }
    Some(
        days_from_civil(y, m, d)
            .checked_mul(86400)?
            .checked_add(h * 3600 + mi * 60 + s)?,
    )
}
pub(crate) fn parse_day_cadence(v: &str) -> Option<u32> {
    let d = v.strip_prefix('P')?.strip_suffix('D')?;
    if d.is_empty() || !d.bytes().all(|byte| byte.is_ascii_digit()) || d.starts_with('0') {
        return None;
    }
    let n = d.parse().ok()?;
    Some(n)
}
pub(crate) fn checked_add_days(s: i64, d: u32) -> Option<i64> {
    s.checked_add(i64::from(d).checked_mul(86400)?)
}
pub(crate) fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = y - i64::from(m <= 2);
    let e = (if y >= 0 { y } else { y - 399 }) / 400;
    let o = y - e * 400;
    let p = m + if m > 2 { -3 } else { 9 };
    let q = (153 * p + 2) / 5 + d - 1;
    e * 146097 + o * 365 + o / 4 - o / 100 + q - 719468
}

pub(crate) fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = (if z >= 0 { z } else { z - 146_096 }) / 146_097;
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    (year + i64::from(month <= 2), month, day)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn strict() {
        assert_eq!(parse_utc_seconds("1970-01-01T00:00:00Z"), Some(0));
        assert!(parse_utc_seconds("2026-01-01T00:00:00+00:00").is_none());
        assert_eq!(parse_day_cadence("P90D"), Some(90));
        assert!(parse_day_cadence("P0D").is_none());
        for invalid in ["P01D", "P+1D", "P 1D", "P4294967296D"] {
            assert!(parse_day_cadence(invalid).is_none(), "{invalid}");
        }
        assert!(parse_day_cadence("P90M").is_none());
        assert!(parse_utc_seconds("2026-01-01T00:00:00+00:00").is_none());
        assert!(parse_utc_seconds("2026-01-01T00:00:00.000Z").is_none());
    }
    #[test]
    fn leap() {
        assert_eq!(parse_utc_seconds("2024-02-29T00:00:00Z"), Some(1709164800));
        assert!(checked_add_days(i64::MAX, 1).is_none());
        assert_eq!(civil_from_days(0), (1970, 1, 1));
    }
}
