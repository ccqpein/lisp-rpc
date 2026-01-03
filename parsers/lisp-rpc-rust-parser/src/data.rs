//! The pure rpc data like (get-book :title "hello world" :version "1984").
//!
//! The first symbol is the name of data, and everything else are the "arguments"

use std::{cell::OnceCell, collections::HashMap, env, error::Error, io::Cursor};

use itertools::Itertools;
use tracing::{debug, error};

use crate::{Atom, Expr, Parser, TypeValue, impl_into_data_for_numbers};

#[derive(Debug, PartialEq, Eq, Clone)]
enum DataErrorType {
    InvalidInput,
    CorruptedData,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct DataError {
    msg: String,
    err_type: DataErrorType,
}

impl std::fmt::Display for DataError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "data operation error {:?}", self)
    }
}

impl Error for DataError {}

pub trait FromExpr {
    fn from_expr(expr: &Expr) -> Result<Self, Box<dyn Error>>
    where
        Self: Sized;
}

pub trait FromStr: FromExpr {
    fn from_str(p: &Parser, s: &str) -> Result<Self, Box<dyn Error>>
    where
        Self: Sized,
    {
        let c = Cursor::new(s);
        let mut tkn = p.tokenize(c);

        let exp = p.read_router(tkn.get(0).ok_or(DataError {
            msg: "empty str".to_string(),
            err_type: DataErrorType::InvalidInput,
        })?)?(p, &mut tkn)?;

        Self::from_expr(&exp)
    }
}

pub trait IntoData {
    fn into_rpc_data(&self) -> Data;
}

// impl the into data for several type
impl_into_data_for_numbers!(i8, i16, i32, i64);

pub trait GetAbleData {
    fn get<'s>(&'s self, k: &'_ str) -> Option<&'s Data>;
}

/// define all the data, list, and map type that can be treat as Data
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Data {
    /// Data is (data-name keyword-data-pairs...)
    Data(ExprData),

    /// List is '(1 2 3 4 "d")
    List(ListData),

    /// Map is '(:a 1 :b 3)
    Map(MapData),

    /// Everything else is value
    Value(TypeValue),

    /// error if something happen
    Error(DataError),
}

impl Data {
    fn from_expr(e: &Expr) -> Result<Self, Box<dyn Error>> {
        match e {
            Expr::List(_) => Ok(Self::Data(ExprData::from_expr(e)?)),
            Expr::Quote(expr) => {
                // list or map
                match expr.as_ref() {
                    Expr::List(exprs) => match &exprs[0] {
                        // Map data
                        Expr::Atom(Atom {
                            value: crate::TypeValue::Keyword(_),
                            ..
                        }) => Ok(Self::Map(MapData::from_expr(e)?)),

                        // List data
                        Expr::Atom(Atom { .. }) => Ok(Self::List(ListData::from_expr(e)?)),

                        _ => Err(Box::new(DataError {
                            msg: format!("cannot generate Data from the expr {:?}", e),
                            err_type: DataErrorType::InvalidInput,
                        })),
                    },
                    Expr::Atom(Atom { value }) => Ok(Self::Value(value.clone())),
                    _ => Err(Box::new(DataError {
                        msg: format!("cannot generate Data from the expr {:?}", e),
                        err_type: DataErrorType::InvalidInput,
                    })),
                }
            }
            Expr::Atom(a) => match &a.value {
                TypeValue::Symbol(_) => {
                    error!("symbol cannot be data");
                    Err(Box::new(DataError {
                        msg: format!("cannot generate Data from the symbol {:?}", a),
                        err_type: DataErrorType::InvalidInput,
                    }))
                }
                vv @ _ => Ok(Self::Value(vv.clone())),
            },
        }
    }

    fn to_string(&self) -> String {
        match self {
            Data::Data(value_data) => value_data.to_string(),
            Data::List(list_data) => list_data.to_string(),
            Data::Map(map_data) => map_data.to_string(),
            Data::Value(type_value) => type_value.to_string(),
            Data::Error(data_error) => format!("{:?}", data_error),
        }
    }

