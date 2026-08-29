//! Deserializer implementation parsing Lisp-RPC S-expressions into Rust types.

use crate::*;

use convert_case::{Case, Casing};
use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};

/// Serde [`de::Deserializer`] for parsing Lisp-RPC S-expressions.
pub struct LispRPCDeserializer<'de> {
    /// The remaining input string slice being parsed.
    pub input: &'de str,
}

impl<'de> LispRPCDeserializer<'de> {
    /// Creates a new deserializer from a string slice.
    pub fn from_str(input: &'de str) -> Self {
        LispRPCDeserializer { input }
    }

    fn eat_whitespace(&mut self) {
        self.input = self.input.trim_start();
    }

    fn peek_char(&self) -> Option<char> {
        self.input.chars().next()
    }

    fn normalize_variant(&self, ident: &str) -> String {
        if !ident.contains('-') && ident.chars().nth(0).unwrap_or('a').is_uppercase() {
            ident.to_string()
        } else {
            ident.to_case(Case::Pascal)
        }
    }
}

impl<'de, 'a> de::Deserializer<'de> for &'a mut LispRPCDeserializer<'de> {
    type Error = LispRPCSerializerError;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.eat_whitespace();
        match self.peek_char() {
            Some('\"') => self.deserialize_str(visitor),
            Some('\'') => {
                if self.input.starts_with("'(:") {
                    self.deserialize_map(visitor)
                } else if self.input.starts_with("'(") {
                    self.deserialize_seq(visitor)
                } else {
                    self.input = &self.input[1..];
                    let end = self
                        .input
                        .find(|c: char| c.is_whitespace() || c == ')')
                        .unwrap_or(self.input.len());
                    let ident = &self.input[..end];
                    self.input = &self.input[end..];
                    visitor.visit_string(self.normalize_variant(ident))
                }
            }
            Some('(') => self.deserialize_struct("", &[], visitor),
            Some('t') => self.deserialize_bool(visitor),
            Some('n') => self.deserialize_bool(visitor),
            Some('0'..='9') | Some('-') => self.deserialize_i64(visitor),
            _ => Err(LispRPCSerializerError::Msg(format!(
                "unexpected input: {}",
                self.input
            ))),
        }
    }

    fn deserialize_bool<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.eat_whitespace();
        if self.input.starts_with('t') {
            self.input = &self.input[1..];
            visitor.visit_bool(true)
        } else if self.input.starts_with("nil") {
            self.input = &self.input[3..];
            visitor.visit_bool(false)
        } else {
            Err(LispRPCSerializerError::Msg(format!(
                "expected bool, found: {}",
                self.input
            )))
        }
    }

    fn deserialize_i8<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_i64(visitor)
    }

    fn deserialize_i16<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_i64(visitor)
    }

    fn deserialize_i32<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_i64(visitor)
    }

    fn deserialize_i64<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.eat_whitespace();
        let end = self
            .input
            .find(|c: char| c.is_whitespace() || c == ')')
            .unwrap_or(self.input.len());
        let s = &self.input[..end];
        self.input = &self.input[end..];
        visitor.visit_i64(
            s.parse()
                .map_err(|_| LispRPCSerializerError::Msg(format!("failed to parse i64: {}", s)))?,
        )
    }

    fn deserialize_u8<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_u64(visitor)
    }

    fn deserialize_u16<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_u64(visitor)
    }

    fn deserialize_u32<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_u64(visitor)
    }

    fn deserialize_u64<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.eat_whitespace();
        let end = self
            .input
            .find(|c: char| c.is_whitespace() || c == ')')
            .unwrap_or(self.input.len());
        let s = &self.input[..end];
        self.input = &self.input[end..];
        visitor.visit_u64(
            s.parse()
                .map_err(|_| LispRPCSerializerError::Msg(format!("failed to parse u64: {}", s)))?,
        )
    }

    fn deserialize_f32<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_f64(visitor)
    }

    fn deserialize_f64<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.eat_whitespace();
        let end = self
            .input
            .find(|c: char| c.is_whitespace() || c == ')')
            .unwrap_or(self.input.len());
        let s = &self.input[..end];
        self.input = &self.input[end..];
        visitor.visit_f64(
            s.parse()
                .map_err(|_| LispRPCSerializerError::Msg(format!("failed to parse f64: {}", s)))?,
        )
    }

    fn deserialize_char<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_str(visitor)
    }

    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.eat_whitespace();
        if !self.input.starts_with('\"') {
            return Err(LispRPCSerializerError::Msg(format!(
                "expected \" at start of string, found: {}",
                self.input
            )));
        }
        self.input = &self.input[1..];
        match self.input.find('\"') {
            Some(len) => {
                let s = &self.input[..len];
                self.input = &self.input[len + 1..];
                visitor.visit_borrowed_str(s)
            }
            None => Err(LispRPCSerializerError::Msg(format!(
                "unterminated string: {}",
                self.input
            ))),
        }
    }

    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_str(visitor)
    }

    fn deserialize_bytes<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        Err(LispRPCSerializerError::NotSupport)
    }

    fn deserialize_byte_buf<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        Err(LispRPCSerializerError::NotSupport)
    }

    fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.eat_whitespace();
        if self.input.starts_with("nil") {
            self.input = &self.input[3..];
            visitor.visit_unit()
        } else {
            Err(LispRPCSerializerError::Msg(format!(
                "expected nil, found: {}",
                self.input
            )))
        }
    }

    fn deserialize_unit_struct<V>(
        self,
        _name: &'static str,
        _visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        Err(LispRPCSerializerError::NotSupport)
    }

    fn deserialize_newtype_struct<V>(
        self,
        _name: &'static str,
        _visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        Err(LispRPCSerializerError::NotSupport)
    }

    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.eat_whitespace();
        if !self.input.starts_with("'(") {
            return Err(LispRPCSerializerError::Msg(format!(
                "expected '(', found: {}",
                self.input
            )));
        }
        self.input = &self.input[2..];
        let value = visitor.visit_seq(LispRPCSeqAccess { de: self })?;
        self.eat_whitespace();
        if !self.input.starts_with(')') {
            return Err(LispRPCSerializerError::Msg(format!(
                "expected ')', found: {}",
                self.input
            )));
        }
        self.input = &self.input[1..];
        Ok(value)
    }

    fn deserialize_tuple<V>(self, _len: usize, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_seq(visitor)
    }

    fn deserialize_tuple_struct<V>(
        self,
        _name: &'static str,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_seq(visitor)
    }

    fn deserialize_map<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.eat_whitespace();
        if !self.input.starts_with("'(") {
            return Err(LispRPCSerializerError::Msg(format!(
                "expected '(', found: {}",
                self.input
            )));
        }
        self.input = &self.input[2..];

        let value = visitor.visit_map(LispRPCMapAccess { de: self })?;

        self.eat_whitespace();
        if !self.input.starts_with(')') {
            return Err(LispRPCSerializerError::Msg(format!(
                "expected ')', found: {}",
                self.input
            )));
        }
        self.input = &self.input[1..];
        Ok(value)
    }

    fn deserialize_struct<V>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.eat_whitespace();
        if self.input.starts_with("'(") {
            return self.deserialize_map(visitor);
        }

        if !self.input.starts_with('(') {
            return Err(LispRPCSerializerError::Msg(format!(
                "expected (, found {}",
                self.input
            )));
        }
        self.input = &self.input[1..];

        // Skip name
        let end = self
            .input
            .find(|c: char| c.is_whitespace() || c == ')')
            .ok_or_else(|| LispRPCSerializerError::Msg("unexpected end".to_string()))?;
        self.input = &self.input[end..];

        let value = visitor.visit_map(LispRPCMapAccess { de: self })?;

        self.eat_whitespace();
        if !self.input.starts_with(')') {
            return Err(LispRPCSerializerError::Msg(format!(
                "expected ')', found: {}",
                self.input
            )));
        }
        self.input = &self.input[1..];
        Ok(value)
    }

    fn deserialize_enum<V>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.eat_whitespace();
        if self.input.starts_with('\'') {
            self.input = &self.input[1..];
            visitor.visit_enum(LispRPCVariantAccess { de: self })
        } else {
            Err(LispRPCSerializerError::Msg(format!(
                "expected '\'' for enum, found: {}",
                self.input
            )))
        }
    }

    fn deserialize_identifier<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.eat_whitespace();
        let end = self
            .input
            .find(|c: char| c.is_whitespace() || c == ')')
            .unwrap_or(self.input.len());
        let ident = &self.input[..end];
        self.input = &self.input[end..];

        visitor.visit_string(ident.to_case(Case::Snake))
    }

    fn deserialize_ignored_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_any(visitor)
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.eat_whitespace();
        if self.input.starts_with("nil") {
            self.input = &self.input[3..];
            visitor.visit_none()
        } else {
            visitor.visit_some(self)
        }
    }
}

