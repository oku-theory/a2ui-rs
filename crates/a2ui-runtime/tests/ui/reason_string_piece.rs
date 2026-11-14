//! `reason!` with a `String` piece: input that skips `Quoted` must not compile.

// The stub mirrors every `crate::` import of `quoted.rs`; the value plays no part here.
mod limits {
    pub(crate) const MAX_QUOTED_CHARS: usize = 512;
}

#[path = "../../src/quoted.rs"]
mod quoted;

fn main() {
    let input = String::from("read from a document");
    let _ = quoted::reason!["$ref '", input, "' does not resolve"];
}
