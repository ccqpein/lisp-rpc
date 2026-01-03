#![feature(iter_array_chunks)]
#![feature(assert_matches)]
pub mod data;
mod macros;

use std::{collections::VecDeque, error::Error, io::Read};
use tracing::error;

#[derive(Debug, PartialEq, Eq)]
pub enum ParserError {
    InvalidStart,
    InvalidToken(&'static str),
    CorruptData(&'static str),
    UnknownToken,
}

impl std::fmt::Display for ParserError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParserError::InvalidStart => write!(f, "parser error: Invalid start token"),
            ParserError::InvalidToken(msg) => write!(f, "parser error: Invalid token: {}", msg),
            ParserError::UnknownToken => write!(f, "parser error: Unknown token"),
            ParserError::CorruptData(msg) => write!(f, "parser error: illegal data: {}", msg),
        }
    }
}

impl Error for ParserError {}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum TypeValue {
    Symbol(String),
    String(String),
    Keyword(String),
    Number(i64),
}

impl TypeValue {
    pub fn to_string(&self) -> String {
        match self {
            TypeValue::Symbol(s) => s.clone(),
            TypeValue::String(s) => format!("\"{}\"", s),
            TypeValue::Keyword(s) => format!(":{}", s),
            TypeValue::Number(d) => d.to_string(),
        }
    }

