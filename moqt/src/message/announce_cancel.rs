use crate::{Deserializer, Result, Serializer};
use bytes::{Buf, BufMut};

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct AnnounceCancel {
    pub track_namespace: String,
}

impl Deserializer for AnnounceCancel {
    fn deserialize<R: Buf>(r: &mut R) -> Result<(Self, usize)> {
        let (track_namespace, tnsl) = String::deserialize(r)?;
        Ok((Self { track_namespace }, tnsl))
    }
}

impl Serializer for AnnounceCancel {
    fn serialize<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        self.track_namespace.serialize(w)
    }
}
