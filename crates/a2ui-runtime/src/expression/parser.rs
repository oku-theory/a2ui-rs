//! Recursive-descent parser for templates, interpolations and calls.

use serde_json::Value;
use unicode_ident::is_xid_continue;

use super::{Expr, MAX_DEPTH, number, syntax};
use crate::error::Error;
use crate::ident::is_identifier;

const DELIMITERS: [char; 7] = ['(', ')', ',', ':', '\'', '"', '}'];

/// Cursor over the template's characters.
pub(super) struct Parser {
    chars: Vec<char>,
    pos: usize,
}

impl Parser {
    pub(super) fn new(input: &str) -> Self {
        Self {
            chars: input.chars().collect(),
            pos: 0,
        }
    }

    /// Parses the whole input as a template.
    pub(super) fn template(mut self) -> Result<Vec<Expr>, Error> {
        let mut parts = Vec::new();
        let mut text = String::new();
        while let Some(c) = self.peek() {
            if self.at("\\${") {
                text.push_str("${");
                self.pos += 3;
            } else if self.at("${") {
                match self.interpolation(1)? {
                    Expr::Literal(Value::String(literal)) => text.push_str(&literal),
                    expr => {
                        flush(&mut parts, &mut text);
                        parts.push(expr);
                    }
                }
            } else {
                text.push(c);
                self.pos += 1;
            }
        }
        flush(&mut parts, &mut text);
        Ok(parts)
    }

    /// Parses `${ expr }`, the cursor being on `$`.
    fn interpolation(&mut self, depth: usize) -> Result<Expr, Error> {
        if depth > MAX_DEPTH {
            return Err(syntax(self.pos, "Max recursion depth exceeded"));
        }
        let open = self.pos;
        self.pos += 2;
        let expr = self.value(depth)?;
        self.skip_ws();
        match self.peek() {
            Some('}') => {
                self.pos += 1;
                Ok(expr)
            }
            None => Err(syntax(open, "Unclosed interpolation")),
            Some(_) => Err(syntax(self.pos, "Unexpected characters")),
        }
    }

    fn value(&mut self, depth: usize) -> Result<Expr, Error> {
        if depth > MAX_DEPTH {
            return Err(syntax(self.pos, "Max recursion depth exceeded"));
        }
        self.skip_ws();
        match self.peek() {
            Some('$') if self.at("${") => self.interpolation(depth + 1),
            Some(quote @ ('\'' | '"')) => {
                self.string(quote).map(|s| Expr::Literal(Value::String(s)))
            }
            _ => {
                let start = self.pos;
                let token = self.token();
                if number::looks_numeric(&token) {
                    // The literal is ASCII, so its byte length is its char count;
                    // whatever follows it is left for the caller to reject.
                    let len = number::literal_len(&token);
                    self.pos = start + len;
                    return number::parse(&token[..len], start).map(Expr::Literal);
                }
                if token.is_empty() {
                    return Err(match self.peek() {
                        None => syntax(start, "Unclosed interpolation"),
                        Some(_) => syntax(start, "Unexpected characters"),
                    });
                }
                self.skip_ws();
                if self.peek() == Some('(') {
                    return self.call(token, start, depth);
                }
                classify(&token, start)
            }
        }
    }

    fn call(&mut self, name: String, start: usize, depth: usize) -> Result<Expr, Error> {
        let bare = name.strip_prefix('@').unwrap_or(&name);
        if !is_identifier(bare) {
            return Err(syntax(start, "Unexpected characters"));
        }
        self.pos += 1;
        let mut args = Vec::new();
        loop {
            self.skip_ws();
            if self.eat(')') {
                return Ok(Expr::Call { name, args });
            }
            let arg_start = self.pos;
            let arg = self.token();
            self.skip_ws();
            if !is_identifier(&arg) || !self.eat(':') {
                return Err(syntax(arg_start, "Expected ':' after argument name"));
            }
            let value = self.value(depth + 1)?;
            args.push((arg, value));
            self.skip_ws();
            if self.eat(',') {
                continue;
            }
            if self.eat(')') {
                return Ok(Expr::Call { name, args });
            }
            return Err(syntax(
                self.pos,
                "Expected ',' or ')' after function arguments",
            ));
        }
    }

    fn string(&mut self, quote: char) -> Result<String, Error> {
        let start = self.pos;
        self.pos += 1;
        let mut out = String::new();
        loop {
            let Some(c) = self.next() else {
                return Err(syntax(start, "Unterminated string literal"));
            };
            match c {
                '\\' => match self.next() {
                    Some('n') => out.push('\n'),
                    Some('t') => out.push('\t'),
                    Some('r') => out.push('\r'),
                    Some(other) => out.push(other),
                    None => return Err(syntax(start, "Unterminated string literal")),
                },
                c if c == quote => return Ok(out),
                c => out.push(c),
            }
        }
    }

    fn token(&mut self) -> String {
        let start = self.pos;
        while self
            .peek()
            .is_some_and(|c| !c.is_whitespace() && !DELIMITERS.contains(&c))
        {
            self.pos += 1;
        }
        self.chars[start..self.pos].iter().collect()
    }

    fn skip_ws(&mut self) {
        while self.peek().is_some_and(char::is_whitespace) {
            self.pos += 1;
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn next(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.pos += 1;
        Some(c)
    }

    fn eat(&mut self, expected: char) -> bool {
        let found = self.peek() == Some(expected);
        if found {
            self.pos += 1;
        }
        found
    }

    fn at(&self, prefix: &str) -> bool {
        prefix
            .chars()
            .enumerate()
            .all(|(i, c)| self.chars.get(self.pos + i) == Some(&c))
    }
}

fn classify(token: &str, start: usize) -> Result<Expr, Error> {
    match token {
        "true" => Ok(Expr::Literal(Value::Bool(true))),
        "false" => Ok(Expr::Literal(Value::Bool(false))),
        "null" => Ok(Expr::Literal(Value::String(String::new()))),
        _ if token
            .chars()
            .all(|c| is_xid_continue(c) || matches!(c, '/' | '.' | '-' | '~')) =>
        {
            Ok(Expr::Path(token.to_owned()))
        }
        _ => Err(syntax(start, "Unexpected characters")),
    }
}

fn flush(parts: &mut Vec<Expr>, text: &mut String) {
    if !text.is_empty() {
        parts.push(Expr::Literal(Value::String(std::mem::take(text))));
    }
}

#[cfg(test)]
#[path = "parser_tests.rs"]
mod tests;
