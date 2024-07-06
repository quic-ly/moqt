use crate::{Deserializer, Serializer, Result};
use bytes::{Buf, BufMut};

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct UnAnnounce {
    pub track_namespace: String,
}

impl Deserializer for UnAnnounce {
    fn deserialize<R: Buf>(r: &mut R) -> Result<Self> {
        let track_namespace = String::deserialize(r)?;
        Ok(Self { track_namespace })
    }
}

impl Serializer for UnAnnounce {
    fn serialize<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        self.track_namespace.serialize(w)
    }
}
