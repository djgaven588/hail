use crate::{KEYWORDS, Location};

#[derive(Debug, Clone)]
pub struct Token {
    pub location: Location,
    pub token_type: TokenType,
    pub lexeme: String,
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
    Minus,
    Plus,
    Colon,
    Scope,
    Semicolon,
    Slash,
    Star,

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

    // Literals
    Identifier,
    String,
    Integer,
    Float,

    // Keywords
    If,
    Else,
    While,
    Fn,
    Return,
    Let,
    Nil,
    True,
    False,
    Struct,
    Import,

    // Special
    DocComment,
    Eof,
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
            location: Location::new(1, 0, 0),
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
                .push(Token::new(self.location.clone(), TokenType::Eof, ""));
            return false;
        };

        match next {
            '\n' => {
                self.location.line += 1;
                self.location.column = 0;
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
            '.' => self.push_token(TokenType::Dot),
            '-' => self.push_token(TokenType::Minus),
            '+' => self.push_token(TokenType::Plus),
            '*' => self.push_token(TokenType::Star),
            ':' => {
                if self.try_match(':') {
                    self.push_token(TokenType::Scope);
                } else {
                    self.push_token(TokenType::Colon);
                }
            }
            ';' => self.push_token(TokenType::Semicolon),
            '/' => {
                self.slash_parse();
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
            v => {
                if v.is_ascii_digit() {
                    self.number_parse();
                } else if v.is_ascii_alphabetic() || v == '_' {
                    self.identifier_parse();
                } else if v.is_whitespace() {
                    // This is whitespace, ignore.
                } else {
                    self.syntax_errors.push(SyntaxError::new(
                        self.location.clone(),
                        SyntaxErrorType::UnexpectedCharacter(v),
                    ));
                }
            }
        }

        self.working_start += self.location.length;
        self.location.column += self.location.length;
        self.location.length = 0;

        true
    }

    fn string_parse(&mut self) {
        while let Some(next) = self.peek()
            && next != '"'
        {
            if next == '\n' {
                self.location.line += 1;
                self.location.column = 0;
            }

            // Discard
            let _ = self.consume();
        }

        // Terminate
        if !self.try_match('"') {
            self.syntax_errors.push(SyntaxError::new(
                self.location.clone(),
                SyntaxErrorType::UnterminatedString,
            ));
        } else {
            self.push_token(TokenType::String);
        }
    }

    fn identifier_parse(&mut self) {
        // We're looking for an identifier or keyword
        while let Some(next) = self.peek()
            && (next.is_ascii_alphanumeric() || next == '_')
        {
            // Chew characters
            let _ = self.consume();
        }

        if let Some(keyword) = KEYWORDS.iter().find(|v| {
            v.0 == &self.source[self.working_start..(self.working_start + self.location.length)]
        }) {
            self.push_token(keyword.1);
        } else {
            self.push_token(TokenType::Identifier);
        }
    }

    fn number_parse(&mut self) {
        // Consume before the decimal (integer part)
        while let Some(next) = self.peek()
            && next.is_ascii_digit()
        {
            // Chew number
            let _ = self.consume();
        }

        // We're looking for a float
        if self.peek().is_some_and(|v| v == '.')
            && self.peek_next().is_some_and(|v| v.is_ascii_digit())
        {
            // Float, with additional digits

            // Chew decimal point
            let _ = self.consume();

            while let Some(next) = self.peek()
                && next.is_ascii_digit()
            {
                // Chew number
                let _ = self.consume();
            }

            self.push_token(TokenType::Float);
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
            // This is a multi-line comment
            let mut level = 1;
            while let Some(next) = self.consume() {
                if next == '\n' {
                    self.location.line += 1;
                    self.location.column = 0;
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
                    self.location.clone(),
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
            self.location.clone(),
            token_type,
            &self.source[self.working_start..(self.working_start + self.location.length)],
        ));
    }

    fn consume(&mut self) -> Option<char> {
        let remaining = &self.source[(self.working_start + self.location.length)..];
        let character = remaining.chars().next()?;
        self.location.length += character.len_utf8();

        Some(character)
    }

    fn try_match(&mut self, expected: char) -> bool {
        let remaining = &self.source[(self.working_start + self.location.length)..];
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
        let remaining = &self.source[(self.working_start + self.location.length)..];
        remaining.chars().next()
    }

    fn peek_next(&self) -> Option<char> {
        let remaining = &self.source[(self.working_start + self.location.length)..];
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
}

#[derive(Debug)]
pub struct SyntaxError {
    location: Location,
    error: SyntaxErrorType,
}

impl SyntaxError {
    pub fn new(location: Location, error: SyntaxErrorType) -> SyntaxError {
        SyntaxError { location, error }
    }
}

#[derive(Debug)]
pub enum SyntaxErrorType {
    UnexpectedCharacter(char),
    UnterminatedString,
    UnterminatedComment,
}
