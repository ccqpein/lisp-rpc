//! Core tokenizer and parser for Lisp-RPC S-expressions.

use anyhow::Result;
use std::{collections::VecDeque, error::Error, io::Read};
use tracing::error;

/// Errors that can occur during Lisp S-expression parsing.
#[derive(Debug, PartialEq, Eq)]
pub enum ParserError {
    /// Unexpected starting token encountered.
    InvalidStart,
    /// Invalid or unexpected token encountered.
    InvalidToken(&'static str),
    /// Corrupted or malformed data encountered.
    CorruptData(&'static str),
    /// Unknown token encountered.
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

/// Numeric value representing an integer or floating-point number.
#[derive(Debug, Clone, Copy)]
pub enum TypeValueNumber {
    /// 64-bit signed integer.
    Int(i64),
    /// 64-bit floating-point number.
    Float(f64),
}

impl TypeValueNumber {
    /// Returns the integer value if this is a [`TypeValueNumber::Int`].
    pub fn to_int(&self) -> Option<i64> {
        match self {
            TypeValueNumber::Int(i) => Some(*i),
            TypeValueNumber::Float(_) => None,
        }
    }

    /// Returns the value as a 64-bit float.
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

/// Primitive atom value types in Lisp-RPC expressions.
#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum TypeValue {
    /// Lisp symbol identifier.
    Symbol(String),
    /// String literal.
    String(String),
    /// Keyword identifier prefixed with a colon.
    Keyword(String),
    /// Numeric literal value.
    Number(TypeValueNumber),
}

impl TypeValue {
    /// Formats the type value as an S-expression token string.
    pub fn to_string(&self) -> String {
        match self {
            TypeValue::Symbol(s) => s.clone(),
            TypeValue::String(s) => format!("\"{}\"", s),
            TypeValue::Keyword(s) => format!(":{}", s),
            TypeValue::Number(d) => d.to_string(),
        }
    }

    /// Extracts the unquoted string content if this is a [`TypeValue::String`].
    pub fn get_string(&self) -> Result<String> {
        match self {
            TypeValue::String(s) => Ok(s.to_string()),
            x @ _ => anyhow::bail!("{:?} isn't the String type that can get string", x),
        }
    }

    /// Creates a [`TypeValue::Symbol`] if the string contains no whitespace.
    pub fn make_symbol(s: &str) -> Result<Self> {
        if s.contains([' ']) {
            Err(anyhow::anyhow!(ParserError::CorruptData(
                "cannot make symbol with this str",
            )))
        } else {
            Ok(Self::Symbol(s.to_string()))
        }
    }

    /// Returns the integer value if this is a [`TypeValue::Number`] containing an integer.
    pub fn to_int(&self) -> Option<i64> {
        match self {
            TypeValue::Number(type_value_number) => type_value_number.to_int(),
            _ => None,
        }
    }

    /// Returns the floating-point value if this is a [`TypeValue::Number`].
    pub fn to_float(&self) -> Option<f64> {
        match self {
            TypeValue::Number(type_value_number) => type_value_number.to_float(),
            _ => None,
        }
    }
}

/// An atomic value token in a Lisp S-expression.
#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct Atom {
    /// The inner typed value of the atom.
    pub value: TypeValue,
}

impl Atom {
    /// Creates an atom containing a symbol value.
    pub fn read(s: &str) -> Self {
        Self {
            value: TypeValue::Symbol(s.to_string()),
        }
    }

    /// Creates an atom containing a string literal value.
    pub fn read_string(s: &str) -> Self {
        Self {
            value: TypeValue::String(s.to_string()),
        }
    }

    /// Creates an atom containing a keyword value.
    pub fn read_keyword(s: &str) -> Self {
        Self {
            value: TypeValue::Keyword(s.to_string()),
        }
    }

    /// Creates an atom containing a numeric value.
    pub fn read_number(_s: &str, n: TypeValueNumber) -> Self {
        Self {
            value: TypeValue::Number(n),
        }
    }

    /// Returns `true` if the atom contains a string value.
    pub fn is_string(&self) -> bool {
        match self.value {
            TypeValue::String(_) => true,
            _ => false,
        }
    }

    /// Formats the atom as an S-expression token string.
    pub fn to_string(&self) -> String {
        self.value.to_string()
    }
}

/// A parsed Lisp-RPC S-expression node.
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Expr {
    /// Atomic value node.
    Atom(Atom),
    /// S-expression list node `(...)`.
    List(Vec<Expr>),
    /// Quoted expression node `'...`.
    Quote(Box<Expr>),

    /// Comment node `;...`.
    Comment(String),
}

impl Expr {
    /// Formats the expression tree as an S-expression string.
    pub fn to_string(&self) -> String {
        match self {
            Expr::Atom(atom) => atom.to_string(),
            Expr::List(exprs) => {
                String::from("(")
                    + &exprs
                        .iter()
                        .map(|a| a.to_string())
                        .collect::<Vec<String>>()
                        .join(" ")
                    + ")"
            }
            Expr::Quote(expr) => String::from("'") + &expr.to_string(),
            Expr::Comment(s) => String::from("; ") + s,
        }
    }

    /// Returns a reference to the element at index `ind` if this is an [`Expr::List`].
    pub fn nth(&self, ind: usize) -> Option<&Self> {
        match self {
            Expr::List(exprs) => exprs.get(ind),
            _ => None,
        }
    }

    /// Returns an iterator over child expressions if this is an [`Expr::List`].
    pub fn iter(&self) -> Option<impl Iterator<Item = &Expr>> {
        match self {
            Expr::List(exprs) => Some(exprs.iter()),
            _ => None,
        }
    }

    /// Returns `true` if this expression is an [`Expr::Comment`].
    pub fn is_comment(&self) -> bool {
        match self {
            Expr::Comment(_) => true,
            _ => false,
        }
    }

    /// Returns an iterator yielding non-comment expressions from an [`Expr::List`].
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
        write!(f, "{}", self.to_string())
    }
}

/// Tokenizer and parser for Lisp S-expressions.
pub struct Parser {
    /// Will read numbers if true; otherwise numbers are parsed as symbols.
    read_number_config: bool,

