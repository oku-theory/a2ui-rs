//! The surface data model: one JSON document read and written by pointer.

use serde_json::{Map, Value};

use crate::error::{Error, IndexOutOfRangeSnafu, NonNumericSegmentSnafu, TraversePrimitiveSnafu};
use crate::pointer::{ListIndex, Pointer, list_index};

/// A JSON document owned by one surface, addressed by [`Pointer`].
#[derive(Debug, Clone, PartialEq)]
pub struct DataModel {
    root: Value,
}

/// A committed change: the written path and the value it held before.
#[derive(Debug, Clone, PartialEq)]
pub struct Write {
    path: Pointer,
    before: Option<Value>,
}

impl Default for DataModel {
    fn default() -> Self {
        Self {
            root: Value::Object(Map::new()),
        }
    }
}

impl DataModel {
    /// A model holding `root`; a `null` root becomes an empty object.
    pub fn new(root: Value) -> Self {
        if root.is_null() {
            Self::default()
        } else {
            Self { root }
        }
    }

    /// The value at `path`; a missing or `null` value is `None`.
    pub fn get(&self, path: &Pointer) -> Option<&Value> {
        descend(&self.root, path.segments())
    }

    /// Writes `value` at `path`, creating missing containers on the way.
    ///
    /// A `null` value deletes. Returns `None` when nothing changed.
    pub fn set(&mut self, path: &Pointer, value: Value) -> Result<Option<Write>, Error> {
        if value.is_null() {
            return self.delete(path);
        }
        let before = self.get(path).cloned();
        if before.as_ref() == Some(&value) {
            return Ok(None);
        }
        self.check_writable(path)?;
        match path.segments().split_last() {
            None => self.root = value,
            Some((last, parents)) => {
                let mut node = &mut self.root;
                for (depth, segment) in parents.iter().enumerate() {
                    let list_next = index_like(&path.segments()[depth + 1]);
                    node = container(slot(node, segment, path)?, list_next);
                }
                *slot(node, last, path)? = value;
            }
        }
        Ok(Some(Write {
            path: path.clone(),
            before,
        }))
    }

    /// Removes the value at `path`; deleting the root resets it to `{}`.
    pub fn delete(&mut self, path: &Pointer) -> Result<Option<Write>, Error> {
        let Some(before) = self.get(path).cloned() else {
            return Ok(None);
        };
        let write = Write {
            path: path.clone(),
            before: Some(before),
        };
        let Some((last, parents)) = path.segments().split_last() else {
            self.root = Value::Object(Map::new());
            return Ok(Some(write));
        };
        let mut node = &mut self.root;
        for segment in parents {
            node = slot(node, segment, path)?;
        }
        match node {
            Value::Object(map) => {
                map.remove(last);
            }
            other => *slot(other, last, path)? = Value::Null,
        }
        Ok(Some(write))
    }

    /// Fails like `set` would, without changing anything.
    fn check_writable(&self, path: &Pointer) -> Result<(), Error> {
        let mut node = Some(&self.root);
        for segment in path.segments() {
            node = match node {
                Some(Value::Object(map)) => {
                    map.get(segment.as_str()).filter(|value| !value.is_null())
                }
                Some(Value::Array(items)) => match list_index(segment) {
                    ListIndex::At(index) => items.get(index).filter(|value| !value.is_null()),
                    ListIndex::TooLarge => return Err(out_of_range(path, segment)),
                    ListIndex::NotIndex => return Err(non_numeric(path, segment)),
                },
                Some(_) => return Err(primitive(path, segment)),
                None if list_index(segment) == ListIndex::TooLarge => {
                    return Err(out_of_range(path, segment));
                }
                None => None,
            };
        }
        Ok(())
    }
}

impl Write {
    /// The pointer that was written.
    pub fn path(&self) -> &Pointer {
        &self.path
    }

    /// Whether the value at `watched` differs before and after this write.
    pub fn changes(&self, watched: &Pointer, model: &DataModel) -> bool {
        if watched.starts_with(&self.path) {
            let suffix = &watched.segments()[self.path.segments().len()..];
            let before = self
                .before
                .as_ref()
                .and_then(|value| descend(value, suffix));
            before != model.get(watched)
        } else if self.path.starts_with(watched) {
            self.before.as_ref() != model.get(&self.path)
        } else {
            false
        }
    }
}

fn descend<'a>(mut node: &'a Value, segments: &[String]) -> Option<&'a Value> {
    for segment in segments {
        node = match node {
            Value::Object(map) => map.get(segment.as_str())?,
            Value::Array(items) => match list_index(segment) {
                ListIndex::At(index) => items.get(index)?,
                ListIndex::TooLarge | ListIndex::NotIndex => return None,
            },
            _ => return None,
        };
    }
    (!node.is_null()).then_some(node)
}

fn index_like(segment: &str) -> bool {
    list_index(segment) != ListIndex::NotIndex
}

/// The child slot for `segment`, created as `null` when missing.
fn slot<'a>(node: &'a mut Value, segment: &str, path: &Pointer) -> Result<&'a mut Value, Error> {
    match node {
        Value::Object(map) => Ok(map.entry(segment.to_owned()).or_insert(Value::Null)),
        Value::Array(items) => {
            let index = match list_index(segment) {
                ListIndex::At(index) => index,
                ListIndex::TooLarge => return Err(out_of_range(path, segment)),
                ListIndex::NotIndex => return Err(non_numeric(path, segment)),
            };
            if items.len() <= index {
                items.resize(index + 1, Value::Null);
            }
            Ok(&mut items[index])
        }
        _ => Err(primitive(path, segment)),
    }
}

/// Turns an empty slot into a list or object; `check_writable` ruled out primitives.
fn container(slot: &mut Value, list: bool) -> &mut Value {
    if slot.is_null() {
        *slot = if list {
            Value::Array(Vec::new())
        } else {
            Value::Object(Map::new())
        };
    }
    slot
}

fn primitive(path: &Pointer, segment: &str) -> Error {
    TraversePrimitiveSnafu {
        path: path.to_string(),
        segment,
    }
    .build()
    .into()
}

fn non_numeric(path: &Pointer, segment: &str) -> Error {
    NonNumericSegmentSnafu {
        path: path.to_string(),
        segment,
    }
    .build()
    .into()
}

fn out_of_range(path: &Pointer, segment: &str) -> Error {
    IndexOutOfRangeSnafu {
        path: path.to_string(),
        segment,
    }
    .build()
    .into()
}

#[cfg(test)]
#[path = "data_tests.rs"]
mod tests;
