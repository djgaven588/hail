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
}

#[derive(Debug)]
pub struct Parser {
    tokens: Vec<Token>,
    statements: Vec<Stmt>,
    parse_errors: Vec<ParseError>,
    current: usize,
}

impl Parser {
    pub fn new(tokens: &[Token]) -> Parser {
        Parser {
            tokens: tokens.to_vec(),
            parse_errors: vec![],
            statements: vec![],
            current: 0,
        }
    }

    /*
    pub fn scan(&mut self) {
        while self.step() {}
    }


    pub fn step(&mut self) -> bool {
        if self.is_end() {
            return true;
        }

        self.expression()

        true
    } */

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
            if self.try_match(&[TokenType::Print]).is_some() {
                self.print_statement();
            } else if self.try_match(&[TokenType::Return]).is_some() {
                self.return_statement();
            } else {
                self.expr_statement();
            }
        }
    }

    fn return_statement(&mut self) {
        let expr = if let Some(expr) = self.parse_precendence(Precedence::Or) {
            Box::new(expr)
        } else {
            self.error(ParseErrorType::ExpectedExpression);
            return;
        };

        self.consume(TokenType::Semicolon, ParseErrorType::MissingSemicolon);
        self.statements.push(Stmt::Return(expr));
    }

    fn expr_statement(&mut self) {
        let expr = if let Some(expr) = self.parse_precendence(Precedence::Or) {
            Box::new(expr)
        } else {
            self.error(ParseErrorType::ExpectedExpression);
            return;
        };

        let stmt = if self.try_match(&[TokenType::Semicolon]).is_some() {
            Stmt::Expression(expr)
        } else {
            Stmt::Return(expr)
        };
        self.statements.push(stmt);
    }

    fn print_statement(&mut self) {
        let expr = if let Some(expr) = self.parse_precendence(Precedence::Or) {
            Box::new(expr)
        } else {
            self.error(ParseErrorType::ExpectedExpression);
            return;
        };
        self.consume(TokenType::Semicolon, ParseErrorType::MissingSemicolon);

        self.statements.push(Stmt::Print(expr));
    }

    fn is_end(&self) -> bool {
        self.tokens[self.current].token_type == TokenType::Eof
    }

    fn parse_precendence(&mut self, precedence: Precedence) -> Option<Expr> {
        if self.is_end() {
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

    fn error(&mut self, error: ParseErrorType) {
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

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
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
    // Expression + if it's returned
    Expression(Box<Expr>),
    Print(Box<Expr>),
    Return(Box<Expr>),
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
            _ => {
                return Err(ParseErrorType::UnexpectedToken);
            }
        })
    }
}
