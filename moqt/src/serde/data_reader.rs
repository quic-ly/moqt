use crate::serde::data_writer::VariableLengthIntegerLength;
use bytes::{Buf, Bytes};
use std::io::{Error, ErrorKind};
use std::result::Result;

/// To use, simply construct a QuicheDataReader using the underlying buffer that
/// you'd like to read fields from, then call one of the Read*() methods to
/// actually do some reading.
///
/// This class keeps an internal iterator to keep track of what's already been
/// read and each successive Read*() call automatically increments said iterator
/// on success. On failure, internal state of the QuicheDataReader should not be
/// trusted and it is up to the caller to throw away the failed instance and
/// handle the error as appropriate. None of the Read*() methods should ever be
/// called after failure, as they will also fail immediately.
pub struct DataReader<'a> {
    data: &'a mut dyn Buf,
}

impl<'a> DataReader<'a> {
    pub fn new(data: &'a mut dyn Buf) -> Self {
        Self { data }
    }

    // Returns true if the underlying buffer has enough room to read the given
    // amount of bytes.
    pub fn can_read(&self, n: usize) -> bool {
        n <= self.data.remaining()
    }

    // Reads an 8/16/24/32/64-bit unsigned integer into the given output
    // parameter. Forwards the internal iterator on success. Returns true on
    // success, false otherwise.
    pub fn read_uint8(&mut self) -> Result<u8, Error> {
        if !self.can_read(1) {
            return Err(Error::from(ErrorKind::UnexpectedEof));
        }
        Ok(self.data.get_u8())
    }
    pub fn read_uint16(&mut self) -> Result<u16, Error> {
        if !self.can_read(2) {
            return Err(Error::from(ErrorKind::UnexpectedEof));
        }
        Ok(self.data.get_u16())
    }
    pub fn read_uint32(&mut self) -> Result<u32, Error> {
        if !self.can_read(4) {
            return Err(Error::from(ErrorKind::UnexpectedEof));
        }
        Ok(self.data.get_u32())
    }
    pub fn read_uint64(&mut self) -> Result<u64, Error> {
        if !self.can_read(8) {
            return Err(Error::from(ErrorKind::UnexpectedEof));
        }
        Ok(self.data.get_u64())
    }

    // Set |result| to 0, then read |num_bytes| bytes in the correct byte order
    // into least significant bytes of |result|.
    pub fn read_bytes_to_uint64(&mut self, n: usize) -> Result<u64, Error> {
        if n > 8 {
            return Err(Error::from(ErrorKind::InvalidInput));
        }

        let bytes = self.read_bytes(n)?;
        let mut r = [0u8; 8];
        r[8 - n..].copy_from_slice(&bytes[..n]);
        Ok(u64::from_be_bytes(r))
    }

    // Reads a string prefixed with 16-bit length into the given output parameter.
    //
    // NOTE: Does not copy but rather references strings in the underlying buffer.
    // This should be kept in mind when handling memory management!
    //
    // Forwards the internal iterator on success.
    // Returns true on success, false otherwise.
    pub fn read_string_piece16(&mut self) -> Result<String, Error> {
        // Read resultant length.
        let l = self.read_uint16()? as usize;
        self.read_string_piece(l)
    }

    // Reads a string prefixed with 8-bit length into the given output parameter.
    //
    // NOTE: Does not copy but rather references strings in the underlying buffer.
    // This should be kept in mind when handling memory management!
    //
    // Forwards the internal iterator on success.
    // Returns true on success, false otherwise.
    pub fn read_string_piece8(&mut self) -> Result<String, Error> {
        // Read resultant length.
        let l = self.read_uint8()? as usize;
        self.read_string_piece(l)
    }

    // Reads a given number of bytes into the given buffer. The buffer
    // must be of adequate size.
    // Forwards the internal iterator on success.
    // Returns true on success, false otherwise.
    pub fn read_string_piece(&mut self, n: usize) -> Result<String, Error> {
        if !self.can_read(n) {
            return Err(Error::from(ErrorKind::UnexpectedEof));
        }

        let bytes = self.read_bytes(n)?;
        String::from_utf8(bytes.to_vec()).map_err(|_| Error::from(ErrorKind::InvalidData))
    }

    // Reads at most a given number of bytes into the provided view.
    pub fn read_at_most(&mut self, n: usize) -> Result<String, Error> {
        let actual_size = n.min(self.data.remaining());
        self.read_string_piece(actual_size)
    }

    // Reads tag represented as 32-bit unsigned integer into given output
    // parameter. Tags are in big endian on the wire (e.g., CHLO is
    // 'C','H','L','O') and are read in byte order, so tags in memory are in big
    // endian.
    pub fn read_tag(&mut self) -> Result<u32, Error> {
        self.read_uint32()
    }

    // Reads a sequence of a fixed number of decimal digits, parses them as an
    // unsigned integer and returns them as a uint64_t.  Forwards internal
    // iterator on success, may forward it even in case of failure.
    pub fn read_decimal64(&mut self, n: usize) -> Result<u64, Error> {
        let digits = self.read_string_piece(n)?;
        digits
            .parse::<u64>()
            .map_err(|_| Error::from(ErrorKind::InvalidData))
    }

