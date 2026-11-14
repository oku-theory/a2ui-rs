//! `reason!` with a `Reason` piece: nested wording would escape every length bound.

// The stub mirrors every `crate::` import of `quoted.rs`; the value plays no part here.
mod limits {
    pub(crate) const MAX_QUOTED_CHARS: usize = 512;
}

#[path = "../../src/quoted.rs"]
mod quoted;

fn main() {
    let inner = quoted::reason!["fixed ", quoted::Quoted::new("input")];
    let _ = quoted::reason!["outer: ", inner];
}
