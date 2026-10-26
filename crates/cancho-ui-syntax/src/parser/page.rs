//! The `page { .. }` declaration of the parser (docs/page.md section 3). Split from `parser.rs` (the 2,000-line ceiling); it is the same `Parser`,
//! in a child module.

use super::*;

impl Parser {
    /// `page { title: ".."; image: ".." alt: ".."; .. }`; the current token is `page`. An entry is one or more `key: "string"` pairs ended by `;`.
    pub(super) fn parse_page_decl(&mut self) -> Result<PageDecl, ParseError> {
        let kw = self.bump();
        self.expect(Token::LBrace)?;
        let mut entries = Vec::new();
        while !matches!(self.peek().token, Token::RBrace) {
            let mut pairs = Vec::new();
            let first = self.peek().span.start;
            loop {
                let (key, key_span) = self.bump_ident("the name of an entry of the page")?;
                self.expect(Token::DotDot)?;
                let (value, value_span) = self.bump_string_lit("a string literal")?;
                pairs.push((key, value, Span::new(key_span.start, value_span.end)));
                if matches!(self.peek().token, Token::Semi) {
                    break;
                }
            }
            let semi = self.expect(Token::Semi)?;
            entries.push(PageEntry {
                pairs,
                span: Span::new(first, semi.span.end),
            });
        }
        let close = self.expect(Token::RBrace)?;
        Ok(PageDecl {
            entries,
            span: Span::new(kw.span.start, close.span.end),
        })
    }
}
