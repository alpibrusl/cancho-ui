//! The interpreter's values and their JSON form (docs/replay.md §2.2).

use serde_json::{Map, Number, Value as Json};

/// An effect a handler can start. These are the only two `await` accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    NetSearch,
    NetGet,
    NetFetch,
    NetPost,
    LocationCurrent,
    Sleep,
}

impl Effect {
    pub fn name(self) -> &'static str {
        match self {
            Effect::NetSearch => "net.search",
            Effect::NetGet => "net.get",
            Effect::NetFetch => "net.fetch",
            Effect::NetPost => "net.post",
            Effect::LocationCurrent => "location.current",
            Effect::Sleep => "sleep",
        }
    }

    /// The cancellation group (the emitter's `path.join("_")`).
    pub fn group(self) -> &'static str {
        match self {
            Effect::NetSearch => "net_search",
            Effect::NetGet => "net_get",
            Effect::NetFetch => "net_fetch",
            Effect::NetPost => "net_post",
            Effect::LocationCurrent => "location_current",
            Effect::Sleep => "sleep",
        }
    }
}

/// A request that has been described but not started.
#[derive(Debug, Clone, PartialEq)]
pub struct Req {
    pub effect: Effect,
    pub args: Vec<Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// Both `Int` and `Float`: the emitted code is JavaScript (docs/replay.md §8).
    Num(f64),
    Str(String),
    Bool(bool),
    List(Vec<Value>),
    Ok(Box<Value>),
    Err(Box<Value>),
    Some(Box<Value>),
    None,
    Cancelled,
    Unit,
    Request(Req),
}

/// A JSON number for a finite double; integral values print without `.0`.
pub fn num_json(n: f64) -> Json {
    if n.is_finite() && n.fract() == 0.0 && n.abs() < 9_007_199_254_740_992.0 {
        Json::Number(Number::from(n as i64))
    } else {
        match Number::from_f64(n) {
            Some(x) => Json::Number(x),
            None => Json::Null,
        }
    }
}

impl Value {
    pub fn to_json(&self) -> Json {
        fn one(key: &str, v: Json) -> Json {
            let mut m = Map::new();
            m.insert(key.to_string(), v);
            Json::Object(m)
        }
        match self {
            Value::Num(n) => num_json(*n),
            Value::Str(s) => Json::String(s.clone()),
            Value::Bool(b) => Json::Bool(*b),
            Value::List(items) => Json::Array(items.iter().map(Value::to_json).collect()),
            Value::Ok(v) => one("Ok", v.to_json()),
            Value::Err(v) => one("Err", v.to_json()),
            Value::Some(v) => one("Some", v.to_json()),
            Value::None => one("None", Json::Null),
            Value::Cancelled => one("Cancelled", Json::Null),
            Value::Unit => Json::Null,
            Value::Request(r) => {
                let mut m = Map::new();
                m.insert("effect".into(), Json::String(r.effect.name().into()));
                m.insert(
                    "args".into(),
                    Json::Array(r.args.iter().map(Value::to_json).collect()),
                );
                one("Request", Json::Object(m))
            }
        }
    }

