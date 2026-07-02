use crate::{
    Location,
    constructor::ConstructError,
    parser::ParseError,
    scanner::{SyntaxError, Token, TokenType},
};
use std::fmt;

const SEPERATOR_BAR_LENGTH: usize = 60;
const TAB_LENGTH: usize = 2;
const CONTEXT_LINES: u32 = 2;

/// ANSI color codes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnsiColor {
    Reset,
    Bold,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    White,
    Gray,
    BrightRed,
    BrightYellow,
    BrightCyan,
}

impl AnsiColor {
    fn code(self) -> &'static str {
        match self {
            AnsiColor::Reset => "\x1b[0m",
            AnsiColor::Bold => "\x1b[1m",
            AnsiColor::Red => "\x1b[31m",
            AnsiColor::Green => "\x1b[32m",
            AnsiColor::Yellow => "\x1b[33m",
            AnsiColor::Blue => "\x1b[34m",
            AnsiColor::Magenta => "\x1b[35m",
            AnsiColor::Cyan => "\x1b[36m",
            AnsiColor::White => "\x1b[97m",
            AnsiColor::Gray => "\x1b[90m",
            AnsiColor::BrightRed => "\x1b[91m",
            AnsiColor::BrightYellow => "\x1b[93m",
            AnsiColor::BrightCyan => "\x1b[96m",
        }
    }

    /// Wrap text with this color, applying reset at the end
    fn apply(self, text: &str) -> String {
        format!("{}{text}{}", self.code(), AnsiColor::Reset.code())
    }
}

impl fmt::Display for AnsiColor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.code())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
    Lint,
}

impl Severity {
    fn title(self) -> &'static str {
        match self {
            Severity::Error => "Error",
            Severity::Warning => "Warning",
            Severity::Lint => "Lint",
        }
    }

    fn color(self) -> AnsiColor {
        match self {
            Severity::Error => AnsiColor::Red,
            Severity::Warning => AnsiColor::BrightYellow,
            Severity::Lint => AnsiColor::BrightCyan,
        }
    }
}

pub trait Diagnostic {
    fn severity(&self) -> Severity;
    fn location(&self) -> Location;
    fn message(&self) -> String;
}

impl Diagnostic for SyntaxError {
    fn severity(&self) -> Severity {
        Severity::Error
    }

    fn location(&self) -> Location {
        self.location
    }

    fn message(&self) -> String {
        self.message()
    }
}

impl Diagnostic for ParseError {
    fn severity(&self) -> Severity {
        Severity::Error
    }

    fn location(&self) -> Location {
        self.token.location
    }

    fn message(&self) -> String {
        self.message()
    }
}

impl Diagnostic for ConstructError {
    fn severity(&self) -> Severity {
        Severity::Error
    }

    fn location(&self) -> Location {
        self.location
    }

    fn message(&self) -> String {
        self.message()
    }
}

/// Get tokens that fall on a specific line (1-indexed), excluding comments and EOF.
fn tokens_on_line<'a>(tokens: &'a [Token], line: u32) -> Vec<&'a Token> {
    tokens
        .iter()
        .filter(|t| {
            t.location.line == line
                && !matches!(t.token_type, TokenType::DocComment | TokenType::Eof)
        })
        .collect()
}