    pub fn make_symbol(s: &str) -> Result<Self, Box<dyn Error>> {
        if s.contains([' ']) {
            Err(Box::new(ParserError::CorruptData(
                "cannot make symbol with this str",
            )))
        } else {
            Ok(Self::Symbol(s.to_string()))
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct Atom {
    pub value: TypeValue,
}

impl Atom {
    pub fn read(s: &str) -> Self {
        Self {
            value: TypeValue::Symbol(s.to_string()),
        }
    }

    pub fn read_string(s: &str) -> Self {
        Self {
            value: TypeValue::String(s.to_string()),
        }
    }

    pub fn read_keyword(s: &str) -> Self {
        Self {
            value: TypeValue::Keyword(s.to_string()),
        }
    }

    pub fn read_number(_s: &str, n: i64) -> Self {
        Self {
            value: TypeValue::Number(n),
        }
    }

    pub fn is_string(&self) -> bool {
        match self.value {
            TypeValue::String(_) => true,
            _ => false,
        }
    }

    pub fn to_string(&self) -> String {
        self.value.to_string()
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Expr {
    Atom(Atom),
    List(Vec<Expr>),
    Quote(Box<Expr>),
}

impl Expr {
    pub fn into_tokens(&self) -> String {
        match self {
            Expr::Atom(atom) => atom.to_string(),
            Expr::List(exprs) => {
                String::from("(")
                    + &exprs
                        .iter()
                        .map(|a| a.into_tokens())
                        .collect::<Vec<String>>()
                        .join(" ")
                    + ")"
            }
            Expr::Quote(expr) => String::from("'") + &expr.into_tokens(),
        }
    }

    pub fn nth(&self, ind: usize) -> Option<&Self> {
        match self {
            Expr::List(exprs) => exprs.get(ind),
            _ => None,
        }
    }

    pub fn iter(&self) -> Option<impl Iterator<Item = &Expr>> {
        match self {
            Expr::List(exprs) => Some(exprs.iter()),
            _ => None,
        }
    }
}

impl std::fmt::Display for Expr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.into_tokens())
    }
}

pub struct Parser {
    /// will read number if this field is true. default is true
    /// turn it off will treat the number as the symbol in Expr
    read_number_config: bool,
}

impl Default for Parser {
    fn default() -> Self {
        Self {
            read_number_config: true,
        }
    }
}

impl Parser {
    pub fn new() -> Self {
        Self {
            read_number_config: true,
        }
    }

    /// set the parser read_number config
    pub fn config_read_number(mut self, v: bool) -> Self {
        self.read_number_config = v;
        self
    }

    /// tokenize the source code
    pub fn tokenize(&self, mut source_code: impl Read) -> VecDeque<String> {
        let mut buf = [0; 1];
        let mut cache = vec![];
        let mut res = vec![];
        loop {
            match source_code.read(&mut buf) {
                Ok(n) if n != 0 => {
                    let c = buf.get(0).unwrap();
                    match c {
                        b'(' | b' ' | b')' | b'\'' | b'"' | b':' | b'\n' => {
                            if !cache.is_empty() {
                                res.push(String::from_utf8(cache.clone()).unwrap());
                                cache.clear();
                            }

                            match res.last() {
                                Some(le) if le == " " && *c == b' ' => continue,
                                _ => (),
                            }

                            res.push(String::from_utf8(vec![*c]).unwrap())
                        }
                        _ => {
                            cache.push(*c);
                        }
                    }
                }
                Ok(_) => break,
                Err(e) => error!("error in tokenize step {}", e),
            }
        }

        if !cache.is_empty() {
            res.push(String::from_utf8(cache.clone()).unwrap());
        }

        res.into()
    }

    pub fn parse_root(&mut self, source_code: impl Read) -> Result<Vec<Expr>, ParserError> {
        let mut tokens = self.tokenize(source_code);
        let mut res = vec![];

        loop {
            match tokens.front() {
                Some(b) => match b.as_str() {
                    "(" => {
                        res.push(self.read_exp(&mut tokens)?);
                    }
                    " " | "\n" => {
                        tokens.pop_front();
                    }
                    _ => {
                        return {
                            println!("{:?}", b);
                            Err(ParserError::InvalidToken("in read_root"))
                        };
                    }
                },
                None => break,
            }
        }

        Ok(res)
    }

    pub fn parse_root_one(&mut self, source_code: impl Read) -> Result<Expr, ParserError> {
        let mut tokens = self.tokenize(source_code);

        loop {
            match tokens.front() {
                Some(b) => match b.as_str() {
                    "(" => {
                        return Ok(self.read_exp(&mut tokens)?);
                    }
                    " " | "\n" => {
                        tokens.pop_front();
                    }
                    _ => {
                        return {
                            println!("{:?}", b);
                            Err(ParserError::InvalidToken("in read_root"))
                        };
                    }
                },
                None => return Err(ParserError::InvalidToken("run out the tokens")),
            }
        }
    }

    /// choose which read function
    fn read_router(
        &self,
        token: &str,
    ) -> Result<fn(&Self, &mut VecDeque<String>) -> Result<Expr, ParserError>, ParserError> {
        match token {
            "(" => Ok(Self::read_exp),
            "'" => Ok(Self::read_quote),
            "\"" => Ok(Self::read_string),
            ":" => Ok(Self::read_keyword),
            _ => Ok(Self::read_atom),
        }
    }

    fn read_atom(&self, tokens: &mut VecDeque<String>) -> Result<Expr, ParserError> {
        let token = tokens
            .pop_front()
            .ok_or(ParserError::InvalidToken("in read_sym"))?;

        if self.read_number_config {
            match token.parse::<i64>() {
                Ok(n) => return Ok(Expr::Atom(Atom::read_number(&token, n))),
                Err(_) => (),
            }
        }

        Ok(Expr::Atom(Atom::read(&token)))
    }

    fn read_quote(&self, tokens: &mut VecDeque<String>) -> Result<Expr, ParserError> {
        tokens
            .pop_front()
            .ok_or(ParserError::InvalidToken("in read_quote"))?;

        let res = match tokens.front() {
            Some(t) => self.read_router(t)?(self, tokens)?,
            None => return Err(ParserError::InvalidToken("in read_quote")),
        };

        Ok(Expr::Quote(Box::new(res)))
    }

    /// start from '\('
    pub fn read_exp(&self, tokens: &mut VecDeque<String>) -> Result<Expr, ParserError> {
        let mut res = vec![];
        tokens.pop_front();

        loop {
            match tokens.front() {
                Some(t) if t == ")" => {
                    tokens.pop_front();
                    break;
                }
                // ignore spaces
                Some(t) if t == " " || t == "\n" => {
                    tokens.pop_front();
                }
                Some(t) => res.push(self.read_router(t)?(self, tokens)?),
                None => return Err(ParserError::InvalidToken("in read_exp, the tokens run out")),
            }
        }

        Ok(Expr::List(res))
    }

    /// start with "
    fn read_string(&self, tokens: &mut VecDeque<String>) -> Result<Expr, ParserError> {
        tokens.pop_front();

        let mut escape = false;
        let mut res = String::new();
        let mut this_token;
        loop {
            this_token = tokens
                .pop_front()
                .ok_or(ParserError::InvalidToken("in read_string"))?;

            if escape {
                res = res + &this_token;
                escape = false;
                continue;
            }

            match this_token.as_str() {
                "\\" => escape = true,
                "\"" => break,
                _ => res = res + &this_token,
            }
        }

        Ok(Expr::Atom(Atom::read_string(&res)))
    }

    /// start with :
    fn read_keyword(&self, tokens: &mut VecDeque<String>) -> Result<Expr, ParserError> {
        tokens.pop_front();

        let token = tokens
            .pop_front()
            .ok_or(ParserError::InvalidToken("in read_keyword"))?;

        Ok(Expr::Atom(Atom::read_keyword(&token)))
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn test_tokenize() {
        let parser = Parser::new();
        //
        let s = "(a b c 123 c)";
        assert_eq!(
            parser.tokenize(Cursor::new(s.as_bytes())),
            vec!["(", "a", " ", "b", " ", "c", " ", "123", " ", "c", ")"]
                .into_iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
        );

        //
        let s = r#"(a '(""))"#;
        assert_eq!(
            parser.tokenize(Cursor::new(s.as_bytes())),
            vec!["(", "a", " ", "'", "(", "\"", "\"", ")", ")"]
                .into_iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
        );

        //
        let s = r#"(a '() '1)"#;
        assert_eq!(
            parser.tokenize(Cursor::new(s.as_bytes())),
            vec!["(", "a", " ", "'", "(", ")", " ", "'", "1", ")"]
                .into_iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
        );

        //
        let s = r#"(def-msg language-perfer :lang 'string)"#;
        assert_eq!(
            parser.tokenize(Cursor::new(s.as_bytes())),
            vec![
                "(",
                "def-msg",
                " ",
                "language-perfer",
                " ",
                ":",
                "lang",
                " ",
                "'",
                "string",
                ")"
            ]
            .into_iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>()
        );

        //
        let s = r#"(def-rpc get-book
                     '(:title 'string :vesion 'string :lang 'language-perfer)
                    'book-info)"#;
        assert_eq!(
            parser.tokenize(Cursor::new(s.as_bytes())),
            vec![
                "(",
                "def-rpc",
                " ",
                "get-book",
                "\n",
                " ",
                "'",
                "(",
                ":",
                "title",
                " ",
                "'",
                "string",
                " ",
                ":",
                "vesion",
                " ",
                "'",
                "string",
                " ",
                ":",
                "lang",
                " ",
                "'",
                "language-perfer",
                ")",
                "\n",
                " ",
                "'",
                "book-info",
                ")"
            ]
            .into_iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>()
        );

        //
        let s = r#"(get-book :title "hello world" :version "1984")"#;
        assert_eq!(
            parser.tokenize(Cursor::new(s.as_bytes())),
            vec![
                "(", "get-book", " ", ":", "title", " ", "\"", "hello", " ", "world", "\"", " ",
                ":", "version", " ", "\"", "1984", "\"", ")"
            ]
            .into_iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>()
        );

        // escapr "
        let s = r#"( get-book :title "hello \"world" :version "1984")"#;
        assert_eq!(
            parser.tokenize(Cursor::new(s.as_bytes())),
            vec![
                "(", " ", "get-book", " ", ":", "title", " ", "\"", "hello", " ", "\\", "\"",
                "world", "\"", " ", ":", "version", " ", "\"", "1984", "\"", ")"
            ]
            .into_iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>()
        );

        // number

        let s = r#"( get-book :id 1984)"#;
        assert_eq!(
            parser.tokenize(Cursor::new(s.as_bytes())),
            vec!["(", " ", "get-book", " ", ":", "id", " ", "1984", ")"]
                .into_iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_read_string() {
        let parser = Parser::new();
        let mut t = parser.tokenize(Cursor::new(r#""hello""#.as_bytes()));
        assert_eq!(
            parser.read_string(&mut t),
            Ok(Expr::Atom(Atom::read_string("hello")))
        );
        assert!(t.is_empty());
    }

    #[test]
    fn test_read_number() {
        let parser = Parser::new().config_read_number(true);

        let mut t = parser.tokenize(Cursor::new(r#"123"#.as_bytes()));

        assert_eq!(
            parser.read_atom(&mut t),
            Ok(Expr::Atom(Atom::read_number("123", 123)))
        );
    }

    #[test]
    fn test_read_exp() {
        let parser = Parser::new().config_read_number(false);
        let mut t = parser.tokenize(Cursor::new("(a b c 123 c)".as_bytes()));
        assert_eq!(
            parser.read_exp(&mut t),
            Ok(Expr::List(
                [
                    Expr::Atom(Atom::read("a")),
                    Expr::Atom(Atom::read("b")),
                    Expr::Atom(Atom::read("c")),
                    Expr::Atom(Atom::read("123")),
                    Expr::Atom(Atom::read("c")),
                ]
                .to_vec()
            ),)
        );
        //dbg!(&t);
        assert!(t.is_empty());

        //
        let mut t = parser.tokenize(Cursor::new("((a) b c 123 c)".as_bytes()));
        assert_eq!(
            parser.read_exp(&mut t),
            Ok(Expr::List(
                [
                    Expr::List([Expr::Atom(Atom::read("a"))].to_vec()),
                    Expr::Atom(Atom::read("b")),
                    Expr::Atom(Atom::read("c")),
                    Expr::Atom(Atom::read("123")),
                    Expr::Atom(Atom::read("c")),
                ]
                .to_vec()
            ),)
        );
        //dbg!(&t);
        assert!(t.is_empty());

        //
        let mut t = parser.tokenize(Cursor::new(
            r#"(def-msg language-perfer :lang 'string)"#.as_bytes(),
        ));
        assert_eq!(
            parser.read_exp(&mut t),
            Ok(Expr::List(
                [
                    Expr::Atom(Atom::read("def-msg")),
                    Expr::Atom(Atom::read("language-perfer")),
                    Expr::Atom(Atom::read_keyword("lang")),
                    Expr::Quote(Box::new(Expr::Atom(Atom::read("string")))),
                ]
                .to_vec()
            ),)
        );
        //dbg!(&t);
        assert!(t.is_empty());

        //
        let mut t = parser.tokenize(Cursor::new(
            r#"(def-rpc get-book
                     '(:title 'string :version 'string :lang 'language-perfer)
                    'book-info)"#
                .as_bytes(),
        ));
        assert_eq!(
            parser.read_exp(&mut t),
            Ok(Expr::List(
                [
                    Expr::Atom(Atom::read("def-rpc")),
                    Expr::Atom(Atom::read("get-book")),
                    Expr::Quote(Box::new(Expr::List(
                        [
                            Expr::Atom(Atom::read_keyword("title")),
                            Expr::Quote(Box::new(Expr::Atom(Atom::read("string")))),
                            Expr::Atom(Atom::read_keyword("version")),
                            Expr::Quote(Box::new(Expr::Atom(Atom::read("string")))),
                            Expr::Atom(Atom::read_keyword("lang")),
                            Expr::Quote(Box::new(Expr::Atom(Atom::read("language-perfer")))),
                        ]
                        .to_vec()
                    ))),
                    Expr::Quote(Box::new(Expr::Atom(Atom::read("book-info")))),
                ]
                .to_vec()
            ),)
        );
        //dbg!(&t);
        assert!(t.is_empty());

        //
        let mut t = parser.tokenize(Cursor::new(
            r#"(get-book :title "hello world" :version "1984")"#.as_bytes(),
        ));

        assert_eq!(
            parser.read_exp(&mut t),
            Ok(Expr::List(
                [
                    Expr::Atom(Atom::read("get-book")),
                    Expr::Atom(Atom::read_keyword("title")),
                    Expr::Atom(Atom::read_string("hello world")),
                    Expr::Atom(Atom::read_keyword("version")),
                    Expr::Atom(Atom::read_string("1984")),
                ]
                .to_vec()
            ),)
        );

        let mut t = parser.tokenize(Cursor::new(
            r#"(get-book :title "hello \"world" :version "1984")"#.as_bytes(),
        ));

        assert_eq!(
            parser.read_exp(&mut t),
            Ok(Expr::List(
                [
                    Expr::Atom(Atom::read("get-book")),
                    Expr::Atom(Atom::read_keyword("title")),
                    Expr::Atom(Atom::read_string("hello \"world")),
                    Expr::Atom(Atom::read_keyword("version")),
                    Expr::Atom(Atom::read_string("1984")),
                ]
                .to_vec()
            ),)
        );

        //

        let parser = Parser::new().config_read_number(true);

        let mut t = parser.tokenize(Cursor::new(
            r#"(get-book :title "hello world" :id 1984)"#.as_bytes(),
        ));

        assert_eq!(
            parser.read_exp(&mut t),
            Ok(Expr::List(
                [
                    Expr::Atom(Atom::read("get-book")),
                    Expr::Atom(Atom::read_keyword("title")),
                    Expr::Atom(Atom::read_string("hello world")),
                    Expr::Atom(Atom::read_keyword("id")),
                    Expr::Atom(Atom::read_number("1984", 1984)),
                ]
                .to_vec()
            ),)
        );
    }

    #[test]
    fn test_read_root() {
        let mut parser = Parser::new();

        let expr = parser
            .parse_root(&mut Cursor::new("(a b c 123 c) (a '(1 2 3))".as_bytes()))
            .unwrap();
        assert_eq!(
            expr,
            vec![
                Expr::List(vec![
                    Expr::Atom(Atom::read("a")),
                    Expr::Atom(Atom::read("b")),
                    Expr::Atom(Atom::read("c")),
                    Expr::Atom(Atom::read_number("123", 123)),
                    Expr::Atom(Atom::read("c")),
                ],),
                Expr::List(vec![
                    Expr::Atom(Atom::read("a")),
                    Expr::Quote(Box::new(Expr::List(vec![
                        Expr::Atom(Atom::read_number("1", 1)),
                        Expr::Atom(Atom::read_number("2", 2)),
                        Expr::Atom(Atom::read_number("3", 3)),
                    ]))),
                ],),
            ],
        );

        let expr = parser
            .parse_root(Cursor::new(r#"('a "hello")"#.as_bytes()))
            .unwrap();
        assert_eq!(
            expr,
            vec![Expr::List(vec![
                Expr::Quote(Box::new(Expr::Atom(Atom::read("a")))),
                Expr::Atom(Atom::read_string("hello")),
            ])],
        );

        //
        let mut t = Cursor::new(
            r#"(def-msg language-perfer :lang 'string)

(def-rpc get-book
                     '(:title 'string :version 'string :lang 'language-perfer)
                    'book-info)"#
                .as_bytes(),
        );

        let s0 = Cursor::new(r#"(def-msg language-perfer :lang 'string)"#.as_bytes());
        let mut t0 = parser.tokenize(s0.clone());

        let s1 = Cursor::new(
            r#"(def-rpc get-book
                     '(:title 'string :version 'string :lang 'language-perfer)
                    'book-info)"#
                .as_bytes(),
        );
        let mut t1 = parser.tokenize(s1.clone());

        let expr = parser.parse_root(&mut t).unwrap();
        assert_eq!(
            expr,
            vec![
                parser.read_exp(&mut t0).unwrap(),
                parser.read_exp(&mut t1).unwrap()
            ]
        );
    }

    #[test]
    fn test_read_root_one() {
        let mut parser = Parser::new();
        let mut t = Cursor::new(
            r#"(def-msg language-perfer :lang 'string)

(def-rpc get-book
                     '(:title 'string :version 'string :lang 'language-perfer)
                    'book-info)"#
                .as_bytes(),
        );

        let expr = parser.parse_root_one(&mut t).unwrap();

        let s0 = Cursor::new(r#"(def-msg language-perfer :lang 'string)"#.as_bytes());
        let mut t0 = parser.tokenize(s0.clone());

        assert_eq!(expr, parser.read_exp(&mut t0).unwrap(),);
    }

    #[test]
    fn test_into_tokens() {
        let mut parser = Parser::new();
        let mut t = Cursor::new(
            r#"(def-msg language-perfer :lang 'string)

(def-rpc get-book
                     '(:title 'string :version 'string :lang 'language-perfer)
                    'book-info)"#
                .as_bytes(),
        );

        let expr = parser.parse_root(&mut t).unwrap();

        assert_eq!(
            expr.into_iter().map(|e|e.into_tokens()).collect::<Vec<String>>(),
            vec![
                "(def-msg language-perfer :lang 'string)".to_string(),
                "(def-rpc get-book '(:title 'string :version 'string :lang 'language-perfer) 'book-info)".to_string(),
            ],
        );
    }
}
