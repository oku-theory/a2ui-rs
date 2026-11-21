//! The locale data the formatting functions read: plurals, separators, currencies.

use std::fmt::Debug;

/// A CLDR plural category, as `pluralize` names them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PluralCategory {
    /// The `zero` category.
    Zero,
    /// The `one` category.
    One,
    /// The `two` category.
    Two,
    /// The `few` category.
    Few,
    /// The `many` category.
    Many,
    /// The `other` category, which every locale has.
    Other,
}

/// How a currency is written: its symbol and on which side of the number.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurrencyFormat {
    /// The text written beside the number, spacing included (`"$"`, `"CHF "`).
    pub symbol: String,
    /// Whether the symbol comes before the number.
    pub prefix: bool,
}

/// The locale data the basic formatting functions follow.
///
/// Implementations must be pure: the same input always gives the same answer.
pub trait Locale: Send + Sync + Debug {
    /// The plural category of `n`.
    fn plural(&self, n: f64) -> PluralCategory;

    /// The character between integer and fraction digits.
    fn decimal_separator(&self) -> char;

    /// The character between groups of three integer digits.
    fn group_separator(&self) -> char;

    /// How an amount in currency `code` (ISO 4217) is written.
    fn currency(&self, code: &str) -> CurrencyFormat;

    /// The fraction digits shown when a call gives none, for `currency` if any.
    fn default_decimals(&self, currency: Option<&str>) -> u8;

    /// The marker of a 12-hour time: before noon, or after when `pm`.
    fn am_pm(&self, pm: bool) -> &str;
}

/// American English, as CLDR writes it.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct EnUs;

impl Locale for EnUs {
    /// `One` for exactly 1, else `Other`.
    fn plural(&self, n: f64) -> PluralCategory {
        if n == 1.0 {
            PluralCategory::One
        } else {
            PluralCategory::Other
        }
    }

    fn decimal_separator(&self) -> char {
        '.'
    }

    fn group_separator(&self) -> char {
        ','
    }

    /// `$`, `€`, `£` and `¥` for USD, EUR, GBP and JPY; else the code and a space. All prefixed.
    fn currency(&self, code: &str) -> CurrencyFormat {
        let symbol = match code {
            "USD" => "$".to_owned(),
            "EUR" => "€".to_owned(),
            "GBP" => "£".to_owned(),
            "JPY" => "¥".to_owned(),
            other => format!("{other} "),
        };
        CurrencyFormat {
            symbol,
            prefix: true,
        }
    }

    /// 0 without a currency and for JPY, else 2.
    fn default_decimals(&self, currency: Option<&str>) -> u8 {
        match currency {
            None | Some("JPY") => 0,
            Some(_) => 2,
        }
    }

    fn am_pm(&self, pm: bool) -> &str {
        if pm { "PM" } else { "AM" }
    }
}

#[cfg(test)]
#[path = "locale_tests.rs"]
mod tests;
