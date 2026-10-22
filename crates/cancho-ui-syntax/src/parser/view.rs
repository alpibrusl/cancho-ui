//! The view block of the parser: `view { .. }`, its elements, `For` and `If`, text bodies and the bindings of an element.
//! Split from `parser.rs` (the 2,000-line ceiling); it is the same `Parser`, in a child module.

use super::*;

impl Parser {
    pub(super) fn parse_view_clause(&mut self) -> Result<ViewBlock, ParseError> {
        let kw = self.expect(Token::View)?;
        self.expect(Token::LBrace)?;
        let body = self.parse_view_node()?;
        let close = self.expect(Token::RBrace)?;
        Ok(ViewBlock {
            body,
            span: Span::new(kw.span.start, close.span.end),
        })
    }

    fn parse_view_node(&mut self) -> Result<ViewNode, ParseError> {
        match &self.peek().token {
            Token::StringLit(_) => self.parse_text_node(),
            Token::Ident(_) => {
                // `For { ... }` — Tier 8 row-repetition node. (The
                // name `For` is therefore not usable as a component
                // kind; it is reserved for this shape.)
                let is_for = matches!(&self.peek().token, Token::Ident(s) if s == "For");
                let followed_by_brace = matches!(
                    self.tokens.get(self.pos + 1).map(|t| &t.token),
                    Some(Token::LBrace)
                );
                if is_for && followed_by_brace {
                    let kw_tok = self.bump();
                    return self.parse_for_node(kw_tok.span);
                }
                // `If { when: cond ... }` is reserved the same way.
                let is_if = matches!(&self.peek().token, Token::Ident(s) if s == "If");
                if is_if && followed_by_brace {
                    let kw_tok = self.bump();
                    return self.parse_if_node(kw_tok.span);
                }
                self.parse_element_node()
            }
            other => Err(ParseError::Unexpected {
                expected: "view node (string literal or element)",
                found: other.clone(),
                line: self.peek().span.start.line,
                column: self.peek().span.start.column,
            }),
        }
    }

    /// `For { over: list as: item ... }` — Tier 8 row repetition.
    /// `over:` names the state-field list; `as:` names the loop
    /// binding visible inside the children. Children are ordinary
    /// view nodes. The `For` token is already consumed by the
    /// caller; `kw_span` is its span.
    fn parse_for_node(&mut self, kw_span: crate::span::Span) -> Result<ViewNode, ParseError> {
        self.expect(Token::LBrace)?;
        let mut over: Option<Expr> = None;
        let mut binding: Option<String> = None;
        loop {
            if let Token::Ident(name) = &self.peek().token {
                let key = name.clone();
                if key == "over" || key == "as" {
                    let key_tok = self.bump();
                    self.expect(Token::DotDot)?;
                    // Tier 11: `over:` is a general (pure)
                    // expression — a state list, or a derived one
                    // via `filter`. `as:` stays a binding name.
                    let val = if key == "over" {
                        self.parse_expr()?
                    } else {
                        let val_tok = self.bump();
                        match val_tok.token {
                            Token::Ident(s) => Expr::Ident {
                                name: s,
                                span: val_tok.span,
                            },
                            other => {
                                return Err(ParseError::Unexpected {
                                    expected: "binding name after `as:`",
                                    found: other,
                                    line: val_tok.span.start.line,
                                    column: val_tok.span.start.column,
                                });
                            }
                        }
                    };
                    if key == "over" {
                        over = Some(val);
                    } else {
                        let val_name = match val {
                            Expr::Ident { name, .. } => name,
                            other => {
                                let s = other.span();
                                return Err(ParseError::Unexpected {
                                    expected: "binding name after `as:`",
                                    found: Token::Eof,
                                    line: s.start.line,
                                    column: s.start.column,
                                });
                            }
                        };
                        binding = Some(val_name);
                    }
                    let _ = key_tok;
                    continue;
                }
            }
            break;
        }
        let over = over.ok_or(ParseError::Unexpected {
            expected: "`over: <state field name>` in For block",
            found: Token::Eof,
            line: kw_span.start.line,
            column: kw_span.start.column,
        })?;
        let binding = binding.ok_or(ParseError::Unexpected {
            expected: "`as: <binding name>` in For block",
            found: Token::Eof,
            line: kw_span.start.line,
            column: kw_span.start.column,
        })?;
        let children = self.parse_view_children()?;
        let close = self.bump(); // }
        Ok(ViewNode::For {
            over,
            binding,
            children,
            span: Span::new(kw_span.start, close.span.end),
        })
    }

