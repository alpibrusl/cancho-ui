//! `page { .. }` (docs/page.md section 2). Split from `parser.rs` (the 2,000-line ceiling); it is the same `Parser`, in a child module.

use super::*;

/// The most extra `key: "value"` pairs after the first value of an entry (`alternate` takes two).
const EXTRAS_MAX: usize = 4;

impl Parser {
    /// `page { key: "value"; key: 1200; alternate: ".." mime: ".." title: ".."; }`; the current token is `page`.
    pub(super) fn parse_page_decl(&mut self) -> Result<PageDecl, ParseError> {
        let kw = self.bump();
        self.expect(Token::LBrace)?;
        let mut entries = Vec::new();
        while !matches!(self.peek().token, Token::RBrace) {
            let (key, key_span) = self.bump_ident("a page setting")?;
            self.expect(Token::DotDot)?;
            let here = self.peek().clone();
            let value = match here.token {
                Token::StringLit(s) => {
                    self.bump();
                    PageValue::Text(s)
                }
                Token::IntLit(n) => {
                    self.bump();
                    PageValue::Int(n)
                }
                other => {
                    return Err(unexpected(
                        other,
                        here.span,
                        "a string literal or a whole number",
                    ));
                }
            };
            let mut extras = Vec::new();
            while let Token::Ident(_) = self.peek().token {
                if extras.len() >= EXTRAS_MAX {
                    break;
                }
                let (k, k_span) = self.bump_ident("a setting of this entry")?;
                self.expect(Token::DotDot)?;
                let (v, _) = self.bump_string_lit("a string literal")?;
                extras.push((k, v, k_span));
            }
            let semi = self.expect(Token::Semi)?;
            entries.push(PageEntry {
                key,
                value,
                extras,
                span: Span::new(key_span.start, semi.span.end),
            });
        }
        let close = self.expect(Token::RBrace)?;
        Ok(PageDecl {
            entries,
            span: Span::new(kw.span.start, close.span.end),
        })
    }
}
