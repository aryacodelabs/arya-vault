//! A bounded JSON reader for untrusted import files (SEC-Y05).
//!
//! `serde_json::Value` would let a small file allocate far more memory than its size
//! (`[1,1,1,...]` needs one 32-byte node per two bytes of input). This module drives the
//! `serde_json` tokenizer with a visitor that enforces, *before* anything is stored:
//! maximum nesting depth, maximum string length, maximum elements per array/object and a
//! budget for the total number of values, and rejects duplicate object keys and trailing
//! data. Limit violations are reported as typed errors, not as free-form messages.

use std::cell::Cell;
use std::fmt;

use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};
use zeroize::Zeroize;

use super::{ImportError, ImportLimits};

/// A parsed JSON value. Numbers are kept as `f64` (all numbers in the supported formats are
/// small integers or flags).
#[derive(Clone, PartialEq)]
pub(crate) enum Json {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

impl Drop for Json {
    // Parsed text may hold passwords: wipe it when the tree is dropped (best effort; the
    // tokenizer's own scratch buffers are outside our control).
    fn drop(&mut self) {
        if let Json::Str(s) = self {
            s.zeroize();
        }
    }
}

impl fmt::Debug for Json {
    // Never print contents: values may be secrets.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Json::Null => f.write_str("Null"),
            Json::Bool(b) => write!(f, "Bool({b})"),
            Json::Num(_) => f.write_str("Num(..)"),
            Json::Str(s) => write!(f, "Str(<{} bytes>)", s.len()),
            Json::Arr(a) => write!(f, "Arr(<{} items>)", a.len()),
            Json::Obj(o) => write!(f, "Obj(<{} entries>)", o.len()),
        }
    }
}

impl Json {
    /// Looks up a key of an object.
    pub(crate) fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Obj(o) => o.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub(crate) fn as_str(&self) -> Option<&str> {
        match self {
            Json::Str(s) => Some(s),
            _ => None,
        }
    }

    pub(crate) fn as_array(&self) -> Option<&[Json]> {
        match self {
            Json::Arr(a) => Some(a),
            _ => None,
        }
    }

    pub(crate) fn as_bool(&self) -> Option<bool> {
        match self {
            Json::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// Integer value of a number that is exactly an integer in `i64` range.
    pub(crate) fn as_i64(&self) -> Option<i64> {
        match self {
            Json::Num(n) if n.fract() == 0.0 && n.abs() < 9.0e15 => Some(*n as i64),
            _ => None,
        }
    }
}

#[derive(Clone, Copy)]
enum Violation {
    Depth,
    TooLong,
    TooMany,
    DuplicateKey,
}

struct Budget {
    max_depth: usize,
    max_str: usize,
    max_items: usize,
    values_left: Cell<usize>,
    violation: Cell<Option<Violation>>,
}

impl Budget {
    fn fail<E: de::Error>(&self, v: Violation) -> E {
        self.violation.set(Some(v));
        E::custom("limit exceeded")
    }

    fn charge<E: de::Error>(&self) -> Result<(), E> {
        match self.values_left.get().checked_sub(1) {
            Some(n) => {
                self.values_left.set(n);
                Ok(())
            }
            None => Err(self.fail(Violation::TooMany)),
        }
    }
}

struct Seed<'a> {
    b: &'a Budget,
    depth: usize,
}

impl<'de> DeserializeSeed<'de> for Seed<'_> {
    type Value = Json;
    fn deserialize<D: de::Deserializer<'de>>(self, d: D) -> Result<Json, D::Error> {
        d.deserialize_any(Visit {
            b: self.b,
            depth: self.depth,
        })
    }
}

struct KeySeed<'a>(&'a Budget);

impl<'de> DeserializeSeed<'de> for KeySeed<'_> {
    type Value = String;
    fn deserialize<D: de::Deserializer<'de>>(self, d: D) -> Result<String, D::Error> {
        struct K<'a>(&'a Budget);
        impl Visitor<'_> for K<'_> {
            type Value = String;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("an object key")
            }
            fn visit_str<E: de::Error>(self, s: &str) -> Result<String, E> {
                if s.len() > self.0.max_str {
                    return Err(self.0.fail(Violation::TooLong));
                }
                Ok(s.to_owned())
            }
        }
        d.deserialize_str(K(self.0))
    }
}

struct Visit<'a> {
    b: &'a Budget,
    depth: usize,
}