    /// generate the root data.
    /// root data has to be expr
    pub fn new<'a>(
        name: &str,
        kv_pairs: impl Iterator<Item = (&'a str, &'a dyn IntoData)>,
    ) -> Result<Self, Box<dyn Error>> {
        Ok(Data::Data(ExprData::new(
            name,
            kv_pairs.map(|(s, x)| {
                (
                    Expr::Atom(Atom {
                        value: TypeValue::Keyword(s.to_string()),
                    }),
                    x.into_rpc_data(),
                )
            }),
        )?))
    }

    /// read the root data.
    pub fn from_root_str(s: &str, parser: Option<&Parser>) -> Result<Self, Box<dyn Error>> {
        let p = match parser {
            Some(p) => p,
            None => &Default::default(),
        };

        match Self::from_str(&p, s) {
            Ok(d) => match d {
                Data::Data(expr_data) => Ok(Self::Data(expr_data)),
                Data::Error(data_error) => Err(Box::new(data_error)),
                _ => Err(Box::new(DataError {
                    msg: "root data has to be expr data".to_string(),
                    err_type: DataErrorType::InvalidInput,
                })),
            },
            e @ Err(_) => e,
        }
    }
}

impl FromExpr for Data {
    fn from_expr(expr: &Expr) -> Result<Self, Box<dyn Error>>
    where
        Self: Sized,
    {
        Self::from_expr(expr)
    }
}

impl FromStr for Data {}

impl std::fmt::Display for Data {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_string())
    }
}

impl IntoData for Data {
    fn into_rpc_data(&self) -> Data {
        self.clone()
    }
}

