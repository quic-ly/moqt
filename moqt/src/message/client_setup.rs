use crate::message::message_parser::ErrorCode;
use crate::message::{Role, Version};
use crate::serde::parameters::ParameterKey;
use crate::{Deserializer, Error, Parameters, Result, Serializer};
use bytes::{Buf, BufMut};

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct ClientSetup {
    pub supported_versions: Vec<Version>,
    pub role: Option<Role>,
    pub path: Option<String>,
    pub(crate) uses_web_transport: bool,
}

impl ClientSetup {
    pub fn new(uses_web_transport: bool) -> Self {
        Self {
            uses_web_transport,
            ..Default::default()
        }
    }
}

impl Deserializer for ClientSetup {
    fn deserialize<R: Buf>(r: &mut R) -> Result<(Self, usize)> {
        let (number_supported_versions, mut tl) = usize::deserialize(r)?;
        let mut supported_versions = BytesMut::with_capacity(number_supported_versions);
        for _ in 0..number_supported_versions {
            let (version, vl) = Version::deserialize(r)?;
            supported_versions.push(version);
            tl += vl;
        }

        let (num_params, npl) = u64::deserialize(r)?;
        tl += npl;

        let mut role: Option<Role> = None;
        let mut path: Option<String> = None;

        // Parse parameters
        for _ in 0..num_params {
            let (key, kl) = u64::deserialize(r)?;
            tl += kl;
            let (size, sl) = usize::deserialize(r)?;
            tl += sl;

            if r.remaining() < size {
                return Err(Error::ErrBufferTooShort);
            }

            if key == ParameterKey::Role as u64 {
                if role.is_some() {
                    return Err(Error::ErrParseError(
                        ErrorCode::ProtocolViolation,
                        "ROLE parameter appears twice in SETUP".to_string(),
                    ));
                }
                let (r, rl) = u64::deserialize(r)?;
                tl += rl;

                if rl != size {
                    return Err(Error::ErrParseError(
                        ErrorCode::ParameterLengthMismatch,
                        "Parameter length does not match varint encoding".to_string(),
                    ));
                }

                role = Some(r.try_into().map_err(|_| {
                    Error::ErrParseError(
                        ErrorCode::ProtocolViolation,
                        "Invalid ROLE parameter".to_string(),
                    )
                })?);
            } else if key == ParameterKey::Path as u64 {
                if path.is_some() {
                    return Err(Error::ErrParseError(
                        ErrorCode::ProtocolViolation,
                        "PATH parameter appears twice in CLIENT_SETUP".to_string(),
                    ));
                }
                let mut buf = vec![0; size];
                r.copy_to_slice(&mut buf);
                tl += size;

                path = Some(String::from_utf8(buf)?);
            }
        }

        if role.is_none() {
            return Err(Error::ErrParseError(
                ErrorCode::ProtocolViolation,
                "ROLE parameter missing from CLIENT_SETUP message".to_string(),
            ));
        }

        Ok((
            Self {
                supported_versions,
                role,
                path,
                uses_web_transport: false,
            },
            tl,
        ))
    }
}

impl Serializer for ClientSetup {
    fn serialize<W: BufMut>(&self, w: &mut W) -> Result<usize> {
        let mut l = self.supported_versions.len().serialize(w)?;
        for supported_version in self.supported_versions.iter() {
            l += supported_version.serialize(w)?;
        }

        let mut parameters = Parameters::new();
        if let Some(role) = self.role.as_ref() {
            parameters.insert(ParameterKey::Role, *role)?;
        }
        if !self.uses_web_transport {
            if let Some(path) = self.path.as_ref() {
                parameters.insert(ParameterKey::Path, path.to_string())?;
            }
        }
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
    fn test_client_setup() -> Result<()> {
        let tests: Vec<(Vec<u8>, ControlMessage)> = vec![
            (
                vec![
                    0x40, 0x40, // type
                    0x02, // versions
                    192, 0, 0, 0, 255, 0, 0, 1, // Draft01
                    192, 0, 0, 0, 255, 0, 0, 2,    // Draft02
                    0x02, // 2 parameters
                    0x00, 0x01, 0x03, // role = PubSub
                    0x01, 0x03, 0x66, 0x6f, 0x6f, // path = "foo"
                ],
                ControlMessage::ClientSetup(ClientSetup {
                    supported_versions: vec![Version::Draft01, Version::Draft02],
                    role: Some(Role::PubSub),
                    path: Some("foo".to_string()),
                    ..Default::default()
                }),
            ),
            (
                vec![
                    0x40, 0x40, 0x01, 0xc0, 0x00, 0x00, 0x00, 0xff, 0x00, 0x00, 0x00,
                    0x02, // 2 parameters
                    0x00, 0x01, 0x03, // role = PubSub
                    0x01, 0x01, 0x65, // path = "e"
                ],
                ControlMessage::ClientSetup(ClientSetup {
                    supported_versions: vec![Version::Draft00],
                    role: Some(Role::PubSub),
                    path: Some("e".to_string()),
                    ..Default::default()
                }),
            ),
        ];

        for (expected_packet, expected_message) in tests {
            let mut cursor: Cursor<&[u8]> = Cursor::new(expected_packet.as_ref());
            let (actual_message, actual_len) = ControlMessage::deserialize(&mut cursor)?;
            assert_eq!(expected_message, actual_message);
            assert_eq!(expected_packet.len(), actual_len);

            let mut actual_packet = vec![];
            let _ = expected_message.serialize(&mut actual_packet)?;
            assert_eq!(expected_packet, actual_packet);
        }

        Ok(())
    }
}
