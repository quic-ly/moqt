use crate::message::FullSequence;
use crate::{Decodable, Encodable, Result};
use bytes::{Buf, BufMut};

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct SubscribeOk {
    pub subscribe_id: u64,

    pub expires: u64,

    pub largest_group_object: Option<FullSequence>,
}

impl Decodable for SubscribeOk {
    fn decode<R: Buf>(r: &mut R) -> Result<Self> {
        let subscribe_id = u64::decode(r)?;

        let expires = u64::decode(r)?;

        let largest_group_object = if bool::decode(r)? {
            Some(FullSequence::decode(r)?)
        } else {
            None
        };

        Ok(Self {
            subscribe_id,

            expires,

            largest_group_object,
        })
    }
}

impl Encodable for SubscribeOk {
    fn encode<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        let mut l = self.subscribe_id.encode(w)?;

        l += self.expires.encode(w)?;

        l += if let Some(largest_group_object) = self.largest_group_object.as_ref() {
            true.encode(w)? + largest_group_object.encode(w)?
        } else {
            false.encode(w)?
        };

        Ok(l)
    }
}
