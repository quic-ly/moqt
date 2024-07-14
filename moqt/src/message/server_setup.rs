use crate::message::{Role, Version};
use crate::serde::parameters::ParameterKey;
use crate::{Deserializer, Error, Parameters, Result, Serializer};
use bytes::{Buf, BufMut};

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct ServerSetup {
    pub supported_version: Version,
    pub role: Role,
}

impl Deserializer for ServerSetup {
    fn deserialize<R: Buf>(r: &mut R) -> Result<(Self, usize)> {
        let (supported_version, svl) = Version::deserialize(r)?;

        let (mut parameters, pl) = Parameters::deserialize(r)?;
        let role: Role = parameters
            .remove(ParameterKey::Role)
            .map_err(|err| Error::ErrParseError(err.to_string()))?
            .ok_or(Error::ErrMissingParameter)?;

        Ok((
            Self {
                supported_version,
                role,
            },
            svl + pl,
        ))
    }
}

impl Serializer for ServerSetup {
    fn serialize<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        let mut l = self.supported_version.serialize(w)?;

        let mut parameters = Parameters::new();
        parameters.insert(ParameterKey::Role, self.role)?;
        l += parameters.serialize(w)?;
        Ok(l)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::message::ControlMessage;
    use std::io::Cursor;

    #[test]
    fn test_server_setup() -> Result<()> {
        let expected_packet: Vec<u8> = vec![
            0x40, 0x41, // type
            192, 0, 0, 0, 255, 0, 0, 1,    // version Draft01
            0x01, // one param
            0x00, 0x01, 0x03, // role = PubSub
        ];

        let expected_message = ControlMessage::ServerSetup(ServerSetup {
            supported_version: Version::Draft01,
            role: Role::PubSub,
        });

        let mut cursor: Cursor<&[u8]> = Cursor::new(expected_packet.as_ref());
        let (actual_message, actual_len) = ControlMessage::deserialize(&mut cursor)?;
        assert_eq!(expected_message, actual_message);
        assert_eq!(expected_packet.len(), actual_len);

        let mut actual_packet = vec![];
        let _ = expected_message.serialize(&mut actual_packet)?;
        assert_eq!(expected_packet, actual_packet);

        Ok(())
    }
}
