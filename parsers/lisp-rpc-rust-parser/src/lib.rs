use anyhow::Result;
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

#[derive(Debug, Clone, Copy)]
pub enum TypeValueNumber {
    Int(i64),
    Float(f64),
}

impl TypeValueNumber {
    pub fn to_int(&self) -> Option<i64> {
        match self {
            TypeValueNumber::Int(i) => Some(*i),
            TypeValueNumber::Float(_) => None,
        }
    }

    pub fn to_float(&self) -> Option<f64> {
        match self {
            TypeValueNumber::Float(f) => Some(*f),
            TypeValueNumber::Int(i) => Some(*i as f64),
        }
    }
}

impl PartialEq for TypeValueNumber {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Int(a), Self::Int(b)) => a == b,
            (Self::Float(a), Self::Float(b)) => {
                if a.is_nan() && b.is_nan() {
                    true
                } else {
                    a == b
                }
            }
            _ => false,
        }
    }
}

impl Eq for TypeValueNumber {}

impl std::hash::Hash for TypeValueNumber {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            Self::Int(i) => {
                0u8.hash(state);
                i.hash(state);
            }
            Self::Float(f) => {
                1u8.hash(state);
                let bits = if f.is_nan() {
                    f64::NAN.to_bits()
                } else if *f == 0.0 {
                    0.0f64.to_bits()
                } else {
                    f.to_bits()
                };
                bits.hash(state);
            }
        }
    }
}

impl std::fmt::Display for TypeValueNumber {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Int(i) => write!(f, "{}", i),
            Self::Float(val) => write!(f, "{}", val),
        }
    }
}

impl std::ops::Add for TypeValueNumber {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Self::Int(a), Self::Int(b)) => Self::Int(a + b),
            (Self::Float(a), Self::Int(b)) => Self::Float(a + b as f64),
            (Self::Int(a), Self::Float(b)) => Self::Float(a as f64 + b),
            (Self::Float(a), Self::Float(b)) => Self::Float(a + b),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum TypeValue {
    Symbol(String),
    String(String),
    Keyword(String),
    Number(TypeValueNumber),
}

impl TypeValue {
    pub fn to_string(&self) -> String {
        match self {
            TypeValue::Symbol(s) => s.clone(),
            TypeValue::String(s) => s.to_string(),
            TypeValue::Keyword(s) => format!(":{}", s),
            TypeValue::Number(d) => d.to_string(),
        }
    }

    pub fn make_symbol(s: &str) -> Result<Self> {
        if s.contains([' ']) {
            Err(anyhow::anyhow!(ParserError::CorruptData(
                "cannot make symbol with this str",
            )))
        } else {
            Ok(Self::Symbol(s.to_string()))
        }
    }

    pub fn to_int(&self) -> Option<i64> {
        match self {
            TypeValue::Number(type_value_number) => type_value_number.to_int(),
            _ => None,
        }
    }