impl GetAbleData for Data {
    fn get<'s>(&'s self, k: &'_ str) -> Option<&'s Data> {
        match self {
            Data::Data(expr_data) => <ExprData as GetAbleData>::get(expr_data, k),
            Data::Map(map_data) => <MapData as GetAbleData>::get(map_data, k),
            _ => None,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ExprData {
    name: String,
    rest_args: Vec<(Expr, Data)>,
    inner_map: OnceCell<DataMap>,
}

impl ExprData {
    fn from_expr(expr: &Expr) -> Result<Self, Box<dyn Error>> {
        let exprs = match expr {
            Expr::List(ee) => ee,
            _ => {
                return Err(Box::new(DataError {
                    msg: "cannot generate ExprData from this expr".to_string(),
                    err_type: DataErrorType::InvalidInput,
                }));
            }
        };

        if exprs.len() < 1 {
            return Err(Box::new(DataError {
                msg: "empty data".to_string(),
                err_type: DataErrorType::InvalidInput,
            }));
        }

        if exprs.len() % 2 != 1 {
            return Err(Box::new(DataError {
                msg: "rest data has to be odd length elements".to_string(),
                err_type: DataErrorType::InvalidInput,
            }));
        }

        let name = match &exprs[0] {
            Expr::Atom(Atom {
                value: crate::TypeValue::Symbol(s),
            }) => s,
            _ => {
                return Err(Box::new(DataError {
                    msg: "data's first element has to be symbol".to_string(),
                    err_type: DataErrorType::InvalidInput,
                }));
            }
        };

        let mut rest_a = vec![];
        for [k, v] in exprs[1..].into_iter().array_chunks() {
            match (k, v) {
                (
                    Expr::Atom(Atom {
                        value: crate::TypeValue::Keyword(_),
                    }),
                    _,
                ) => rest_a.push((k.clone(), Data::from_expr(v)?)),
                _ => {
                    return Err(Box::new(DataError {
                        msg: "has to be keyword value pairs".to_string(),
                        err_type: DataErrorType::InvalidInput,
                    }));
                }
            }
        }

        Ok(Self {
            name: name.to_string(),
            rest_args: rest_a,
            inner_map: OnceCell::new(), // generate when get method called
        })
    }

    /// make new expr data
    fn new<'a>(
        name: &str,
        rest_args: impl Iterator<Item = (Expr, Data)>,
    ) -> Result<Self, Box<dyn Error>> {
        let _ = TypeValue::make_symbol(name)?;
        Ok(Self {
            name: name.to_string(),
            rest_args: rest_args.collect(),
            inner_map: OnceCell::new(),
        })
    }

    /// the name of the expr, always the first element depending on the spec
    pub fn get_name(&self) -> &str {
        &self.name
    }

    /// generate the data
    fn to_string(&self) -> String {
        format!(
            "({} {})",
            self.name,
            self.rest_args
                .iter()
                .map(|(k, v)| format!("{} {}", k.to_string(), v.to_string()))
                .join(" ")
        )
    }

    pub fn get(&self, k: &str) -> Option<&Data> {
        let m = self
            .inner_map
            .get_or_init(|| DataMap::new(&self.rest_args).unwrap());
        m.get(k)
    }
}

impl FromExpr for ExprData {
    fn from_expr(expr: &Expr) -> Result<Self, Box<dyn Error>>
    where
        Self: Sized,
    {
        Self::from_expr(expr)
    }
}

impl FromStr for ExprData {}

impl IntoData for ExprData {
    fn into_rpc_data(&self) -> Data {
        Data::Data(self.clone())
    }
}

impl GetAbleData for ExprData {
    fn get<'s>(&'s self, k: &'_ str) -> Option<&'s Data> {
        self.get(k)
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ListData {
    inner_data: Vec<Data>,
}

impl FromExpr for ListData {
    fn from_expr(expr: &Expr) -> Result<Self, Box<dyn Error>>
    where
        Self: Sized,
    {
        Self::from_expr(expr)
    }
}

impl FromStr for ListData {}

impl IntoData for ListData {
    fn into_rpc_data(&self) -> Data {
        Data::List(self.clone())
    }
}

impl ListData {
    pub fn from_expr(expr: &Expr) -> Result<Self, Box<dyn Error>> {
        match expr {
            Expr::Quote(expr) => match expr.as_ref() {
                Expr::List(exprs) => {
                    let mut res = vec![];
                    for e in exprs {
                        res.push(Data::from_expr(e)?);
                    }

                    Ok(Self { inner_data: res })
                }
                _ => Err(Box::new(DataError {
                    msg: "cannot generate ListData from this expr, not list after quote"
                        .to_string(),
                    err_type: DataErrorType::InvalidInput,
                })),
            },
            _ => Err(Box::new(DataError {
                msg: "cannot generate ListData from this expr, need quoted".to_string(),
                err_type: DataErrorType::InvalidInput,
            })),
        }
    }

    fn to_string(&self) -> String {
        format!(
            "'({})",
            self.inner_data.iter().map(|d| d.to_string()).join(" ")
        )
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct MapData {
    kwrds: Vec<String>,
    map: DataMap,
}

impl MapData {
    pub fn from_expr(expr: &Expr) -> Result<Self, Box<dyn Error>> {
        let mut kwrds = vec![];
        let map = match expr {
            Expr::Quote(e2) => match e2.as_ref() {
                Expr::List(ee) => {
                    for [k, _] in ee.iter().array_chunks() {
                        match k {
                            Expr::Atom(Atom {
                                value: crate::TypeValue::Keyword(k),
                            }) => {
                                kwrds.push(k.to_string());
                            }
                            _ => {
                                return Err(Box::new(DataError {
                                    msg: "MapData has to be keyword pairs like '(:a 1 :b 2)"
                                        .to_string(),
                                    err_type: DataErrorType::InvalidInput,
                                }));
                            }
                        }
                    }

                    DataMap::from_exprs(&ee)?
                }
                _ => {
                    return Err(Box::new(DataError {
                        msg: "MapData has to be quoted like '(:a 1 :b 2)".to_string(),
                        err_type: DataErrorType::InvalidInput,
                    }));
                }
            },
            _ => {
                return Err(Box::new(DataError {
                    msg: "MapData has to be quoted like '(:a 1 :b 2)".to_string(),
                    err_type: DataErrorType::InvalidInput,
                }));
            }
        };

        Ok(Self { kwrds, map })
    }

    pub fn to_string(&self) -> String {
        format!(
            "'({})",
            self.kwrds
                .iter()
                .map(|k| [
                    format!(":{}", k.to_string()),
                    self.map
                        .get(k)
                        .unwrap_or(&Data::Error(DataError {
                            msg: "corrupted data".to_string(),
                            err_type: DataErrorType::CorruptedData
                        }))
                        .to_string()
                ])
                .flatten()
                .join(" ")
        )
    }

    fn get(&self, k: &str) -> Option<&Data> {
        self.map.get(k)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &Data)> {
        self.map.iter()
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }
}

impl FromExpr for MapData {
    fn from_expr(expr: &Expr) -> Result<Self, Box<dyn Error>>
    where
        Self: Sized,
    {
        Self::from_expr(expr)
    }
}

impl FromStr for MapData {}

impl IntoData for MapData {
    fn into_rpc_data(&self) -> Data {
        Data::Map(self.clone())
    }
}

impl GetAbleData for MapData {
    fn get<'s>(&'s self, k: &'_ str) -> Option<&'s Data> {
        self.get(k)
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
struct DataMap {
    hash_map: HashMap<String, Data>,
}

impl DataMap {
    fn from_exprs(exprs: &[Expr]) -> Result<Self, Box<dyn Error>> {
        let mut table = HashMap::new();
        for [k, v] in exprs.iter().array_chunks() {
            match (k, v) {
                (
                    Expr::Atom(Atom {
                        value: crate::TypeValue::Keyword(k),
                    }),
                    _,
                ) => {
                    table.insert(k.to_string(), Data::from_expr(v)?);
                }
                _ => {
                    return Err(Box::new(DataError {
                        msg: "has to be keyword value pairs for making the data map".to_string(),
                        err_type: DataErrorType::InvalidInput,
                    }));
                }
            }
        }

        Ok(Self { hash_map: table })
    }

    fn new(kv: &[(Expr, Data)]) -> Result<Self, Box<dyn Error>> {
        let mut table = HashMap::new();

        for (e, d) in kv {
            match (e, d) {
                (
                    Expr::Atom(Atom {
                        value: TypeValue::Keyword(k),
                    }),
                    dd,
                ) => table.insert(k.to_string(), dd.clone()),
                _ => {
                    return Err(Box::new(DataError {
                        msg: "has to be keyword value pairs for making the data map".to_string(),
                        err_type: DataErrorType::InvalidInput,
                    }));
                }
            };
        }

        Ok(Self { hash_map: table })
    }

    pub fn get(&self, k: &'_ str) -> Option<&Data> {
        match self.hash_map.get(k) {
            Some(vv) => Some(vv),
            None => None,
        }
    }

    pub fn to_string(&self) -> String {
        self.hash_map
            .iter()
            .map(|(k, v)| format!(":{} {}", k, v.to_string()))
            .join(" ")
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &Data)> {
        self.hash_map.iter()
    }

    pub fn len(&self) -> usize {
        self.hash_map.len()
    }
}

impl FromIterator<(String, Data)> for DataMap {
    fn from_iter<T: IntoIterator<Item = (String, Data)>>(iter: T) -> Self {
        Self {
            hash_map: iter.into_iter().collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::assert_matches::assert_matches;

    use super::*;

    #[test]
    fn test_read_data_from_str() {
        let s = r#"(get-book :title "hello world" :version "1984")"#;
        let p = Parser::new();
        let d = ExprData::from_str(&p, s);
        //dbg!(&d);
        assert!(d.is_ok());

        let dd = d.unwrap();
        assert_eq!(dd.get_name(), "get-book");

        assert_eq!(dd.get_name(), "get-book");
        assert_eq!(
            dd.get("title"),
            Some(&Data::from_str(&p, r#""hello world""#).unwrap())
        );

        //

        let s = r#"(get-book :title "hello world" :version 1984)"#;

        let d = ExprData::from_str(&Parser::new().config_read_number(true), s).unwrap();

        assert_eq!(d.get_name(), "get-book");
        assert_eq!(
            d.get("version"),
            Some(&Data::Value(TypeValue::Number(1984)))
        );

        //

        let s = r#"(rpc-call :version 1 :aa 2)"#;
        let d = ExprData::from_str(&Default::default(), s);
        assert!(d.is_ok())
    }

    #[test]
    fn test_read_nest_data() {
        let s = r#"(get-book :title "hello world" :version "1984" :lang '(:lang "english" :encoding 77))"#;
        let d = Data::from_str(&Default::default(), s).unwrap();
        //dbg!(&d);
        assert!(matches!(d, Data::Data(_)));

        assert_eq!(
            d.get("title"),
            Some(&Data::Value(TypeValue::String("hello world".to_string())))
        );

        assert_eq!(
            d.get("version"),
            Some(&Data::Value(TypeValue::String("1984".to_string())))
        );

        assert!(matches!(d.get("lang"), Some(&Data::Map(_))));

        let Some(Data::Map(dd)) = d.get("lang") else {
            panic!()
        };

        assert_eq!(
            dd.get("lang"),
            Some(&Data::Value(TypeValue::String("english".to_string())))
        );

        assert_eq!(
            dd.get("encoding"),
            Some(&Data::Value(TypeValue::Number(77)))
        );

        //
        let s = r#"(book-info :id "123" :title "hello world" :version "1984" :lang (language-perfer :lang "english"))"#;
        let d = Data::from_str(&Default::default(), s).unwrap();

        assert!(matches!(d, Data::Data(_)));

        assert_eq!(
            d.get("title"),
            Some(&Data::Value(TypeValue::String("hello world".to_string())))
        );

        assert_eq!(
            d.get("id"),
            Some(&Data::Value(TypeValue::String("123".to_string())))
        );

        assert_eq!(
            d.get("version"),
            Some(&Data::Value(TypeValue::String("1984".to_string())))
        );

        assert!(matches!(d.get("lang"), Some(&Data::Data(_))));

        let Some(Data::Data(dd)) = d.get("lang") else {
            panic!()
        };

        assert_eq!(
            dd.get("lang"),
            Some(&Data::Value(TypeValue::String("english".to_string())))
        );

        assert_eq!(dd.get_name(), "language-perfer");
    }

    #[test]
    fn test_read_data_from_str_nesty() {
        let s = r#"(get-book :title "hello world" :version '(1 2 3 4) :map '(:a 2 :r 4))"#;
        let p = Parser::new().config_read_number(true);

        //dbg!(p.parse_root(Cursor::new(s)));

        let d = Data::from_str(&p, s).unwrap();

        //dbg!(&d);
        assert_matches!(d, Data::Data(ExprData { .. }));

        assert_eq!(
            d.to_string(),
            r#"(get-book :title "hello world" :version '(1 2 3 4) :map '(:a 2 :r 4))"#
        );

        let Data::Data(d) = d else { panic!() };

        assert_eq!(
            d.get("version"),
            Some(&Data::List(
                ListData::from_str(&p, r#"'(1 2 3 4)"#).unwrap()
            ))
        );

        assert_eq!(
            d.get("map"),
            Some(&Data::Map(
                MapData::from_str(&p, r#"'(:a 2 :r 4)"#).unwrap()
            ))
        );

        assert_eq!(
            d.to_string(),
            r#"(get-book :title "hello world" :version '(1 2 3 4) :map '(:a 2 :r 4))"#
        )
    }

    #[test]
    fn test_data_to_str() {
        let p = Parser::new();
        let s = r#"(get-book :title "hello world" :version "1984")"#;
        let d = ExprData::from_str(&p, s).unwrap();

        assert_eq!(s, d.to_string());

        //

        let e = ExprData::new("a b", [].into_iter());
        assert!(e.is_err());

        //

        let e = ExprData::new("a-b", [].into_iter());
        assert!(e.is_ok());
        assert_eq!(e.unwrap().to_string(), "(a-b )")
    }

    #[test]
    fn test_get_data() {
        let p = Parser::new();
        let e =
            ExprData::from_str(&p, r#"(get-book :title "hello world" :version "1984")"#).unwrap();

        assert_eq!(
            e.get("title"),
            Some(&Data::Value(TypeValue::String("hello world".to_string()))),
        );
    }

    #[test]
    fn test_make_map_data() {
        let p = Parser::new();
        let e = Data::from_str(
            &p,
            r#"'(:title 'string :version 'string :lang 'language-perfer)"#,
        )
        .unwrap();

        matches!(e, Data::Map(_));
        assert_eq!(
            e.get("version"),
            Some(&Data::Value(TypeValue::Symbol("string".to_string())))
        );

        assert_eq!(
            e.get("lang"),
            Some(&Data::Value(TypeValue::Symbol(
                "language-perfer".to_string()
            )))
        );

        //

        let e = Data::from_str(
            &p,
            r#"'(:title 'string :vesion 'string :lang '(:lang 'string :encoding 'number))"#,
        )
        .unwrap();
        matches!(e, Data::Map(_));
        assert_eq!(
            e.get("lang"),
            Some(&Data::from_str(&p, r#"'(:lang 'string :encoding 'number)"#,).unwrap())
        );

        assert_eq!(
            e,
            Data::Map(
                MapData::from_str(
                    &p,
                    r#"'(:title 'string :vesion 'string :lang '(:lang 'string :encoding 'number))"#,
                )
                .unwrap()
            )
        );
    }
}