    /// The children of a `For` or `If`: only elements (`Child { ... }`) and text bodies, up to the closing brace.
    fn parse_view_children(&mut self) -> Result<Vec<ViewNode>, ParseError> {
        let mut children = Vec::new();
        // Children: only elements (`Child { ... }`) and string/text
        // bodies. No further bindings after `over:`/`as:`.
        loop {
            match &self.peek().token {
                Token::RBrace => break,
                Token::StringLit(_) => children.push(self.parse_text_node()?),
                Token::Ident(_) => {
                    // Lookahead `{` next -> child element.
                    let is_element = matches!(
                        self.tokens.get(self.pos + 1).map(|t| &t.token),
                        Some(Token::LBrace)
                    );
                    if !is_element {
                        return Err(ParseError::Unexpected {
                            expected: "child element (`Name { ... }`) or text",
                            found: self.peek().token.clone(),
                            line: self.peek().span.start.line,
                            column: self.peek().span.start.column,
                        });
                    }
                    children.push(self.parse_element_node()?);
                }
                other => {
                    return Err(ParseError::Unexpected {
                        expected: "child element, text, or `}`",
                        found: other.clone(),
                        line: self.peek().span.start.line,
                        column: self.peek().span.start.column,
                    });
                }
            }
        }
        Ok(children)
    }

    /// `If { when: cond ... }` (docs/view-conditional.md). The `If` token is already consumed; `kw_span` is its span.
    /// `when:` is a general expression, checked to be a pure `Bool` after parsing; the children are ordinary view nodes.
    fn parse_if_node(&mut self, kw_span: crate::span::Span) -> Result<ViewNode, ParseError> {
        self.expect(Token::LBrace)?;
        let is_when = matches!(&self.peek().token, Token::Ident(k) if k == "when")
            && matches!(
                self.tokens.get(self.pos + 1).map(|t| &t.token),
                Some(Token::DotDot)
            );
        if !is_when {
            return Err(ParseError::Unexpected {
                expected: "`when: <condition>` first in an If block",
                found: self.peek().token.clone(),
                line: self.peek().span.start.line,
                column: self.peek().span.start.column,
            });
        }
        self.bump();
        self.bump();
        let cond = self.parse_expr()?;
        let children = self.parse_view_children()?;
        let close = self.bump(); // }
        Ok(ViewNode::If {
            cond,
            children,
            span: Span::new(kw_span.start, close.span.end),
        })
    }

    fn parse_text_node(&mut self) -> Result<ViewNode, ParseError> {
        // Text body: either a string literal (with `{expr}`
        // interpolations) or a general expression whose result
        // becomes the text. Tier 6 supports both because
        // `Text { match result { ... } }` needs the expression
        // form to render a `Result` value conditionally.
        // A string literal followed by `+` is the start of a concatenation, an expression like any other
        // (`Text { "ready: " + ready }`); on its own it is the interpolated text.
        let concatenation = matches!(
            self.tokens.get(self.pos + 1).map(|t| &t.token),
            Some(Token::Plus)
        );
        if matches!(self.peek().token, Token::StringLit(_)) && !concatenation {
            let (value, tok_span) = self.bump_string_lit("string literal")?;
            let parts = interpolation_parts(&value, tok_span)?;
            Ok(ViewNode::Text {
                value: TextValue::Interpolated(parts),
                span: tok_span,
            })
        } else {
            // The body is a general expression. Render it as a Text
            // with a single Expr part.
            let start = self.peek().span.start;
            let expr = self.parse_expr()?;
            let end = expr.span().end;
            Ok(ViewNode::Text {
                value: TextValue::Interpolated(vec![TextPart::Expr {
                    expr,
                    span: Span::new(start, end),
                }]),
                span: Span::new(start, end),
            })
        }
    }

