use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

pub fn current_utc_compact_timestamp() -> String {
    format_compact_timestamp(OffsetDateTime::now_utc())
}

pub fn current_utc_date() -> String {
    format_utc_date(OffsetDateTime::now_utc())
}

pub fn current_utc_rfc3339() -> String {
    format_utc_rfc3339(OffsetDateTime::now_utc())
}

pub fn format_compact_timestamp(datetime: OffsetDateTime) -> String {
    format!(
        "{:04}{:02}{:02}-{:02}{:02}{:02}Z",
        datetime.year(),
        u8::from(datetime.month()),
        datetime.day(),
        datetime.hour(),
        datetime.minute(),
        datetime.second()
    )
}

pub fn format_utc_date(datetime: OffsetDateTime) -> String {
    format!(
        "{:04}-{:02}-{:02}",
        datetime.year(),
        u8::from(datetime.month()),
        datetime.day()
    )
}

pub fn format_utc_rfc3339(datetime: OffsetDateTime) -> String {
    datetime
        .format(&Rfc3339)
        .expect("Rfc3339 formatting should succeed")
}
