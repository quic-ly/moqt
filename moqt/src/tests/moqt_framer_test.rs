use crate::message::message_framer::MessageFramer;
use crate::message::message_test::{
    create_test_message, MessageStructuredData, TestMessageBase, TestObjectDatagramMessage,
    TestStreamHeaderGroupMessage, TestStreamHeaderTrackMessage, TestStreamMiddlerGroupMessage,
    TestStreamMiddlerTrackMessage,
};
use crate::message::object::{ObjectForwardingPreference, ObjectHeader, ObjectStatus};
use crate::message::subscribe::Subscribe;
use crate::message::subscribe_update::SubscribeUpdate;
use crate::message::{ControlMessage, FilterType, FullSequence, MessageType};
use crate::{Error, Result};
use bytes::{BufMut, Bytes};
use rstest::rstest;

struct TestFramerParams {
    message_type: MessageType,
    uses_web_transport: bool,
}

impl TestFramerParams {
    fn new(message_type: MessageType, uses_web_transport: bool) -> Self {
        Self {
            message_type,
            uses_web_transport,
        }
    }
}

struct TestFramer {
    message_type: MessageType,
    uses_web_transport: bool,
}

impl TestFramer {
    fn new(params: &TestFramerParams) -> Self {
        Self {
            message_type: params.message_type,
            uses_web_transport: params.uses_web_transport,
        }
    }

    fn make_message(&self) -> Box<dyn TestMessageBase> {
        create_test_message(self.message_type, self.uses_web_transport)
    }

    fn serialize_message<W: BufMut>(
        &self,
        structured_data: MessageStructuredData,
        w: &mut W,
    ) -> Result<usize> {
        match self.message_type {
            MessageType::ObjectStream
            | MessageType::StreamHeaderTrack
            | MessageType::StreamHeaderGroup => {
                let object_header =
                    if let MessageStructuredData::Object(object_header) = structured_data {
                        object_header
                    } else {
                        return Err(Error::ErrInvalidMessageType(self.message_type as u64));
                    };
                MessageFramer::serialize_object(object_header, true, Bytes::from_static(b"foo"), w)
            }
            MessageType::ObjectDatagram => {
                Err(Error::ErrInvalidMessageType(self.message_type as u64))
            }
            _ => {
                let control_message =
                    if let MessageStructuredData::Control(control_message) = structured_data {
                        control_message
                    } else {
                        return Err(Error::ErrInvalidMessageType(self.message_type as u64));
                    };
                MessageFramer::serialize_control_message(control_message, w)
            }
        }
    }
}

#[rstest(
    params => [
    (MessageType::ObjectStream, true), // ObjectDatagram is a unique set of tests.
    (MessageType::StreamHeaderTrack, true),
    (MessageType::StreamHeaderGroup, true),
    (MessageType::Subscribe, true),
    (MessageType::SubscribeOk, true),
    (MessageType::SubscribeError, true),
    (MessageType::UnSubscribe, true),
    (MessageType::SubscribeDone, true),
    (MessageType::SubscribeUpdate, true),
    (MessageType::Announce, true),
    (MessageType::AnnounceOk, true),
    (MessageType::AnnounceError, true),
    (MessageType::AnnounceCancel, true),
    (MessageType::UnAnnounce, true),
    (MessageType::TrackStatusRequest, true),
    (MessageType::TrackStatus, true),
    (MessageType::ClientSetup, true),
    (MessageType::ClientSetup, false),
    (MessageType::ServerSetup, true),
    (MessageType::GoAway, true),
    ]
)]
fn test_framer_one_message(params: (MessageType, bool)) -> Result<()> {
    let tester = TestFramer::new(&TestFramerParams::new(params.0, params.1));
    let message = tester.make_message();
    let structured_data = message.structured_data();
    let mut buffer = vec![];
    let size = tester.serialize_message(structured_data, &mut buffer)?;
    assert_eq!(size, buffer.len());
    assert_eq!(buffer.len(), message.packet_sample().len());
    assert_eq!(&buffer[..], message.packet_sample());
    Ok(())
}