/// Color a token based on its type.
fn color_token(token: &Token) -> AnsiColor {
    match token.token_type {
        // String literals
        TokenType::String => AnsiColor::Green,
        // Numbers
        TokenType::Integer | TokenType::Float => AnsiColor::Cyan,
        // Keywords
        TokenType::If
        | TokenType::Else
        | TokenType::While
        | TokenType::Loop
        | TokenType::For
        | TokenType::Fn
        | TokenType::Return
        | TokenType::Break
        | TokenType::Continue
        | TokenType::Let
        | TokenType::True
        | TokenType::False
        | TokenType::Struct
        | TokenType::Import
        | TokenType::Mut
        | TokenType::Const
        | TokenType::In
        | TokenType::As => AnsiColor::Yellow,
        // Comments special
        TokenType::DocComment => AnsiColor::Gray,
        // Operators
        TokenType::Plus
        | TokenType::Minus
        | TokenType::Star
        | TokenType::Slash
        | TokenType::Percent
        | TokenType::Bang
        | TokenType::Equal
        | TokenType::And
        | TokenType::Or
        | TokenType::BinaryAnd
        | TokenType::BinaryAndEqual
        | TokenType::BinaryOr
        | TokenType::BinaryOrEqual
        | TokenType::BinaryXor
        | TokenType::BinaryXorEqual
        | TokenType::BinaryShiftLeft
        | TokenType::BinaryShiftRight
        | TokenType::Arrow
        | TokenType::SlashEqual
        | TokenType::PlusEqual
        | TokenType::MinusEqual
        | TokenType::StarEqual
        | TokenType::PercentEqual => AnsiColor::Magenta,
        TokenType::EqualEqual
        | TokenType::BangEqual
        | TokenType::GreaterEqual
        | TokenType::LessEqual => AnsiColor::Magenta,
        // Punctuation
        TokenType::LeftParenthese
        | TokenType::RightParenthese
        | TokenType::LeftBrace
        | TokenType::RightBrace
        | TokenType::LeftSquare
        | TokenType::RightSquare
        | TokenType::Comma
        | TokenType::Dot
        | TokenType::DotDot
        | TokenType::DotDotEqual
        | TokenType::Colon
        | TokenType::Scope
        | TokenType::Semicolon
        | TokenType::Less
        | TokenType::Greater => AnsiColor::Gray,
        // Identifiers and literals without special coloring
        TokenType::Identifier => AnsiColor::White,
        TokenType::Eof => AnsiColor::Reset,
    }
}

/// Calculate the number of visible characters in a string (handles tabs)
fn visible_length(s: &str) -> usize {
    s.chars()
        .map(|c| if c == '\t' { TAB_LENGTH } else { 1 })
        .sum()
}

/// Highlight a single line using token information.
/// Iterates through tokens for the line and colors their lexemes,
/// leaving whitespace and other characters uncolored (except error highlighting).
fn highlight_line_with_tokens<'a>(
    line: &str,
    severity: Severity,
    column: u16,
    length: u16,
    tokens: &[&'a Token],
) -> String {
    let chars: Vec<char> = line.chars().collect();
    let len = chars.len();

    let mut result = String::new();
    let mut char_idx = 0;

    for token in tokens {
        // Calculate the visible column position of this token's start
        let token_start_col = (token.location.column as usize).saturating_sub(1);

        // Skip whitespace before this token
        while char_idx < len && char_idx < token_start_col {
            let ch = chars[char_idx];
            if ch == '\t' {
                result.push_str(&" ".repeat(TAB_LENGTH));
            } else {
                result.push(ch);
            }
            char_idx += 1;
        }

        // Color the token's lexeme
        let is_error = token.location.column >= column && (token.location.column - column) < length;
        if is_error {
            result.push_str(&AnsiColor::Bold.apply(&severity.color().apply(&token.lexeme)));
        } else {
            let color = color_token(token);
            // Check for type annotation hint (identifier followed by ':')
            let should_bold = if token.token_type == TokenType::Identifier
                && char_idx + token.lexeme.len() < len
                && chars[char_idx + token.lexeme.len()] == ':'
            {
                AnsiColor::Bold.apply(&color.apply(&token.lexeme))
            } else {
                color.apply(&token.lexeme)
            };
            result.push_str(&should_bold);
        }

        char_idx += token.lexeme.len();
    }

    // Color any remaining characters after the last token
    while char_idx < len {
        let ch = chars[char_idx];
        result.push(ch);
        char_idx += 1;
    }

    result
}

