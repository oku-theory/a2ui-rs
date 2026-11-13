//! Load rule 4: the reference keywords a v1.0 catalog document may hold.

use serde_json::{Map, Value};
use snafu::ensure;

use super::position::find_data;
use super::walk::{REF_KEY, Segment, find_member_in, find_ref, pointer};
use super::{
    COMMON_TYPES_URIS, ENTRY_SECTIONS, RefPointer, is_pointer_fragment, is_supported_ref,
    pointer_of, written_ref,
};
use crate::error::{Error, ExternalRefSnafu, ForbiddenKeywordSnafu, MalformedCatalogSnafu};
use crate::pointer::BAD_ESCAPE;
use crate::quoted::{Quoted, reason};

/// The keywords the reference check reads when they hold a scalar.
const KEYWORDS: [&str; 7] = [
    "$anchor",
    "$dynamicAnchor",
    "$recursiveAnchor",
    "$dynamicRef",
    "$recursiveRef",
    "$id",
    "$schema",
];

const ANCHORS: &str = "anchors are not supported; use a JSON Pointer";
const DYNAMIC: &str = "dynamic references are not supported; use $ref";
const NESTED: &str = "only the catalog's top level may declare it";

/// Checks the reference keywords of a catalog document (load rule 4).
///
/// A key holding an object or array is walked whatever its name, so a property named `$ref`
/// holding a schema is a property. A scalar under a keyword is checked: `$ref` must be a
/// supported string with an empty or JSON Pointer fragment whose `~` escapes are RFC 6901's
/// (so every walk decodes its tokens alike, `serde_json`'s lenient lookup included), anchors
/// and dynamic references are refused, and `$id` and `$schema` may only sit at the top level.
/// A pointer must name the theme or a `$defs`, component or function entry (common types: a
/// `$defs` entry), or a node in one, since the validation schema keeps nothing else. A pointer
/// below a component or function entry (`#/components/A/properties`) is refused: the
/// validation schema rewrites those entries, so it would dangle there. The walk does not tell
/// schemas from data, so a `const` or `default` holding such a key is refused too; a `$ref` in
/// data must also be written as the validation schema writes it. A numeric `$anchor` or `$id`
/// is left to the schema build, whose meta-schema check rejects it.
///
/// `#` and `catalog.json#` name the whole document, which the validation schema does not keep,
/// so they are refused like `#/title`. Pointers are built only for an error. The document is at
/// most `MAX_JSON_DEPTH` deep; it walks with [`find_member_in`], which recurses.
pub(crate) fn check_references(document: &Map<String, Value>) -> Result<(), Error> {
    let first_error = find_member_in(document, &mut Vec::new(), &mut |key, value, path| {
        let checked = match value {
            Value::String(reference) if key == REF_KEY => check_ref(reference, path),
            _ if key == REF_KEY => Err(MalformedCatalogSnafu {
                reason: reason!["$ref at ", Quoted::new(&pointer(path)), " must be a string"],
            }
            .build()
            .into()),
            _ => check_keyword(key, value, path),
        };
        checked.err()
    });
    first_error.map_or_else(|| check_data_refs(document), Err)
}

/// Refuses a `$ref` in data that the runtime would rewrite.
///
/// Plan 2b, Decision 19. The theme and every `$defs`, component and function entry are
/// schemas, whose data [`find_data`] finds. To the validator a `$ref` there is part of a value,
/// which `const` and `enum` compare, so it must already be written as the runtime writes it:
/// [`written_ref`] in the theme and `$defs`, which the validation schema copies, and
/// [`localize_ref`](super::localize_ref) in components and functions, which are compiled from
/// their bundle and never see the theme.
fn check_data_refs(document: &Map<String, Value>) -> Result<(), Error> {
    let with_theme = document.contains_key("theme");
    let theme = document
        .get_key_value("theme")
        .map(|(key, theme)| (vec![Segment::Key(key)], theme, with_theme));
    let entries = ENTRY_SECTIONS
        .into_iter()
        .filter_map(|section| document.get_key_value(section))
        .filter_map(|(section, entries)| Some((section, entries.as_object()?)))
        .flat_map(|(section, entries)| {
            let retargeted = with_theme && section == "$defs";
            entries.iter().map(move |(name, entry)| {
                let path = vec![Segment::Key(section), Segment::Key(name)];
                (path, entry, retargeted)
            })
        });
    let first_error = theme
        .into_iter()
        .chain(entries)
        .find_map(|(mut path, root, retargeted)| {
            find_data(root, &mut path, &mut |data, at| {
                find_ref(data, &mut |reference, below| {
                    let written = written_ref(reference, retargeted);
                    (written != reference).then(|| {
                        let path: Vec<_> = at.iter().chain(below).copied().collect();
                        MalformedCatalogSnafu {
                            reason: reason![
                                "$ref '",
                                Quoted::new(reference),
                                "' at ",
                                Quoted::new(&pointer(&path)),
                                " is data, which the runtime would rewrite; write it '",
                                Quoted::new(&written),
                                "'",
                            ],
                        }
                        .build()
                        .into()
                    })
                })
            })
        });
    first_error.map_or(Ok(()), Err)
}

