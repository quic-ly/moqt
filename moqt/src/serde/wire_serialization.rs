use crate::moqt_messages::MoqtVersion;
use crate::serde::data_writer::DataWriter;
use bytes::Bytes;
use log::error;
use std::marker::PhantomData;

pub trait WireType {
    fn get_length_on_wire(&self) -> usize;
    fn serialize_into_writer(&self, writer: &mut DataWriter<'_>) -> bool;
}

pub trait LengthWireType: WireType {
    fn from_length(length: usize) -> Self;
}

pub trait RefWireType<'a, T>: WireType {
    fn from_ref(value: &'a T) -> Self;
}

// ------------------- WireType() wrapper definitions -------------------
// Base struct for WireUint8/16/32/64
pub struct WireFixedSizeIntBase<T>
where
    T: Copy + Into<u64>, // Ensures T is a numeric type
{
    value: T,
    _marker: PhantomData<T>, // Marker to indicate generic type T
}

impl<T> WireFixedSizeIntBase<T>
where
    T: Copy + Into<u64>, // Ensures T is a numeric type
{
    pub fn new(value: T) -> Self {
        Self {
            value,
            _marker: PhantomData,
        }
    }

    pub fn value(&self) -> T {
        self.value
    }
}

impl<T> WireType for WireFixedSizeIntBase<T>
where
    T: Copy + Into<u64>,
{
    fn get_length_on_wire(&self) -> usize {
        std::mem::size_of::<T>()
    }

    fn serialize_into_writer(&self, writer: &mut DataWriter<'_>) -> bool {
        let value_size = size_of::<T>();
        let value_as_u64: u64 = self.value().into();
        let value_bytes = Bytes::copy_from_slice(&value_as_u64.to_be_bytes()[8 - value_size..]); // Take only the relevant bytes
        writer.write_bytes(value_bytes);
        true
    }
}

// Fixed-size integer types corresponding to (8), (16), (32), and (64) bits
pub struct WireUint8(WireFixedSizeIntBase<u8>);
pub struct WireUint16(WireFixedSizeIntBase<u16>);
pub struct WireUint32(WireFixedSizeIntBase<u32>);
pub struct WireUint64(WireFixedSizeIntBase<u64>);

macro_rules! impl_wire_fixed_size_int {
    ($type_name:ident, $inner_type:ty) => {
        impl $type_name {
            pub fn new(value: $inner_type) -> Self {
                Self(WireFixedSizeIntBase::new(value))
            }

            pub fn value(&self) -> $inner_type {
                self.0.value()
            }
        }

        impl WireType for $type_name {
            fn get_length_on_wire(&self) -> usize {
                self.0.get_length_on_wire()
            }

            fn serialize_into_writer(&self, writer: &mut DataWriter<'_>) -> bool {
                self.0.serialize_into_writer(writer)
            }
        }
    };
}

// Implement for all fixed-size types
impl_wire_fixed_size_int!(WireUint8, u8);
impl_wire_fixed_size_int!(WireUint16, u16);
impl_wire_fixed_size_int!(WireUint32, u32);
impl_wire_fixed_size_int!(WireUint64, u64);

/// Represents a 62-bit variable-length non-negative integer.  Those are
/// described in the Section 16 of RFC 9000, and are denoted as (i) in type
/// descriptions.
pub struct WireVarInt62(pub u64);

impl WireType for WireVarInt62 {
    fn get_length_on_wire(&self) -> usize {
        DataWriter::get_var_int62_len(self.0) as usize
    }
    fn serialize_into_writer(&self, writer: &mut DataWriter<'_>) -> bool {
        writer.write_var_int62(self.0)
    }
}

impl LengthWireType for WireVarInt62 {
    fn from_length(length: usize) -> Self {
        Self(length as u64)
    }
}

impl RefWireType<'_, MoqtVersion> for WireVarInt62 {
    fn from_ref(value: &MoqtVersion) -> Self {
        Self(*value as u64)
    }
}

/// Represents unframed raw string.
pub struct WireBytes<'a>(pub &'a Bytes);
impl WireType for WireBytes<'_> {
    fn get_length_on_wire(&self) -> usize {
        self.0.len()
    }
    fn serialize_into_writer(&self, writer: &mut DataWriter<'_>) -> bool {
        writer.write_bytes(self.0.clone())
    }
}

/// Represents a string where another wire type is used as a length prefix.
pub struct WireStringWithLengthPrefix<'a, T> {
    value: &'a str,
    marker: PhantomData<T>,
}

impl<'a, T> WireStringWithLengthPrefix<'a, T>
where
    T: LengthWireType,
{
    pub fn new(value: &'a str) -> Self {
        Self {
            value,
            marker: PhantomData,
        }
    }
}

impl<T> WireType for WireStringWithLengthPrefix<'_, T>
where
    T: LengthWireType,
{
    fn get_length_on_wire(&self) -> usize {
        let length_prefix = T::from_length(self.value.len());
        length_prefix.get_length_on_wire() + self.value.len()
    }
    fn serialize_into_writer(&self, writer: &mut DataWriter<'_>) -> bool {
        let length_prefix = T::from_length(self.value.len());
        if !length_prefix.serialize_into_writer(writer) {
            error!("Failed to serialize the length prefix");
            return false;
        }
        if !writer.write_string_piece(self.value) {
            error!("Failed to serialize the string proper");
            return false;
        }
        true
    }
}

