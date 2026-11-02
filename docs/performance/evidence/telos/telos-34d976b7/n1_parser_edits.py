EDITS = [
# glob import
("""        while self.eat(&TokenKind::Dot) {
            if self.at(&TokenKind::LBrace) {""",
"""        while self.eat(&TokenKind::Dot) {
            if self.at(&TokenKind::Star) {
                let star = self.bump();
                segments.push(Ident {
                    name: "*".to_string(),
                    span: star.span,
                });
                path_end = self.prev_end;
                break;
            }
            if self.at(&TokenKind::LBrace) {"""),
("""    // import := 'import' path ('.' '{' ident (',' ident)* '}')?""",
"""    // import := 'import' path ('.' '{' ident (',' ident)* '}' | '.' '*')?"""),
# tuple field
("""                TokenKind::Dot => {
                    self.bump();
                    let name = self.ident()?;
                    if self.at(&TokenKind::LParen) {""",
"""                TokenKind::Dot if matches!(self.peek_second(), TokenKind::Int(_)) => {
                    self.bump();
                    let TokenKind::Int(index) = self.bump().kind else {
                        unreachable!("peeked an integer")
                    };
                    ExprKind::TupleField {
                        receiver: Box::new(expr),
                        index: usize::try_from(index).unwrap_or(usize::MAX),
                    }
                }
                TokenKind::Dot => {
                    self.bump();
                    let name = self.ident()?;
                    if self.at(&TokenKind::LParen) {"""),
("""    // postfix := primary ( '(' args ')' | '.' ident ('(' args ')')? | '[' expr ']' | '?' )*""",
"""    // postfix := primary ( '(' args ')' | '.' ident ('(' args ')')? | '.' INT | '[' expr ']' | '?' )*"""),
# record update
("""                if name.is_uppercase() && !self.no_record && self.at(&TokenKind::LBrace) {
                    let fields = self.braced_list(Self::field_init)?;
                    ExprKind::Record { name, fields }
                } else {""",
"""                if name.is_uppercase() && !self.no_record && self.at(&TokenKind::LBrace) {
                    let (fields, base) = self.record_body()?;
                    match base {
                        Some(base) => ExprKind::RecordUpdate {
                            name,
                            fields,
                            base: Box::new(base),
                        },
                        None => ExprKind::Record { name, fields },
                    }
                } else {"""),
# break, continue, closures
("""            TokenKind::KwTrue => {
                self.bump();
                ExprKind::Bool(true)
            }""",
"""            TokenKind::KwBreak => {
                self.bump();
                ExprKind::Break
            }
            TokenKind::KwContinue => {
                self.bump();
                ExprKind::Continue
            }
            TokenKind::Pipe | TokenKind::OrOr => {
                self.enter(token.span)?;
                let closure = self.closure()?;
                self.leave();
                ExprKind::Closure(closure)
            }
            TokenKind::KwTrue => {
                self.bump();
                ExprKind::Bool(true)
            }"""),
("""    /// E001 for the literal `9223372036854775808` anywhere but after `-`.""",
"""    /// `'{' (field_init sep)* ('..' expr sep?)? '}'`: the fields of a record
    /// literal and the base of a record update, which comes last.
    fn record_body(&mut self) -> PResult<(Vec<FieldInit>, Option<Expr>)> {
        self.open(&TokenKind::LBrace, false)?;
        self.skip_newlines();
        let mut fields = Vec::new();
        let mut base = None;
        while !self.at_close_brace()? {
            if base.is_some() {
                return Err(self.expected("`}` after the base of a record update"));
            }
            if self.eat(&TokenKind::DotDot) {
                self.skip_newlines();
                base = Some(self.expr()?);
            } else {
                fields.push(self.field_init()?);
            }
            match self.peek() {
                TokenKind::Comma | TokenKind::Newline => {
                    while matches!(self.peek(), TokenKind::Comma | TokenKind::Newline) {
                        if self.at(&TokenKind::Comma) {
                            self.bump();
                        } else {
                            self.skip_newlines();
                        }
                    }
                }
                TokenKind::RBrace => {}
                _ => return Err(self.expected("`,`, newline or `}`")),
            }
        }
        self.close(&TokenKind::RBrace)?;
        Ok((fields, base))
    }

    // closure := ('|' (param (',' param)*)? '|' | '||') (block | expr)
    // param   := ident (':' type)?

    fn closure(&mut self) -> PResult<Closure> {
        let mut params = Vec::new();
        if !self.eat(&TokenKind::OrOr) {
            self.expect(&TokenKind::Pipe)?;
            while !self.at(&TokenKind::Pipe) {
                let name = self.ident()?;
                let ty = if self.eat(&TokenKind::Colon) {
                    Some(self.ty()?)
                } else {
                    None
                };
                params.push(ClosureParam {
                    span: self.span_from(name.span.start),
                    name,
                    ty,
                });
                if !self.eat(&TokenKind::Comma) {
                    break;
                }
            }
            self.expect(&TokenKind::Pipe)?;
        }
        let body = if self.at(&TokenKind::LBrace) {
            let block = self.block()?;
            Expr {
                span: block.span,
                kind: ExprKind::Block(block),
            }
        } else {
            self.expr()?
        };
        Ok(Closure {
            params,
            body: Box::new(body),
        })
    }

    /// E001 for the literal `9223372036854775808` anywhere but after `-`."""),
# range patterns
("""    fn pattern_alternative(&mut self) -> PResult<Pattern> {
        let token = self.peek_token();""",
"""    /// One alternative, or `start..=end` between two bounds. The checker
    /// and typer decide what a bound may be (`docs/slice.md` section 13.7).
    fn pattern_alternative(&mut self) -> PResult<Pattern> {
        let start = self.pattern_atom()?;
        if self.at(&TokenKind::DotDot) {
            let span = self.peek_token().span;
            self.report(
                "E001",
                span,
                "a range pattern includes its end: write `..=`".to_string(),
            );
            return Err(Fail);
        }
        if !self.eat(&TokenKind::DotDotEq) {
            return Ok(start);
        }
        let end = self.pattern_atom()?;
        Ok(Pattern {
            span: Span::new(start.span.start, end.span.end),
            kind: PatternKind::Range {
                start: Box::new(start),
                end: Box::new(end),
            },
        })
    }

    fn pattern_atom(&mut self) -> PResult<Pattern> {
        let token = self.peek_token();"""),
("""            TokenKind::Int(i64::MIN) => return Err(self.int_too_large(token)),
            TokenKind::Int(value) => {
                self.bump();
                PatternKind::Int(*value)
            }
            TokenKind::Str(value) => {
                self.bump();
                PatternKind::Str(value.clone())
            }""",
"""            TokenKind::Int(i64::MIN) => return Err(self.int_too_large(token)),
            TokenKind::Int(value) => {
                self.bump();
                PatternKind::Int(*value)
            }
            TokenKind::Minus if matches!(self.peek_second(), TokenKind::Int(_)) => {
                self.bump();
                let TokenKind::Int(value) = self.bump().kind else {
                    unreachable!("peeked an integer")
                };
                PatternKind::Int(value.wrapping_neg())
            }
            TokenKind::Str(value) => {
                self.bump();
                PatternKind::Str(value.clone())
            }"""),
]
