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

    #[inline(always)]
    fn eat_whitespace(&mut self) {
        let bytes = self.input.as_bytes();
        let mut i = 0;
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i > 0 {
            // SAFETY: i is on an ASCII whitespace boundary
            self.input = unsafe { self.input.get_unchecked(i..) };
        }
        if !self.input.is_empty() && !self.input.as_bytes()[0].is_ascii() {
            self.input = self.input.trim_start();
        }
    }

    #[inline(always)]
    fn next_token(&mut self) -> &'de str {
        let bytes = self.input.as_bytes();
        let len = bytes.len();
        let mut i = 0;
        while i < len {
            let b = bytes[i];
            if b == b')' || b.is_ascii_whitespace() {
                break;
            }
            if !b.is_ascii() {
                let end = i
                    + self.input[i..]
                        .find(|c: char| c.is_whitespace() || c == ')')
                        .unwrap_or(len - i);
                let s = &self.input[..end];
                self.input = &self.input[end..];
                return s;
            }
            i += 1;
        }
        // SAFETY: i is on an ASCII boundary within valid UTF-8 self.input
        let s = unsafe { self.input.get_unchecked(..i) };
        self.input = unsafe { self.input.get_unchecked(i..) };
        s
    }

    fn peek_char(&self) -> Option<char> {
        let b = *self.input.as_bytes().first()?;
        if b.is_ascii() {
            Some(b as char)
        } else {
            self.input.chars().next()
        }
    }

    fn normalize_variant(&self, ident: &str) -> String {
        if !ident.contains('-') && ident.chars().nth(0).unwrap_or('a').is_uppercase() {
            ident.to_string()
        } else {
            ident.to_case(Case::Pascal)
        }
    }

    #[inline]
    fn visit_normalized_variant<V>(
        &self,
        ident: &'de str,
        visitor: V,
    ) -> Result<V::Value, LispRPCSerializerError>
    where
        V: Visitor<'de>,
    {
        let bytes = ident.as_bytes();
        if !bytes.is_empty() {
            if bytes[0].is_ascii_uppercase() && !bytes.contains(&b'-') {
                return visitor.visit_borrowed_str(ident);
            }
            if bytes.len() <= 64 {
                let mut buf = [0u8; 64];
                let mut out_len = 0;
                let mut cap_next = true;
                let mut simple = true;
                for (i, &b) in bytes.iter().enumerate() {
                    if b == b'-' {
                        if i == 0 || i + 1 == bytes.len() || cap_next {
                            simple = false;
                            break;
                        }
                        cap_next = true;
                    } else if b.is_ascii_lowercase() {
                        buf[out_len] = if cap_next { b.to_ascii_uppercase() } else { b };
                        out_len += 1;
                        cap_next = false;
                    } else {
                        simple = false;
                        break;
                    }
                }
                if simple {
                    // SAFETY: buf[..out_len] only contains ASCII alphabetic bytes
                    let s = unsafe { std::str::from_utf8_unchecked(&buf[..out_len]) };
                    return visitor.visit_str(s);
                }
            }
        }
        visitor.visit_string(self.normalize_variant(ident))
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
                    let ident = self.next_token();
                    self.visit_normalized_variant(ident, visitor)
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
        let s = self.next_token();
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
        let s = self.next_token();
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
        let s = self.next_token();
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
            .as_bytes()
            .iter()
            .position(|&b| b == b')' || b.is_ascii_whitespace())
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
        let ident = self.next_token();
        let bytes = ident.as_bytes();
        if !bytes.is_empty() && bytes.len() <= 64 {
            let mut has_hyphen = false;
            let mut simple = true;
            let mut prev_hyphen = false;
            for (i, &b) in bytes.iter().enumerate() {
                if b == b'-' {
                    if i == 0 || i + 1 == bytes.len() || prev_hyphen {
                        simple = false;
                        break;
                    }
                    has_hyphen = true;
                    prev_hyphen = true;
                } else if b.is_ascii_lowercase() {
                    prev_hyphen = false;
                } else {
                    simple = false;
                    break;
                }
            }
            if simple {
                if !has_hyphen {
                    return visitor.visit_borrowed_str(ident);
                }
                let mut buf = [0u8; 64];
                for (i, &b) in bytes.iter().enumerate() {
                    buf[i] = if b == b'-' { b'_' } else { b };
                }
                // SAFETY: buf[..bytes.len()] only contains ASCII lowercase and '_'
                let s = unsafe { std::str::from_utf8_unchecked(&buf[..bytes.len()]) };
                return visitor.visit_str(s);
            }
        }

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
        let ident = self.de.next_token();
        self.de.visit_normalized_variant(ident, visitor)
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf unit unit_struct newtype_struct seq tuple
        tuple_struct map struct enum identifier ignored_any option
    }
}
