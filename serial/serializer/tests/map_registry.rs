use lisp_rpc_rust_serializer::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct LanguagePerfer {
    lang: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct GlobalLanguage {
    name: String,
}

#[test]
fn test_local_map_registration() {
    let mut buf = Vec::with_capacity(1024);
    let mut s = LispRPCSerializer::new(&mut buf);

    let lp = LanguagePerfer {
        lang: "eng".to_string(),
    };

    s.register_map_type("LanguagePerfer");
    lp.serialize(&mut s).unwrap();

    let serialized = std::str::from_utf8(&s.output[..s.pos]).unwrap();
    assert_eq!(serialized, r#"'(:lang "eng")"#);
}

#[test]
fn test_global_dispatch_serialization() {
    let mut buf = Vec::with_capacity(1024);
    let mut s = LispRPCSerializer::new(&mut buf);

    register_global_map_type("GlobalLanguage");

    let gl = GlobalLanguage {
        name: "rust".to_string(),
    };

    gl.serialize(&mut s).unwrap();

    let serialized = std::str::from_utf8(&s.output[..s.pos]).unwrap();
    assert_eq!(
        serialized,
        r#"'(:name "rust")"#
    );

    let mut ds = LispRPCDeserializer::from_str(serialized);
    let gld = GlobalLanguage::deserialize(&mut ds).unwrap();
    assert_eq!(gld, gl);
}

#[test]
fn test_clear_global_registry() {
    let mut buf = Vec::with_capacity(1024);
    
    register_global_map_type("GlobalLanguage");
    let gl = GlobalLanguage { name: "rust".to_string() };
    
    let mut s1 = LispRPCSerializer::new(&mut buf);
    gl.serialize(&mut s1).unwrap();
    assert_eq!(std::str::from_utf8(&s1.output[..s1.pos]).unwrap(), r#"'(:name "rust")"#);
    
    clear_global_map_types();
    buf.clear();
    let mut s2 = LispRPCSerializer::new(&mut buf);
    gl.serialize(&mut s2).unwrap();
    assert_eq!(std::str::from_utf8(&s2.output[..s2.pos]).unwrap(), r#"(global-language :name "rust")"#);
}