#[test]
fn test_group_middler() -> Result<()> {
    let header = TestStreamHeaderGroupMessage::new();
    let header_object_header =
        if let MessageStructuredData::Object(object_header) = header.structured_data() {
            object_header
        } else {
            assert!(false);
            return Err(Error::ErrInvalidMessageType(0));
        };

    let mut buffer1 = vec![];
    let buffer1_size = MessageFramer::serialize_object(
        header_object_header,
        true,
        Bytes::from_static(b"foo"),
        &mut buffer1,
    )?;
    assert_eq!(buffer1_size, buffer1.len());
    assert_eq!(buffer1.len(), header.total_message_size());
    assert_eq!(&buffer1[..], header.packet_sample());

    let middler = TestStreamMiddlerGroupMessage::new();
    let middler_object_header =
        if let MessageStructuredData::Object(object_header) = middler.structured_data() {
            object_header
        } else {
            assert!(false);
            return Err(Error::ErrInvalidMessageType(0));
        };
    let mut buffer2 = vec![];
    let buffer2_size = MessageFramer::serialize_object(
        middler_object_header,
        false,
        Bytes::from_static(b"bar"),
        &mut buffer2,
    )?;
    assert_eq!(buffer2_size, buffer2.len());
    assert_eq!(buffer2.len(), middler.total_message_size());
    assert_eq!(&buffer2[..], middler.packet_sample());
    Ok(())
}

#[test]
fn test_track_middler() -> Result<()> {
    let header = TestStreamHeaderTrackMessage::new();
    let header_object_header =
        if let MessageStructuredData::Object(object_header) = header.structured_data() {
            object_header
        } else {
            assert!(false);
            return Err(Error::ErrInvalidMessageType(0));
        };
    let mut buffer1 = vec![];
    let buffer1_size = MessageFramer::serialize_object(
        header_object_header,
        true,
        Bytes::from_static(b"foo"),
        &mut buffer1,
    )?;
    assert_eq!(buffer1_size, buffer1.len());
    assert_eq!(buffer1.len(), header.total_message_size());
    assert_eq!(&buffer1[..], header.packet_sample());

    let middler = TestStreamMiddlerTrackMessage::new();
    let middler_object_header =
        if let MessageStructuredData::Object(object_header) = middler.structured_data() {
            object_header
        } else {
            assert!(false);
            return Err(Error::ErrInvalidMessageType(0));
        };
    let mut buffer2 = vec![];
    let buffer2_size = MessageFramer::serialize_object(
        middler_object_header,
        false,
        Bytes::from_static(b"bar"),
        &mut buffer2,
    )?;
    assert_eq!(buffer2_size, buffer2.len());
    assert_eq!(buffer2.len(), middler.total_message_size());
    assert_eq!(&buffer2[..], middler.packet_sample());
    Ok(())
}

#[test]
fn test_bad_object_input() -> Result<()> {
    let mut object = ObjectHeader {
        subscribe_id: 3,
        track_alias: 4,
        group_id: 5,
        object_id: 6,
        object_send_order: 7,
        object_status: ObjectStatus::Normal,
        object_forwarding_preference: ObjectForwardingPreference::Object,
        object_payload_length: None,
    };
    let mut buffer = vec![];
    object.object_forwarding_preference = ObjectForwardingPreference::Datagram;
    assert!(
        MessageFramer::serialize_object_header(object, false, &mut buffer).is_err(),
        "must be first"
    );
    buffer.clear();
    object.object_forwarding_preference = ObjectForwardingPreference::Group;
    assert!(
        MessageFramer::serialize_object_header(object, false, &mut buffer).is_err(),
        "requires knowing the object length"
    );
    buffer.clear();
    object.object_payload_length = Some(5);
    object.object_status = ObjectStatus::EndOfGroup;
    assert!(
        MessageFramer::serialize_object_header(object, false, &mut buffer).is_err(),
        "Object status must be kNormal if payload is non-empty"
    );
    buffer.clear();
    Ok(())
}