    fn parse_element_node(&mut self) -> Result<ViewNode, ParseError> {
        let (name, name_span) = self.bump_ident("element name")?;
        // `For { ... }` can appear anywhere a normal element can
        // (including as a child of another element). Route it to
        // the Tier 8 loop parser. The name token is already
        // consumed here.
        if name == "For" {
            return self.parse_for_node(name_span);
        }
        if name == "If" {
            return self.parse_if_node(name_span);
        }
        let kind = match name.as_str() {
            "Column" => ElementKind::Column,
            "Row" => ElementKind::Row,
            "Table" => ElementKind::Table,
            "Text" => ElementKind::Text,
            "Button" => ElementKind::Button,
            "TextField" => ElementKind::TextField,
            other => ElementKind::Named(other.to_string()),
        };

        if !matches!(self.peek().token, Token::LBrace) {
            return Err(ParseError::Unexpected {
                expected: "{",
                found: self.peek().token.clone(),
                line: self.peek().span.start.line,
                column: self.peek().span.start.column,
            });
        }
        let open = self.bump(); // {
        let mut accessibility = Accessibility {
            role: None,
            label: None,
            label_expr: None,
            span: open.span,
        };
        let mut bindings: Vec<Binding> = Vec::new();
        let mut children = Vec::new();

        loop {
            match &self.peek().token {
                Token::Ident(name) if name == "role" => {
                    self.bump();
                    self.expect(Token::DotDot)?;
                    let role_tok = self.bump();
                    let role = match role_tok.token {
                        Token::Ident(s) => parse_role(&s),
                        other => {
                            return Err(ParseError::Unexpected {
                                expected: "role identifier",
                                found: other,
                                line: role_tok.span.start.line,
                                column: role_tok.span.start.column,
                            });
                        }
                    };
                    accessibility.role = Some(role);
                    accessibility.span = Span::new(accessibility.span.start, role_tok.span.end);
                }
                Token::Ident(name) if name == "label" => {
                    let _kw = self.bump();
                    let _ = _kw;
                    self.expect(Token::DotDot)?;
                    // A lone string literal is the static label (a `{` in it is refused); anything else is an
                    // expression, evaluated at each render (docs/row-actions.md section 1).
                    let lone_literal = matches!(self.peek().token, Token::StringLit(_))
                        && !self.next_is_binary_operator();
                    if lone_literal {
                        let (s, label_span) = self.bump_string_lit("label string literal")?;
                        accessibility.label = Some(static_text(s, label_span)?);
                        accessibility.span = Span::new(accessibility.span.start, label_span.end);
                    } else {
                        match &self.peek().token {
                            Token::StringLit(_)
                            | Token::Ident(_)
                            | Token::IntLit(_)
                            | Token::FloatLit(_)
                            | Token::LParen => {}
                            other => {
                                return Err(ParseError::Unexpected {
                                    expected: "label string literal or expression",
                                    found: other.clone(),
                                    line: self.peek().span.start.line,
                                    column: self.peek().span.start.column,
                                });
                            }
                        }
                        let expr = self.parse_expr()?;
                        accessibility.span = Span::new(accessibility.span.start, expr.span().end);
                        accessibility.label_expr = Some(expr);
                    }
                }
                Token::Ident(name) if name == "tone" => {
                    // `tone: "error"` or `tone: outcome_tone` (a String state field): a status text's tone
                    // (docs/presentation.md section 3). It is a binding like the others; it may come first because
                    // a `Text` has no other place to put one.
                    let key_tok = self.bump();
                    self.expect(Token::DotDot)?;
                    let value = self.parse_binding_value("tone", &kind)?;
                    let span = Span::new(key_tok.span.start, self.tokens[self.pos - 1].span.end);
                    bindings.push(Binding {
                        key: "tone".to_string(),
                        value,
                        span,
                    });
                }
                Token::RBrace => break,
                Token::StringLit(_) => break,
                Token::Ident(_) => break,
                Token::On => break,
                Token::Match | Token::If => break,
                other => {
                    return Err(ParseError::Unexpected {
                        expected: "accessibility key, binding, view node, or `}`",
                        found: other.clone(),
                        line: self.peek().span.start.line,
                        column: self.peek().span.start.column,
                    });
                }
            }
        }

        // Body. `Text` is special: its body is a single string literal
        // (the text content, with `{expr}` interpolations). Other
        // elements take bindings (label:, on:, ...) and children.
        let kind_is_text = matches!(kind, ElementKind::Text);
        if kind_is_text {
            // After accessibility, expect a string literal
            // (with `{expr}` interpolations) OR a general
            // expression whose result becomes the text. Tier 6
            // supports the expression form so `Text { match
            // result { ... } }` works.
            match &self.peek().token {
                Token::StringLit(_)
                | Token::Ident(_)
                | Token::IntLit(_)
                | Token::FloatLit(_)
                | Token::LParen
                | Token::Minus
                | Token::Match
                | Token::If => {
                    children.push(self.parse_text_node()?);
                }
                Token::RBrace => {
                    let span = self.peek().span;
                    children.push(ViewNode::Text {
                        value: TextValue::Interpolated(vec![TextPart::Literal(String::new())]),
                        span,
                    });
                }
                other => {
                    return Err(ParseError::Unexpected {
                        expected: "string literal (text content) or expression",
                        found: other.clone(),
                        line: self.peek().span.start.line,
                        column: self.peek().span.start.column,
                    });
                }
            }
        } else {
            // Bindings and children. To distinguish a binding
            // (`label: "x"`) from a child element (`Text { ... }`),
            // look ahead one token: bindings have `:` next, elements
            // have `{` next.
            while !matches!(self.peek().token, Token::RBrace) {
                match &self.peek().token {
                    Token::Ident(name) if name == "role" || name == "label" => {
                        return Err(ParseError::Unexpected {
                            expected: "binding or view node (accessibility keys must come first)",
                            found: Token::Ident(name.to_string()),
                            line: self.peek().span.start.line,
                            column: self.peek().span.start.column,
                        });
                    }
                    Token::Ident(_) | Token::On => {
                        // Lookahead: `{` next -> child element,
                        // `:` next -> binding. (`on:` is a binding
                        // even though `on` is a keyword for event
                        // handlers at the component-member level.)
                        let is_element = matches!(
                            self.tokens.get(self.pos + 1).map(|t| &t.token),
                            Some(Token::LBrace)
                        );
                        if is_element {
                            children.push(self.parse_element_node()?);
                        } else {
                            let (key, key_span) = self.bump_binding_key()?;
                            self.expect(Token::DotDot)?;
                            let value = self.parse_binding_value(&key, &kind)?;
                            let span = Span::new(key_span.start, self.peek().span.end);
                            bindings.push(Binding { key, value, span });
                        }
                    }
                    Token::StringLit(_) => children.push(self.parse_text_node()?),
                    _ => {
                        return Err(ParseError::Unexpected {
                            expected: "binding or view node",
                            found: self.peek().token.clone(),
                            line: self.peek().span.start.line,
                            column: self.peek().span.start.column,
                        });
                    }
                }
            }
        }
        let close = self.bump(); // }
        Ok(ViewNode::Element {
            kind,
            accessibility,
            bindings,
            children,
            span: Span::new(name_span.start, close.span.end),
        })
    }

