//! `reason!` with a borrowed `&str` piece: input that skips `Quoted` must not compile.

// The stub mirrors every `crate::` import of `quoted.rs`; the value plays no part here.
mod limits {
    pub(crate) const MAX_QUOTED_CHARS: usize = 512;
}

#[path = "../../src/quoted.rs"]
mod quoted;

fn main() {
    let document = String::from("read from a document");
    let input: &str = &document;
    let _ = quoted::reason!["$ref '", input, "' does not resolve"];
}
