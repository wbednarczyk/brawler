//! Tokenizer for the criterion / formula DSL (ADR 0046).

use super::ExprError;
use rust_decimal::Decimal;
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Number(Decimal),
    Percent(Decimal),
    /// A bare identifier (metric key, function name, or boolean keyword).
    Ident(String),
    Plus,
    Minus,
    Star,
    Slash,
    Gte,
    Lte,
    Gt,
    Lt,
    Eq,
    Approx,
    LParen,
    RParen,
    Comma,
}

/// Tokenize `input` into a flat token stream. Identifiers and numbers are the
/// only multi-char lexemes; everything else is punctuation. Whitespace is
/// insignificant.
pub fn tokenize(input: &str) -> Result<Vec<Token>, ExprError> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];

        if c.is_whitespace() {
            i += 1;
            continue;
        }

        match c {
            '+' => {
                tokens.push(Token::Plus);
                i += 1;
            }
            '-' => {
                tokens.push(Token::Minus);
                i += 1;
            }
            '*' => {
                tokens.push(Token::Star);
                i += 1;
            }
            '/' => {
                tokens.push(Token::Slash);
                i += 1;
            }
            '(' => {
                tokens.push(Token::LParen);
                i += 1;
            }
            ')' => {
                tokens.push(Token::RParen);
                i += 1;
            }
            ',' => {
                tokens.push(Token::Comma);
                i += 1;
            }
            '>' => {
                if chars.get(i + 1) == Some(&'=') {
                    tokens.push(Token::Gte);
                    i += 2;
                } else {
                    tokens.push(Token::Gt);
                    i += 1;
                }
            }
            '<' => {
                if chars.get(i + 1) == Some(&'=') {
                    tokens.push(Token::Lte);
                    i += 2;
                } else {
                    tokens.push(Token::Lt);
                    i += 1;
                }
            }
            '=' => {
                if chars.get(i + 1) == Some(&'=') {
                    tokens.push(Token::Eq);
                    i += 2;
                } else {
                    // A single '=' is accepted as equality for ergonomics.
                    tokens.push(Token::Eq);
                    i += 1;
                }
            }
            // `~=` is the only token starting with `~`; a bare `~` falls through
            // to the catch-all below and is rejected as an unexpected character.
            '~' if chars.get(i + 1) == Some(&'=') => {
                tokens.push(Token::Approx);
                i += 2;
            }
            _ if c.is_ascii_digit() || c == '.' => {
                let start = i;
                while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                    i += 1;
                }
                let raw: String = chars[start..i].iter().collect();
                let value = Decimal::from_str(&raw)
                    .map_err(|_| ExprError::Lex(format!("invalid number '{raw}'")))?;
                if i < chars.len() && chars[i] == '%' {
                    i += 1;
                    tokens.push(Token::Percent(value));
                } else {
                    tokens.push(Token::Number(value));
                }
            }
            _ if is_ident_start(c) => {
                let start = i;
                while i < chars.len() && is_ident_part(chars[i]) {
                    i += 1;
                }
                let ident: String = chars[start..i].iter().collect();
                tokens.push(Token::Ident(ident));
            }
            _ => {
                return Err(ExprError::Lex(format!("unexpected character '{c}'")));
            }
        }
    }

    Ok(tokens)
}

fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}

fn is_ident_part(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}