    fn parse_binding_value(
        &mut self,
        key: &str,
        kind: &ElementKind,
    ) -> Result<BindingValue, ParseError> {
        // Currently the only supported binding values are:
        //   - string literal (e.g. `label: "Increment"`),
        //   - `on: EventName` (event reference),
        //   - `role: heading` etc.
        // Phase 2 adds typed expressions.
        match key {
            "on" | "submit" => {
                let name_tok = self.bump();
                let name = match name_tok.token {
                    Token::Ident(s) => s,
                    other => {
                        return Err(ParseError::Unexpected {
                            expected: "event name",
                            found: other,
                            line: name_tok.span.start.line,
                            column: name_tok.span.start.column,
                        });
                    }
                };
                // `on: Pick(m)`: the arguments are expressions of the view (docs/row-actions.md section 2).
                if key == "on" && matches!(self.peek().token, Token::LParen) {
                    self.bump();
                    let mut args = Vec::new();
                    if !matches!(self.peek().token, Token::RParen) {
                        loop {
                            args.push(self.parse_expr()?);
                            if matches!(self.peek().token, Token::Comma) {
                                self.bump();
                            } else {
                                break;
                            }
                        }
                    }
                    self.expect(Token::RParen)?;
                    return Ok(BindingValue::EventCall { name, args });
                }
                Ok(BindingValue::EventRef(name))
            }
            "role" => {
                let role_tok = self.bump();
                let role = match role_tok.token {
                    Token::Ident(s) => parse_role(&s),
                    other => {
                        return Err(ParseError::Unexpected {
                            expected: "role identifier",
                            found: other,
                            line: role_tok.span.start.line,
                            column: role_tok.span.start.column,
                        });
                    }
                };
                Ok(BindingValue::Role(role))
            }
            "tone" => {
                // A literal (`"error"`) or the name of a String state field.
                let tok = self.bump();
                match tok.token {
                    Token::StringLit(s) => Ok(BindingValue::String(static_text(s, tok.span)?)),
                    Token::Ident(s) => Ok(BindingValue::StateRef(s)),
                    other => Err(ParseError::Unexpected {
                        expected: "`\"ok\"`, `\"warn\"`, `\"error\"` or the name of a String state field",
                        found: other,
                        line: tok.span.start.line,
                        column: tok.span.start.column,
                    }),
                }
            }
            "value" => {
                // `value: <ident>` — bind to a state field, or
                // `value: <string>` for a literal initial value.
                let val_tok = self.bump();
                match val_tok.token {
                    Token::Ident(s) => Ok(BindingValue::StateRef(s)),
                    Token::StringLit(s) => Ok(BindingValue::String(static_text(s, val_tok.span)?)),
                    other => Err(ParseError::Unexpected {
                        expected: "state field name (identifier) or string literal",
                        found: other,
                        line: val_tok.span.start.line,
                        column: val_tok.span.start.column,
                    }),
                }
            }
            _ => {
                // Default: string literal, or a constant initial
                // value for a component-instance property (Tier
                // 13): an int/float/bool literal lifts to `Prop`.
                // String literals stay `String` (plain labels
                // everywhere else); the shape pass interprets a
                // `String` value on an instance as a string
                // property. Tier 14: on an instance, a bare
                // identifier is a *dynamic property* — the value
                // is read from the parent component's state (or an
                // enclosing `For` binding) at each render.
                let lit_tok = self.bump();
                match lit_tok.token {
                    Token::StringLit(s) => Ok(BindingValue::String(static_text(s, lit_tok.span)?)),
                    Token::IntLit(v) => Ok(BindingValue::Prop(Expr::IntLit {
                        value: v,
                        span: lit_tok.span,
                    })),
                    Token::FloatLit(v) => Ok(BindingValue::Prop(Expr::FloatLit {
                        value: v,
                        span: lit_tok.span,
                    })),
                    Token::Ident(k) if k == "true" || k == "false" => {
                        Ok(BindingValue::Prop(Expr::BoolLit {
                            value: k == "true",
                            span: lit_tok.span,
                        }))
                    }
                    Token::Ident(s)
                        if matches!(kind, ElementKind::Named(_)) || key == "disabled" =>
                    {
                        // `disabled: sending` names a Bool state field (docs/controls.md section 1).
                        Ok(BindingValue::StateRef(s))
                    }
                    other => Err(ParseError::Unexpected {
                        expected: "string literal (or a constant property value: int/float/bool literal)",
                        found: other,
                        line: lit_tok.span.start.line,
                        column: lit_tok.span.start.column,
                    }),
                }
            }
        }
    }
}

fn parse_role(s: &str) -> Role {
    match s {
        "text" => Role::Text,
        "heading" => Role::Heading,
        "button" => Role::Button,
        other => Role::Other(other.to_string()),
    }
}
