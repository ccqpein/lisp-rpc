use lisp_rpc_serializer::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct LanguagePerfer {
    lang: String,
}

impl ToRPCType for LanguagePerfer {
    fn to_rpc_type(&self) -> RPCType {
        RPCType::Msg("language-perfer".to_string())
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BookInfo {
    lang: LanguagePerfer,
    title: String,
    version: String,
    id: String,
}

impl ToRPCType for BookInfo {
    fn to_rpc_type(&self) -> RPCType {
        RPCType::Msg("book-info".to_string())
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetBook {
    title: String,
    version: String,
    lang: GetBookLangTmp,
    authors: Authors,
}

impl ToRPCType for GetBook {
    fn to_rpc_type(&self) -> RPCType {
        RPCType::RPC("get-book".to_string())
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetBookLangTmp {
    lang: String,
    encoding: i64,
}

impl ToRPCType for GetBookLangTmp {
    fn to_rpc_type(&self) -> RPCType {
        RPCType::Map
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Authors {
    names: Vec<String>,
}

impl ToRPCType for Authors {
    fn to_rpc_type(&self) -> RPCType {
        RPCType::Msg("authors".to_string())
    }
}

#[test]
fn test_basic_serialization() {
    let mut s = LispRPCSerializer {
        output: String::new(),
    };

    let lp = LanguagePerfer {
        lang: "eng".to_string(),
    };

    lp.serialize(&mut s).unwrap();
    dbg!(s.output);
}

#[test]
fn test_seq_serialization() {
    let mut s = LispRPCSerializer {
        output: String::new(),
    };

    let a = Authors {
        names: vec!["James bond".to_string(), "Steve Jobs".to_string()],
    };

    a.serialize(&mut s).unwrap();
    dbg!(s.output);
}

#[test]
fn test_advance_serialization() {
    let mut s = LispRPCSerializer {
        output: String::new(),
    };

    let gb = GetBook {
        title: "aa".to_string(),
        version: "v1".to_string(),
        lang: GetBookLangTmp {
            lang: "eng".to_string(),
            encoding: 64,
        },
        authors: Authors { names: vec![] },
    };

    gb.serialize(&mut s).unwrap();
    dbg!(&s.output);

    let mut ds = LispRPCDeserializer::from_str(&s.output);
    let gbd = GetBook::deserialize(&mut ds).unwrap();
    dbg!(gbd);
}