struct LispRPCSeqAccess<'a, 'de: 'a> {
    de: &'a mut LispRPCDeserializer<'de>,
}

impl<'de, 'a> SeqAccess<'de> for LispRPCSeqAccess<'a, 'de> {
    type Error = LispRPCSerializerError;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
    where
        T: DeserializeSeed<'de>,
    {
        self.de.eat_whitespace();
        if self.de.input.starts_with(')') {
            return Ok(None);
        }
        seed.deserialize(&mut *self.de).map(Some)
    }
}

struct LispRPCMapAccess<'a, 'de: 'a> {
    de: &'a mut LispRPCDeserializer<'de>,
}

impl<'de, 'a> MapAccess<'de> for LispRPCMapAccess<'a, 'de> {
    type Error = LispRPCSerializerError;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Self::Error>
    where
        K: DeserializeSeed<'de>,
    {
        self.de.eat_whitespace();
        if self.de.input.starts_with(')') {
            return Ok(None);
        }
        if !self.de.input.starts_with(':') {
            return Err(LispRPCSerializerError::Msg(format!(
                "expected :, found {}",
                self.de.input
            )));
        }
        self.de.input = &self.de.input[1..];
        seed.deserialize(&mut *self.de).map(Some)
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, Self::Error>
    where
        V: DeserializeSeed<'de>,
    {
        seed.deserialize(&mut *self.de)
    }
}

