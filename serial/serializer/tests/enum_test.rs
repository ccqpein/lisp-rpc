use lisp_rpc_rust_serializer::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum MyEnum {
    ABC,
    OtherVariant,
    Simple,
    AllUPPER,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnumStruct {
    e: MyEnum,
    s: MyEnum,
}

#[test]
fn test_enum_deserialization() {
    // Test with quote ' and kebab-case
    let serialized = r#"(enum-struct :e 'a-b-c :s 'simple)"#;
    let mut ds = LispRPCDeserializer::from_str(serialized);
    let result: EnumStruct = EnumStruct::deserialize(&mut ds).unwrap();
    assert_eq!(result.e, MyEnum::ABC);
    assert_eq!(result.s, MyEnum::Simple);
}

#[test]
fn test_enum_pascal_deserialization() {
    // Test OtherVariant
    let serialized = r#"(enum-struct :e 'other-variant :s 'simple)"#;
    let mut ds = LispRPCDeserializer::from_str(serialized);
    let result: EnumStruct = EnumStruct::deserialize(&mut ds).unwrap();
    assert_eq!(result.e, MyEnum::OtherVariant);
}

#[test]
fn test_enum_preserve_uppercase() {
    // Test that ABC stays ABC
    let serialized = r#"(enum-struct :e 'ABC :s 'simple)"#;
    let mut ds = LispRPCDeserializer::from_str(serialized);
    let result: EnumStruct = EnumStruct::deserialize(&mut ds).unwrap();
    assert_eq!(result.e, MyEnum::ABC);

    let serialized = r#"(enum-struct :e 'OtherVariant :s 'simple)"#;
    let mut ds = LispRPCDeserializer::from_str(serialized);
    let result: EnumStruct = EnumStruct::deserialize(&mut ds).unwrap();
    assert_eq!(result.e, MyEnum::OtherVariant);

    let serialized = r#"(enum-struct :e 'AllUPPER :s 'simple)"#;
    let mut ds = LispRPCDeserializer::from_str(serialized);
    let result: EnumStruct = EnumStruct::deserialize(&mut ds).unwrap();
    assert_eq!(result.e, MyEnum::AllUPPER);
}
