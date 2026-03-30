use crate::scanner::{Token, TokenType};

#[derive(Debug)]
pub struct ParseError {
    token: Token,
    error: ParseErrorType,
}

impl ParseError {
    pub fn new(token: Token, error: ParseErrorType) -> ParseError {
        ParseError { token, error }
    }
}

#[derive(Debug)]
enum ParseErrorType {
    ExpectedClosingParenthese,
    ExpectedExpression,
    UnexpectedEndOfFile,
    UnexpectedToken,
    MissingSemicolon,
    StatementInvalid,
    ExpectedIdentifier,
}

#[derive(Debug)]
pub struct Parser {
    tokens: Vec<Token>,
    statements: Vec<Stmt>,
    parse_errors: Vec<ParseError>,
    current: usize,
    panicing: bool,
}

impl Parser {
    pub fn new(tokens: &[Token]) -> Parser {
        Parser {
            tokens: tokens.to_vec(),
            parse_errors: vec![],
            statements: vec![],
            current: 0,
            panicing: false,
        }
    }

    /// Called in the event that we had a parsing error to try and recover
    fn synchronize(&mut self) {
        while !self.is_end()
            && let Some(token) = self.peek()
        {
            match token.token_type {
                TokenType::Semicolon
                | TokenType::If
                | TokenType::While
                | TokenType::Fn
                | TokenType::Return
                | TokenType::Let
                | TokenType::Struct
                | TokenType::Import
                | TokenType::DocComment
                | TokenType::Eof => {
                    break;
                }
                _ => {}
            }

            let _ = self.advance();
        }
    }

    pub fn parse(&mut self) {
        while !self.is_end() {
            // We continue regardless for parsing reasons
            if let Some(stmt) = self.declaration() {
                self.statements.push(stmt);
            }
        }
    }

    fn declaration(&mut self) -> Option<Stmt> {
        let stmt = if self.try_match(&[TokenType::Let]).is_some() {
            self.var_declaration()
        } else if self
            .peek_next()
            .is_some_and(|v| v.token_type == TokenType::Equal)
            && self.try_match(&[TokenType::Identifier]).is_some()
        {
            self.var_assignment()
        } else {
            self.statement()
        };

        if self.panicing {
            // Get the parser into *any* state we can continue with.
            self.synchronize();
            self.panicing = false;
        }

        stmt
    }

    fn var_declaration(&mut self) -> Option<Stmt> {
        let mutable = self.try_match(&[TokenType::Mutable]).is_some();

        let Some(name) = self.try_match(&[TokenType::Identifier]).cloned() else {
            self.error(ParseErrorType::ExpectedIdentifier);
            return None;
        };

        let initializer = if self.try_match(&[TokenType::Equal]).is_some() {
            if let Some(expr) = self.parse_precendence(Precedence::Assignment) {
                Some(Box::new(expr))
            } else {
                return None;
            }
        } else {
            None
        };
        self.consume(TokenType::Semicolon, ParseErrorType::MissingSemicolon);

        Some(Stmt::Variable(name.lexeme, initializer, mutable))
    }

    fn var_assignment(&mut self) -> Option<Stmt> {
        let Some(token) = self.last().cloned() else {
            self.error(ParseErrorType::ExpectedIdentifier);
            return None;
        };

        let assignment = if self.try_match(&[TokenType::Equal]).is_some() {
            if let Some(expr) = self.parse_precendence(Precedence::Assignment) {
                Box::new(expr)
            } else {
                return None;
            }
        } else {
            self.error(ParseErrorType::ExpectedExpression);
            return None;
        };
        self.consume(TokenType::Semicolon, ParseErrorType::MissingSemicolon);

        Some(Stmt::Assign(token, assignment))
    }

    fn statement(&mut self) -> Option<Stmt> {
        if self.try_match(&[TokenType::LeftBrace]).is_some() {
            self.block()
        } else if self.try_match(&[TokenType::Print]).is_some() {
            self.print_statement()
        } else if self.try_match(&[TokenType::Return]).is_some() {
            self.return_statement()
        } else {
            self.expr_statement()
        }
    }

