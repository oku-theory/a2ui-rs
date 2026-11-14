//! Schema positions: the keywords holding subschemas, and the ones holding data.
//!
//! In JSON Schema 2020-12 only keywords known to take subschemas hold schemas (core §9.4.2),
//! and `const`, `enum`, `default` and `examples` hold instances (validation §6.1.2, §6.1.3,
//! §9.2, §9.5). The draft-7 forms (`dependencies`, an array `items`) are not schema positions
//! here. Describing common types walks the tables (plan 2b, Decision 6); finding data reads
//! only the name maps, so a keyword they miss cannot hide data (Decision 19).

use serde_json::Value;

use super::walk::Segment;

/// Keywords whose value maps names to subschemas.
pub(crate) const SCHEMA_MAPS: [&str; 4] = [
    "properties",
    "patternProperties",
    "$defs",
    "dependentSchemas",
];

/// Keywords whose value is a list of subschemas.
pub(crate) const SCHEMA_LISTS: [&str; 4] = ["allOf", "anyOf", "oneOf", "prefixItems"];

/// Keywords whose value is one subschema.
pub(crate) const SCHEMA_ONE: [&str; 10] = [
    "items",
    "not",
    "if",
    "then",
    "else",
    "additionalProperties",
    "unevaluatedProperties",
    "unevaluatedItems",
    "contains",
    "propertyNames",
];

/// Keywords whose value is an instance, not a schema.
const DATA: [&str; 4] = ["const", "enum", "default", "examples"];

/// Calls `visit` on each data value under `schema`, until one returns `Some`.
///
/// It fails toward finding too much: every object and array is walked, and a data keyword is
/// data wherever it sits, except as a name under a [`SCHEMA_MAPS`] keyword (a property named
/// `const`). A node outside a schema position may still be one, through a `$ref` to it or a
/// keyword the tables miss (`dependencies`), so the walk does not trust the tables to tell.
/// Data is not walked further: `visit` gets each value and its path, `path` being the path to
/// `schema`. It recurses once per level, so `schema` must be at most `MAX_JSON_DEPTH` deep
/// (load rule 1 checks it).
pub(super) fn find_data<'a, T>(
    schema: &'a Value,
    path: &mut Vec<Segment<'a>>,
    visit: &mut impl FnMut(&'a Value, &[Segment<'a>]) -> Option<T>,
) -> Option<T> {
    match schema {
        Value::Object(map) => map.iter().find_map(|(key, value)| {
            path.push(Segment::Key(key));
            let found = match value {
                _ if DATA.contains(&key.as_str()) => visit(value, path),
                Value::Object(names) if SCHEMA_MAPS.contains(&key.as_str()) => {
                    names.iter().find_map(|(name, named)| {
                        path.push(Segment::Key(name));
                        let found = find_data(named, path, visit);
                        path.pop();
                        found
                    })
                }
                _ => find_data(value, path, visit),
            };
            path.pop();
            found
        }),
        Value::Array(items) => items.iter().enumerate().find_map(|(index, item)| {
            path.push(Segment::Index(index));
            let found = find_data(item, path, visit);
            path.pop();
            found
        }),
        _ => None,
    }
}

#[cfg(test)]
#[path = "position_tests.rs"]
mod tests;
