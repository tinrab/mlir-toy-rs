//! The lexer of the Toy language (chapter 1).
//!
//! The lexer turns source text into a stream of [`Token`]s, one at a time, and tracks the location of each token for diagnostics.

use std::{
    fmt::{self, Display, Formatter},
    iter::Peekable,
    str::Chars,
    sync::Arc,
};

use crate::ast::Location;

/// A token of the Toy language.
#[derive(Clone, Debug, PartialEq)]
pub enum Token {
    Eof,
    // Keywords.
    Return,
    Var,
    Def,
    Struct,
    // Primary tokens.
    Identifier(String),
    Number(f64),
    /// Any other single character, such as punctuation or an operator.
    Char(char),
}

impl Display for Token {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Eof => formatter.write_str("<eof>"),
            Self::Return => formatter.write_str("'return'"),
            Self::Var => formatter.write_str("'var'"),
            Self::Def => formatter.write_str("'def'"),
            Self::Struct => formatter.write_str("'struct'"),
            Self::Identifier(name) => write!(formatter, "identifier '{name}'"),
            Self::Number(value) => write!(formatter, "number '{value}'"),
            Self::Char(char) => write!(formatter, "'{char}'"),
        }
    }
}

/// Splits Toy source text into tokens.
///
/// The lexer always looks at one "current" token; the parser consumes it with [`Lexer::advance`].
pub struct Lexer<'a> {
    chars: Peekable<Chars<'a>>,
    file: Arc<str>,
    /// The location of the next character in `chars`.
    line: u32,
    column: u32,
    current: Token,
    /// The location of the first character of `current`.
    location: Location,
}

impl<'a> Lexer<'a> {
    /// Creates a lexer for `source` and reads its first token.
    /// `file` is only used for locations.
    #[must_use]
    pub fn new(source: &'a str, file: &str) -> Self {
        let file = Arc::<str>::from(file);
        let mut lexer = Self {
            chars: source.chars().peekable(),
            location: Location {
                file: Arc::clone(&file),
                line: 1,
                column: 1,
            },
            file,
            line: 1,
            column: 1,
            current: Token::Eof,
        };
        lexer.advance();
        lexer
    }

    /// Returns the current token.
    #[must_use]
    pub fn current(&self) -> &Token {
        &self.current
    }

    /// Returns the location of the first character of the current token.
    #[must_use]
    pub fn location(&self) -> Location {
        self.location.clone()
    }

    /// Moves to the next token.
    pub fn advance(&mut self) {
        self.current = self.next_token();
    }

    fn next_char(&mut self) -> Option<char> {
        let char = self.chars.next()?;
        if char == '\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }
        Some(char)
    }

    fn next_char_if(&mut self, predicate: impl Fn(char) -> bool) -> Option<char> {
        let &char = self.chars.peek()?;
        predicate(char).then(|| self.next_char()).flatten()
    }

    fn next_token(&mut self) -> Token {
        loop {
            // Skip any whitespace.
            while self.next_char_if(char::is_whitespace).is_some() {}

            // Save the location before reading the token's characters.
            self.location = Location {
                file: Arc::clone(&self.file),
                line: self.line,
                column: self.column,
            };

            let Some(char) = self.next_char() else {
                return Token::Eof;
            };

            // Comment until the end of the line.
            if char == '#' {
                while self.next_char_if(|char| char != '\n').is_some() {}
                continue;
            }

            // Identifier: [a-zA-Z][a-zA-Z0-9_]*
            if char.is_ascii_alphabetic() {
                let mut identifier = String::from(char);
                while let Some(char) =
                    self.next_char_if(|char| char.is_ascii_alphanumeric() || char == '_')
                {
                    identifier.push(char);
                }

                return match identifier.as_str() {
                    "return" => Token::Return,
                    "def" => Token::Def,
                    "struct" => Token::Struct,
                    "var" => Token::Var,
                    _ => Token::Identifier(identifier),
                };
            }

            // Number: [0-9] ([0-9.])*
            if char.is_ascii_digit() {
                let mut number = String::from(char);
                while let Some(char) =
                    self.next_char_if(|char| char.is_ascii_digit() || char == '.')
                {
                    number.push(char);
                }

                return Token::Number(parse_number(&number));
            }

            // Otherwise, just return the character itself.
            return Token::Char(char);
        }
    }
}

/// Parses the longest prefix of `number` that is a valid float, like C's `strtod`: `1.2.3` is read as `1.2`.
fn parse_number(number: &str) -> f64 {
    (1..=number.len())
        .rev()
        .find_map(|length| number[..length].parse().ok())
        .expect("a number token starts with a digit")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokens(source: &str) -> Vec<(Token, u32, u32)> {
        let mut lexer = Lexer::new(source, "test.toy");
        let mut tokens = Vec::new();
        while *lexer.current() != Token::Eof {
            let location = lexer.location();
            tokens.push((lexer.current().clone(), location.line, location.column));
            lexer.advance();
        }
        tokens
    }

    #[test]
    fn test_lexes_tokens_with_locations() {
        assert_eq!(
            tokens("def main() {\n  var a<2> = [1, 2.5];\n}"),
            [
                (Token::Def, 1, 1),
                (Token::Identifier("main".into()), 1, 5),
                (Token::Char('('), 1, 9),
                (Token::Char(')'), 1, 10),
                (Token::Char('{'), 1, 12),
                (Token::Var, 2, 3),
                (Token::Identifier("a".into()), 2, 7),
                (Token::Char('<'), 2, 8),
                (Token::Number(2.0), 2, 9),
                (Token::Char('>'), 2, 10),
                (Token::Char('='), 2, 12),
                (Token::Char('['), 2, 14),
                (Token::Number(1.0), 2, 15),
                (Token::Char(','), 2, 16),
                (Token::Number(2.5), 2, 18),
                (Token::Char(']'), 2, 21),
                (Token::Char(';'), 2, 22),
                (Token::Char('}'), 3, 1),
            ]
        );
    }

    #[test]
    fn test_skips_comments() {
        assert_eq!(
            tokens("# comment\nreturn # another\n;"),
            [(Token::Return, 2, 1), (Token::Char(';'), 3, 1)]
        );
    }

    #[test]
    fn test_reads_longest_valid_number_prefix() {
        assert_eq!(parse_number("1.2.3"), 1.2);
        assert_eq!(parse_number("42"), 42.0);
    }
}
