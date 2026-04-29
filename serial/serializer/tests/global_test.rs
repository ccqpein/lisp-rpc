use lisp_rpc_rust_serializer::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct GlobalLanguage {
    name: String,
}

#[test]
fn test_global_dispatch_serialization() {
    let mut buf = Vec::with_capacity(1024);
    let mut s = LispRPCSerializer::new(&mut buf);

    // Register GlobalLanguage globally
    register_global_map_type("GlobalLanguage");

    let gl = GlobalLanguage {
        name: "rust".to_string(),
    };

    // Serialize normally - it should pick up the map style from the GLOBAL registry
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
