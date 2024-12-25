use bytes::BufMut;
use log::error;

/// Maximum value that can be properly encoded using RFC 9000 62-bit Variable
/// Length Integer encoding.
#[allow(non_upper_case_globals)]
pub const kVarInt62MaxValue: u64 = 0x3fffffffffffffff;

/// RFC 9000 62-bit Variable Length Integer encoding masks
/// If a uint64_t anded with a mask is not 0 then the value is encoded
/// using that length (or is too big, in the case of kVarInt62ErrorMask).
/// Values must be checked in order (error, 8-, 4-, and then 2- bytes)
/// and if none are non-0, the value is encoded in 1 byte.
#[allow(non_upper_case_globals)]
pub const kVarInt62ErrorMask: u64 = 0xc000000000000000;
#[allow(non_upper_case_globals)]
pub const kVarInt62Mask8Bytes: u64 = 0x3fffffffc0000000;
#[allow(non_upper_case_globals)]
pub const kVarInt62Mask4Bytes: u64 = 0x000000003fffc000;
#[allow(non_upper_case_globals)]
pub const kVarInt62Mask2Bytes: u64 = 0x0000000000003fc0;

#[derive(Copy, Clone, PartialOrd, PartialEq)]
#[allow(non_camel_case_types)]
#[repr(u8)]
pub enum VariableLengthIntegerLength {
    // Length zero means the variable length integer is not present.
    VARIABLE_LENGTH_INTEGER_LENGTH_0 = 0,
    VARIABLE_LENGTH_INTEGER_LENGTH_1 = 1,
    VARIABLE_LENGTH_INTEGER_LENGTH_2 = 2,
    VARIABLE_LENGTH_INTEGER_LENGTH_4 = 4,
    VARIABLE_LENGTH_INTEGER_LENGTH_8 = 8,
}
/// By default we write the IETF long header length using the 2-byte encoding
/// of variable length integers, even when the length is below 64, which allows
/// us to fill in the length before knowing what the length actually is.
#[allow(non_upper_case_globals)]
pub const kDefaultLongHeaderLengthLength: VariableLengthIntegerLength =
    VariableLengthIntegerLength::VARIABLE_LENGTH_INTEGER_LENGTH_2;

/// This class provides facilities for packing binary data.
///
/// The DataWriter supports appending primitive values (int, string, etc)
/// to a frame instance.  The internal memory buffer is exposed as the "data"
/// of the DataWriter.
pub struct DataWriter<'a> {
    buffer: &'a mut dyn BufMut,
}

impl<'a> DataWriter<'a> {
    // Creates a DataWriter where |buffer| is not owned
    // using NETWORK_BYTE_ORDER endianness.
    pub fn new(buffer: &'a mut dyn BufMut) -> Self {
        Self { buffer }
    }

    // Returns the size of the DataWriter's data.
    pub fn remaining(&self) -> usize {
        self.buffer.remaining_mut()
    }

    // Methods for adding to the payload.  These values are appended to the end
    // of the DataWriter payload.

    // Writes 8/16/32/64-bit unsigned integers.
    pub fn write_uint8(&mut self, value: u8) -> bool {
        if self.remaining() < 1 {
            return false;
        }
        self.buffer.put_u8(value);
        true
    }
    pub fn write_uint16(&mut self, value: u16) -> bool {
        if self.remaining() < 2 {
            return false;
        }
        self.buffer.put_u16(value);
        true
    }
    pub fn write_uint32(&mut self, value: u32) -> bool {
        if self.remaining() < 4 {
            return false;
        }
        self.buffer.put_u32(value);
        true
    }
    pub fn write_uint64(&mut self, value: u64) -> bool {
        if self.remaining() < 8 {
            return false;
        }
        self.buffer.put_u64(value);
        true
    }

    // Writes least significant |num_bytes| of a 64-bit unsigned integer
    pub fn write_bytes_to_uint64(&mut self, num_bytes: usize, value: u64) -> bool {
        if num_bytes > 8 {
            return false;
        }

        let be_bytes = &value.to_be_bytes()[8 - num_bytes..];
        self.write_bytes(be_bytes)
    }

