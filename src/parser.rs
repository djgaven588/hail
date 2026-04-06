use crate::{
    Location,
    scanner::{Token, TokenType},
};

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
    ExpectedParameters,
    InvalidOperator,
    UnexpectedEndOfFile,
    UnexpectedToken,
    MissingSemicolon,
    StatementInvalid,
    ExpectedIdentifier,
    ExpectedBlock,
    ExpectedCondition,
    ExpectedForBody,
    ExpectedAssignmentOp,
    ExpectedComma,
}

#[derive(Debug)]
pub struct Parser {
    tokens: Vec<Token>,
    statements: Vec<Stmt>,
    parse_errors: Vec<ParseError>,
    current: usize,
    panicking: bool,
}

impl Parser {
    pub fn new(tokens: &[Token]) -> Parser {
        Parser {
            tokens: tokens.to_vec(),
            parse_errors: vec![],
            statements: vec![],
            current: 0,
            panicking: false,
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
        let stmt = if self.try_match(&[TokenType::Fn]).is_some() {
            self.function_declaration()
        } else if self.try_match(&[TokenType::Let]).is_some() {
            self.var_declaration()
        } else if self.try_match(&[TokenType::Const]).is_some() {
            self.const_declaration()
        } else if self
            .peek_next()
            .is_some_and(|v| v.token_type.is_assignment_op())
            && self.try_match(&[TokenType::Identifier]).is_some()
        {
            self.var_assignment()
        } else {
            self.statement()
        };

        if self.panicking {
            // Get the parser into *any* state we can continue with.
            self.synchronize();
            self.panicking = false;
        }

        stmt
    }

    fn function_declaration(&mut self) -> Option<Stmt> {
        let Some(name) = self.try_match(&[TokenType::Identifier]).cloned() else {
            self.error(ParseErrorType::ExpectedIdentifier);
            return None;
        };

        self.consume(
            TokenType::LeftParenthese,
            ParseErrorType::ExpectedParameters,
        );

        let mut parameters = vec![];

        // Loop until we run out of parameters
        loop {
            // Check if this variable will be mutable, it's fine if it isn't.
            let mutability: VariableMutability = if self.try_match(&[TokenType::Mut]).is_some() {
                VariableMutability::Mutable
            } else if self.try_match(&[TokenType::Const]).is_some() {
                VariableMutability::Constant
            } else {
                VariableMutability::Immutable
            };

            // We *must* have an identifier
            let Some(identifier) = self.try_match(&[TokenType::Identifier]) else {
                break;
            };

            parameters.push((mutability, identifier.clone()));

            // Remove trailing commas, we're done if there is none.
            if self.try_match(&[TokenType::Comma]).is_none() {
                break;
            }
        }

        self.consume(
            TokenType::RightParenthese,
            ParseErrorType::ExpectedClosingParenthese,
        );

        self.consume(TokenType::LeftBrace, ParseErrorType::ExpectedBlock);

        let body = self.block_statement()?;

        Some(Stmt::Function(name, parameters, Box::new(body)))
    }

    fn const_declaration(&mut self) -> Option<Stmt> {
        let Some(name) = self.try_match(&[TokenType::Identifier]).cloned() else {
            self.error(ParseErrorType::ExpectedIdentifier);
            return None;
        };

        self.consume(TokenType::Equal, ParseErrorType::ExpectedExpression);

        let Some(initializer) = self.statement() else {
            return None;
        };

        self.consume(TokenType::Semicolon, ParseErrorType::MissingSemicolon);

        Some(Stmt::DefineVariable(
            name,
            Some(Box::new(initializer)),
            VariableMutability::Constant,
        ))
    }

    fn var_declaration(&mut self) -> Option<Stmt> {
        let mutable = self.try_match(&[TokenType::Mut]).is_some();

        let Some(name) = self.try_match(&[TokenType::Identifier]).cloned() else {
            self.error(ParseErrorType::ExpectedIdentifier);
            return None;
        };

        let initializer = if self.try_match(&[TokenType::Equal]).is_some() {
            if let Some(expr) = self.statement() {
                Some(Box::new(expr))
            } else {
                self.error(ParseErrorType::ExpectedAssignmentOp);
                return None;
            }
        } else {
            None
        };

        self.consume(TokenType::Semicolon, ParseErrorType::MissingSemicolon);

        Some(Stmt::DefineVariable(
            name,
            initializer,
            if mutable {
                VariableMutability::Mutable
            } else {
                VariableMutability::Immutable
            },
        ))
    }

    fn var_assignment(&mut self) -> Option<Stmt> {
        let Some(token) = self.last().cloned() else {
            self.error(ParseErrorType::ExpectedIdentifier);
            return None;
        };

        // Try and match an assignment operation
        // =, +=, ect.
        let (op, assignment) = if let Some(op) = self.try_match(TokenType::assignment_ops()) {
            let op = op.token_type.unwrap_assignment_op();
            if let Some(expr) = self.statement() {
                (op, Box::new(expr))
            } else {
                return None;
            }
        } else {
            self.error(ParseErrorType::ExpectedExpression);
            return None;
        };

        self.consume(TokenType::Semicolon, ParseErrorType::MissingSemicolon);

        Some(Stmt::Assign(token.lexeme, op, assignment))
    }

    fn statement(&mut self) -> Option<Stmt> {
        if self.try_match(&[TokenType::For]).is_some() {
            self.for_statement()
        } else if self.try_match(&[TokenType::While]).is_some() {
            self.while_statement()
        } else if self.try_match(&[TokenType::If]).is_some() {
            self.if_statement()
        } else if self.try_match(&[TokenType::LeftBrace]).is_some() {
            self.block_statement()
        } else if self.try_match(&[TokenType::Print]).is_some() {
            self.print_statement()
        } else if self.try_match(&[TokenType::Return]).is_some() {
            self.return_statement()
        } else {
            self.expr_statement()
        }
    }

    fn for_statement(&mut self) -> Option<Stmt> {
        // (initializer; condition; chaser) { body }
        self.consume(TokenType::LeftParenthese, ParseErrorType::ExpectedForBody);

        let initializer = self.declaration()?;

        let condition = self.declaration()?;

        self.consume(TokenType::Semicolon, ParseErrorType::MissingSemicolon);

        let chaser = self.declaration()?;

        self.consume(
            TokenType::RightParenthese,
            ParseErrorType::ExpectedClosingParenthese,
        );

        self.consume(TokenType::LeftBrace, ParseErrorType::ExpectedBlock);

        let body = self.block_statement()?;

        Some(Stmt::Block(vec![
            // Preamble
            initializer,
            Stmt::While(
                Box::new(condition),
                Box::new(Stmt::Block(vec![
                    // Encapsulated Body
                    Stmt::Block(vec![body]),
                    // ^ Prevents redeclaration within body impacting loop
                    chaser,
                ])),
            ),
        ]))
    }

    fn while_statement(&mut self) -> Option<Stmt> {
        let condition = self.statement()?;

        self.consume(TokenType::LeftBrace, ParseErrorType::ExpectedBlock);

        let body = self.block_statement()?;

        Some(Stmt::While(Box::new(condition), Box::new(body)))
    }

    fn if_statement(&mut self) -> Option<Stmt> {
        let condition = self.statement()?;

        self.consume(TokenType::LeftBrace, ParseErrorType::ExpectedBlock);

        let body = self.block_statement()?;
        let else_block = if self.try_match(&[TokenType::Else]).is_some() {
            self.consume(TokenType::LeftBrace, ParseErrorType::ExpectedBlock);
            Some(self.block_statement()?.into())
        } else {
            None
        };

        Some(Stmt::If(Box::new(condition), Box::new(body), else_block))
    }

    fn block_statement(&mut self) -> Option<Stmt> {
        let mut stmts = vec![];

        while self.try_match(&[TokenType::RightBrace]).is_none() && !self.is_end() {
            // Only push actual declarations, we keep going for parser reasons
            if let Some(stmt) = self.declaration() {
                stmts.push(stmt);
            }
        }

        if stmts.is_empty() {
            // Default fill statement with nil return value
            // Other things expect at least 1 statement, returning nil by default seems sane.
            stmts.push(Stmt::Expression(
                Box::new(Expr::Nil(self.last().unwrap().location)),
                true,
            ));
        }

        Some(Stmt::Block(stmts))
    }

    fn return_statement(&mut self) -> Option<Stmt> {
        if let Some(token) = self.try_match(&[TokenType::Semicolon]) {
            return Some(Stmt::Return(token.location, None));
        }

        let expr = if let Some(expr) = self.statement() {
            Box::new(expr)
        } else {
            self.error(ParseErrorType::ExpectedExpression);
            return None;
        };

        let token = self.consume(TokenType::Semicolon, ParseErrorType::MissingSemicolon)?;

        Some(Stmt::Return(token.location, Some(expr)))
    }

    fn expr_statement(&mut self) -> Option<Stmt> {
        let expr = self.expression()?.into();

        Some(Stmt::Expression(expr, true))
    }

    fn expression(&mut self) -> Option<Expr> {
        if let Some(expr) = self.parse_precendence(Precedence::Assignment) {
            Some(expr)
        } else {
            self.error(ParseErrorType::ExpectedExpression);
            None
        }
    }

    fn print_statement(&mut self) -> Option<Stmt> {
        let expr = if let Some(expr) = self.statement() {
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
            self.panicking = true;
            self.parse_errors.push(ParseError::new(
                self.peek().unwrap().clone(),
                ParseErrorType::UnexpectedEndOfFile,
            ));
            return None;
        }

        let mut left = if let (Some(prefix), _, _) = match self.peek()?.token_type.rule() {
            Ok(val) => val,
            Err(err) => {
                self.error(err);
                let _ = self.advance();
                return None;
            }
        } {
            let _ = self.advance();
            prefix(self)?
        } else {
            self.error(ParseErrorType::UnexpectedToken);
            let _ = self.advance();
            return None;
        };

        while !self.is_end() {
            let Some(next) = self.peek().cloned() else {
                return None;
            };

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

    fn call(&mut self, callee: Expr) -> Option<Expr> {
        let mut params = vec![];
        loop {
            // See if we've hit the end
            if self.try_match(&[TokenType::RightParenthese]).is_some() {
                break;
            }

            params.push(Box::new(self.parse_precendence(Precedence::Assignment)?));

            // Try and remove the comma that follows
            let _ = self.try_match(&[TokenType::Comma]);
        }

        Some(Expr::Call(Box::new(callee), params))
    }

    fn grouping(&mut self) -> Option<Expr> {
        let expr = self.parse_precendence(Precedence::Assignment)?;

        self.consume(
            TokenType::RightParenthese,
            ParseErrorType::ExpectedClosingParenthese,
        );

        Some(expr)
    }

    fn logical(&mut self, left: Expr) -> Option<Expr> {
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
            self.error(ParseErrorType::ExpectedCondition);
            return None;
        };

        Some(Expr::Condition(
            Box::new(left),
            token.clone(),
            Box::new(right),
        ))
    }

    fn binary(&mut self, left: Expr) -> Option<Expr> {
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

        let op = match token.token_type {
            TokenType::Minus => BinaryOp::Minus,
            TokenType::Plus => BinaryOp::Plus,
            TokenType::Slash => BinaryOp::Divide,
            TokenType::Star => BinaryOp::Multiply,
            TokenType::BangEqual => BinaryOp::BangEqual,
            TokenType::EqualEqual => BinaryOp::EqualEqual,
            TokenType::Greater => BinaryOp::Greater,
            TokenType::GreaterEqual => BinaryOp::GreaterEqual,
            TokenType::Less => BinaryOp::Less,
            TokenType::LessEqual => BinaryOp::LessEqual,
            TokenType::BinaryAnd => todo!(),
            TokenType::BinaryOr => todo!(),
            _ => {
                self.error(ParseErrorType::InvalidOperator);
                return None;
            }
        };

        Some(Expr::Binary(Box::new(left), op, Box::new(right)))
    }

    fn unary(&mut self) -> Option<Expr> {
        let token = self.last()?.clone();
        let Some(left) = self.parse_precendence(Precedence::Unary) else {
            self.error(ParseErrorType::ExpectedExpression);
            return None;
        };

        let op = match token.token_type {
            TokenType::Minus => UnaryOp::Negate,
            TokenType::Bang => UnaryOp::Invert,
            _ => {
                self.error(ParseErrorType::InvalidOperator);
                return None;
            }
        };

        match token.token_type {
            TokenType::Minus => Some(Expr::Unary(op, Box::new(left))),
            TokenType::Bang => Some(Expr::Unary(op, Box::new(left))),
            a => unreachable!("{a:?}"),
        }
    }

    fn integer(&mut self) -> Option<Expr> {
        let token = self.last()?;
        match token.token_type {
            TokenType::Integer => Some(Expr::Integer(
                token.location,
                token
                    .lexeme
                    .parse()
                    .expect("Integer literal should be valid."),
            )),
            a => unreachable!("{a:?}"),
        }
    }

    fn float(&mut self) -> Option<Expr> {
        let token = self.last()?;
        match token.token_type {
            TokenType::Float => Some(Expr::Float(
                token.location,
                token
                    .lexeme
                    .parse()
                    .expect("Float literal should be valid."),
            )),
            a => unreachable!("{a:?}"),
        }
    }

    fn bool(&mut self) -> Option<Expr> {
        let token = self.last()?;
        match token.token_type {
            TokenType::True => Some(Expr::Bool(token.location, true)),
            TokenType::False => Some(Expr::Bool(token.location, false)),
            a => unreachable!("{a:?}"),
        }
    }
    fn string(&mut self) -> Option<Expr> {
        let token = self.last()?;
        match token.token_type {
            TokenType::String => Some(Expr::String(
                token.location,
                token.lexeme[1..(token.lexeme.len() - 1)].to_string(),
            )),
            a => unreachable!("{a:?}"),
        }
    }

    fn variable(&mut self) -> Option<Expr> {
        let token = self.last()?;
        match token.token_type {
            TokenType::Identifier => Some(Expr::Variable(token.location, token.lexeme.clone())),
            a => unreachable!("{a:?}"),
        }
    }

    fn error(&mut self, error: ParseErrorType) {
        if self.panicking {
            // Prevent further errors from coming through while we're within the same panic
            return;
        }

        self.panicking = true;
        self.parse_errors.push(ParseError::new(
            self.peek()
                .expect("Parse error shouldn't occur on missing token..?")
                .clone(),
            error,
        ));
    }

    fn consume(&mut self, token_type: TokenType, error: ParseErrorType) -> Option<Token> {
        if let Some(token) = self.try_match(&[token_type]) {
            Some(token.clone())
        } else {
            self.error(error);
            None
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VariableMutability {
    Immutable,
    Mutable,
    Constant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AssignmentOp {
    Equal,
    PlusEqual,
    MinusEqual,
    MultiplyEqual,
    DivideEqual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnaryOp {
    Negate,
    Invert,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BinaryOp {
    Plus,
    Minus,
    Multiply,
    Divide,
    Greater,
    GreaterEqual,
    Less,
    LessEqual,
    EqualEqual,
    BangEqual,
}

#[derive(Debug, Clone)]
pub enum Stmt {
    // Name, initializer, mutable
    DefineVariable(Token, Option<Box<Stmt>>, VariableMutability),
    // Evaluate, can return
    Expression(Box<Expr>, bool),
    Print(Box<Stmt>),
    Return(Location, Option<Box<Stmt>>),
    // Name, assignment
    Assign(String, AssignmentOp, Box<Stmt>),
    Block(Vec<Stmt>),
    // Condition, body, otherwise
    If(Box<Stmt>, Box<Stmt>, Option<Box<Stmt>>),
    // Condition, body
    While(Box<Stmt>, Box<Stmt>),
    // Name, parameter names (with mutability), body
    Function(Token, Vec<(VariableMutability, Token)>, Box<Stmt>),
}

impl Stmt {
    pub fn get_location(&self) -> Location {
        match self {
            Stmt::DefineVariable(token, _, _) => token.location,
            Stmt::Expression(expr, _) => expr.get_location(),
            Stmt::Print(stmt) => stmt.get_location(),
            Stmt::Return(location, _) => *location,
            Stmt::Assign(_, _, stmt) => stmt.get_location(),
            Stmt::Block(stmts) => stmts
                .last()
                .expect("Block should always have at least 1 statement.")
                .get_location(),
            Stmt::If(expr, _, _) => expr.get_location(),
            Stmt::While(stmt, _) => stmt.get_location(),
            Stmt::Function(token, _, _) => token.location,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Expr {
    Float(Location, f64),
    Integer(Location, i64),
    Bool(Location, bool),
    String(Location, String),
    Nil(Location),
    Unary(UnaryOp, Box<Expr>),
    Binary(Box<Expr>, BinaryOp, Box<Expr>),
    Variable(Location, String),
    Condition(Box<Expr>, Token, Box<Expr>),
    // Callee and its parameters
    Call(Box<Expr>, Vec<Box<Expr>>),
}

impl Expr {
    pub fn get_location(&self) -> Location {
        match self {
            Expr::Float(location, _) => *location,
            Expr::Integer(location, _) => *location,
            Expr::Bool(location, _) => *location,
            Expr::String(location, _) => *location,
            Expr::Nil(location) => *location,
            Expr::Unary(_, expr) => expr.get_location(),
            Expr::Binary(expr, _, _) => expr.get_location(),
            Expr::Variable(location, _) => *location,
            Expr::Condition(_, token, _) => token.location,
            Expr::Call(expr, _) => expr.get_location(),
        }
    }
}

impl Expr {
    pub fn display(&self, level: usize) -> String {
        match self {
            Expr::Float(_, token) => "\t".repeat(level) + token.to_string().as_str() + "\n",
            Expr::Integer(_, token) => "\t".repeat(level) + token.to_string().as_str() + "\n",
            Expr::Bool(_, token) => "\t".repeat(level) + token.to_string().as_str() + "\n",
            Expr::String(_, token) => "\t".repeat(level) + token.to_string().as_str() + "\n",
            Expr::Nil(_) => "\t".repeat(level) + "Nil\n",
            Expr::Unary(op, expr) => {
                "\t".repeat(level)
                    + format!("{:?}{}\n", op, expr.display(level + 1).as_str()).as_str()
            }
            Expr::Binary(expr, op, expr1) => {
                expr.display(level + 1)
                    + "\t".repeat(level).as_str()
                    + format!("{:?}\n{}", op, expr1.display(level + 1)).as_str()
            }
            Expr::Condition(expr, token, expr1) => {
                expr.display(level + 1)
                    + "\t".repeat(level).as_str()
                    + format!("{:?}\n{}", token.token_type, expr1.display(level + 1)).as_str()
            }
            Expr::Variable(_, identifier) => {
                "\t".repeat(level) + "Variable: " + identifier.as_str() + "\n"
            }
            Expr::Call(expr, exprs) => {
                "\t".repeat(level) + format!("Call '{expr:?}' with [{exprs:?}]\n").as_str()
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
            TokenType::Or => (None, Some(Parser::logical), Precedence::Or),
            TokenType::And => (None, Some(Parser::logical), Precedence::And),
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
            TokenType::LeftParenthese => {
                (Some(Parser::grouping), Some(Parser::call), Precedence::Call)
            }
            TokenType::RightParenthese => (None, None, Precedence::None),
            TokenType::Greater
            | TokenType::GreaterEqual
            | TokenType::Less
            | TokenType::LessEqual => (None, Some(Parser::binary), Precedence::Comparison),
            TokenType::EqualEqual | TokenType::BangEqual => {
                (None, Some(Parser::binary), Precedence::Equality)
            }
            TokenType::Semicolon => (None, None, Precedence::None),
            TokenType::LeftBrace => (None, None, Precedence::None),
            TokenType::RightBrace => (None, None, Precedence::None),
            _ => {
                return Err(ParseErrorType::UnexpectedToken);
            }
        })
    }
}
