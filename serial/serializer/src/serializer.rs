use std::{
    collections::HashSet,
    error::Error as StdError,
    fmt::{self, Display},
    sync::{LazyLock, RwLock},
};

use serde::ser::{
    Serialize, SerializeMap, SerializeSeq, SerializeStruct, SerializeStructVariant, SerializeTuple,
    SerializeTupleStruct, SerializeTupleVariant, Serializer,
};

use convert_case::{Case, Casing};

/// Global registry of struct names that should be serialized as maps
pub static GLOBAL_MAP_TYPES: LazyLock<RwLock<HashSet<&'static str>>> =
    LazyLock::new(|| RwLock::new(HashSet::new()));

/// Register a struct name globally to always be serialized as a map
pub fn register_global_map_type(name: &'static str) {
    if let Ok(mut map) = GLOBAL_MAP_TYPES.write() {
        map.insert(name);
    }
}

#[derive(Debug)]
pub enum LispRPCSerializerError {
    Msg(String),
    BufferOverflow(String),
    NotSupport,
}

impl fmt::Display for LispRPCSerializerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LispRPCSerializerError::Msg(msg) | LispRPCSerializerError::BufferOverflow(msg) => {
                f.write_str(msg)
            }
            LispRPCSerializerError::NotSupport => f.write_str("not support"),
        }
    }
}

impl StdError for LispRPCSerializerError {}

impl serde::ser::Error for LispRPCSerializerError {
    fn custom<T>(msg: T) -> Self
    where
        T: Display,
    {
        Self::Msg(format!("{msg}"))
    }
}

impl serde::de::Error for LispRPCSerializerError {
    fn custom<T>(msg: T) -> Self
    where
        T: Display,
    {
        Self::Msg(format!("{msg}"))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LispRPCStyle {
    /// Standard struct: (name :field value)
    Struct,
    /// Map style: '(:field value)
    Map,
}

/// the serializer of Msg/RPC/List (Vec)/V RPCType
pub struct LispRPCSerializer<'s> {
    pub output: &'s mut Vec<u8>,
    pub pos: usize,
    pub style: LispRPCStyle,
    /// Instance-specific registry of struct names that should be serialized as maps
    pub map_types: HashSet<&'static str>,
}

impl<'s> LispRPCSerializer<'s> {
    pub fn new(buffer: &'s mut Vec<u8>) -> Self {
        Self {
            output: buffer,
            pos: 0,
            style: LispRPCStyle::Struct,
            map_types: HashSet::new(),
        }
    }

    /// Register a struct name to this instance to always be serialized as a map
    pub fn register_map_type(&mut self, name: &'static str) {
        self.map_types.insert(name);
    }

    fn write_bytes(&mut self, bytes: &[u8]) -> Result<(), LispRPCSerializerError> {
        self.output.extend_from_slice(bytes);
        self.pos += bytes.len();
        Ok(())
    }
}

impl<'a, 's: 'a> SerializeSeq for &'a mut LispRPCSerializer<'s> {
    type Ok = ();

    type Error = LispRPCSerializerError;

    fn serialize_element<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        if !self.output.ends_with(b"(") {
            self.write_bytes(b" ")?;
        }

        value.serialize(&mut **self)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.write_bytes(b")")?;
        Ok(())
    }
}

impl<'a, 's: 'a> SerializeTuple for &'a mut LispRPCSerializer<'s> {
    type Ok = ();

    type Error = LispRPCSerializerError;

    fn serialize_element<T>(&mut self, _value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        Err(LispRPCSerializerError::NotSupport)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Err(LispRPCSerializerError::NotSupport)
    }
}

impl<'a, 's: 'a> SerializeTupleStruct for &'a mut LispRPCSerializer<'s> {
    type Ok = ();

    type Error = LispRPCSerializerError;

    fn serialize_field<T>(&mut self, _value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        Err(LispRPCSerializerError::NotSupport)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Err(LispRPCSerializerError::NotSupport)
    }
}

impl<'a, 's: 'a> SerializeMap for &'a mut LispRPCSerializer<'s> {
    type Ok = ();

    type Error = LispRPCSerializerError;

    fn serialize_key<T>(&mut self, _key: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        Err(LispRPCSerializerError::NotSupport)
    }

    fn serialize_value<T>(&mut self, _value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        Err(LispRPCSerializerError::NotSupport)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Err(LispRPCSerializerError::NotSupport)
    }
}

impl<'a, 's: 'a> SerializeTupleVariant for &'a mut LispRPCSerializer<'s> {
    type Ok = ();

    type Error = LispRPCSerializerError;

    fn serialize_field<T>(&mut self, _value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        Err(LispRPCSerializerError::NotSupport)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Err(LispRPCSerializerError::NotSupport)
    }
}