    /// Token queue populated by [`tokenize`](Parser::tokenize).
    pub tokens: VecDeque<String>,

    /// Parsed expression nodes populated by [`parse`](Parser::parse).
    pub exprs: Vec<Expr>,
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
    /// Creates a new parser instance with default configuration.
    pub fn new() -> Self {
        Self {
            read_number_config: true,
            tokens: VecDeque::new(),
            exprs: vec![],
        }
    }

    /// Configures whether numeric strings are parsed into numbers rather than symbols.
    pub fn config_read_number(mut self, v: bool) -> Self {
        self.read_number_config = v;
        self
    }

    /// Tokenizes the input reader into the token queue.
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

    /// Parses all tokens in the parser into expression nodes.
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

    /// Parses a single expression from the token queue.
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

    /// Returns the appropriate parse function for the given opening token.
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

    /// Reads an atom expression from the token queue.
    pub fn read_atom(&mut self) -> Result<Expr, ParserError> {
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

    /// Reads a quoted expression from the token queue.
    pub fn read_quote(&mut self) -> Result<Expr, ParserError> {
        self.tokens
            .pop_front()
            .ok_or(ParserError::InvalidToken("in read_quote"))?;

        let res = match self.tokens.front() {
            Some(t) => self.read_router(t)?(self)?,
            None => return Err(ParserError::InvalidToken("in read_quote")),
        };

        Ok(Expr::Quote(Box::new(res)))
    }

    /// Reads a list expression enclosed in parentheses.
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

    /// Reads a string literal enclosed in double quotes.
    pub fn read_string(&mut self) -> Result<Expr, ParserError> {
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

    /// Reads a keyword token prefixed with a colon.
    pub fn read_keyword(&mut self) -> Result<Expr, ParserError> {
        self.tokens.pop_front();

        let token = self
            .tokens
            .pop_front()
            .ok_or(ParserError::InvalidToken("in read_keyword"))?;

        Ok(Expr::Atom(Atom::read_keyword(&token)))
    }

    /// Reads a comment line prefixed with a semicolon.
    pub fn read_comment(&mut self) -> Result<Expr, ParserError> {
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
    /// Returns an iterator over all parsed expression nodes.
    pub fn iter_expr(&self) -> impl Iterator<Item = &Expr> {
        self.exprs.iter()
    }
}
