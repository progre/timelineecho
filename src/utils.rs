pub mod format_rfc3339 {
    use chrono::{DateTime, FixedOffset, SecondsFormat};
    use serde::{de, Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(date: &DateTime<FixedOffset>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&date.to_rfc3339_opts(SecondsFormat::Millis, true))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<DateTime<FixedOffset>, D::Error>
    where
        D: Deserializer<'de>,
    {
        DateTime::parse_from_rfc3339(&String::deserialize(deserializer)?).map_err(de::Error::custom)
    }
}
