use byteorder::{ByteOrder, LittleEndian};
use std::time;

///
/// An input_event is built up using 24bs
/// [ 0 - 8 ] [8 - 16 ] [ 16 - 18 ] [ 18 - 20 ] [ 20 - 24 ]
///   secs      usecs      type        code        value
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RawEvent {
    pub time: time::SystemTime,
    pub typ: u16,
    pub code: u16,
    pub value: u32,
}

impl From<&[u8; 16]> for RawEvent {
    fn from(buf: &[u8; 16]) -> Self {
        let seconds = LittleEndian::read_u32(&buf[0..4]);
        let microseconds = LittleEndian::read_u32(&buf[4..8]);
        let time = time::UNIX_EPOCH + time::Duration::new(seconds as u64, microseconds * 1_000);

        let typ = LittleEndian::read_u16(&buf[8..10]);
        let code = LittleEndian::read_u16(&buf[10..12]);
        let value = LittleEndian::read_u32(&buf[12..16]);

        Self {
            time,
            typ,
            code,
            value,
        }
    }
}