    fn block(&mut self) -> Option<Stmt> {
        let mut stmts = vec![];

        while self.try_match(&[TokenType::RightBrace]).is_none() && !self.is_end() {
            // Only push actual declarations, we keep going for parser reasons
            if let Some(stmt) = self.declaration() {
                stmts.push(stmt);
            }
        }

        Some(Stmt::Block(stmts))
    }

    fn return_statement(&mut self) -> Option<Stmt> {
        let expr = if let Some(expr) = self.parse_precendence(Precedence::Or) {
            Box::new(expr)
        } else {
            self.error(ParseErrorType::ExpectedExpression);
            return None;
        };

        self.consume(TokenType::Semicolon, ParseErrorType::MissingSemicolon);

        Some(Stmt::Return(expr))
    }

    fn expr_statement(&mut self) -> Option<Stmt> {
        let expr = if let Some(expr) = self.parse_precendence(Precedence::Or) {
            Box::new(expr)
        } else {
            self.error(ParseErrorType::ExpectedExpression);
            return None;
        };

        let stmt = if self.try_match(&[TokenType::Semicolon]).is_some() {
            Stmt::Expression(expr)
        } else {
            Stmt::Return(expr)
        };

        Some(stmt)
    }

    fn print_statement(&mut self) -> Option<Stmt> {
        let expr = if let Some(expr) = self.parse_precendence(Precedence::Or) {
            Box::new(expr)
        } else {
            self.error(ParseErrorType::ExpectedExpression);
            return None;
        };
        self.consume(TokenType::Semicolon, ParseErrorType::MissingSemicolon);

        Some(Stmt::Print(expr))
    }

    fn is_end(&self) -> bool {
        self.tokens[self.current].token_type == TokenType::Eof
    }

    fn parse_precendence(&mut self, precedence: Precedence) -> Option<Expr> {
        if self.is_end() {
            self.panicing = true;
            self.parse_errors.push(ParseError::new(
                self.peek().unwrap().clone(),
                ParseErrorType::UnexpectedEndOfFile,
            ));
            return None;
        }

        println!("Advancing precedence");

        let mut left = if let (Some(prefix), _, _) = match self.peek()?.token_type.rule() {
            Ok(val) => val,
            Err(err) => {
                self.error(err);
                let _ = self.advance();
                return None;
            }
        } {
            let _ = self.advance();
            println!("Rule");
            prefix(self)?
        } else {
            self.error(ParseErrorType::UnexpectedToken);
            let _ = self.advance();
            println!("No rule");
            return None;
        };

        println!("Parse Expr: {left:?}");

        while !self.is_end() {
            let Some(next) = self.peek().cloned() else {
                return None;
            };

            println!("Checking");

            let (_, infix, token_precedence) = match next.token_type.rule() {
                Ok(val) => val,
                Err(err) => {
                    self.error(err);
                    let _ = self.advance();
                    return None;
                }
            };

            println!("Precedence: {token_precedence:?}, {precedence:?}");

            if token_precedence < precedence {
                break;
            }

            println!("Calling infix");

            let _ = self.advance();
            left = if let Some(infix) = infix {
                infix(self, left)?
            } else {
                self.error(ParseErrorType::StatementInvalid);
                return None;
            }
        }

        Some(left)
    }

    fn grouping(&mut self) -> Option<Expr> {
        println!("Grouping");
        let expr = self.parse_precendence(Precedence::Or)?;

        self.consume(
            TokenType::RightParenthese,
            ParseErrorType::ExpectedClosingParenthese,
        );

        Some(expr)
    }

    fn binary(&mut self, left: Expr) -> Option<Expr> {
        println!("Binary");
        let token = self.last()?.clone();
        let precedence = match token.token_type.rule() {
            Ok(val) => val.2,
            Err(err) => {
                self.error(err);
                let _ = self.advance();
                return None;
            }
        };
        let Some(right) = self.parse_precendence(precedence) else {
            self.error(ParseErrorType::ExpectedExpression);
            return None;
        };

        Some(Expr::Binary(Box::new(left), token.clone(), Box::new(right)))
    }