impl<'a, 's: 'a> SerializeStructVariant for &'a mut LispRPCSerializer<'s> {
    type Ok = ();

    type Error = LispRPCSerializerError;

    fn serialize_field<T>(&mut self, _key: &'static str, _value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        Err(LispRPCSerializerError::NotSupport)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Err(LispRPCSerializerError::NotSupport)
    }
}

impl<'a, 's: 'a> SerializeStruct for &'a mut LispRPCSerializer<'s> {
    type Ok = ();

    type Error = LispRPCSerializerError;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        if !self.output.ends_with(b"(") {
            self.write_bytes(b" ")?;
        }
        self.write_bytes(b":")?;
        self.write_bytes(key.to_case(Case::Kebab).as_bytes())?;
        self.write_bytes(b" ")?;

        value.serialize(&mut **self)?;
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.write_bytes(b")")?;
        Ok(())
    }
}

/// impl the Serializer, most important one is struct and primary types
impl<'a, 's: 'a> Serializer for &'a mut LispRPCSerializer<'s> {
    type Ok = ();

    type Error = LispRPCSerializerError;

    type SerializeSeq = Self;

    type SerializeTuple = Self;

    type SerializeTupleStruct = Self;

    type SerializeTupleVariant = Self;

    type SerializeMap = Self;

    type SerializeStruct = Self;

    type SerializeStructVariant = Self;

    fn serialize_bool(self, v: bool) -> Result<Self::Ok, Self::Error> {
        self.write_bytes(if v { b"t" } else { b"nil" })?;
        Ok(())
    }

    fn serialize_i8(self, v: i8) -> Result<Self::Ok, Self::Error> {
        self.serialize_i64(i64::from(v))
    }

    fn serialize_i16(self, v: i16) -> Result<Self::Ok, Self::Error> {
        self.serialize_i64(i64::from(v))
    }

    fn serialize_i32(self, v: i32) -> Result<Self::Ok, Self::Error> {
        self.serialize_i64(i64::from(v))
    }

    fn serialize_i64(self, v: i64) -> Result<Self::Ok, Self::Error> {
        self.write_bytes(v.to_string().as_bytes())
    }

    fn serialize_u8(self, v: u8) -> Result<Self::Ok, Self::Error> {
        self.serialize_u64(u64::from(v))
    }

    fn serialize_u16(self, v: u16) -> Result<Self::Ok, Self::Error> {
        self.serialize_u64(u64::from(v))
    }

    fn serialize_u32(self, v: u32) -> Result<Self::Ok, Self::Error> {
        self.serialize_u64(u64::from(v))
    }

    fn serialize_u64(self, v: u64) -> Result<Self::Ok, Self::Error> {
        self.write_bytes(v.to_string().as_bytes())
    }

    fn serialize_f32(self, v: f32) -> Result<Self::Ok, Self::Error> {
        self.serialize_f64(f64::from(v))
    }

    fn serialize_f64(self, v: f64) -> Result<Self::Ok, Self::Error> {
        self.write_bytes(v.to_string().as_bytes())
    }

    fn serialize_char(self, v: char) -> Result<Self::Ok, Self::Error> {
        self.serialize_str(&v.to_string())
    }

    fn serialize_str(self, v: &str) -> Result<Self::Ok, Self::Error> {
        self.write_bytes(b"\"")?;
        self.write_bytes(v.as_bytes())?;
        self.write_bytes(b"\"")?;
        Ok(())
    }

    fn serialize_bytes(self, _v: &[u8]) -> Result<Self::Ok, Self::Error> {
        Err(LispRPCSerializerError::NotSupport)
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        self.serialize_unit()
    }

    fn serialize_some<T>(self, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        Err(LispRPCSerializerError::NotSupport)
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<Self::Ok, Self::Error> {
        Err(LispRPCSerializerError::NotSupport)
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        Err(LispRPCSerializerError::NotSupport)
    }

    fn serialize_newtype_struct<T>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T>(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + Serialize,
    {
        Err(LispRPCSerializerError::NotSupport)
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        self.write_bytes(b"'(")?;
        Ok(self)
    }

    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        Err(LispRPCSerializerError::NotSupport)
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        Err(LispRPCSerializerError::NotSupport)
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        Err(LispRPCSerializerError::NotSupport)
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        Err(LispRPCSerializerError::NotSupport)
    }

    fn serialize_struct(
        self,
        name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        let is_map = self.map_types.contains(name)
            || GLOBAL_MAP_TYPES
                .read()
                .map(|map| map.contains(name))
                .unwrap_or(false);

        let style = if is_map {
            LispRPCStyle::Map
        } else {
            self.style
        };

        match style {
            LispRPCStyle::Struct => {
                self.write_bytes(b"(")?;
                self.write_bytes(name.to_case(Case::Kebab).as_bytes())?;
            }
            LispRPCStyle::Map => {
                self.write_bytes(b"'(")?;
            }
        }
        Ok(self)
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        Err(LispRPCSerializerError::NotSupport)
    }
}
