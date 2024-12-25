use crate::moqt_priority::{
    kMoqtControlStreamSendOrder, send_order_for_stream, update_send_order_for_subscriber_priority,
    MoqtDeliveryOrder,
};

#[test]
fn test_track_priorities() {
    // MoQT track priorities are descending (0 is highest), but WebTransport send
    // order is ascending.
    assert!(
        send_order_for_stream(0x10, 0x80, 0, None, MoqtDeliveryOrder::kAscending)
            > send_order_for_stream(0x80, 0x80, 0, None, MoqtDeliveryOrder::kAscending)
    );
    assert!(
        send_order_for_stream(0x80, 0x10, 0, None, MoqtDeliveryOrder::kAscending)
            > send_order_for_stream(0x80, 0x80, 0, None, MoqtDeliveryOrder::kAscending)
    );
    // Subscriber priority takes precedence over the sender priority.
    assert!(
        send_order_for_stream(0x10, 0x80, 0, None, MoqtDeliveryOrder::kAscending)
            > send_order_for_stream(0x80, 0x10, 0, None, MoqtDeliveryOrder::kAscending)
    );
    // Test extreme priority values (0x00 and 0xff).
    assert!(
        send_order_for_stream(0x00, 0x80, 0, None, MoqtDeliveryOrder::kAscending)
            > send_order_for_stream(0xff, 0x80, 0, None, MoqtDeliveryOrder::kAscending)
    );
    assert!(
        send_order_for_stream(0x80, 0x00, 0, None, MoqtDeliveryOrder::kAscending)
            > send_order_for_stream(0x80, 0xff, 0, None, MoqtDeliveryOrder::kAscending)
    );
}

#[test]
fn test_control_stream() {
    assert!(
        kMoqtControlStreamSendOrder
            > send_order_for_stream(0x00, 0x00, 0, None, MoqtDeliveryOrder::kAscending),
    );
}

#[test]
fn test_stream_per_group() {
    assert!(
        send_order_for_stream(0x80, 0x80, 0, None, MoqtDeliveryOrder::kAscending)
            > send_order_for_stream(0x80, 0x80, 1, None, MoqtDeliveryOrder::kAscending),
    );
    assert!(
        send_order_for_stream(0x80, 0x80, 1, None, MoqtDeliveryOrder::kDescending)
            > send_order_for_stream(0x80, 0x80, 0, None, MoqtDeliveryOrder::kDescending),
    );
}

#[test]
fn test_stream_per_object() {
    // Objects within the same group.
    assert!(
        send_order_for_stream(0x80, 0x80, 0, Some(0), MoqtDeliveryOrder::kAscending)
            > send_order_for_stream(0x80, 0x80, 0, Some(1), MoqtDeliveryOrder::kAscending),
    );
    assert!(
        send_order_for_stream(0x80, 0x80, 0, Some(0), MoqtDeliveryOrder::kDescending)
            > send_order_for_stream(0x80, 0x80, 0, Some(1), MoqtDeliveryOrder::kDescending),
    );
    // Objects of different groups.
    assert!(
        send_order_for_stream(0x80, 0x80, 0, Some(1), MoqtDeliveryOrder::kAscending)
            > send_order_for_stream(0x80, 0x80, 1, Some(0), MoqtDeliveryOrder::kAscending),
    );
    assert!(
        send_order_for_stream(0x80, 0x80, 1, Some(1), MoqtDeliveryOrder::kDescending)
            > send_order_for_stream(0x80, 0x80, 0, Some(0), MoqtDeliveryOrder::kDescending),
    );
}

#[test]
fn test_update_send_order_for_subscriber_priority() {
    assert_eq!(
        update_send_order_for_subscriber_priority(
            send_order_for_stream(0x80, 0x80, 0, None, MoqtDeliveryOrder::kAscending),
            0x10
        ),
        send_order_for_stream(0x10, 0x80, 0, None, MoqtDeliveryOrder::kAscending)
    );
}
