#[macro_export]
macro_rules! define_type_enum {
    ($name:ident { $($y:ident),+ $(,)? }) => {
        #[derive(
            strum::EnumIter,
            strum::EnumString,
            strum::IntoStaticStr,
            Debug,
            Clone,
            Copy,
            Serialize,
            PartialEq,
            Eq,
            PartialOrd,
            Ord,
        )]
        #[strum(serialize_all = "snake_case")]
        #[strum(ascii_case_insensitive)]
        pub enum $name {
            $(
                $y
            ),+
        }

        impl<'de> serde::de::Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                let buf = <&str>::deserialize(deserializer)?;
                <$name as std::str::FromStr>::from_str(buf).map_err(serde::de::Error::custom)
            }
        }
    };
}
