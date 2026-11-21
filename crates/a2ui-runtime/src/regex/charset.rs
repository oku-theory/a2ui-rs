//! Character sets as sorted, merged `char` ranges.

/// The highest Unicode code point.
const MAX_CODE_POINT: u32 = 0x10_FFFF;
/// The first UTF-16 surrogate, which no `char` holds.
const SURROGATE_START: u32 = 0xD800;
/// The last UTF-16 surrogate.
const SURROGATE_END: u32 = 0xDFFF;

/// A set of `char`s held as sorted, disjoint, non-adjacent inclusive ranges.
///
/// Two ranges either side of the surrogate block are adjacent: no `char` lies between them.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct CharSet {
    ranges: Vec<(char, char)>,
}

impl CharSet {
    /// The set holding no character.
    pub(crate) fn empty() -> Self {
        Self::default()
    }

    /// The set holding `c` alone.
    pub(crate) fn single(c: char) -> Self {
        Self::range(c, c)
    }

    /// The characters from `lo` to `hi` inclusive; empty when `lo > hi`.
    pub(crate) fn range(lo: char, hi: char) -> Self {
        Self::from_code_range(u32::from(lo), u32::from(hi))
    }

    /// The characters among the code points `lo` to `hi`, surrogates and values past U+10FFFF
    /// left out.
    pub(crate) fn from_code_range(lo: u32, hi: u32) -> Self {
        let mut ranges = Vec::new();
        push_code_range(&mut ranges, lo, hi);
        Self::normalized(ranges)
    }

    /// ASCII digits, the set of `\d`.
    pub(crate) fn digit() -> Self {
        Self::range('0', '9')
    }

    /// ASCII word characters, the set of `\w` and `\b`.
    pub(crate) fn word() -> Self {
        Self::from_pairs(&[('0', '9'), ('A', 'Z'), ('_', '_'), ('a', 'z')])
    }

    /// ECMAScript `WhiteSpace` and `LineTerminator`, the set of `\s`.
    pub(crate) fn space() -> Self {
        Self::from_pairs(&[
            ('\u{9}', '\u{D}'),
            (' ', ' '),
            ('\u{A0}', '\u{A0}'),
            ('\u{1680}', '\u{1680}'),
            ('\u{2000}', '\u{200A}'),
            ('\u{2028}', '\u{2029}'),
            ('\u{202F}', '\u{202F}'),
            ('\u{205F}', '\u{205F}'),
            ('\u{3000}', '\u{3000}'),
            ('\u{FEFF}', '\u{FEFF}'),
        ])
    }

    /// Every character but the four line terminators, the set of `.`.
    pub(crate) fn dot() -> Self {
        Self::from_pairs(&[('\n', '\n'), ('\r', '\r'), ('\u{2028}', '\u{2029}')]).negate()
    }

    /// The ranges of this set, sorted and disjoint.
    pub(crate) fn ranges(&self) -> &[(char, char)] {
        &self.ranges
    }

    /// Whether `c` belongs to this set.
    pub(crate) fn contains(&self, c: char) -> bool {
        self.ranges
            .binary_search_by(|&(lo, hi)| {
                if hi < c {
                    std::cmp::Ordering::Less
                } else if lo > c {
                    std::cmp::Ordering::Greater
                } else {
                    std::cmp::Ordering::Equal
                }
            })
            .is_ok()
    }

    /// Whether this set and `other` share a character.
    pub(crate) fn overlaps(&self, other: &Self) -> bool {
        let (mut mine, mut theirs) = (
            self.ranges.iter().peekable(),
            other.ranges.iter().peekable(),
        );
        while let (Some(&&(lo_a, hi_a)), Some(&&(lo_b, hi_b))) = (mine.peek(), theirs.peek()) {
            if lo_a <= hi_b && lo_b <= hi_a {
                return true;
            }
            if hi_a < hi_b {
                mine.next();
            } else {
                theirs.next();
            }
        }
        false
    }

    /// The characters in this set or in `other`.
    pub(crate) fn union(&self, other: &Self) -> Self {
        let ranges = self
            .ranges
            .iter()
            .chain(&other.ranges)
            .map(|&(lo, hi)| (u32::from(lo), u32::from(hi)))
            .collect();
        Self::normalized(ranges)
    }

    /// The characters not in this set.
    pub(crate) fn negate(&self) -> Self {
        let mut ranges = Vec::new();
        let mut next = 0_u32;
        for &(lo, hi) in &self.ranges {
            let lo = u32::from(lo);
            if lo > next {
                push_code_range(&mut ranges, next, lo - 1);
            }
            next = u32::from(hi) + 1;
        }
        push_code_range(&mut ranges, next, MAX_CODE_POINT);
        Self::normalized(ranges)
    }

    /// The union of the inclusive `pairs`.
    fn from_pairs(pairs: &[(char, char)]) -> Self {
        let ranges = pairs
            .iter()
            .map(|&(lo, hi)| (u32::from(lo), u32::from(hi)))
            .collect();
        Self::normalized(ranges)
    }

    /// Sorts and merges code point ranges free of surrogates into `char` ranges.
    ///
    /// Every bound given is a `char`'s code point; [`push_code_range`] guarantees it.
    fn normalized(mut ranges: Vec<(u32, u32)>) -> Self {
        ranges.sort_unstable();
        let mut merged: Vec<(u32, u32)> = Vec::with_capacity(ranges.len());
        for (lo, hi) in ranges {
            match merged.last_mut() {
                Some(last) if lo <= next_code_point(last.1) => last.1 = last.1.max(hi),
                _ => merged.push((lo, hi)),
            }
        }
        let ranges = merged
            .into_iter()
            .filter_map(|(lo, hi)| Some((char::from_u32(lo)?, char::from_u32(hi)?)))
            .collect();
        Self { ranges }
    }
}

/// The code point after `code`, skipping the surrogate block.
fn next_code_point(code: u32) -> u32 {
    if code == SURROGATE_START - 1 {
        SURROGATE_END + 1
    } else {
        code.saturating_add(1)
    }
}

/// Pushes the parts of `lo..=hi` that are `char`s: surrogates and values past U+10FFFF dropped.
fn push_code_range(ranges: &mut Vec<(u32, u32)>, lo: u32, hi: u32) {
    let hi = hi.min(MAX_CODE_POINT);
    if lo > hi {
        return;
    }
    if lo < SURROGATE_START {
        ranges.push((lo, hi.min(SURROGATE_START - 1)));
    }
    if hi > SURROGATE_END {
        ranges.push((lo.max(SURROGATE_END + 1), hi));
    }
}

#[cfg(test)]
#[path = "charset_tests.rs"]
mod tests;