impl<'a, T> RefWireType<'a, String> for WireStringWithLengthPrefix<'a, T>
where
    T: LengthWireType,
{
    fn from_ref(value: &'a String) -> Self {
        Self::new(value)
    }
}

/// Represents VarInt62-prefixed strings.
pub type WireStringWithVarInt62Length<'a> = WireStringWithLengthPrefix<'a, WireVarInt62>;

/// Allows std::optional to be used with this API. For instance, if the spec
/// defines
///   [Context ID (i)]
/// and the value is stored as std::optional<uint64> context_id, this can be
/// recorded as
///   WireOptional<WireVarInt62>(context_id)
/// When optional is absent, nothing is written onto the wire.
pub struct WireOptional<T>
where
    T: WireType,
{
    value: Option<T>,
}

impl<T> WireOptional<T>
where
    T: WireType,
{
    pub fn new(value: Option<T>) -> Self {
        Self { value }
    }
}

impl<T> WireType for WireOptional<T>
where
    T: WireType,
{
    fn get_length_on_wire(&self) -> usize {
        if let Some(ref inner_value) = self.value {
            inner_value.get_length_on_wire()
        } else {
            0
        }
    }

    fn serialize_into_writer(&self, writer: &mut DataWriter<'_>) -> bool {
        if let Some(ref inner_value) = self.value {
            inner_value.serialize_into_writer(writer)
        } else {
            // Return the default "success" status if no value is present.
            true
        }
    }
}

/// Allows multiple entries of the same type to be serialized in a single call.
pub struct WireSpan<'a, W, T> {
    value: &'a [T],
    marker: PhantomData<W>,
}

impl<'a, W, T> WireSpan<'a, W, T>
where
    W: RefWireType<'a, T>,
{
    pub fn new(value: &'a [T]) -> Self {
        Self {
            value,
            marker: PhantomData,
        }
    }
}

impl<'a, W, T> WireType for WireSpan<'a, W, T>
where
    W: RefWireType<'a, T>,
{
    fn get_length_on_wire(&self) -> usize {
        let mut total = 0;
        for value in self.value {
            total += W::from_ref(value).get_length_on_wire();
        }
        total
    }
    fn serialize_into_writer(&self, writer: &mut DataWriter<'_>) -> bool {
        for (i, value) in self.value.iter().enumerate() {
            if !W::from_ref(value).serialize_into_writer(writer) {
                error!("Failed to serialize vector value #{}", i);
                return false;
            }
        }
        true
    }
}

#[macro_export]
macro_rules! compute_length_on_wire {
    // Base case: No arguments
    () => {
        0
    };
    // Single argument (last in recursion)
    ($first:expr) => {
        $first.get_length_on_wire()
    };
    // Recursive case: Process the first argument and recurse
    ($first:expr, $($rest:expr),*) => {
        $first.get_length_on_wire() + compute_length_on_wire!($($rest),*)
    };
}

#[macro_export]
macro_rules! serialize_into_writer {
    // Base case: no arguments
    ($writer:expr, $argno:expr) => {
        true
    };

    // Recursive case
    ($writer:expr, $argno:expr, $first:expr $(, $rest:expr)*) => {{
        // Serialize the first argument
        if $first.serialize_into_writer($writer) {
            // Continue with the rest of the arguments
            serialize_into_writer!($writer, $argno + 1 $(, $rest)*)
        } else {
            false
        }
    }};
}

/// SerializeIntoBuffer(allocator, d1, d2, ... dN) computes the length required
/// to store the supplied data, allocates the buffer of appropriate size using
/// |allocator|, and serializes the result into it.  In a rare event that the
/// serialization fails (e.g. due to invalid varint62 value), an empty buffer is
/// returned.
#[macro_export]
macro_rules! serialize_into_buffer {
    ($($data:expr),*) => {{
        let buffer_size = compute_length_on_wire!($($data),*);
        if buffer_size == 0 {
            return BytesMut::new();
        }

        let mut buffer = BytesMut::with_capacity(buffer_size);
        let mut writer = DataWriter::new(&mut buffer);

        if !serialize_into_writer!(&mut writer, 0 $(, $data)*) {
            error!("Failed to serialize data");
            return BytesMut::new();
        }

        if buffer.len() != buffer_size {
            error!(
                "Excess {} bytes allocated while serializing",
                buffer_size - buffer.len()
            );
            return BytesMut::new();
        }

        buffer
    }};
}

#[macro_export]
macro_rules! serialize_into_string {
    ($($data:expr),*) => {{
        let buffer_size = compute_length_on_wire!($($data),*);
        if buffer_size == 0 {
            return String::new();
        }

        let mut buffer = BytesMut::with_capacity(buffer_size);
        let mut writer = DataWriter::new(&mut buffer);

        if !serialize_into_writer!(&mut writer, 0 $(, $data)*) {
            error!("Failed to serialize data");
            return String::new();
        }

        if buffer.len() != buffer_size {
            error!()(
                "Excess {} bytes allocated while serializing",
                buffer_size - buffer.len()
            );
            return String::new();
        }

        // Convert buffer to String
        match String::from_utf8(buffer) {
            Ok(s) => s,
            Err(e) => {
                error!("UTF-8 conversion error: {}", e);
                String::new()
            }
        }
    }};
}