impl<'de> Visitor<'de> for Visit<'_> {
    type Value = Json;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a JSON value")
    }

    fn visit_unit<E: de::Error>(self) -> Result<Json, E> {
        self.b.charge()?;
        Ok(Json::Null)
    }

    fn visit_bool<E: de::Error>(self, v: bool) -> Result<Json, E> {
        self.b.charge()?;
        Ok(Json::Bool(v))
    }

    fn visit_i64<E: de::Error>(self, v: i64) -> Result<Json, E> {
        self.b.charge()?;
        Ok(Json::Num(v as f64))
    }

    fn visit_u64<E: de::Error>(self, v: u64) -> Result<Json, E> {
        self.b.charge()?;
        Ok(Json::Num(v as f64))
    }

    fn visit_f64<E: de::Error>(self, v: f64) -> Result<Json, E> {
        self.b.charge()?;
        Ok(Json::Num(v))
    }

    fn visit_str<E: de::Error>(self, v: &str) -> Result<Json, E> {
        self.b.charge()?;
        // Checked before the copy is made.
        if v.len() > self.b.max_str {
            return Err(self.b.fail(Violation::TooLong));
        }
        Ok(Json::Str(v.to_owned()))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Json, A::Error> {
        self.b.charge()?;
        if self.depth >= self.b.max_depth {
            return Err(self.b.fail(Violation::Depth));
        }
        let mut out = Vec::new();
        while let Some(v) = seq.next_element_seed(Seed {
            b: self.b,
            depth: self.depth + 1,
        })? {
            if out.len() >= self.b.max_items {
                return Err(self.b.fail(Violation::TooMany));
            }
            out.push(v);
        }
        Ok(Json::Arr(out))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Json, A::Error> {
        self.b.charge()?;
        if self.depth >= self.b.max_depth {
            return Err(self.b.fail(Violation::Depth));
        }
        let mut out: Vec<(String, Json)> = Vec::new();
        while let Some(k) = map.next_key_seed(KeySeed(self.b))? {
            if out.len() >= self.b.max_items {
                return Err(self.b.fail(Violation::TooMany));
            }
            if out.iter().any(|(existing, _)| *existing == k) {
                return Err(self.b.fail(Violation::DuplicateKey));
            }
            let v = map.next_value_seed(Seed {
                b: self.b,
                depth: self.depth + 1,
            })?;
            out.push((k, v));
        }
        Ok(Json::Obj(out))
    }
}

/// Parses `bytes` (UTF-8, optional BOM) under `limits`.
pub(crate) fn parse(bytes: &[u8], limits: &ImportLimits) -> Result<Json, ImportError> {
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF][..]).unwrap_or(bytes);
    let budget = Budget {
        max_depth: limits.max_depth,
        max_str: limits.max_field_bytes,
        max_items: limits.max_records.max(1024),
        values_left: Cell::new(limits.max_json_values()),
        violation: Cell::new(None),
    };
    let mut de = serde_json::Deserializer::from_slice(bytes);
    let result = Seed {
        b: &budget,
        depth: 0,
    }
    .deserialize(&mut de)
    .and_then(|v| de.end().map(|()| v));
    match result {
        Ok(v) => Ok(v),
        Err(_) => Err(match budget.violation.get() {
            Some(Violation::Depth) => ImportError::TooDeeplyNested,
            Some(Violation::TooLong) => ImportError::FieldTooLarge,
            Some(Violation::TooMany) => ImportError::TooManyRecords,
            Some(Violation::DuplicateKey) => ImportError::InvalidJson,
            None => ImportError::InvalidJson,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lim() -> ImportLimits {
        ImportLimits::default()
    }

    #[test]
    fn parses_ordinary_documents() {
        let j = parse(br#"{"a":[1,true,null,"x"],"b":{"c":-2.5}}"#, &lim()).unwrap();
        assert_eq!(j.get("a").unwrap().as_array().unwrap().len(), 4);
        assert_eq!(j.get("b").unwrap().get("c"), Some(&Json::Num(-2.5)));
        assert_eq!(
            parse(b"\xEF\xBB\xBF[1]", &lim()).unwrap(),
            Json::Arr(vec![Json::Num(1.0)])
        );
    }

    #[test]
    fn rejects_malformed_documents() {
        for bad in [
            &b""[..],
            b"{",
            b"[1,]",
            b"{\"a\":1,}",
            b"tru",
            b"[1] x",
            b"{\"a\":1}{}",
            b"\"\xff\"",
            b"nan",
        ] {
            assert!(
                matches!(parse(bad, &lim()), Err(ImportError::InvalidJson)),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn duplicate_keys_are_rejected() {
        assert!(matches!(
            parse(br#"{"a":1,"a":2}"#, &lim()),
            Err(ImportError::InvalidJson)
        ));
    }

    #[test]
    fn depth_limit_is_enforced_without_deep_recursion() {
        let l = ImportLimits {
            max_depth: 16,
            ..ImportLimits::default()
        };
        let ok = format!("{}{}", "[".repeat(16), "]".repeat(16));
        assert!(parse(ok.as_bytes(), &l).is_ok());
        let bad = format!("{}{}", "[".repeat(17), "]".repeat(17));
        assert!(matches!(
            parse(bad.as_bytes(), &l),
            Err(ImportError::TooDeeplyNested)
        ));
        let huge = "[".repeat(1_000_000);
        assert!(matches!(
            parse(huge.as_bytes(), &l),
            Err(ImportError::TooDeeplyNested | ImportError::InvalidJson)
        ));
        let objs = "{\"a\":".repeat(100_000);
        assert!(matches!(
            parse(objs.as_bytes(), &l),
            Err(ImportError::TooDeeplyNested | ImportError::InvalidJson)
        ));
    }

    #[test]
    fn string_and_value_budgets_are_enforced() {
        let l = ImportLimits {
            max_field_bytes: 8,
            ..ImportLimits::default()
        };
        assert!(parse(br#"["12345678"]"#, &l).is_ok());
        assert!(matches!(
            parse(br#"["123456789"]"#, &l),
            Err(ImportError::FieldTooLarge)
        ));
        assert!(matches!(
            parse(br#"{"123456789":1}"#, &l),
            Err(ImportError::FieldTooLarge)
        ));
        // A flat array of tiny values must hit the value budget, not allocate millions of nodes.
        let l = ImportLimits {
            max_records: 10,
            ..ImportLimits::default()
        };
        let many = format!("[{}0]", "0,".repeat(100_000));
        assert!(matches!(
            parse(many.as_bytes(), &l),
            Err(ImportError::TooManyRecords)
        ));
    }
}