    // Returns the length in bytes of a variable length integer based on the next
    // two bits available. Returns 1, 2, 4, or 8 on success, and 0 on failure.
    pub fn peek_var_int62_length(&self) -> VariableLengthIntegerLength {
        if !self.data.has_remaining() {
            VariableLengthIntegerLength::VARIABLE_LENGTH_INTEGER_LENGTH_0
        } else {
            // Peek at the buffer
            let next = self.data.chunk()[0];
            let v = 1u8 << ((next & 0b11000000) >> 6);
            match v {
                0 => VariableLengthIntegerLength::VARIABLE_LENGTH_INTEGER_LENGTH_0,
                1 => VariableLengthIntegerLength::VARIABLE_LENGTH_INTEGER_LENGTH_1,
                2 => VariableLengthIntegerLength::VARIABLE_LENGTH_INTEGER_LENGTH_2,
                4 => VariableLengthIntegerLength::VARIABLE_LENGTH_INTEGER_LENGTH_4,
                _ => VariableLengthIntegerLength::VARIABLE_LENGTH_INTEGER_LENGTH_8,
            }
        }
    }

    // Read an RFC 9000 62-bit Variable Length Integer and place the result in
    // |*result|. Returns false if there is not enough space in the buffer to read
    // the number, true otherwise. If false is returned, |*result| is not altered.
    pub fn read_var_int62(&mut self) -> Result<u64, Error> {
        let remaining = self.data.remaining();

        if remaining != 0 {
            let next = self.data.chunk();
            match next[0] & 0xc0 {
                0xc0 => {
                    // Leading 0b11...... is 8 byte encoding
                    if remaining >= 8 {
                        let v = (((next[0] & 0x3f) as u64) << 56)
                            + ((next[1] as u64) << 48)
                            + ((next[2] as u64) << 40)
                            + ((next[3] as u64) << 32)
                            + ((next[4] as u64) << 24)
                            + ((next[5] as u64) << 16)
                            + ((next[6] as u64) << 8)
                            + next[7] as u64;
                        self.data.advance(8);
                        Ok(v)
                    } else {
                        Err(Error::from(ErrorKind::InvalidData))
                    }
                }

                0x80 => {
                    // Leading 0b10...... is 4 byte encoding
                    if remaining >= 4 {
                        let v = (((next[0] & 0x3f) as u64) << 24)
                            + ((next[1] as u64) << 16)
                            + ((next[2] as u64) << 8)
                            + next[3] as u64;
                        self.data.advance(4);
                        Ok(v)
                    } else {
                        Err(Error::from(ErrorKind::InvalidData))
                    }
                }
                0x40 => {
                    // Leading 0b01...... is 2 byte encoding
                    if remaining >= 2 {
                        let v = (((next[0] & 0x3f) as u64) << 8) + next[1] as u64;
                        self.data.advance(2);
                        Ok(v)
                    } else {
                        Err(Error::from(ErrorKind::InvalidData))
                    }
                }
                0x00 => {
                    // Leading 0b00...... is 1 byte encoding
                    let v = (next[0] & 0x3f) as u64;
                    self.data.advance(1);
                    Ok(v)
                }
                _ => Err(Error::from(ErrorKind::InvalidData)),
            }
        } else {
            Err(Error::from(ErrorKind::UnexpectedEof))
        }
    }

    // Reads a string prefixed with a RFC 9000 62-bit variable Length integer
    // length into the given output parameter.
    //
    // NOTE: Does not copy but rather references strings in the underlying buffer.
    // This should be kept in mind when handling memory management!
    //
    // Returns false if there is not enough space in the buffer to read
    // the number and subsequent string, true otherwise.
    pub fn read_string_piece_var_int62(&mut self) -> Result<String, Error> {
        let l = self.read_var_int62()? as usize;
        self.read_string_piece(l)
    }

    // Reads a string prefixed with a RFC 9000 varint length prefix, and copies it
    // into the provided string.
    //
    // Returns false if there is not enough space in the buffer to read
    // the number and subsequent string, true otherwise.
    pub fn read_string_var_int62(&mut self) -> Result<String, Error> {
        self.read_string_piece_var_int62()
    }

    // Returns the remaining payload as a absl::string_view.
    //
    // NOTE: Does not copy but rather references strings in the underlying buffer.
    // This should be kept in mind when handling memory management!
    //
    // Forwards the internal iterator.
    pub fn read_remaining_payload(&mut self) -> Bytes {
        self.data.copy_to_bytes(self.data.remaining())
    }

    // Returns the remaining payload as a absl::string_view.
    //
    // NOTE: Does not copy but rather references strings in the underlying buffer.
    // This should be kept in mind when handling memory management!
    //
    // DOES NOT forward the internal iterator.
    pub fn peek_remaining_payload(&mut self) -> &[u8] {
        self.data.chunk()
    }

    // Returns the entire payload as a absl::string_view.
    //
    // NOTE: Does not copy but rather references strings in the underlying buffer.
    // This should be kept in mind when handling memory management!
    //
    // DOES NOT forward the internal iterator.
    //pub fn FullPayload(&mut self) -> Result<Bytes, Error> {}

    // Returns the part of the payload that has been already read as a
    // absl::string_view.
    //
    // NOTE: Does not copy but rather references strings in the underlying buffer.
    // This should be kept in mind when handling memory management!
    //
    // DOES NOT forward the internal iterator.
    //pub fn PreviouslyReadPayload(&mut self) -> Result<Bytes, Error> {}

    // Reads a given number of bytes into the given buffer. The buffer
    // must be of adequate size.
    // Forwards the internal iterator on success.
    // Returns true on success, false otherwise.
    pub fn read_bytes(&mut self, n: usize) -> Result<Bytes, Error> {
        if !self.can_read(n) {
            return Err(Error::from(ErrorKind::UnexpectedEof));
        }

        Ok(self.data.copy_to_bytes(n))
    }
}
