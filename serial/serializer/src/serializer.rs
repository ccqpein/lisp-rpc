
use std::{
    error::Error as StdError,
    fmt::{self, Display},
};

use serde::ser::{
    Serialize, SerializeMap, SerializeSeq, SerializeStruct, SerializeStructVariant, SerializeTuple,
    SerializeTupleStruct, SerializeTupleVariant, Serializer,
};

use convert_case::{Case, Casing};

#[derive(Debug)]
pub enum LispRPCSerializerError {
    Msg(String),
    NotSupport,
}

impl fmt::Display for LispRPCSerializerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LispRPCSerializerError::Msg(msg) => f.write_str(msg),
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

/// the serializer of Msg/RPC/List (Vec)/V RPCType
pub struct LispRPCSerializer {
    pub output: String,
}

impl<'a> SerializeSeq for &'a mut LispRPCSerializer {
    type Ok = ();

    type Error = LispRPCSerializerError;

    fn serialize_element<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        if !self.output.ends_with("'(") {
            self.output += " ";
        }

        value.serialize(&mut **self)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.output += ")";
        Ok(())
    }
}

impl<'a> SerializeTuple for &'a mut LispRPCSerializer {
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

impl<'a> SerializeTupleStruct for &'a mut LispRPCSerializer {
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

impl<'a> SerializeMap for &'a mut LispRPCSerializer {
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

impl<'a> SerializeTupleVariant for &'a mut LispRPCSerializer {
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

impl<'a> SerializeStructVariant for &'a mut LispRPCSerializer {
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

impl<'a> SerializeStruct for &'a mut LispRPCSerializer {
    type Ok = ();

    type Error = LispRPCSerializerError;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + Serialize,
    {
        self.output.push_str(" :");
        self.output.push_str(key);
        self.output.push_str(" ");

        value.serialize(&mut **self)?;
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.output.push(')');
        Ok(())
    }
}

/// impl the Serializer, most important one is struct and primary types
impl<'a> Serializer for &'a mut LispRPCSerializer {
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
        self.output += if v { "t" } else { "nil" };
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
        self.output += &v.to_string();
        Ok(())
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
        self.output += &v.to_string();
        Ok(())
    }

    fn serialize_f32(self, v: f32) -> Result<Self::Ok, Self::Error> {
        self.serialize_f64(f64::from(v))
    }

    fn serialize_f64(self, v: f64) -> Result<Self::Ok, Self::Error> {
        self.output += &v.to_string();
        Ok(())
    }

    fn serialize_char(self, v: char) -> Result<Self::Ok, Self::Error> {
        self.serialize_str(&v.to_string())
    }

    fn serialize_str(self, v: &str) -> Result<Self::Ok, Self::Error> {
        self.output.push_str("\"");
        self.output.push_str(v);
        self.output.push_str("\"");
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
        _value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + Serialize,
    {
        Err(LispRPCSerializerError::NotSupport)
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
        self.output += "'(";
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
        self.output.push('(');
        self.output.push_str(&name.to_case(Case::Kebab));
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
