//! Precision-preserving real-world Event times. Validation never rewrites input.
use crate::db::DbError;
use chrono::{DateTime, NaiveDate, Offset};
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct EventTime {
    pub starts_at: Option<String>,
    pub starts_at_precision: Option<String>,
    pub ends_at: Option<String>,
    pub ends_at_precision: Option<String>,
    pub timezone: Option<String>,
}
impl EventTime {
    pub fn validate(&self) -> Result<(), DbError> {
        let zone = self
            .timezone
            .as_ref()
            .map(|s| {
                s.parse::<Tz>()
                    .map_err(|_| DbError::Invalid("timezone must be an IANA timezone".into()))
            })
            .transpose()?;
        let start = endpoint(
            self.starts_at.as_deref(),
            self.starts_at_precision.as_deref(),
            zone,
        )?;
        let end = endpoint(
            self.ends_at.as_deref(),
            self.ends_at_precision.as_deref(),
            zone,
        )?;
        if let (Some(start), Some(end)) = (start, end) {
            if self.starts_at_precision != self.ends_at_precision {
                return Err(DbError::Invalid(
                    "Event range endpoints must use matching precision".into(),
                ));
            }
            if end < start {
                return Err(DbError::Invalid(
                    "Event end must be at or after start".into(),
                ));
            }
        }
        Ok(())
    }
}
fn endpoint(
    value: Option<&str>,
    precision: Option<&str>,
    zone: Option<Tz>,
) -> Result<Option<i128>, DbError> {
    let (value, precision) = match (value, precision) {
        (None, None) => return Ok(None),
        (Some(v), Some(p)) => (v, p),
        _ => {
            return Err(DbError::Invalid(
                "Event endpoint and precision must both be supplied or both null".into(),
            ));
        }
    };
    let invalid = || {
        DbError::Invalid(format!(
            "invalid Event {precision} endpoint: use YYYY, YYYY-MM, YYYY-MM-DD or RFC3339 with an offset"
        ))
    };
    if precision == "instant" {
        let instant = DateTime::parse_from_rfc3339(value).map_err(|_| invalid())?;
        if let Some(zone) = zone
            && instant.with_timezone(&zone).offset().fix() != *instant.offset()
        {
            return Err(DbError::Invalid(
                "Event instant offset disagrees with timezone at that instant".into(),
            ));
        }
        return Ok(Some(
            i128::from(instant.timestamp()) * 1_000_000_000
                + i128::from(instant.timestamp_subsec_nanos()),
        ));
    }
    let expected = match precision {
        "year" => 4,
        "month" => 7,
        "day" => 10,
        _ => return Err(invalid()),
    };
    if value.len() != expected
        || !value.bytes().enumerate().all(|(i, c)| {
            if i == 4 || i == 7 {
                c == b'-'
            } else {
                c.is_ascii_digit()
            }
        })
    {
        return Err(invalid());
    }
    let year = value[..4].parse::<i32>().map_err(|_| invalid())?;
    let month = if expected >= 7 {
        value[5..7].parse::<u32>().map_err(|_| invalid())?
    } else {
        1
    };
    let day = if expected == 10 {
        value[8..10].parse::<u32>().map_err(|_| invalid())?
    } else {
        1
    };
    if year == 0 || NaiveDate::from_ymd_opt(year, month, day).is_none() {
        return Err(invalid());
    }
    Ok(Some(
        i128::from(year) * 10000 + i128::from(month) * 100 + i128::from(day),
    ))
}
#[cfg(test)]
mod tests {
    use super::*;
    fn range(a: &str, b: &str, p: &str) -> EventTime {
        EventTime {
            starts_at: Some(a.into()),
            starts_at_precision: Some(p.into()),
            ends_at: Some(b.into()),
            ends_at_precision: Some(p.into()),
            timezone: None,
        }
    }
    #[test]
    fn calendar_and_unknown() {
        assert!(EventTime::default().validate().is_ok());
        for (a, b, p) in [
            ("2026", "2027", "year"),
            ("2026-01", "2026-10", "month"),
            ("2024-02-29", "2024-03-01", "day"),
        ] {
            assert!(range(a, b, p).validate().is_ok());
            assert!(range(b, a, p).validate().is_err());
        }
        assert!(range("2026-02-29", "2026-03-01", "day").validate().is_err());
        assert!(range("2026-1", "2026-12", "month").validate().is_err());
        let mut v = range("2026", "2026-10", "year");
        v.ends_at_precision = Some("month".into());
        assert!(v.validate().is_err());
        v = EventTime::default();
        v.starts_at = Some("2026".into());
        assert!(v.validate().is_err());
    }
    #[test]
    fn instant_offsets_and_dst_roundtrip() {
        let mut v = range(
            "2026-10-08T14:00:00+11:00",
            "2026-10-08T03:00:00Z",
            "instant",
        );
        assert!(v.validate().is_ok());
        v.timezone = Some("Australia/Sydney".into());
        assert!(v.validate().is_err());
        v.ends_at = Some("2026-10-08T14:00:00+11:00".into());
        assert!(v.validate().is_ok());
        let json = serde_json::to_string(&v).unwrap();
        assert_eq!(serde_json::from_str::<EventTime>(&json).unwrap(), v);
        v.starts_at = Some("2026-07-08T14:00:00+11:00".into());
        assert!(v.validate().is_err());
        v.starts_at = Some("2026-07-08T14:00:00+10:00".into());
        assert!(v.validate().is_ok());
        v.starts_at = Some("2026-07-08T14:00:00".into());
        assert!(v.validate().is_err());
    }
}
