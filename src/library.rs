//! (pro) Saved calculation library, persisted in localStorage.
//!
//! Deliberately serde-free (payload size): each entry is hand-serialized
//! JSON with a fixed schema — `{id, name, kind, payload}` where `payload`
//! is the calc's input field values; loading re-runs the calc from them.

use web_sys::window;

const KEY: &str = "thermo-properties-library-v1";

/// One saved calculation.
#[derive(Debug, Clone)]
pub struct Entry {
    pub id: String,
    pub name: String,
    /// `"lookup" | "refcycle" | "rankine"`.
    pub kind: String,
    /// The calc's inputs as a JSON object string (field-name → value).
    pub payload: String,
}

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

fn storage() -> Option<web_sys::Storage> {
    window()?.local_storage().ok().flatten()
}

/// All saved calcs, oldest first.
pub fn list() -> Vec<Entry> {
    let Some(raw) = storage().and_then(|s| s.get_item(KEY).ok().flatten()) else {
        return Vec::new();
    };
    parse_library(&raw)
}

/// Appends a calc and returns its id.
pub fn save(name: &str, kind: &str, payload: &str) -> Option<String> {
    let mut all = list();
    let id = format!("c{}", now_millis());
    all.push(Entry {
        id: id.clone(),
        name: name.to_string(),
        kind: kind.to_string(),
        payload: payload.to_string(),
    });
    write_back(&all);
    Some(id)
}

pub fn delete(id: &str) {
    let all: Vec<Entry> = list().into_iter().filter(|e| e.id != id).collect();
    write_back(&all);
}

fn write_back(entries: &[Entry]) {
    let Some(storage) = storage() else {
        return;
    };
    let mut out = String::from("[");
    for (i, e) in entries.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&format!(
            "{{\"id\":\"{}\",\"name\":\"{}\",\"kind\":\"{}\",\"payload\":{}}}",
            json_escape(&e.id),
            json_escape(&e.name),
            json_escape(&e.kind),
            // payload is already a JSON object string
            e.payload
        ));
    }
    out.push(']');
    let _ = storage.set_item(KEY, &out);
}

fn parse_library(raw: &str) -> Vec<Entry> {
    // Minimal fixed-schema reader: walk the top-level array's objects and
    // pull the four known keys. Not a general JSON parser — the schema is
    // written by this same module, so the shape is trusted; malformed data
    // yields an empty library rather than a panic.
    let mut entries = Vec::new();
    let bytes: Vec<char> = raw.chars().collect();
    let mut i = 0;
    let n = bytes.len();
    let find_key = |i: &mut usize, key: &str| -> Option<String> {
        // Find `"key":` then read a JSON string OR a raw object at *i.
        let pat = format!("\"{key}\":");
        let pat: Vec<char> = pat.chars().collect();
        while *i + pat.len() <= n {
            if bytes[*i..*i + pat.len()] == pat[..] {
                *i += pat.len();
                break;
            }
            *i += 1;
        }
        if *i >= n {
            return None;
        }
        match bytes[*i] {
            '"' => {
                *i += 1;
                let mut out = String::new();
                while *i < n && bytes[*i] != '"' {
                    if bytes[*i] == '\\' && *i + 1 < n {
                        let esc = bytes[*i + 1];
                        out.push(match esc {
                            'n' => '\n',
                            'r' => '\r',
                            't' => '\t',
                            other => other,
                        });
                        *i += 2;
                    } else {
                        out.push(bytes[*i]);
                        *i += 1;
                    }
                }
                *i += 1; // closing quote
                Some(out)
            }
            '{' => {
                // Raw object: copy balanced braces verbatim.
                let start = *i;
                let mut depth = 0i32;
                while *i < n {
                    if bytes[*i] == '{' {
                        depth += 1;
                    } else if bytes[*i] == '}' {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    *i += 1;
                }
                *i += 1;
                Some(bytes[start..*i].iter().collect())
            }
            _ => None,
        }
    };

    while i < n {
        if bytes[i] == '{' {
            let obj_start = i;
            // Extract this object's span, then pull keys within it.
            let mut depth = 0i32;
            let mut j = i;
            while j < n {
                if bytes[j] == '{' {
                    depth += 1;
                } else if bytes[j] == '}' {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                j += 1;
            }
            let mut k = obj_start;
            let id = find_key(&mut k, "id").unwrap_or_default();
            let name = find_key(&mut k, "name").unwrap_or_default();
            let kind = find_key(&mut k, "kind").unwrap_or_default();
            let payload = find_key(&mut k, "payload").unwrap_or_default();
            if !id.is_empty() {
                entries.push(Entry { id, name, kind, payload });
            }
            i = j + 1;
        } else {
            i += 1;
        }
    }
    entries
}

fn now_millis() -> u64 {
    window()
        .and_then(|w| w.performance())
        .map(|p| p.now() as u64)
        .unwrap_or_else(|| std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0))
}

/// Extracts a string field from a payload JSON object (same mini-reader).
pub fn payload_field(payload: &str, key: &str) -> Option<String> {
    let bytes: Vec<char> = payload.chars().collect();
    let n = bytes.len();
    let mut i = 0;
    let pat = format!("\"{key}\":");
    let pat: Vec<char> = pat.chars().collect();
    while i + pat.len() <= n {
        if bytes[i..i + pat.len()] == pat[..] {
            i += pat.len();
            break;
        }
        i += 1;
    }
    if i >= n || bytes[i] != '"' {
        return None;
    }
    i += 1;
    let mut out = String::new();
    while i < n && bytes[i] != '"' {
        if bytes[i] == '\\' && i + 1 < n {
            out.push(bytes[i + 1]);
            i += 2;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    Some(out)
}

/// Builds a payload JSON object from `(key, value)` pairs.
pub fn payload(fields: &[(&str, &str)]) -> String {
    let mut out = String::from("{");
    for (i, (k, v)) in fields.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&format!("\"{}\":\"{}\"", json_escape(k), json_escape(v)));
    }
    out.push('}');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_entries() {
        let payload = payload(&[("fluid", "r134a"), ("t", "0")]);
        let lib = format!(
            "[{{\"id\":\"c1\",\"name\":\"Test calc\",\"kind\":\"lookup\",\"payload\":{}}}]",
            payload
        );
        let entries = parse_library(&lib);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "Test calc");
        assert_eq!(entries[0].kind, "lookup");
        assert_eq!(entries[0].payload, payload);
        assert_eq!(
            payload_field(&entries[0].payload, "fluid").as_deref(),
            Some("r134a")
        );
    }

    #[test]
    fn malformed_library_yields_empty() {
        assert!(parse_library("not json at all").is_empty());
        assert!(parse_library("[{broken").is_empty());
    }
}