    /// The inverse of `to_json` for the values a trace may carry. `Err` is the
    /// reason a JSON value has no interpreter value.
    pub fn from_json(j: &Json) -> Result<Value, String> {
        match j {
            Json::Null => Ok(Value::Unit),
            Json::Bool(b) => Ok(Value::Bool(*b)),
            Json::Number(n) => match n.as_f64() {
                Some(f) if f.is_finite() => Ok(Value::Num(f)),
                _ => Err("a number that is not a finite double".into()),
            },
            Json::String(s) => Ok(Value::Str(s.clone())),
            Json::Array(items) => items
                .iter()
                .map(Value::from_json)
                .collect::<Result<Vec<_>, _>>()
                .map(Value::List),
            Json::Object(m) => {
                let mut entries = m.iter();
                let (Some((k, v)), None) = (entries.next(), entries.next()) else {
                    return Err(
                        "an object must be a variant with exactly one key: Ok, Err, Some, None or Cancelled"
                            .into(),
                    );
                };
                match k.as_str() {
                    "Ok" => Ok(Value::Ok(Box::new(Value::from_json(v)?))),
                    "Err" => Ok(Value::Err(Box::new(Value::from_json(v)?))),
                    "Some" => Ok(Value::Some(Box::new(Value::from_json(v)?))),
                    "None" if v.is_null() => Ok(Value::None),
                    "Cancelled" if v.is_null() => Ok(Value::Cancelled),
                    "None" | "Cancelled" => {
                        Err(format!("`{k}` carries no payload; write {{\"{k}\": null}}"))
                    }
                    other => Err(format!(
                        "`{other}` is not a variant; the variants are Ok, Err, Some, None, Cancelled"
                    )),
                }
            }
        }
    }

    /// The variant tag, for `match`.
    pub fn tag(&self) -> Option<&'static str> {
        match self {
            Value::Ok(_) => Some("Ok"),
            Value::Err(_) => Some("Err"),
            Value::Some(_) => Some("Some"),
            Value::None => Some("None"),
            Value::Cancelled => Some("Cancelled"),
            _ => None,
        }
    }

    pub fn payload(&self) -> Option<&Value> {
        match self {
            Value::Ok(v) | Value::Err(v) | Value::Some(v) => Some(v),
            _ => None,
        }
    }
}

/// How JavaScript prints a number, for the range where that is the shortest
/// round-trip decimal; `None` outside it (JavaScript switches to exponent
/// form there and replay does not imitate it).
pub fn js_number(n: f64) -> Option<String> {
    if n == 0.0 {
        return Some("0".into());
    }
    let a = n.abs();
    if n.is_finite() && (1e-6..1e21).contains(&a) {
        Some(format!("{n}"))
    } else {
        None
    }
}

/// `expected` is a subset of `actual`: every key of an expected object is
/// present and a subset; an expected array has the same length and matches
/// element by element; numbers compare as numbers (docs/replay.md §2.3).
pub fn subset(expected: &Json, actual: &Json) -> bool {
    match (expected, actual) {
        (Json::Object(e), Json::Object(a)) => e
            .iter()
            .all(|(k, ev)| a.get(k).is_some_and(|av| subset(ev, av))),
        (Json::Array(e), Json::Array(a)) => {
            e.len() == a.len() && e.iter().zip(a).all(|(x, y)| subset(x, y))
        }
        (Json::Number(e), Json::Number(a)) => e.as_f64() == a.as_f64(),
        (e, a) => e == a,
    }
}

#[cfg(test)]
mod tests {
    //! `Value::from_json` (#62): an object must be a variant with exactly one key. The count and the extraction are
    //! one operation now, so zero keys and two keys are answers (an `Err` text), and the variants still parse.
    use super::*;
    use serde_json::json;

    #[test]
    fn an_object_with_zero_or_two_keys_is_refused_with_the_same_text() {
        let none = Value::from_json(&json!({})).expect_err("zero keys");
        let two = Value::from_json(&json!({"Ok": 1, "Err": 2})).expect_err("two keys");
        assert_eq!(none, two);
        assert!(none.contains("exactly one key"), "{none}");
    }

    #[test]
    fn a_one_key_object_is_a_variant() {
        assert!(matches!(
            Value::from_json(&json!({"Ok": 1})),
            Ok(Value::Ok(_))
        ));
        assert!(matches!(
            Value::from_json(&json!({"Err": "x"})),
            Ok(Value::Err(_))
        ));
        assert!(matches!(
            Value::from_json(&json!({"Some": 1})),
            Ok(Value::Some(_))
        ));
        assert!(matches!(
            Value::from_json(&json!({"None": null})),
            Ok(Value::None)
        ));
        assert!(Value::from_json(&json!({"Other": 1})).is_err());
    }
}
