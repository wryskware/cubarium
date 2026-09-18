//! Exact persistence for float vectors.
//!
//! The brief requires the checkpoint to hold the **exact** central weights and Adam moments,
//! and the export to round-trip weight for weight. Plain JSON numbers do not deliver that
//! here: `serde_json` writes an `f64` with a shortest round-trip representation, but its
//! default **parser** is a fast approximate one that can land one unit in the last place away
//! from the written value. Measured before the fix: about 11% of arbitrary `f64` values came
//! back changed. This crate now enables `serde_json`'s `float_roundtrip` feature (Cargo.toml),
//! so plain numbers parse correctly rounded too; the hex encoding stays because exactness
//! should be a property of the format, not of a dependency feature someone can drop.
//!
//! One ULP is nothing to a weight and everything to a claim of exactness: a resumed run whose
//! centre differs in the last bit is not the run that was saved, and a test that asserts it is
//! would be asserting a near-miss. So every float vector this module persists is written as
//! **lowercase hex over little-endian IEEE-754 bytes** — which is also exactly the tensor
//! layout the interface contract §5 states for a policy's weights. The encoding is exact by
//! construction and does not depend on any JSON library's parser.
//!
//! 10,215 values encode to 163,440 hex characters (about 160 KiB), so a whole checkpoint —
//! centre plus both Adam moments — stays under half a megabyte.

use serde::Deserializer;
use serde::de::{Error, Unexpected};

/// `#[serde(with = "crate::es::bits::hex_f64s")]` on a `Vec<f64>`.
pub mod hex_f64s {
    use super::{decode, encode};
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(v: &[f64], s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&encode(v))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<f64>, D::Error> {
        let text = String::deserialize(d)?;
        decode::<D>(&text)
    }
}

/// Little-endian IEEE-754 bytes, lowercase hex, no separators.
pub fn encode(v: &[f64]) -> String {
    let mut out = String::with_capacity(v.len() * 16);
    for x in v {
        for b in x.to_le_bytes() {
            out.push(char::from_digit(u32::from(b >> 4), 16).expect("nibble"));
            out.push(char::from_digit(u32::from(b & 0xf), 16).expect("nibble"));
        }
    }
    out
}

pub(crate) fn decode<'de, D: Deserializer<'de>>(text: &str) -> Result<Vec<f64>, D::Error> {
    if !text.len().is_multiple_of(16) {
        return Err(D::Error::invalid_value(
            Unexpected::Str(&text[..text.len().min(32)]),
            &"a hex string whose length is a multiple of 16",
        ));
    }
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(text.len() / 16);
    for chunk in bytes.chunks(16) {
        let mut raw = [0u8; 8];
        for (i, pair) in chunk.chunks(2).enumerate() {
            let hi = nibble::<D>(pair[0])?;
            let lo = nibble::<D>(pair[1])?;
            raw[i] = hi << 4 | lo;
        }
        out.push(f64::from_le_bytes(raw));
    }
    Ok(out)
}

fn nibble<'de, D: Deserializer<'de>>(c: u8) -> Result<u8, D::Error> {
    (c as char)
        .to_digit(16)
        .map(|d| d as u8)
        .ok_or_else(|| D::Error::invalid_value(Unexpected::Char(c as char), &"a hex digit"))
}

/// Decode outside a `Deserializer`, for a caller holding a hex string directly.
pub fn decode_str(text: &str) -> Result<Vec<f64>, String> {
    decode::<&mut serde_json::Deserializer<serde_json::de::StrRead<'_>>>(text)
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Holder {
        #[serde(with = "hex_f64s")]
        v: Vec<f64>,
    }

    /// The defect this module was written for, stated as a test: with `float_roundtrip`
    /// enabled, plain JSON numbers now round-trip every `f64` on this build (the count below
    /// pins that the feature is on), and the hex encoding round-trips regardless.
    #[test]
    fn hex_round_trips_exactly_and_the_plain_parser_is_correctly_rounded() {
        let mut values = Vec::new();
        let mut x = 0.180_897_503_983_798_34f64;
        for i in 0..20_000u64 {
            x = (x * 1.000_000_1 + (i as f64) * 1e-9).fract();
            values.push(x);
        }
        values.extend([0.0, -0.0, 1.0, -1.0, f64::MIN_POSITIVE, f64::MAX, f64::MIN]);

        let plain_mismatches = values
            .iter()
            .filter(|x| {
                let s = serde_json::to_string(x).expect("write");
                let y: f64 = serde_json::from_str(&s).expect("read");
                y.to_bits() != x.to_bits()
            })
            .count();
        assert_eq!(
            plain_mismatches, 0,
            "serde_json's `float_roundtrip` feature must stay enabled in this crate's Cargo.toml: \
             without it about one f64 in nine parses one ULP off and `replay` is not exact"
        );

        let holder = Holder { v: values.clone() };
        let json = serde_json::to_string(&holder).expect("write");
        let back: Holder = serde_json::from_str(&json).expect("read");
        for (a, b) in values.iter().zip(&back.v) {
            assert_eq!(a.to_bits(), b.to_bits(), "hex must be exact: {a} vs {b}");
        }
    }

    #[test]
    fn the_encoding_is_little_endian_ieee_754() {
        assert_eq!(encode(&[1.0]), "000000000000f03f");
        assert_eq!(encode(&[]), "");
        assert_eq!(decode_str("000000000000f03f").expect("decode"), vec![1.0]);
    }

    #[test]
    fn a_malformed_string_is_refused() {
        assert!(decode_str("abc").is_err());
        assert!(decode_str("zzzzzzzzzzzzzzzz").is_err());
        assert!(serde_json::from_str::<Holder>(r#"{"v":"00"}"#).is_err());
    }

    #[test]
    fn nan_and_infinity_survive_the_encoding() {
        // Plain JSON cannot represent them at all; the byte encoding does not care.
        let v = vec![f64::NAN, f64::INFINITY, f64::NEG_INFINITY];
        let back = decode_str(&encode(&v)).expect("decode");
        assert!(back[0].is_nan());
        assert_eq!(back[1], f64::INFINITY);
        assert_eq!(back[2], f64::NEG_INFINITY);
    }
}