struct LispRPCVariantAccess<'a, 'de: 'a> {
    de: &'a mut LispRPCDeserializer<'de>,
}

impl<'de, 'a> de::EnumAccess<'de> for LispRPCVariantAccess<'a, 'de> {
    type Error = LispRPCSerializerError;
    type Variant = Self;

    fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self::Variant), Self::Error>
    where
        V: de::DeserializeSeed<'de>,
    {
        let val = seed.deserialize(LispRPCVariantDeserializer { de: self.de })?;
        Ok((val, self))
    }
}

impl<'de, 'a> de::VariantAccess<'de> for LispRPCVariantAccess<'a, 'de> {
    type Error = LispRPCSerializerError;

    fn unit_variant(self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value, Self::Error>
    where
        T: de::DeserializeSeed<'de>,
    {
        seed.deserialize(self.de)
    }

    fn tuple_variant<V>(self, _len: usize, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        Err(LispRPCSerializerError::NotSupport)
    }

    fn struct_variant<V>(
        self,
        _fields: &'static [&'static str],
        _visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        Err(LispRPCSerializerError::NotSupport)
    }
}

struct LispRPCVariantDeserializer<'a, 'de: 'a> {
    de: &'a mut LispRPCDeserializer<'de>,
}

impl<'de, 'a> de::Deserializer<'de> for LispRPCVariantDeserializer<'a, 'de> {
    type Error = LispRPCSerializerError;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.de.eat_whitespace();
        let end = self
            .de
            .input
            .find(|c: char| c.is_whitespace() || c == ')')
            .unwrap_or(self.de.input.len());
        let ident = &self.de.input[..end];
        self.de.input = &self.de.input[end..];

        visitor.visit_string(self.de.normalize_variant(ident))
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf unit unit_struct newtype_struct seq tuple
        tuple_struct map struct enum identifier ignored_any option
    }
}
