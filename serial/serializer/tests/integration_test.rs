use lisp_rpc_serializer::*;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct LanguagePerfer {
    lang: String,
}

impl ToRPCType for LanguagePerfer {
    fn to_rpc_type(&self) -> RPCType {
        RPCType::Msg("language-perfer".to_string())
    }
}

#[derive(Debug, Serialize)]
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
