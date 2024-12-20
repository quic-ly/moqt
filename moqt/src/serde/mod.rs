use crate::{Error, Result};
use bytes::{Buf, BufMut, Bytes};

pub mod data_writer;
pub mod parameters;
pub mod varint;
pub mod wire_serialization;

pub trait Deserializer {
    fn deserialize<B>(r: &mut B) -> Result<(Self, usize)>
    where
        Self: Sized,
        B: Buf;
}

pub trait Serializer {
    fn serialize<B: BufMut>(&self, w: &mut B) -> Result<usize>;
}

impl Serializer for bool {
    /// Encode a varint to the given writer.
    fn serialize<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        if !w.has_remaining_mut() {
            return Err(Error::ErrBufferTooShort);
        }
        w.put_u8(*self as u8);
        Ok(1)
    }
}

impl Deserializer for bool {
    fn deserialize<R: Buf>(r: &mut R) -> Result<(Self, usize)> {
        if !r.has_remaining() {
            return Err(Error::ErrBufferTooShort);
        }
        let b = r.get_u8();
        match b {
            0 => Ok((false, 1)),
            1 => Ok((true, 1)),
            _ => Err(Error::ErrInvalidBooleanValue(b)),
        }
    }
}

impl Serializer for Bytes {
    /// Encode a varint to the given writer.
    fn serialize<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        if !w.has_remaining_mut() {
            return Err(Error::ErrBufferTooShort);
        }
        w.put(self.slice(..));
        Ok(self.len())
    }
}

impl Deserializer for Bytes {
    fn deserialize<R: Buf>(r: &mut R) -> Result<(Self, usize)> {
        let l = r.remaining();
        Ok((r.copy_to_bytes(l), l))
    }
}

impl Deserializer for String {
    fn deserialize<B: Buf>(r: &mut B) -> Result<(Self, usize)> {
        let (size, l) = usize::deserialize(r)?;
        if r.remaining() < size {
            return Err(Error::ErrBufferTooShort);
        }

        let mut buf = vec![0; size];
        r.copy_to_slice(&mut buf);
        let str = String::from_utf8(buf)?;

        Ok((str, size + l))
    }
}

impl Serializer for String {
    fn serialize<B: BufMut>(&self, w: &mut B) -> Result<usize> {
        let l = self.len().serialize(w)?;
        if w.remaining_mut() < self.len() {
            return Err(Error::ErrBufferTooShort);
        }
        w.put(self.as_ref());
        Ok(l + self.len())
    }
}