    fn unary(&mut self) -> Option<Expr> {
        println!("Unary");
        let token = self.last()?.clone();
        let Some(left) = self.parse_precendence(Precedence::Call) else {
            self.error(ParseErrorType::ExpectedExpression);
            return None;
        };
        match token.token_type {
            TokenType::Minus => Some(Expr::Unary(token.clone(), Box::new(left))),
            TokenType::Bang => Some(Expr::Unary(token.clone(), Box::new(left))),
            a => unreachable!("{a:?}"),
        }
    }

    fn integer(&mut self) -> Option<Expr> {
        println!("Integer");
        let token = self.last()?;
        match token.token_type {
            TokenType::Integer => Some(Expr::Integer(
                token
                    .lexeme
                    .parse()
                    .expect("Integer literal should be valid."),
            )),
            a => unreachable!("{a:?}"),
        }
    }

    fn float(&mut self) -> Option<Expr> {
        println!("Float");
        let token = self.last()?;
        match token.token_type {
            TokenType::Float => Some(Expr::Float(
                token
                    .lexeme
                    .parse()
                    .expect("Float literal should be valid."),
            )),
            a => unreachable!("{a:?}"),
        }
    }

    fn bool(&mut self) -> Option<Expr> {
        println!("Bool");
        let token = self.last()?;
        match token.token_type {
            TokenType::True => Some(Expr::Bool(true)),
            TokenType::False => Some(Expr::Bool(false)),
            a => unreachable!("{a:?}"),
        }
    }
    fn string(&mut self) -> Option<Expr> {
        println!("String");
        let token = self.last()?;
        match token.token_type {
            TokenType::String => Some(Expr::String(
                token.lexeme[1..(token.lexeme.len() - 1)].to_string(),
            )),
            a => unreachable!("{a:?}"),
        }
    }

    fn variable(&mut self) -> Option<Expr> {
        println!("Variable");
        let token = self.last()?;
        match token.token_type {
            TokenType::Identifier => Some(Expr::Variable(token.clone())),
            a => unreachable!("{a:?}"),
        }
    }

    fn error(&mut self, error: ParseErrorType) {
        if self.panicing {
            // Prevent further errors from coming through while we're within the same panic
            return;
        }

        self.panicing = true;
        self.parse_errors.push(ParseError::new(
            self.peek()
                .expect("Parse error shouldn't occur on missing token..?")
                .clone(),
            error,
        ));
    }

    fn consume(&mut self, token_type: TokenType, error: ParseErrorType) -> bool {
        if self.try_match(&[token_type]).is_some() {
            true
        } else {
            self.error(error);
            false
        }
    }

    fn last(&mut self) -> Option<&Token> {
        self.tokens.get(self.current.saturating_sub(1))
    }

    fn advance(&mut self) -> Option<&Token> {
        if let Some(token) = self.tokens.get(self.current) {
            self.current = (self.current + 1).min(self.tokens.len().saturating_sub(1));
            Some(token)
        } else {
            None
        }
    }

    fn try_match(&mut self, expected: &[TokenType]) -> Option<&Token> {
        if let Some(token) = self.tokens.get(self.current)
            && expected.contains(&token.token_type)
        {
            self.current = (self.current + 1).min(self.tokens.len().saturating_sub(1));
            Some(token)
        } else {
            None
        }
    }

