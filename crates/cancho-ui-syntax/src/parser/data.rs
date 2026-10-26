//! The `data Name { .. }` declaration of the parser (docs/data.md section 2). Split from `parser.rs` (the 2,000-line ceiling).

use super::*;

impl Parser {
    /// `data Name { file: "data/x.json" sha256: ".."; }`; the current token is `data`.
    pub(super) fn parse_data_decl(&mut self) -> Result<DataDecl, ParseError> {
        let kw = self.bump();
        let (name, _) = self.bump_ident("the data's name")?;
        self.expect(Token::LBrace)?;
        let (word, word_span) = self.bump_ident("`file`")?;
        if word != "file" {
            return Err(unexpected(Token::Ident(word), word_span, "`file`"));
        }
        self.expect(Token::DotDot)?;
        let (file, _) = self.bump_string_lit("the path of the file, as a string")?;
        let (word, word_span) = self.bump_ident("`sha256`")?;
        if word != "sha256" {
            return Err(unexpected(Token::Ident(word), word_span, "`sha256`"));
        }
        self.expect(Token::DotDot)?;
        let (sha256, _) = self.bump_string_lit("the sha256 of the file, as a string")?;
        self.expect(Token::Semi)?;
        let close = self.expect(Token::RBrace)?;
        Ok(DataDecl {
            name,
            file,
            sha256,
            span: Span::new(kw.span.start, close.span.end),
        })
    }
}
