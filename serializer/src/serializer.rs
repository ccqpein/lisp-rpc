//! Serializer implementation transforming Rust data structures into Lisp-RPC S-expressions.

use std::{
    collections::HashSet,
    error::Error as StdError,
    fmt::{self, Display},
    sync::{
        LazyLock, RwLock,
        atomic::{AtomicBool, Ordering},
    },
};

use serde::ser::{
    Serialize, SerializeMap, SerializeSeq, SerializeStruct, SerializeStructVariant, SerializeTuple,
    SerializeTupleStruct, SerializeTupleVariant, Serializer,
};

use convert_case::{Case, Casing};

/// Fast atomic flag indicating whether any global map types are currently registered.
static HAS_GLOBAL_MAP_TYPES: AtomicBool = AtomicBool::new(false);

/// Global registry of struct names that should be serialized in map style `'(:key value)`.
pub static GLOBAL_MAP_TYPES: LazyLock<RwLock<HashSet<&'static str>>> =
    LazyLock::new(|| RwLock::new(HashSet::new()));

/// Registers a struct name globally to always be serialized as a map.
pub fn register_global_map_type(name: &'static str) {
    if let Ok(mut map) = GLOBAL_MAP_TYPES.write() {
        map.insert(name);
        HAS_GLOBAL_MAP_TYPES.store(true, Ordering::Release);
    }
}

/// Clears all globally registered map types.
pub fn clear_global_map_types() {
    if let Ok(mut map) = GLOBAL_MAP_TYPES.write() {
        map.clear();
        HAS_GLOBAL_MAP_TYPES.store(false, Ordering::Release);
    }
}

/// Error type for serialization failures in Lisp-RPC.
#[derive(Debug)]
pub enum LispRPCSerializerError {
    /// Custom error message.
    Msg(String),
    /// Buffer overflow error message.
    BufferOverflow(String),
    /// Operation or data type is not supported.
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

/// Formatting style for serializing structures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LispRPCStyle {
    /// Standard named struct format `(name :field value ...)`.
    Struct,
    /// Quoted anonymous map format `'(:field value ...)`.
    Map,
}

/// Serde [`Serializer`] for Lisp-RPC S-expression formatting.
pub struct LispRPCSerializer<'s> {
    /// Destination byte buffer.
    pub output: &'s mut Vec<u8>,
    /// Number of bytes written.
    pub pos: usize,
    /// Current serialization style.
    pub style: LispRPCStyle,
    /// Instance-specific registry of struct names that should be serialized as maps.
    pub map_types: HashSet<&'static str>,
}

impl<'s> LispRPCSerializer<'s> {
    /// Creates a new serializer writing to the provided buffer.
    pub fn new(buffer: &'s mut Vec<u8>) -> Self {
        Self {
            output: buffer,
            pos: 0,
            style: LispRPCStyle::Struct,
            map_types: HashSet::new(),
        }
    }

    /// Registers a struct name to this instance to always be serialized as a map.
    pub fn register_map_type(&mut self, name: &'static str) {
        self.map_types.insert(name);
    }

    #[inline]
    fn write_bytes(&mut self, bytes: &[u8]) -> Result<(), LispRPCSerializerError> {
        self.output.extend_from_slice(bytes);
        self.pos += bytes.len();
        Ok(())
    }

    #[inline]
    fn write_u64(&mut self, mut n: u64) -> Result<(), LispRPCSerializerError> {
        if n < 10 {
            let b = [b'0' + n as u8];
            return self.write_bytes(&b);
        }
        let mut tmp = [0u8; 20];
        let mut i = 20;
        while n > 0 {
            i -= 1;
            tmp[i] = b'0' + (n % 10) as u8;
            n /= 10;
        }
        self.write_bytes(&tmp[i..])
    }

