//! Hand-written Pratt (precedence-climbing) parser for the DSL (ADR 0046).
//!
//! No parser crate: the grammar is small and a Pratt parser keeps it local,
//! deterministic, and fully testable. Precedence (loosest → tightest):
//! `OR` < `AND` < `NOT` < comparison < `+ -` < `* /` < unary `-` < primary.

use super::ast::{BinOp, Expr, Func, UnaryOp};
use super::lexer::{tokenize, Token};
use super::ExprError;

pub fn parse(input: &str) -> Result<Expr, ExprError> {
    let tokens = tokenize(input)?;
    if tokens.is_empty() {
        return Err(ExprError::Parse("empty expression".to_owned()));
    }
    let mut parser = Parser { tokens, pos: 0 };
    let expr = parser.parse_expr(0)?;
    if parser.pos != parser.tokens.len() {
        return Err(ExprError::Parse(format!(
            "unexpected trailing input near token {}",
            parser.pos
        )));
    }
    Ok(expr)
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

/// Binding power of an infix operator (left, right). Higher binds tighter.
fn infix_binding_power(token: &Token) -> Option<(BinOp, u8, u8)> {
    let op = match token {
        Token::Ident(kw) if kw.eq_ignore_ascii_case("or") => return Some((BinOp::Or, 1, 2)),
        Token::Ident(kw) if kw.eq_ignore_ascii_case("and") => return Some((BinOp::And, 3, 4)),
        Token::Gte => BinOp::Gte,
        Token::Lte => BinOp::Lte,
        Token::Gt => BinOp::Gt,
        Token::Lt => BinOp::Lt,
        Token::Eq => BinOp::Eq,
        Token::Approx => BinOp::Approx,
        Token::Plus => BinOp::Add,
        Token::Minus => BinOp::Sub,
        Token::Star => BinOp::Mul,
        Token::Slash => BinOp::Div,
        _ => return None,
    };
    // Comparisons are non-associative-ish at level 5/6; arithmetic above them.
    let bp = match op {
        BinOp::Gte | BinOp::Lte | BinOp::Gt | BinOp::Lt | BinOp::Eq | BinOp::Approx => (5, 6),
        BinOp::Add | BinOp::Sub => (7, 8),
        BinOp::Mul | BinOp::Div => (9, 10),
        BinOp::And | BinOp::Or => unreachable!(),
    };
    Some((op, bp.0, bp.1))
}

impl Parser {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn advance(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.pos).cloned();
        if token.is_some() {
            self.pos += 1;
        }
        token
    }

    fn expect(&mut self, expected: &Token, context: &str) -> Result<(), ExprError> {
        match self.advance() {
            Some(ref t) if t == expected => Ok(()),
            other => Err(ExprError::Parse(format!(
                "expected {context}, found {other:?}"
            ))),
        }
    }

    /// Precedence-climbing core.
    fn parse_expr(&mut self, min_bp: u8) -> Result<Expr, ExprError> {
        let mut left = self.parse_prefix()?;

        while let Some(token) = self.peek() {
            // `NOT` is prefix-only; boolean keywords handled via infix table.
            let Some((op, l_bp, r_bp)) = infix_binding_power(token) else {
                break;
            };
            if l_bp < min_bp {
                break;
            }
            self.advance();
            let right = self.parse_expr(r_bp)?;
            left = Expr::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    fn parse_prefix(&mut self) -> Result<Expr, ExprError> {
        match self.advance() {
            Some(Token::Number(value)) => Ok(Expr::Number { value }),
            Some(Token::Percent(value)) => Ok(Expr::Percent { value }),
            Some(Token::Minus) => {
                let operand = self.parse_expr(11)?;
                Ok(Expr::Unary {
                    op: UnaryOp::Neg,
                    operand: Box::new(operand),
                })
            }
            Some(Token::LParen) => {
                let inner = self.parse_expr(0)?;
                self.expect(&Token::RParen, "closing ')'")?;
                Ok(inner)
            }
            Some(Token::Ident(ident)) => {
                if ident.eq_ignore_ascii_case("not") {
                    let operand = self.parse_expr(3)?;
                    return Ok(Expr::Unary {
                        op: UnaryOp::Not,
                        operand: Box::new(operand),
                    });
                }
                if ident.eq_ignore_ascii_case("and") || ident.eq_ignore_ascii_case("or") {
                    return Err(ExprError::Parse(format!(
                        "'{ident}' is a binary operator, not a value"
                    )));
                }
                // A function call if directly followed by '('.
                if self.peek() == Some(&Token::LParen) {
                    let func = Func::from_name(&ident.to_ascii_lowercase())
                        .ok_or_else(|| ExprError::Parse(format!("unknown function '{ident}'")))?;
                    self.advance(); // consume '('
                    let args = self.parse_args()?;
                    return Ok(Expr::Call { func, args });
                }
                Ok(Expr::Metric { key: ident })
            }
            other => Err(ExprError::Parse(format!(
                "expected a value, found {other:?}"
            ))),
        }
    }

    fn parse_args(&mut self) -> Result<Vec<Expr>, ExprError> {
        let mut args = Vec::new();
        if self.peek() == Some(&Token::RParen) {
            self.advance();
            return Ok(args);
        }
        loop {
            args.push(self.parse_expr(0)?);
            match self.advance() {
                Some(Token::Comma) => continue,
                Some(Token::RParen) => break,
                other => {
                    return Err(ExprError::Parse(format!(
                        "expected ',' or ')' in argument list, found {other:?}"
                    )))
                }
            }
        }
        Ok(args)
    }
}