/// Render source context around an error location.
/// Shows up to `context_lines` lines before and after the error.
fn render_context(
    source: &str,
    severity: Severity,
    location: Location,
    tokens: &[Token],
) -> String {
    let lines = source.lines().collect::<Vec<_>>();

    // Clamp line numbers to available range so we always show something useful
    // (Unterminated string at the end of a file, line 10 in a 5 line file, ect.)
    if lines.is_empty() {
        return String::new();
    }

    let error_line = location.line.saturating_sub(1).min(lines.len() as u32 - 1);
    let actual_error_line = error_line + 1; // Back to 1 indexed for display

    // Context
    let start_line = actual_error_line.saturating_sub(CONTEXT_LINES).max(1);
    let end_line = std::cmp::min(actual_error_line + CONTEXT_LINES, lines.len() as u32);

    let mut result = String::new();

    for line_num in start_line..=end_line {
        if let Some(line) = lines.get(line_num as usize - 1).copied() {
            // Line number column
            let line_num_str = format!("{line_num:>4} │ ");

            if line_num == actual_error_line {
                // Error line: highlight with red background indicator
                result.push_str(&severity.color().apply(&line_num_str));
                let line_tokens = tokens_on_line(tokens, line_num);
                let highlighted = highlight_line_with_tokens(
                    line,
                    severity,
                    location.column,
                    location.length,
                    &line_tokens,
                );
                result.push_str(&highlighted);
                result.push('\n');

                // Caret row pointing to the error column
                let caret_prefix = "   │ ";
                let spaces_before =
                    visible_length(&line[..(location.column as usize).min(line.len())]);
                let carets: String = std::iter::repeat('^')
                    .take(location.length.max(1) as usize)
                    .collect();

                result.push_str(caret_prefix);
                result.push(' ');
                result.push_str(&" ".repeat(spaces_before));
                result.push_str(&severity.color().apply(&carets));
                result.push('\n');
            } else {
                // Non-error line: normal formatting
                result.push_str(&line_num_str);
                let line_tokens = tokens_on_line(tokens, line_num);
                let highlighted = highlight_line_with_tokens(line, severity, 0, 0, &line_tokens);
                result.push_str(&highlighted);
                result.push('\n');
            }
        }
    }

    result
}

/// Format a complete diagnostic with header, context, and message.
///
/// ## Example:
/// ──[ Hail | Error @ ./scripts/basic.hail ]───────────────────
///   1 │ 1 * 1 * 3 + 7 / 2
///   2 │ $$$ /*
///   │      ^^
///   3 │
///   4 │ a
///
///Unterminated multi-line comment.
pub fn format_diagnostic(
    source: &str,
    path: Option<&str>,
    diagnostic: &dyn Diagnostic,
    tokens: &[Token],
) -> String {
    let severity = diagnostic.severity();
    let location = diagnostic.location();
    let message = diagnostic.message();

    // Title bar
    let title_color = severity.color();
    let mut title_bar = format!("──[ Hail | {} ", severity.title());
    if let Some(path) = path {
        title_bar.push_str(&format!(
            "@ \x1b]8;;file://{}\x1b\\{}\x1b]8;;\x1b\\ ",
            path, path
        ));
    }

    title_bar.push_str("]──");

    title_bar.push_str(&"─".repeat(SEPERATOR_BAR_LENGTH.saturating_sub(
        title_bar.chars().filter(|v| !v.is_control()).count()
            - "]8;;file://{}]8;;".len()
            - path.map_or(0, |v| v.len()),
    )));

    let mut output = String::new();
    output.push_str(&title_color.apply(&title_bar));
    output.push('\n');

    // Context lines
    let context = render_context(source, severity, location, tokens);
    if !context.is_empty() {
        output.push_str(&context);
    } else {
        // No source available or line out of bounds - show minimal info
        output.push_str("   │ (source not available)\n");
    }

    // Message
    output.push('\n');
    output.push_str(&severity.color().apply(&message));
    output.push('\n');

    output
}

/// Format multiple diagnostics at once, with separators
pub fn format_diagnostics<T>(
    source: &str,
    path: Option<&str>,
    diagnostics: &[T],
    tokens: &[Token],
) -> String
where
    T: Diagnostic,
{
    if diagnostics.is_empty() {
        return String::new();
    }

    let mut output = String::new();

    for (i, diagnostic) in diagnostics.iter().enumerate() {
        if i > 0 {
            // Separator between diagnostics
            output.push_str(&AnsiColor::Gray.apply("─".repeat(SEPERATOR_BAR_LENGTH).as_str()));
            output.push('\n');
        }
        output.push_str(&format_diagnostic(source, path, diagnostic, tokens));
    }

    output
}