#[test]
fn test_datagram() -> Result<()> {
    let datagram = TestObjectDatagramMessage::new();
    let object = ObjectHeader {
        subscribe_id: 3,
        track_alias: 4,
        group_id: 5,
        object_id: 6,
        object_send_order: 7,
        object_status: ObjectStatus::Normal,
        object_forwarding_preference: ObjectForwardingPreference::Object,
        object_payload_length: None,
    };
    let payload = Bytes::from_static(b"foo");
    let mut buffer = vec![];
    let buffer_size = MessageFramer::serialize_object_datagram(object, payload, &mut buffer)?;
    assert_eq!(buffer.len(), buffer_size);
    assert_eq!(buffer.len(), datagram.total_message_size());
    assert_eq!(&buffer[..], datagram.packet_sample());
    Ok(())
}

#[test]
fn test_all_subscribe_inputs() -> Result<()> {
    for start_group in [None, Some(4)] {
        for start_object in [None, Some(0)] {
            for end_group in [None, Some(7)] {
                for end_object in [None, Some(3)] {
                    let expected_filter_type;
                    if !start_group.is_some()
                        && !start_object.is_some()
                        && !end_group.is_some()
                        && !end_object.is_some()
                    {
                        expected_filter_type = FilterType::LatestObject;
                    } else if !start_group.is_some()
                        && start_object.is_some()
                        && *start_object.as_ref().unwrap() == 0
                        && !end_group.is_some()
                        && !end_object.is_some()
                    {
                        expected_filter_type = FilterType::LatestGroup;
                    } else if start_group.is_some()
                        && start_object.is_some()
                        && !end_group.is_some()
                        && !end_object.is_some()
                    {
                        expected_filter_type = FilterType::AbsoluteStart(FullSequence {
                            group_id: start_group.unwrap(),
                            object_id: start_object.unwrap(),
                        });
                    } else if start_group.is_some() && start_object.is_some() && end_group.is_some()
                    {
                        if let Some(&end_object) = end_object.as_ref() {
                            expected_filter_type = FilterType::AbsoluteRange(
                                FullSequence {
                                    group_id: start_group.unwrap(),
                                    object_id: start_object.unwrap(),
                                },
                                FullSequence {
                                    group_id: end_group.unwrap(),
                                    object_id: end_object,
                                },
                            );
                        } else {
                            expected_filter_type = FilterType::AbsoluteRange(
                                FullSequence {
                                    group_id: start_group.unwrap(),
                                    object_id: start_object.unwrap(),
                                },
                                FullSequence {
                                    group_id: end_group.unwrap(),
                                    object_id: u64::MAX,
                                },
                            );
                        }
                    } else {
                        continue;
                    }

                    let subscribe = Subscribe {
                        subscribe_id: 3,
                        track_alias: 4,
                        track_namespace: "foo".to_string(),
                        track_name: "abcd".to_string(),
                        filter_type: expected_filter_type,
                        authorization_info: None,
                    };
                    let mut buffer = vec![];
                    let _ = MessageFramer::serialize_control_message(
                        ControlMessage::Subscribe(subscribe),
                        &mut buffer,
                    )?;
                    // Go to the filter type.
                    let read = buffer[12];
                    assert_eq!(read, expected_filter_type.value());
                    assert!(!buffer.is_empty());
                    if let FilterType::AbsoluteRange(_, _) = expected_filter_type {
                        if let Some(&end_object) = end_object.as_ref() {
                            let object_id = buffer[12 + 4] as u64;
                            assert_eq!(object_id, end_object + 1);
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

#[test]
fn test_subscribe_end_before_start() -> Result<()> {
    let mut subscribe = Subscribe {
        subscribe_id: 3,
        track_alias: 4,
        track_namespace: "foo".to_string(),
        track_name: "abcd".to_string(),
        filter_type: FilterType::AbsoluteRange(
            FullSequence {
                group_id: 4,
                object_id: 4,
            },
            FullSequence {
                group_id: 3,
                object_id: u64::MAX,
            },
        ),
        authorization_info: Some("bar".to_string()),
    };
    let mut buffer = vec![];
    assert!(
        MessageFramer::serialize_control_message(
            ControlMessage::Subscribe(subscribe.clone()),
            &mut buffer,
        )
        .is_err(),
        "Invalid object range"
    );
    buffer.clear();
    subscribe.filter_type = FilterType::AbsoluteRange(
        FullSequence {
            group_id: 4,
            object_id: 4,
        },
        FullSequence {
            group_id: 4,
            object_id: 1,
        },
    );
    assert!(
        MessageFramer::serialize_control_message(
            ControlMessage::Subscribe(subscribe),
            &mut buffer,
        )
            .is_err(),
        "Invalid object range"
    );
    buffer.clear();
    Ok(())
}

#[test]
fn test_subscribe_latest_group_nonzero_object() -> Result<()> {
    let subscribe = Subscribe {
        subscribe_id: 3,
        track_alias: 4,
        track_namespace: "foo".to_string(),
        track_name: "abcd".to_string(),
        filter_type: FilterType::AbsoluteStart(FullSequence {
            group_id: u64::MAX,
            object_id: 3,
        }),
        authorization_info: Some("bar".to_string()),
    };
    let mut buffer = vec![];
    assert!(
        MessageFramer::serialize_control_message(
            ControlMessage::Subscribe(subscribe),
            &mut buffer,
        )
            .is_err(),
        "Invalid object range"
    );
    Ok(())
}

#[test]
fn test_subscribe_update_end_group_only() -> Result<()> {
    let subscribe_update = SubscribeUpdate {
        subscribe_id: 3,
        start_group_object: FullSequence {
            group_id: 4,
            object_id: 3,
        },
        end_group_object: Some(FullSequence {
            group_id: 4,
            object_id: u64::MAX,
        }),
        authorization_info: Some("bar".to_string()),
    };
    let mut buffer = vec![];
    let _ = MessageFramer::serialize_control_message(
        ControlMessage::SubscribeUpdate(subscribe_update),
        &mut buffer,
    )?;
    assert!(!buffer.is_empty());
    let end_group = buffer[4];
    assert_eq!(end_group, 5);
    let end_object = buffer[4 + 1];
    assert_eq!(end_object, 0);
    Ok(())
}

#[test]
fn test_subscribe_update_increments_end() -> Result<()> {
    let subscribe_update = SubscribeUpdate {
        subscribe_id: 3,
        start_group_object: FullSequence {
            group_id: 4,
            object_id: 3,
        },
        end_group_object: Some(FullSequence {
            group_id: 4,
            object_id: 6,
        }),
        authorization_info: Some("bar".to_string()),
    };
    let mut buffer = vec![];
    let _ = MessageFramer::serialize_control_message(
        ControlMessage::SubscribeUpdate(subscribe_update),
        &mut buffer,
    )?;
    assert!(!buffer.is_empty());
    let end_group = buffer[4];
    assert_eq!(end_group, 5);
    let end_object = buffer[4 + 1];
    assert_eq!(end_object, 7);
    Ok(())
}

#[test]
fn test_subscribe_update_invalid_range() -> Result<()> {
    let subscribe_update = SubscribeUpdate {
        subscribe_id: 3,
        start_group_object: FullSequence {
            group_id: 4,
            object_id: 3,
        },
        end_group_object: Some(FullSequence {
            group_id: u64::MAX,
            object_id: 6,
        }),
        authorization_info: Some("bar".to_string()),
    };
    let mut buffer = vec![];
    assert!(
        MessageFramer::serialize_control_message(
            ControlMessage::SubscribeUpdate(subscribe_update),
            &mut buffer,
        )
        .is_err(),
        "Invalid object range"
    );
    Ok(())
}
