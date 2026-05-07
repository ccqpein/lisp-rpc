use lisp_rpc_rust_serializer::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct LanguagePerfer {
    lang: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct OptionStruct {
    b: Option<i32>,
    c: Option<i32>,
    d: Option<String>,
}

#[test]
fn test_basic_serialization() {
    let mut buf = Vec::with_capacity(1024);
    let mut s = LispRPCSerializer::new(&mut buf);

    let lp = LanguagePerfer {
        lang: "eng".to_string(),
    };

    lp.serialize(&mut s).unwrap();

    let serialized = std::str::from_utf8(&s.output[..s.pos]).unwrap();
    assert_eq!(serialized, r#"(language-perfer :lang "eng")"#);

    let mut ds = LispRPCDeserializer::from_str(serialized);
    let lpd = LanguagePerfer::deserialize(&mut ds).unwrap();
    assert_eq!(lpd, lp);
}

#[test]
fn test_option_deserialization() {
    let serialized = r#"(option-struct :b 1 :d "aaa")"#;
    //let mut ds = LispRPCDeserializer::from_str(serialized);
    //let result: Result<OptionStruct, _> = OptionStruct::deserialize(&mut ds);
    let result = lisp_rpc_from_str::<OptionStruct>(serialized);
    assert_eq!(
        result.unwrap(),
        OptionStruct {
            b: Some(1),
            c: None,
            d: Some("aaa".to_string()),
        }
    );
}

#[test]
fn test_option_some_deserialization() {
    let serialized = r#"(option-struct :b 1 :c 2)"#;
    let mut ds = LispRPCDeserializer::from_str(serialized);
    let result: Result<OptionStruct, _> = OptionStruct::deserialize(&mut ds);
    assert_eq!(
        result.unwrap(),
        OptionStruct {
            b: Some(1),
            c: Some(2),
            d: None,
        }
    );
}

#[test]
fn test_option_nil_deserialization() {
    let serialized = r#"(option-struct :b nil :c 2)"#;
    let mut ds = LispRPCDeserializer::from_str(serialized);
    let result: Result<OptionStruct, _> = OptionStruct::deserialize(&mut ds);
    assert_eq!(
        result.unwrap(),
        OptionStruct {
            b: None,
            c: Some(2),
            d: None,
        }
    );
}

#[test]
fn test_option_all_missing_deserialization() {
    let serialized = r#"(option-struct)"#;
    let mut ds = LispRPCDeserializer::from_str(serialized);
    let result: Result<OptionStruct, _> = OptionStruct::deserialize(&mut ds);
    assert_eq!(
        result.unwrap(),
        OptionStruct {
            b: None,
            c: None,
            d: None,
        }
    );
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClientArgs {
    pub a_b: Option<String>,

    pub c_d: Option<String>,
}

#[test]
fn test_deserial_from_kebab_case() {
    let data = r#"
        (client-args :a-b "xx"
                     :c-d "1234")"#;

    assert_eq!(
        lisp_rpc_from_str::<ClientArgs>(&data).unwrap(),
        ClientArgs {
            a_b: Some("xx".to_string()),
            c_d: Some("1234".to_string()),
        },
    );
}

#[test]
fn test_deserial_from_kebab_case_partial() {
    let data = r#"
        (client-args :a-b "xx")"#;

    assert_eq!(
        lisp_rpc_from_str::<ClientArgs>(&data).unwrap(),
        ClientArgs {
            a_b: Some("xx".to_string()),
            c_d: None,
        },
    );
}
