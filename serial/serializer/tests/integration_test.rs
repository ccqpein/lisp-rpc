use lisp_rpc_rust_serializer::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct LanguagePerfer {
    lang: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BookInfo {
    lang: LanguagePerfer,
    title: String,
    version: String,
    id: String,
}

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct GetBook {
    title: String,
    version: String,
    lang: GetBookLangTmp,
    authors: Authors,
}

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct GetBookLangTmp {
    lang: String,
    encoding: i64,
}

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct Authors {
    names: Vec<String>,
}

#[test]
fn test_basic_serialization() {
    let mut buf = [0u8; 1024];
    let mut s = LispRPCSerializer::new(&mut buf);

    let lp = LanguagePerfer {
        lang: "eng".to_string(),
    };

    lp.serialize(&mut s).unwrap();
    //dbg!(s.output);

    assert_eq!(
        std::str::from_utf8(&s.output[..s.pos]).unwrap(),
        r#"(language-perfer :lang "eng")"#
    )
}

#[test]
fn test_seq_serialization() {
    let mut buf = [0u8; 1024];
    let mut s = LispRPCSerializer::new(&mut buf);

    let a = Authors {
        names: vec!["James bond".to_string(), "Steve Jobs".to_string()],
    };

    a.serialize(&mut s).unwrap();
    //dbg!(s.output);
    assert_eq!(
        std::str::from_utf8(&s.output[..s.pos]).unwrap(),
        r#"(authors :names '("James bond" "Steve Jobs"))"#
    )
}

#[test]
fn test_advance_serialization() {
    let mut buf = [0u8; 1024];
    let mut s = LispRPCSerializer::new(&mut buf);

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
    //dbg!(&s.output);

    let serialized = std::str::from_utf8(&s.output[..s.pos]).unwrap();
    assert_eq!(
        serialized,
        r#"(get-book :title "aa" :version "v1" :lang (get-book-lang-tmp :lang "eng" :encoding 64) :authors (authors :names '()))"#
    );

    let mut ds = LispRPCDeserializer::from_str(serialized);
    let gbd = GetBook::deserialize(&mut ds).unwrap();
    assert_eq!(gbd, gb);
}