    #[inline]
    fn write_i64(&mut self, n: i64) -> Result<(), LispRPCSerializerError> {
        if n < 0 {
            self.write_bytes(b"-")?;
            self.write_u64(n.wrapping_neg() as u64)
        } else {
            self.write_u64(n as u64)
        }
    }

    /// Writes a field key in kebab-case without heap allocation when `key` is standard ASCII snake_case.
    #[inline]
    fn write_kebab_field(&mut self, key: &str) -> Result<(), LispRPCSerializerError> {
        let bytes = key.as_bytes();
        if !bytes.is_empty() {
            let mut has_underscore = false;
            let mut simple_snake = true;
            let mut prev_underscore = false;
            for (i, &b) in bytes.iter().enumerate() {
                if b == b'_' {
                    if i == 0 || i + 1 == bytes.len() || prev_underscore {
                        simple_snake = false;
                        break;
                    }
                    has_underscore = true;
                    prev_underscore = true;
                } else if b.is_ascii_lowercase() {
                    prev_underscore = false;
                } else {
                    simple_snake = false;
                    break;
                }
            }
            if simple_snake {
                if !has_underscore {
                    return self.write_bytes(bytes);
                }
                self.output.reserve(bytes.len());
                for &b in bytes {
                    self.output.push(if b == b'_' { b'-' } else { b });
                }
                self.pos += bytes.len();
                return Ok(());
            }
        }
        self.write_bytes(key.to_case(Case::Kebab).as_bytes())
    }

    /// Writes a struct name in kebab-case without heap allocation when `name` is standard ASCII PascalCase.
    #[inline]
    fn write_kebab_struct_name(&mut self, name: &str) -> Result<(), LispRPCSerializerError> {
        let bytes = name.as_bytes();
        if !bytes.is_empty() {
            let mut simple_pascal = true;
            let mut prev_upper = false;
            let mut upper_count = 0usize;
            for (i, &b) in bytes.iter().enumerate() {
                if b.is_ascii_uppercase() {
                    if prev_upper {
                        simple_pascal = false;
                        break;
                    }
                    if i > 0 {
                        upper_count += 1;
                    }
                    prev_upper = true;
                } else if b.is_ascii_lowercase() {
                    prev_upper = false;
                } else {
                    simple_pascal = false;
                    break;
                }
            }
            if simple_pascal {
                let added_len = bytes.len() + upper_count;
                self.output.reserve(added_len);
                for (i, &b) in bytes.iter().enumerate() {
                    if b.is_ascii_uppercase() {
                        if i > 0 {
                            self.output.push(b'-');
                        }
                        self.output.push(b.to_ascii_lowercase());
                    } else {
                        self.output.push(b);
                    }
                }
                self.pos += added_len;
                return Ok(());
            }
        }
        self.write_bytes(name.to_case(Case::Kebab).as_bytes())
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
        self.write_kebab_field(key)?;
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
        self.write_i64(v)
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
        self.write_u64(v)
    }

    fn serialize_f32(self, v: f32) -> Result<Self::Ok, Self::Error> {
        self.serialize_f64(f64::from(v))
    }

    fn serialize_f64(self, v: f64) -> Result<Self::Ok, Self::Error> {
        self.write_bytes(v.to_string().as_bytes())
    }

    fn serialize_char(self, v: char) -> Result<Self::Ok, Self::Error> {
        let mut tmp = [0u8; 4];
        self.serialize_str(v.encode_utf8(&mut tmp))
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
        self.write_bytes(b"nil")
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
        let is_map = (!self.map_types.is_empty() && self.map_types.contains(name))
            || (HAS_GLOBAL_MAP_TYPES.load(Ordering::Acquire)
                && GLOBAL_MAP_TYPES
                    .read()
                    .map(|map| map.contains(name))
                    .unwrap_or(false));

        let style = if is_map {
            LispRPCStyle::Map
        } else {
            self.style
        };

        match style {
            LispRPCStyle::Struct => {
                self.write_bytes(b"(")?;
                self.write_kebab_struct_name(name)?;
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