    fn check(&self, expected: TokenType) -> bool {
        self.peek().is_some_and(|v| v.token_type == expected)
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.current)
    }

    fn peek_next(&self) -> Option<&Token> {
        self.tokens.get(self.current + 1)
    }

    pub fn errors(&self) -> &[ParseError] {
        &self.parse_errors
    }

    pub fn get(&self) -> Option<&[Stmt]> {
        if self.parse_errors.is_empty() {
            Some(&self.statements)
        } else {
            None
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Precedence {
    None = 0,
    Assignment = 10,
    Or = 20,
    And = 30,
    Equality = 40,
    Comparison = 50,
    Term = 60,
    Factor = 70,
    Unary = 80,
    Call = 90,
    Primary = 100,
}

#[derive(Debug)]
pub enum Stmt {
    // Name, initializer, mutable
    Variable(String, Option<Box<Expr>>, bool),
    Expression(Box<Expr>),
    Print(Box<Expr>),
    Return(Box<Expr>),
    // Name, assignment
    Assign(Token, Box<Expr>),
    Block(Vec<Stmt>),
}

#[derive(Debug)]
pub enum Expr {
    Float(f64),
    Integer(i64),
    Bool(bool),
    String(String),
    Nil,
    Unary(Token, Box<Expr>),
    Binary(Box<Expr>, Token, Box<Expr>),
    Variable(Token),
}

impl Expr {
    pub fn display(&self, level: usize) -> String {
        match self {
            Expr::Float(token) => "\t".repeat(level) + &token.to_string() + "\n",
            Expr::Integer(token) => "\t".repeat(level) + &token.to_string() + "\n",
            Expr::Bool(token) => "\t".repeat(level) + &token.to_string() + "\n",
            Expr::String(token) => "\t".repeat(level) + &token.to_string() + "\n",
            Expr::Nil => "\t".repeat(level) + "Nil\n",
            Expr::Unary(token, expr) => {
                "\t".repeat(level) + &format!("{:?}{}\n", token.token_type, expr.display(level + 1))
            }
            Expr::Binary(expr, token, expr1) => {
                expr.display(level + 1)
                    + &"\t".repeat(level)
                    + &format!("{:?}\n{}", token.token_type, expr1.display(level + 1))
            }
            Expr::Variable(identifier) => {
                "\t".repeat(level) + "Variable: " + &identifier.lexeme.to_string() + "\n"
            } /*
              Expr::Assign(identifier, value) => {
                  "\t".repeat(level)
                      + "Assign: "
                      + &identifier.lexeme.to_string()
                      + " = \n"
                      + &value.display(level + 1)
                      + "\n"
              } */
        }
    }
}

type PrefixParseFunc = fn(&mut Parser) -> Option<Expr>;
type InfixParseFunc = fn(&mut Parser, Expr) -> Option<Expr>;
impl TokenType {
    pub fn rule(
        &self,
    ) -> Result<(Option<PrefixParseFunc>, Option<InfixParseFunc>, Precedence), ParseErrorType> {
        Ok(match self {
            TokenType::Or => (None, Some(Parser::binary), Precedence::Or),
            TokenType::And => (None, Some(Parser::binary), Precedence::And),
            TokenType::Minus => (Some(Parser::unary), Some(Parser::binary), Precedence::Term),
            TokenType::Bang => (Some(Parser::unary), None, Precedence::Term),
            TokenType::Plus => (None, Some(Parser::binary), Precedence::Term),
            TokenType::Star => (None, Some(Parser::binary), Precedence::Factor),
            TokenType::Slash => (None, Some(Parser::binary), Precedence::Factor),
            TokenType::Integer => (Some(Parser::integer), None, Precedence::Primary),
            TokenType::Float => (Some(Parser::float), None, Precedence::Primary),
            TokenType::String => (Some(Parser::string), None, Precedence::Primary),
            TokenType::True => (Some(Parser::bool), None, Precedence::Primary),
            TokenType::False => (Some(Parser::bool), None, Precedence::Primary),
            TokenType::Identifier => (Some(Parser::variable), None, Precedence::Primary),
            TokenType::LeftParenthese => (Some(Parser::grouping), None, Precedence::Call),
            TokenType::RightParenthese => (None, None, Precedence::None),
            TokenType::Greater
            | TokenType::GreaterEqual
            | TokenType::Less
            | TokenType::LessEqual => (None, Some(Parser::binary), Precedence::Comparison),
            TokenType::EqualEqual | TokenType::BangEqual => {
                (None, Some(Parser::binary), Precedence::Equality)
            }
            TokenType::Semicolon => (None, None, Precedence::None),
            //TokenType::Equal => (Some(Parser::assign), None, Precedence::Assignment),
            _ => {
                return Err(ParseErrorType::UnexpectedToken);
            }
        })
    }
}
