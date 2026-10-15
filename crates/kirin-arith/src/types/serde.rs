//! Exact JSON representations for values that JSON numbers cannot preserve.
macro_rules! float_bits {
    ($name:ident, $float:ty, $bits:ty) => {
        pub(super) mod $name {
            use serde::{Deserialize, Serialize};
            pub fn serialize<S: serde::Serializer>(v: &$float, s: S) -> Result<S::Ok, S::Error> {
                v.to_bits().serialize(s)
            }
            pub fn deserialize<'de, D: serde::Deserializer<'de>>(d: D) -> Result<$float, D::Error> {
                <$bits>::deserialize(d).map(<$float>::from_bits)
            }
        }
    };
}
macro_rules! decimal {
    ($name:ident, $integer:ty) => {
        pub(super) mod $name {
            use serde::{Deserialize, Serialize};
            pub fn serialize<S: serde::Serializer>(v: &$integer, s: S) -> Result<S::Ok, S::Error> {
                v.to_string().serialize(s)
            }
            pub fn deserialize<'de, D: serde::Deserializer<'de>>(
                d: D,
            ) -> Result<$integer, D::Error> {
                String::deserialize(d)?
                    .parse()
                    .map_err(serde::de::Error::custom)
            }
        }
    };
}
float_bits!(f32_bits, f32, u32);
float_bits!(f64_bits, f64, u64);
decimal!(i128_decimal, i128);
decimal!(u128_decimal, u128);