    pub fn write_string_piece(&mut self, val: &str) -> bool {
        self.write_bytes(val.as_bytes())
    }

    pub fn write_string_piece16(&mut self, val: &str) -> bool {
        if val.len() > u16::MAX as usize {
            return false;
        }
        if !self.write_uint16(val.len() as u16) {
            return false;
        }
        self.write_bytes(val.as_bytes())
    }

    pub fn write_bytes(&mut self, data: &[u8]) -> bool {
        let remaining_bytes = self.buffer.remaining_mut();
        if remaining_bytes < data.len() {
            return false;
        }
        self.buffer.put_slice(data);
        true
    }

    pub fn write_repeated_byte(&mut self, byte: u8, count: usize) -> bool {
        if self.remaining() < count {
            return false;
        }
        for _ in 0..count {
            self.buffer.put_u8(byte);
        }
        true
    }
    // Fills the remaining buffer with null characters.
    pub fn write_padding(&mut self) -> bool {
        if self.remaining() == usize::MAX {
            return false;
        }
        self.write_repeated_byte(0x00, self.remaining())
    }
    // Write padding of |count| bytes.
    pub fn write_padding_bytes(&mut self, count: usize) -> bool {
        self.write_repeated_byte(0x00, count)
    }

    // Write tag as a 32-bit unsigned integer to the payload. As tags are already
    // converted to big endian (e.g., CHLO is 'C','H','L','O') in memory by TAG or
    // MakeQuicTag and tags are written in byte order, so tags on the wire are
    // in big endian.
    pub fn write_tag(&mut self, tag: u32) -> bool {
        self.write_uint32(tag)
    }

    /// Write a 62-bit unsigned integer using RFC 9000 Variable Length Integer
    /// encoding. Returns false if the value is out of range or if there is no room
    /// in the buffer.
    pub fn write_var_int62(&mut self, value: u64) -> bool {
        let remaining_bytes = self.buffer.remaining_mut();

        if (value & kVarInt62ErrorMask) == 0 {
            // We know the high 2 bits are 0 so |value| is legal.
            // We can do the encoding.
            if (value & kVarInt62Mask8Bytes) != 0 {
                // Someplace in the high-4 bytes is a 1-bit. Do an 8-byte
                // encoding.
                if remaining_bytes >= 8 {
                    self.buffer.put_u8(((value >> 56) & 0x3f) as u8 + 0xc0);
                    self.buffer.put_u8(((value >> 48) & 0xff) as u8);
                    self.buffer.put_u8(((value >> 40) & 0xff) as u8);
                    self.buffer.put_u8(((value >> 32) & 0xff) as u8);
                    self.buffer.put_u8(((value >> 24) & 0xff) as u8);
                    self.buffer.put_u8(((value >> 16) & 0xff) as u8);
                    self.buffer.put_u8(((value >> 8) & 0xff) as u8);
                    self.buffer.put_u8((value & 0xff) as u8);
                    return true;
                }
                return false;
            }
            // The high-order-4 bytes are all 0, check for a 1, 2, or 4-byte
            // encoding
            if (value & kVarInt62Mask4Bytes) != 0 {
                // The encoding will not fit into 2 bytes, Do a 4-byte
                // encoding.
                if remaining_bytes >= 4 {
                    self.buffer.put_u8(((value >> 24) & 0x3f) as u8 + 0x80);
                    self.buffer.put_u8(((value >> 16) & 0xff) as u8);
                    self.buffer.put_u8(((value >> 8) & 0xff) as u8);
                    self.buffer.put_u8((value & 0xff) as u8);
                    return true;
                }
                return false;
            }
            // The high-order bits are all 0. Check to see if the number
            // can be encoded as one or two bytes. One byte encoding has
            // only 6 significant bits (bits 0xffffffff ffffffc0 are all 0).
            // Two byte encoding has more than 6, but 14 or less significant
            // bits (bits 0xffffffff ffffc000 are 0 and 0x00000000 00003fc0
            // are not 0)
            if (value & kVarInt62Mask2Bytes) != 0 {
                // Do 2-byte encoding
                if remaining_bytes >= 2 {
                    self.buffer.put_u8(((value >> 8) & 0x3f) as u8 + 0x40);
                    self.buffer.put_u8((value & 0xff) as u8);
                    return true;
                }
                return false;
            }
            if remaining_bytes >= 1 {
                // Do 1-byte encoding
                self.buffer.put_u8((value & 0x3f) as u8);
                return true;
            }
            return false;
        }
        // Can not encode, high 2 bits not 0
        false
    }

