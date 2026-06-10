use crate::{Location, parser::AssignmentOp};

pub const KEYWORDS: [(&str, TokenType); 18] = [
    ("let", TokenType::Let),
    ("if", TokenType::If),
    ("else", TokenType::Else),
    ("while", TokenType::While),
    ("for", TokenType::For),
    ("loop", TokenType::Loop),
    ("fn", TokenType::Fn),
    ("return", TokenType::Return),
    ("break", TokenType::Break),
    ("continue", TokenType::Continue),
    ("true", TokenType::True),
    ("false", TokenType::False),
    ("struct", TokenType::Struct),
    ("import", TokenType::Import),
    ("mut", TokenType::Mut),
    ("const", TokenType::Const),
    ("in", TokenType::In),
    ("as", TokenType::As),
];

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub location: Location,
    pub token_type: TokenType,
    pub lexeme: String,
}

impl Token {
    pub fn new(location: Location, token_type: TokenType, lexeme: &str) -> Token {
        Token {
            location,
            token_type,
            lexeme: lexeme.to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenType {
    // Single Characters
    LeftParenthese,
    RightParenthese,
    LeftBrace,
    RightBrace,
    LeftSquare,
    RightSquare,
    Comma,
    Dot,
    DotDot,
    DotDotEqual,
    Minus,
    Plus,
    Colon,
    Scope,
    Semicolon,
    Slash,
    Star,
    Percent,

    //
    Bang,
    BangEqual,
    Equal,
    EqualEqual,
    Greater,
    GreaterEqual,
    Less,
    LessEqual,
    BinaryAnd,
    And,
    BinaryOr,
    Or,
    PlusEqual,
    MinusEqual,
    StarEqual,
    SlashEqual,
    PercentEqual,
    Arrow,

    // Literals
    Identifier,
    String,
    Integer,
    Float,

    // Keywords
    If,
    Else,
    While,
    Loop,
    For,
    Fn,
    Return,
    Break,
    Continue,
    Let,
    True,
    False,
    Struct,
    Import,
    Mut,
    Const,
    In,
    As,

    // Special
    DocComment,
    Eof,
}

impl TokenType {
    pub fn is_assignment_op(&self) -> bool {
        match self {
            TokenType::Equal
            | TokenType::PlusEqual
            | TokenType::MinusEqual
            | TokenType::StarEqual
            | TokenType::SlashEqual => true,
            _ => false,
        }
    }

    pub fn assignment_ops() -> &'static [TokenType] {
        &[
            TokenType::Equal,
            TokenType::PlusEqual,
            TokenType::MinusEqual,
            TokenType::StarEqual,
            TokenType::SlashEqual,
        ]
    }

    pub fn unwrap_assignment_op(&self) -> AssignmentOp {
        match self {
            TokenType::Equal => AssignmentOp::Equal,
            TokenType::PlusEqual => AssignmentOp::PlusEqual,
            TokenType::MinusEqual => AssignmentOp::MinusEqual,
            TokenType::StarEqual => AssignmentOp::MultiplyEqual,
            TokenType::SlashEqual => AssignmentOp::DivideEqual,
            a => unreachable!("{a:?}"),
        }
    }
}

#[derive(Debug)]
pub struct Scanner {
    source: String,
    syntax_errors: Vec<SyntaxError>,
    tokens: Vec<Token>,
    working_start: usize,
    location: Location,
    is_done: bool,
}

impl Scanner {
    pub fn new(source: String) -> Scanner {
        Scanner {
            is_done: false,
            source,
            syntax_errors: vec![],
            tokens: vec![],
            working_start: 0,
            location: Location::new(1, 1, 0),
        }
    }

    pub fn scan(&mut self) {
        while self.step() {}
    }

    pub fn step(&mut self) -> bool {
        if self.is_done {
            return true;
        }

        let Some(next) = self.consume() else {
            self.is_done = true;
            self.tokens
                .push(Token::new(self.location, TokenType::Eof, ""));
            return false;
        };

        match next {
            '\n' => {
                self.location.line += 1;
                self.location.column = 1;
                self.location.length = 0;
                self.working_start += 1;
                return true;
            }
            '(' => self.push_token(TokenType::LeftParenthese),
            ')' => self.push_token(TokenType::RightParenthese),
            '{' => self.push_token(TokenType::LeftBrace),
            '}' => self.push_token(TokenType::RightBrace),
            '[' => self.push_token(TokenType::LeftSquare),
            ']' => self.push_token(TokenType::RightSquare),
            ',' => self.push_token(TokenType::Comma),
            '.' => {
                // Distinguish '.', '..', and '..='
                if self.try_match('.') {
                    if self.try_match('=') {
                        self.push_token(TokenType::DotDotEqual);
                    } else {
                        self.push_token(TokenType::DotDot);
                    }
                } else {
                    self.push_token(TokenType::Dot)
                }
            }
            '-' => {
                if self.try_match('=') {
                    self.push_token(TokenType::MinusEqual);
                } else if self.try_match('>') {
                    self.push_token(TokenType::Arrow);
                } else {
                    self.push_token(TokenType::Minus);
                }
            }
            '+' => {
                if self.try_match('=') {
                    self.push_token(TokenType::PlusEqual);
                } else {
                    self.push_token(TokenType::Plus);
                }
            }
            '*' => {
                if self.try_match('=') {
                    self.push_token(TokenType::StarEqual);
                } else {
                    self.push_token(TokenType::Star);
                }
            }
            ':' => {
                if self.try_match(':') {
                    self.push_token(TokenType::Scope);
                } else {
                    self.push_token(TokenType::Colon);
                }
            }
            ';' => self.push_token(TokenType::Semicolon),
            '/' => {
                if self.try_match('=') {
                    self.push_token(TokenType::SlashEqual);
                } else {
                    self.slash_parse();
                }
            }
            '!' => {
                if self.try_match('=') {
                    self.push_token(TokenType::BangEqual);
                } else {
                    self.push_token(TokenType::Bang);
                }
            }
            '=' => {
                if self.try_match('=') {
                    self.push_token(TokenType::EqualEqual);
                } else {
                    self.push_token(TokenType::Equal);
                }
            }
            '<' => {
                if self.try_match('=') {
                    self.push_token(TokenType::LessEqual);
                } else {
                    self.push_token(TokenType::Less);
                }
            }
            '>' => {
                if self.try_match('=') {
                    self.push_token(TokenType::GreaterEqual);
                } else {
                    self.push_token(TokenType::Greater);
                }
            }
            '&' => {
                if self.try_match('&') {
                    self.push_token(TokenType::And);
                } else {
                    self.push_token(TokenType::BinaryAnd);
                }
            }
            '|' => {
                if self.try_match('|') {
                    self.push_token(TokenType::Or);
                } else {
                    self.push_token(TokenType::BinaryOr);
                }
            }
            '"' => {
                self.string_parse();
            }
            '%' => {
                if self.try_match('=') {
                    self.push_token(TokenType::PercentEqual);
                } else {
                    self.push_token(TokenType::Percent);
                }
            }
            v => {
                if v.is_ascii_digit() {
                    self.number_parse();
                } else if v.is_ascii_alphabetic() || v == '_' || v == '<' || v == '>' {
                    // The < and > are here to give the illusion of generic types
                    self.identifier_parse();
                } else if v.is_whitespace() {
                    // This is whitespace, ignore.
                } else {
                    self.syntax_errors.push(SyntaxError::new(
                        self.location,
                        SyntaxErrorType::UnexpectedCharacter(v),
                    ));
                }
            }
        }

        self.working_start += self.location.length as usize;
        self.location.column += self.location.length;
        self.location.length = 0;

        true
    }

    fn string_parse(&mut self) {
        // Save start location before consuming characters so errors point to the opening quote
        let mut noted_location = self.location;

        let mut final_string = "".to_string();

        let mut backslashed = false;
        while let Some(next) = self.peek()
            && (backslashed || next != '"')
        {
            let char = self.consume().expect("Should have character");

            if next == '\n' {
                self.location.line += 1;
                self.location.column = 1;
            }

            if backslashed {
                match next {
                    'n' => {
                        // Build string
                        final_string.push('\n');
                    }
                    '\\' | '"' => {
                        final_string.push(next);
                    }
                    'x' => {
                        // Hex escape?
                        // This is ugly, I know, it's also easy :P
                        if self.peek().is_some_and(|v| v == '1')
                            && self.peek_next().is_some_and(|v| v == 'b')
                        {
                            // Extra consumes
                            self.consume().expect("Should have.");
                            self.consume().expect("Should have.");

                            final_string.push('\x1b');
                        } else {
                            self.syntax_errors.push(SyntaxError::new(
                                self.location,
                                SyntaxErrorType::UnexpectedCharacter('x'),
                            ));
                        }
                    }
                    char => {
                        self.syntax_errors.push(SyntaxError::new(
                            self.location,
                            SyntaxErrorType::UnexpectedCharacter(char),
                        ));
                    }
                }
            } else if next == '\\' {
                // Next character is free to go
                backslashed = true;
                continue;
            } else {
                // Build string
                final_string.push(char);
            }

            // End backslashing
            backslashed = false;
        }

        // Terminate
        if !self.try_match('"') {
            self.syntax_errors.push(SyntaxError::new(
                noted_location,
                SyntaxErrorType::UnterminatedString,
            ));
        } else {
            noted_location.column = self.location.column + 1;
            noted_location.length = self.location.length.saturating_sub(1);

            self.tokens
                .push(Token::new(noted_location, TokenType::String, &final_string));
        }
    }

    fn identifier_parse(&mut self) {
        // We're looking for an identifier or keyword
        while let Some(next) = self.peek()
            && (next.is_ascii_alphanumeric() || next == '_' || next == '<' || next == '>')
        {
            // Chew characters
            let _ = self.consume();
        }

        if let Some(keyword) = KEYWORDS.iter().find(|v| {
            v.0 == &self.source
                [self.working_start..(self.working_start + self.location.length as usize)]
        }) {
            self.push_token(keyword.1);
        } else {
            self.push_token(TokenType::Identifier);
        }
    }

    fn number_parse(&mut self) {
        // Consume before the decimal (integer part)
        // Underscores are allowed for readability
        while let Some(next) = self.peek()
            && (next.is_ascii_digit() || next == '_')
        {
            // Chew number
            let _ = self.consume();
        }

        // We're looking for a float
        if self.peek().is_some_and(|v| v == '.') {
            if self
                .peek_next()
                .is_some_and(|v| v.is_ascii_digit() || (v != '.' && !v.is_ascii_alphanumeric()))
            {
                // Chew decimal point
                let _ = self.consume();

                // Float, with additional digits
                while let Some(next) = self.peek()
                    && (next.is_ascii_digit() || next == '_')
                {
                    // Chew number
                    let _ = self.consume();
                }

                self.push_token(TokenType::Float);
            } else {
                // Probably a dot call on an integer, don't chew the decimal
                self.push_token(TokenType::Integer);
            }
        } else {
            self.push_token(TokenType::Integer);
        }
    }

    fn slash_parse(&mut self) {
        // Comment, doc comment, or slash?
        if self.try_match('/') {
            // We want this as a comment, is it a doc comment?
            if self.try_match('/') {
                // It's a *doc* comment!
                while let Some(next) = self.peek()
                    && next != '\n'
                {
                    // Chew comment
                    let _ = self.consume();
                }

                self.push_token(TokenType::DocComment);
            } else {
                // Normal comment
                while self.peek().is_some_and(|v| v != '\n') {
                    // Chew comment
                    let _ = self.consume();
                }
            }
        } else if self.try_match('*') {
            // This is a multi-line comment, save start location before consuming
            let error_location = self.location;
            let mut level = 1;
            while let Some(next) = self.consume() {
                if next == '\n' {
                    self.location.line += 1;
                    self.location.column = 1;
                    continue;
                }
                // Another level
                if next == '/' && self.try_match('*') {
                    level += 1;
                } else if next == '*' && self.try_match('/') {
                    level -= 1;
                    // We've made it to the bottom of the stack
                    if level == 0 {
                        break;
                    }
                }
            }

            if level == 0 {
                // Just a comment
            } else {
                self.syntax_errors.push(SyntaxError::new(
                    error_location,
                    SyntaxErrorType::UnterminatedComment,
                ));
            }
        } else {
            // It's just a slash
            self.push_token(TokenType::Slash);
        }
    }

    fn push_token(&mut self, token_type: TokenType) {
        self.tokens.push(Token::new(
            self.location,
            token_type,
            &self.source[self.working_start..(self.working_start + self.location.length as usize)],
        ));
    }

    fn consume(&mut self) -> Option<char> {
        let remaining = &self.source[(self.working_start + self.location.length as usize)..];
        let character = remaining.chars().next()?;
        self.location.length += character.len_utf8() as u16;

        Some(character)
    }

    fn try_match(&mut self, expected: char) -> bool {
        let remaining = &self.source[(self.working_start + self.location.length as usize)..];
        let next = remaining.chars().next();
        if next.is_some_and(|v| v == expected) {
            // We don't need to care about the result, but we need to still consume it
            let _ = self.consume();
            true
        } else {
            false
        }
    }

    fn peek(&self) -> Option<char> {
        let remaining = &self.source[(self.working_start + self.location.length as usize)..];
        remaining.chars().next()
    }

    fn peek_next(&self) -> Option<char> {
        let remaining = &self.source[(self.working_start + self.location.length as usize)..];
        let mut chars = remaining.chars();
        chars.next()?;
        chars.next()
    }

    pub fn errors(&self) -> &[SyntaxError] {
        &self.syntax_errors
    }

    pub fn get(&self) -> Option<&[Token]> {
        if !self.is_done {
            None
        } else {
            Some(&self.tokens)
        }
    }

    /// Returns the source text that was scanned
    pub fn source(&self) -> &str {
        &self.source
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct SyntaxError {
    pub location: Location,
    error: SyntaxErrorType,
}

impl SyntaxError {
    pub fn new(location: Location, error: SyntaxErrorType) -> SyntaxError {
        SyntaxError { location, error }
    }

    pub fn message(&self) -> String {
        match &self.error {
            SyntaxErrorType::UnexpectedCharacter(c) => format!("Unexpected character '{c}'."),
            SyntaxErrorType::UnterminatedString => "Unterminated string literal.".to_string(),
            SyntaxErrorType::UnterminatedComment => "Unterminated multi-line comment.".to_string(),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum SyntaxErrorType {
    UnexpectedCharacter(char),
    UnterminatedString,
    UnterminatedComment,
}
