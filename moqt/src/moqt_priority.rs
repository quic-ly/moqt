use crate::webtransport;

/// Priority that can be assigned to a track or individual streams associated
/// with the track by either the publisher or the subscriber.
pub type MoqtPriority = u8;

/// Indicates the desired order of delivering groups associated with a given
/// track.
#[allow(non_camel_case_types)]
#[derive(Default, Debug, Copy, Clone, Eq, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum MoqtDeliveryOrder {
    #[default]
    kAscending = 0x01,
    kDescending = 0x02,
}

const fn flip(num_bits: u64, number: u64) -> u64 {
    assert!(num_bits <= 63);
    (1u64 << num_bits) - 1 - number
}

const fn only_lowest_nbits(n: u64, value: u64) -> u64 {
    assert!(n <= 62);
    value & ((1u64 << (n + 1)) - 1)
}

/// Computes WebTransport send order for an MoQT data stream with the specified
/// parameters.
/// The send order is packed into a signed 64-bit integer as follows:
///   63: always zero to indicate a positive number
///   62: 0 for data streams, 1 for control streams
///   54-61: subscriber priority
///   46-53: publisher priority
///     (if stream-per-group)
///   0-45: group ID
///     (if stream-per-object)
///   20-45: group ID
///   0-19: object ID
pub fn send_order_for_stream(
    subscriber_priority: MoqtPriority,
    publisher_priority: MoqtPriority,
    mut group_id: u64,
    subgroup_id: Option<u64>,
    delivery_order: MoqtDeliveryOrder,
) -> webtransport::SendOrder {
    if let Some(subgroup_id) = subgroup_id {
        return send_order_for_stream_with_subgroup_id(
            subscriber_priority,
            publisher_priority,
            group_id,
            subgroup_id,
            delivery_order,
        );
    }

    let track_bits: i64 = ((flip(8, subscriber_priority as u64) << 54)
        | (flip(8, publisher_priority as u64) << 46)) as i64;
    group_id = only_lowest_nbits(46, group_id);
    if delivery_order == MoqtDeliveryOrder::kAscending {
        group_id = flip(46, group_id);
    }
    track_bits | group_id as i64
}

fn send_order_for_stream_with_subgroup_id(
    subscriber_priority: MoqtPriority,
    publisher_priority: MoqtPriority,
    mut group_id: u64,
    mut subgroup_id: u64,
    delivery_order: MoqtDeliveryOrder,
) -> webtransport::SendOrder {
    let track_bits: i64 = ((flip(8, subscriber_priority as u64) << 54)
        | (flip(8, publisher_priority as u64) << 46)) as i64;
    group_id = only_lowest_nbits(26, group_id);
    subgroup_id = only_lowest_nbits(20, subgroup_id);
    if delivery_order == MoqtDeliveryOrder::kAscending {
        group_id = flip(26, group_id);
    }
    subgroup_id = flip(20, subgroup_id);
    track_bits | ((group_id as i64) << 20) | subgroup_id as i64
}

/// Returns |send_order| updated with the new |subscriber_priority|.
pub fn update_send_order_for_subscriber_priority(
    send_order: webtransport::SendOrder,
    subscriber_priority: MoqtPriority,
) -> webtransport::SendOrder {
    let mut new_send_order: webtransport::SendOrder =
        only_lowest_nbits(54, send_order as u64) as i64;
    let sub_bits: i64 = (flip(8, subscriber_priority as u64) as i64) << 54;
    new_send_order |= sub_bits;
    new_send_order
}

/// WebTransport send order set on the MoQT control stream.
#[allow(non_upper_case_globals)]
pub const kMoqtControlStreamSendOrder: webtransport::SendOrder = i64::MAX;

/// WebTransport send order set on MoQT bandwidth probe streams.
#[allow(non_upper_case_globals)]
pub const kMoqtProbeStreamSendOrder: webtransport::SendOrder = i64::MIN;

#[cfg(test)]
mod test {
    use super::*;

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
}