    // Same as write_var_int62(uint64_t), but forces an encoding size to write to.
    // This is not as optimized as write_var_int62(uint64_t). Returns false if the
    // value does not fit in the specified write_length or if there is no room in
    // the buffer.
    pub fn write_var_int62_with_forced_length(
        &mut self,
        value: u64,
        write_length: VariableLengthIntegerLength,
    ) -> bool {
        let remaining_bytes = self.buffer.remaining_mut();
        if remaining_bytes < write_length as usize {
            return false;
        }

        let min_length = DataWriter::get_var_int62_len(value);
        if write_length < min_length {
            error!(
                "Cannot write value {} with write_length {}",
                value as u8, write_length as u8
            );
            return false;
        }
        if write_length == min_length {
            return self.write_var_int62(value);
        }

        if write_length == VariableLengthIntegerLength::VARIABLE_LENGTH_INTEGER_LENGTH_2 {
            return self.write_uint8(0b01000000) && self.write_uint8(value as u8);
        }
        if write_length == VariableLengthIntegerLength::VARIABLE_LENGTH_INTEGER_LENGTH_4 {
            return self.write_uint8(0b10000000)
                && self.write_uint8(0)
                && self.write_uint16(value as u16);
        }
        if write_length == VariableLengthIntegerLength::VARIABLE_LENGTH_INTEGER_LENGTH_8 {
            return self.write_uint8(0b11000000)
                && self.write_uint8(0)
                && self.write_uint16(0)
                && self.write_uint32(value as u32);
        }

        error!("Invalid write_length {}", write_length as u8);
        false
    }

    // Writes a string piece as a consecutive length/content pair. The
    // length uses RFC 9000 Variable Length Integer encoding.
    pub fn write_string_piece_var_int62(&mut self, string_piece: &str) -> bool {
        if !self.write_var_int62(string_piece.len() as u64) {
            return false;
        }
        if !string_piece.is_empty() && !self.write_bytes(string_piece.as_bytes()) {
            return false;
        }
        true
    }

    /// Utility function to return the number of bytes needed to encode
    /// the given value using IETF VarInt62 encoding. Returns the number
    /// of bytes required to encode the given integer or 0 if the value
    /// is too large to encode.
    pub fn get_var_int62_len(value: u64) -> VariableLengthIntegerLength {
        if (value & kVarInt62ErrorMask) != 0 {
            error!(
                "Attempted to encode a value, {}, that is too big for VarInt62",
                value
            );
            return VariableLengthIntegerLength::VARIABLE_LENGTH_INTEGER_LENGTH_0;
        }
        if (value & kVarInt62Mask8Bytes) != 0 {
            return VariableLengthIntegerLength::VARIABLE_LENGTH_INTEGER_LENGTH_8;
        }
        if (value & kVarInt62Mask4Bytes) != 0 {
            return VariableLengthIntegerLength::VARIABLE_LENGTH_INTEGER_LENGTH_4;
        }
        if (value & kVarInt62Mask2Bytes) != 0 {
            return VariableLengthIntegerLength::VARIABLE_LENGTH_INTEGER_LENGTH_2;
        }
        VariableLengthIntegerLength::VARIABLE_LENGTH_INTEGER_LENGTH_1
    }
}