    pub fn to_float(&self) -> Option<f64> {
        match self {
            TypeValue::Number(type_value_number) => type_value_number.to_float(),
            _ => None,
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

    pub fn read_number(_s: &str, n: TypeValueNumber) -> Self {
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

    /// Comment
    Comment(String),
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
            Expr::Comment(s) => String::from("; ") + s,
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

    pub fn is_comment(&self) -> bool {
        match self {
            Expr::Comment(_) => true,
            _ => false,
        }
    }

    /// Clean all comment expr from List. Unwrap List directly
    pub fn filter_out_all_comments(&self) -> Result<impl Iterator<Item = &Expr>> {
        match self {
            Expr::List(_) => match self.iter() {
                Some(rest_expr) => Ok(rest_expr.filter(|e| !e.is_comment())),
                None => Err(anyhow::anyhow!("Empty")),
            },
            _ => Err(anyhow::anyhow!("Not the Expr::List")),
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

    tokens: VecDeque<String>,

    exprs: Vec<Expr>,
}

impl Default for Parser {
    fn default() -> Self {
        Self {
            read_number_config: true,
            tokens: VecDeque::new(),
            exprs: vec![],
        }
    }
}

impl Parser {
    pub fn new() -> Self {
        Self {
            read_number_config: true,
            tokens: VecDeque::new(),
            exprs: vec![],
        }
    }

    /// set the parser read_number config
    pub fn config_read_number(mut self, v: bool) -> Self {
        self.read_number_config = v;
        self
    }

    /// tokenize the source code
    pub fn tokenize(&mut self, mut source_code: impl Read) -> Result<()> {
        let mut buf = [0; 1];
        let mut cache = vec![];
        let mut res = vec![];
        loop {
            match source_code.read(&mut buf) {
                Ok(n) if n != 0 => {
                    let c = buf.get(0).unwrap();
                    match c {
                        b'(' | b' ' | b')' | b'\'' | b'"' | b':' | b'\n' | b';' => {
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

        self.tokens = res.into();

        Ok(())
    }

    /// parse all tokens in parser to exprs
    pub fn parse(&mut self) -> Result<(), ParserError> {
        let mut res = vec![];

        loop {
            match self.tokens.front() {
                Some(b) => match b.as_str() {
                    "(" => {
                        res.push(self.read_exp()?);
                    }
                    "'" => {
                        res.push(self.read_quote()?);
                    }
                    "\"" => {
                        res.push(self.read_string()?);
                    }
                    ":" => {
                        res.push(self.read_keyword()?);
                    }
                    ";" => {
                        res.push(self.read_comment()?);
                    }
                    " " | "\n" => {
                        self.tokens.pop_front();
                    }
                    _ => {
                        println!("{:?}", b);
                        return Err(ParserError::InvalidToken("in read_root"));
                    }
                },
                None => break,
            }
        }

        self.exprs = res;
        Ok(())
    }

    /// only parse one expr from inner tokens
    pub fn parse_one(&mut self) -> Result<(), ParserError> {
        loop {
            match self.tokens.front() {
                Some(b) => match b.as_str() {
                    "(" => {
                        let e = self.read_exp()?;
                        self.exprs.push(e);
                        return Ok(());
                    }
                    "'" => {
                        let e = self.read_quote()?;
                        self.exprs.push(e);
                        return Ok(());
                    }
                    "\"" => {
                        let e = self.read_string()?;
                        self.exprs.push(e);
                        return Ok(());
                    }
                    ":" => {
                        let e = self.read_keyword()?;
                        self.exprs.push(e);
                        return Ok(());
                    }
                    ";" => {
                        let e = self.read_comment()?;
                        self.exprs.push(e);
                        return Ok(());
                    }
                    " " | "\n" => {
                        self.tokens.pop_front();
                    }
                    _ => {
                        println!("{:?}", b);
                        return Err(ParserError::InvalidToken("in read_root"));
                    }
                },
                None => return Err(ParserError::InvalidToken("run out the tokens")),
            }
        }
    }

    /// choose which read function
    pub fn read_router(
        &self,
        token: &str,
    ) -> Result<fn(&mut Self) -> Result<Expr, ParserError>, ParserError> {
        match token {
            "(" => Ok(Self::read_exp),
            "'" => Ok(Self::read_quote),
            "\"" => Ok(Self::read_string),
            ":" => Ok(Self::read_keyword),
            ";" => Ok(Self::read_comment),
            _ => Ok(Self::read_atom),
        }
    }

    fn read_atom(&mut self) -> Result<Expr, ParserError> {
        let token = self
            .tokens
            .pop_front()
            .ok_or(ParserError::InvalidToken("in read_sym"))?;

        if self.read_number_config {
            if let Ok(n) = token.parse::<i64>() {
                return Ok(Expr::Atom(Atom::read_number(
                    &token,
                    TypeValueNumber::Int(n),
                )));
            }
            if token.chars().next().map_or(false, |c| {
                c.is_ascii_digit() || c == '.' || c == '+' || c == '-'
            }) {
                if let Ok(f) = token.parse::<f64>() {
                    return Ok(Expr::Atom(Atom::read_number(
                        &token,
                        TypeValueNumber::Float(f),
                    )));
                }
            }
        }

        Ok(Expr::Atom(Atom::read(&token)))
    }

    fn read_quote(&mut self) -> Result<Expr, ParserError> {
        self.tokens
            .pop_front()
            .ok_or(ParserError::InvalidToken("in read_quote"))?;

        let res = match self.tokens.front() {
            Some(t) => self.read_router(t)?(self)?,
            None => return Err(ParserError::InvalidToken("in read_quote")),
        };

        Ok(Expr::Quote(Box::new(res)))
    }

    /// start from '\('
    pub fn read_exp(&mut self) -> Result<Expr, ParserError> {
        let mut res = vec![];
        self.tokens.pop_front();

        loop {
            match self.tokens.front() {
                Some(t) if t == ")" => {
                    self.tokens.pop_front();
                    break;
                }
                // ignore spaces
                Some(t) if t == " " || t == "\n" => {
                    self.tokens.pop_front();
                }
                Some(t) => res.push(self.read_router(t)?(self)?),
                None => return Err(ParserError::InvalidToken("in read_exp, the tokens run out")),
            }
        }

        Ok(Expr::List(res))
    }

    /// start with "
    fn read_string(&mut self) -> Result<Expr, ParserError> {
        self.tokens.pop_front();

        let mut escape = false;
        let mut res = String::new();
        let mut this_token;
        loop {
            this_token = self
                .tokens
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
    fn read_keyword(&mut self) -> Result<Expr, ParserError> {
        self.tokens.pop_front();

        let token = self
            .tokens
            .pop_front()
            .ok_or(ParserError::InvalidToken("in read_keyword"))?;

        Ok(Expr::Atom(Atom::read_keyword(&token)))
    }

    /// start with ;
    fn read_comment(&mut self) -> Result<Expr, ParserError> {
        //dbg!(&tokens);
        self.tokens.pop_front();

        let mut start = false;
        let mut res = String::new();
        let mut this_token;

        loop {
            this_token = match self.tokens.pop_front() {
                Some(tt) => tt,
                None => break,
            };

            if !start {
                match this_token.as_str() {
                    ";" | " " => continue,
                    _ => start = true,
                }
            }

            // new line end comment reading
            if this_token == "\n" {
                break;
            }

            res = res + &this_token
        }

        if res != "" {
            Ok(Expr::Comment(res.trim_end().to_string()))
        } else {
            Ok(Expr::Comment(String::new()))
        }
    }
}

impl Parser {
    pub fn iter_expr(&self) -> impl Iterator<Item = &Expr> {
        self.exprs.iter()
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    fn tokenize_to_vec(s: &str) -> Vec<String> {
        let mut parser = Parser::new();
        parser.tokenize(Cursor::new(s.as_bytes())).unwrap();
        parser.tokens.into_iter().collect::<Vec<_>>()
    }

    #[test]
    fn test_add_type_value_number() {
        assert_eq!(
            TypeValueNumber::Int(1) + TypeValueNumber::Int(2),
            TypeValueNumber::Int(3)
        );
        assert_eq!(
            TypeValueNumber::Float(1.5) + TypeValueNumber::Int(2),
            TypeValueNumber::Float(3.5)
        );
        assert_eq!(
            TypeValueNumber::Int(1) + TypeValueNumber::Float(2.5),
            TypeValueNumber::Float(3.5)
        );
        assert_eq!(
            TypeValueNumber::Float(1.5) + TypeValueNumber::Float(2.5),
            TypeValueNumber::Float(4.0)
        );
    }

    #[test]
    fn test_to_int_and_to_float() {
        let int_val = TypeValueNumber::Int(42);
        let float_val = TypeValueNumber::Float(3.14);

        assert_eq!(int_val.to_int(), Some(42));
        assert_eq!(int_val.to_float(), None);
        assert_eq!(float_val.to_int(), None);
        assert_eq!(float_val.to_float(), Some(3.14));

        let int_tv = TypeValue::Number(int_val);
        let float_tv = TypeValue::Number(float_val);
        let sym_tv = TypeValue::Symbol("foo".to_string());

        assert_eq!(int_tv.to_int(), Some(42));
        assert_eq!(int_tv.to_float(), None);
        assert_eq!(float_tv.to_int(), None);
        assert_eq!(float_tv.to_float(), Some(3.14));
        assert_eq!(sym_tv.to_int(), None);
        assert_eq!(sym_tv.to_float(), None);
    }

    #[test]
    fn test_tokenize() {
        //
        let s = "(a b c 123 c)";
        assert_eq!(
            tokenize_to_vec(s),
            vec!["(", "a", " ", "b", " ", "c", " ", "123", " ", "c", ")"]
                .into_iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
        );

        //
        let s = r#"(a '(""))"#;
        assert_eq!(
            tokenize_to_vec(s),
            vec!["(", "a", " ", "'", "(", "\"", "\"", ")", ")"]
                .into_iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
        );

        //
        let s = r#"(a '() '1)"#;
        assert_eq!(
            tokenize_to_vec(s),
            vec!["(", "a", " ", "'", "(", ")", " ", "'", "1", ")"]
                .into_iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
        );

        //
        let s = r#"(def-msg language-perfer :lang 'string)"#;
        assert_eq!(
            tokenize_to_vec(s),
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
            tokenize_to_vec(s),
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
            tokenize_to_vec(s),
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
            tokenize_to_vec(s),
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
            tokenize_to_vec(s),
            vec!["(", " ", "get-book", " ", ":", "id", " ", "1984", ")"]
                .into_iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
        );

        let s = r#"( get-book :price 19.84)"#;
        assert_eq!(
            tokenize_to_vec(s),
            vec!["(", " ", "get-book", " ", ":", "price", " ", "19.84", ")"]
                .into_iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_comment_tokenize() {
        let s = r#"(def-rpc get-book  ;; comment
                     '(:title 'string :vesion 'string :lang 'language-perfer)
                    'book-info)"#;
        assert_eq!(
            tokenize_to_vec(s),
            vec![
                "(",
                "def-rpc",
                " ",
                "get-book",
                " ", // one space
                ";",
                ";",
                " ",
                "comment",
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
    }

    #[test]
    fn test_read_string() {
        let mut parser = Parser::new();
        parser
            .tokenize(Cursor::new(r#""hello""#.as_bytes()))
            .unwrap();
        assert_eq!(
            parser.read_string(),
            Ok(Expr::Atom(Atom::read_string("hello")))
        );
        assert!(parser.tokens.is_empty());
    }

    #[test]
    fn test_read_number() {
        let mut parser = Parser::new().config_read_number(true);

        parser.tokenize(Cursor::new(r#"123"#.as_bytes())).unwrap();

        assert_eq!(
            parser.read_atom(),
            Ok(Expr::Atom(Atom::read_number(
                "123",
                TypeValueNumber::Int(123)
            )))
        );

        parser.tokenize(Cursor::new(r#"1.2"#.as_bytes())).unwrap();

        assert_eq!(
            parser.read_atom(),
            Ok(Expr::Atom(Atom::read_number(
                "1.2",
                TypeValueNumber::Float(1.2)
            )))
        );

        parser
            .tokenize(Cursor::new(r#"12344.22131"#.as_bytes()))
            .unwrap();

        assert_eq!(
            parser.read_atom(),
            Ok(Expr::Atom(Atom::read_number(
                "12344.22131",
                TypeValueNumber::Float(12344.22131)
            )))
        );
    }

    #[test]
    fn test_read_exp() {
        let mut parser = Parser::new().config_read_number(false);
        parser
            .tokenize(Cursor::new("(a b c 123 c)".as_bytes()))
            .unwrap();
        assert_eq!(
            parser.read_exp(),
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
        assert!(parser.tokens.is_empty());

        //
        parser
            .tokenize(Cursor::new("((a) b c 123 c)".as_bytes()))
            .unwrap();
        assert_eq!(
            parser.read_exp(),
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
        assert!(parser.tokens.is_empty());

        //
        parser
            .tokenize(Cursor::new(
                r#"(def-msg language-perfer :lang 'string)"#.as_bytes(),
            ))
            .unwrap();
        assert_eq!(
            parser.read_exp(),
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
        assert!(parser.tokens.is_empty());

        //
        parser
            .tokenize(Cursor::new(
                r#"(def-rpc get-book
                     '(:title 'string :version 'string :lang 'language-perfer)
                    'book-info)"#
                    .as_bytes(),
            ))
            .unwrap();
        assert_eq!(
            parser.read_exp(),
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
        assert!(parser.tokens.is_empty());

        //
        parser
            .tokenize(Cursor::new(
                r#"(get-book :title "hello world" :version "1984")"#.as_bytes(),
            ))
            .unwrap();

        assert_eq!(
            parser.read_exp(),
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

        parser
            .tokenize(Cursor::new(
                r#"(get-book :title "hello \"world" :version "1984")"#.as_bytes(),
            ))
            .unwrap();

        assert_eq!(
            parser.read_exp(),
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

        let mut parser = Parser::new().config_read_number(true);

        parser
            .tokenize(Cursor::new(
                r#"(get-book :title "hello world" :id 1984)"#.as_bytes(),
            ))
            .unwrap();

        assert_eq!(
            parser.read_exp(),
            Ok(Expr::List(
                [
                    Expr::Atom(Atom::read("get-book")),
                    Expr::Atom(Atom::read_keyword("title")),
                    Expr::Atom(Atom::read_string("hello world")),
                    Expr::Atom(Atom::read_keyword("id")),
                    Expr::Atom(Atom::read_number("1984", TypeValueNumber::Int(1984))),
                ]
                .to_vec()
            ),)
        );
    }

    #[test]
    fn test_read_comment() {
        let mut parser = Parser::new();

        parser
            .tokenize(Cursor::new(r#";; comment "#.as_bytes()))
            .unwrap();

        assert_eq!(
            parser.read_comment(),
            Ok(Expr::Comment("comment".to_string()))
        );

        parser
            .tokenize(Cursor::new(
                r#";; comment
                 ddddd "#
                    .as_bytes(),
            ))
            .unwrap();

        assert_eq!(
            parser.read_comment(),
            Ok(Expr::Comment("comment".to_string()))
        )
    }

    #[test]
    fn test_read_root() {
        let mut parser = Parser::new();

        parser
            .tokenize(Cursor::new("(a b c 123 c) (a '(1 2 3))".as_bytes()))
            .unwrap();
        parser.parse().unwrap();
        let expr = parser.exprs.clone();
        assert_eq!(
            expr,
            vec![
                Expr::List(vec![
                    Expr::Atom(Atom::read("a")),
                    Expr::Atom(Atom::read("b")),
                    Expr::Atom(Atom::read("c")),
                    Expr::Atom(Atom::read_number("123", TypeValueNumber::Int(123))),
                    Expr::Atom(Atom::read("c")),
                ],),
                Expr::List(vec![
                    Expr::Atom(Atom::read("a")),
                    Expr::Quote(Box::new(Expr::List(vec![
                        Expr::Atom(Atom::read_number("1", TypeValueNumber::Int(1))),
                        Expr::Atom(Atom::read_number("2", TypeValueNumber::Int(2))),
                        Expr::Atom(Atom::read_number("3", TypeValueNumber::Int(3))),
                    ]))),
                ],),
            ],
        );

        parser
            .tokenize(Cursor::new(r#"('a "hello")"#.as_bytes()))
            .unwrap();

        parser.parse().unwrap();
        let expr = parser.exprs.clone();
        assert_eq!(
            expr,
            vec![Expr::List(vec![
                Expr::Quote(Box::new(Expr::Atom(Atom::read("a")))),
                Expr::Atom(Atom::read_string("hello")),
            ])],
        );

        //
        let t = Cursor::new(
            r#"(def-msg language-perfer :lang 'string)

(def-rpc get-book
                     '(:title 'string :version 'string :lang 'language-perfer)
                    'book-info)"#
                .as_bytes(),
        );

        let s0 = Cursor::new(r#"(def-msg language-perfer :lang 'string)"#.as_bytes());
        let mut parser0 = Parser::new();
        parser0.tokenize(s0).unwrap();
        let expr0 = parser0.read_exp().unwrap();

        let s1 = Cursor::new(
            r#"(def-rpc get-book
                     '(:title 'string :version 'string :lang 'language-perfer)
                    'book-info)"#
                .as_bytes(),
        );
        let mut parser1 = Parser::new();
        parser1.tokenize(s1).unwrap();
        let expr1 = parser1.read_exp().unwrap();

        parser.tokenize(t).unwrap();
        parser.parse().unwrap();
        let expr = parser.exprs.clone();
        assert_eq!(expr, vec![expr0, expr1]);
    }

    #[test]
    fn test_read_root_one() {
        let mut parser = Parser::new();
        let t = Cursor::new(
            r#"(def-msg language-perfer :lang 'string)

(def-rpc get-book
                     '(:title 'string :version 'string :lang 'language-perfer)
                    'book-info)"#
                .as_bytes(),
        );
        parser.tokenize(t).unwrap();
        parser.parse_one().unwrap();
        let expr = parser.exprs.clone();

        let s0 = Cursor::new(r#"(def-msg language-perfer :lang 'string)"#.as_bytes());
        let mut parser2 = Parser::new();
        parser2.tokenize(s0).unwrap();
        let expr0 = parser2.read_exp().unwrap();

        assert_eq!(expr[0], expr0);
    }

    #[test]
    fn test_read_root_one_not_expr_data() {
        let mut parser = Parser::new();
        let t = Cursor::new(r#":lang "string""#.as_bytes());
        parser.tokenize(t).unwrap();

        parser.parse_one().unwrap();
        assert_eq!(parser.exprs[0], Expr::Atom(Atom::read_keyword("lang")));

        parser.parse_one().unwrap();
        assert_eq!(parser.exprs[1], Expr::Atom(Atom::read_string("string")));
    }

    #[test]
    fn test_into_tokens() {
        let mut parser = Parser::new();
        let t = Cursor::new(
            r#"(def-msg language-perfer :lang 'string)

(def-rpc get-book
                     '(:title 'string :version 'string :lang 'language-perfer)
                    'book-info)"#
                .as_bytes(),
        );
        parser.tokenize(t).unwrap();
        parser.parse().unwrap();
        let expr = parser.exprs;

        assert_eq!(
            expr.into_iter().map(|e|e.into_tokens()).collect::<Vec<String>>(),
            vec![
                "(def-msg language-perfer :lang 'string)".to_string(),
                "(def-rpc get-book '(:title 'string :version 'string :lang 'language-perfer) 'book-info)".to_string(),
            ],
        );
    }
}