fn check_ref(reference: &str, path: &[Segment<'_>]) -> Result<(), Error> {
    ensure!(
        is_supported_ref(reference),
        ExternalRefSnafu {
            reference,
            pointer: pointer(path),
        }
    );
    let fragment = reference.split_once('#').map(|(_, fragment)| fragment);
    ensure!(
        fragment.is_some_and(is_pointer_fragment),
        MalformedCatalogSnafu {
            reason: reason![
                "$ref '",
                Quoted::new(reference),
                "' at ",
                Quoted::new(&pointer(path)),
                " has a fragment that is neither empty nor a JSON Pointer",
            ],
        }
    );
    let Some(decoded) = pointer_of(reference) else {
        return MalformedCatalogSnafu {
            reason: reason![
                "$ref '",
                Quoted::new(reference),
                "' at ",
                Quoted::new(&pointer(path)),
                " has a fragment that is not UTF-8",
            ],
        }
        .fail()
        .map_err(Into::into);
    };
    // Every token is decoded here, once: every reader takes its tokens from a `RefPointer`.
    let Some(target) = RefPointer::parse(&decoded) else {
        return MalformedCatalogSnafu {
            reason: reason![
                "$ref '",
                Quoted::new(reference),
                "' at ",
                Quoted::new(&pointer(path)),
                " has a JSON Pointer where ",
                BAD_ESCAPE,
            ],
        }
        .fail()
        .map_err(Into::into);
    };
    ensure!(
        is_kept_target(reference, &target),
        MalformedCatalogSnafu {
            reason: reason![
                "$ref '",
                Quoted::new(reference),
                "' at ",
                Quoted::new(&pointer(path)),
                " names no definition, entry or theme, which are the only parts of the catalog \
                 the validation schema keeps",
            ],
        }
    );
    ensure!(
        !is_inside_entry(&target),
        MalformedCatalogSnafu {
            reason: reason![
                "$ref '",
                Quoted::new(reference),
                "' at ",
                Quoted::new(&pointer(path)),
                " points inside a component or function, which the validation schema flattens; \
                 reference it whole or move the schema to $defs",
            ],
        }
    );
    Ok(())
}

/// Whether a supported `reference` to `pointer` names a node the validation schema keeps.
///
/// A common types reference must name a `$defs` entry or a node in one. A catalog reference
/// must name the theme, or a `$defs`, component or function entry or a node in one. The whole
/// document (`#`), a bare section (`#/$defs`) and any other top-level key (`#/title`) are not
/// kept as they are, so a `$ref` to them would dangle or change meaning there.
fn is_kept_target(reference: &str, pointer: &RefPointer) -> bool {
    let from_common_types = reference
        .split_once('#')
        .is_some_and(|(document, _)| COMMON_TYPES_URIS.contains(&document));
    let [section, rest @ ..] = pointer.tokens() else {
        return false;
    };
    if from_common_types {
        return section == "$defs" && !rest.is_empty();
    }
    section == "theme" || (ENTRY_SECTIONS.contains(&section.as_str()) && !rest.is_empty())
}

/// Whether `pointer` names a node below a component or function, such as `/components/A/x`.
///
/// The validation schema rewrites those entries (flattened components, closed function
/// `args`), so no stable node is left for such a pointer; `$defs` entries are copied as is.
fn is_inside_entry(pointer: &RefPointer) -> bool {
    matches!(
        pointer.tokens(),
        [section, _, _, ..] if section == "components" || section == "functions"
    )
}

/// `key` as one of the static [`KEYWORDS`], when it is one.
fn keyword(key: &str) -> Option<&'static str> {
    KEYWORDS.into_iter().find(|keyword| *keyword == key)
}

fn check_keyword(key: &str, value: &Value, path: &[Segment<'_>]) -> Result<(), Error> {
    let Some(name) = keyword(key) else {
        return Ok(());
    };
    let reason = match (name, value) {
        ("$anchor" | "$dynamicAnchor", Value::String(_)) | ("$recursiveAnchor", Value::Bool(_)) => {
            ANCHORS
        }
        ("$dynamicRef" | "$recursiveRef", Value::String(_)) => DYNAMIC,
        ("$id" | "$schema", Value::String(_)) if path.len() > 1 => NESTED,
        _ => return Ok(()),
    };
    Err(ForbiddenKeywordSnafu {
        keyword: name,
        pointer: pointer(path),
        reason,
    }
    .build()
    .into())
}

#[cfg(test)]
#[path = "check_tests.rs"]
mod tests;
