use byteorder::{LittleEndian, ReadBytesExt};
use bytes::{Buf, BufMut, BytesMut};
use std::io::Cursor;
use tokio_util::codec::{Decoder, Encoder};

pub const FUTU_PROTO_MAGIC: [u8; 2] = *b"FT";

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct FutuHeader {
    pub sz_header: u8,
    pub sz_body: u32,
    pub n_proto_id: u32,
    pub n_proto_fmt_type: u8,
    pub n_proto_ver: u8,
    pub n_serial_no: u32,
    pub n_body_len: u32,
    pub arr_body_sha1: [u8; 20],
    pub arr_reserved: [u8; 8],
}

impl FutuHeader {
    pub fn new(proto_id: u32, serial_no: u32, body_len: u32) -> Self {
        Self {
            sz_header: 44, // Default Magic+Header Length for Futu Open API
            sz_body: body_len,
            n_proto_id: proto_id,
            n_proto_fmt_type: 0, // 0 = Protobuf
            n_proto_ver: 0,
            n_serial_no: serial_no,
            n_body_len: body_len,
            arr_body_sha1: [0; 20], // Typically zeroed out if no encryption
            arr_reserved: [0; 8],
        }
    }
}

pub struct FutuCodec;

impl Decoder for FutuCodec {
    type Item = (FutuHeader, Vec<u8>);
    type Error = std::io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if src.len() < 44 {
            return Ok(None);
        }

        let mut buf = Cursor::new(&src[..44]);

        let mut magic = [0u8; 2];
        std::io::Read::read_exact(&mut buf, &mut magic)?;

        if magic != FUTU_PROTO_MAGIC {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Invalid Protocol Magic",
            ));
        }

        let proto_id = buf.read_u32::<LittleEndian>()?;
        let proto_fmt_type = std::io::Read::bytes(&mut buf).next().unwrap()?;
        let proto_ver = std::io::Read::bytes(&mut buf).next().unwrap()?;
        let serial_no = buf.read_u32::<LittleEndian>()?;
        let body_len = buf.read_u32::<LittleEndian>()?;

        let mut arr_body_sha1 = [0u8; 20];
        std::io::Read::read_exact(&mut buf, &mut arr_body_sha1)?;

        let mut arr_reserved = [0u8; 8];
        std::io::Read::read_exact(&mut buf, &mut arr_reserved)?;

        let total_frame_len = 44 + body_len as usize;

        if src.len() < total_frame_len {
            src.reserve(total_frame_len - src.len());
            return Ok(None);
        }

        let header = FutuHeader {
            sz_header: 44,
            sz_body: body_len,
            n_proto_id: proto_id,
            n_proto_fmt_type: proto_fmt_type,
            n_proto_ver: proto_ver,
            n_serial_no: serial_no,
            n_body_len: body_len,
            arr_body_sha1,
            arr_reserved,
        };

        src.advance(44); // consume header
        let body = src[..body_len as usize].to_vec();
        src.advance(body_len as usize); // consume body

        Ok(Some((header, body)))
    }
}

impl Encoder<(FutuHeader, Vec<u8>)> for FutuCodec {
    type Error = std::io::Error;

    fn encode(
        &mut self,
        item: (FutuHeader, Vec<u8>),
        dst: &mut BytesMut,
    ) -> Result<(), Self::Error> {
        let (header, body) = item;
        dst.reserve(44 + body.len());

        dst.put_slice(&FUTU_PROTO_MAGIC);
        dst.put_u32_le(header.n_proto_id);
        dst.put_u8(header.n_proto_fmt_type);
        dst.put_u8(header.n_proto_ver);
        dst.put_u32_le(header.n_serial_no);
        dst.put_u32_le(body.len() as u32);
        dst.put_slice(&header.arr_body_sha1);
        dst.put_slice(&header.arr_reserved);
        dst.put_slice(&body);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::BytesMut;

    #[test]
    fn codec_round_trips_header_and_body() {
        let header = FutuHeader::new(1001, 42, 3);
        let body = vec![1, 2, 3];
        let mut codec = FutuCodec;
        let mut encoded = BytesMut::new();

        codec
            .encode((header.clone(), body.clone()), &mut encoded)
            .unwrap();

        let mut decoded = encoded.clone();
        let (actual_header, actual_body) = codec.decode(&mut decoded).unwrap().unwrap();

        assert_eq!(actual_header.n_proto_id, header.n_proto_id);
        assert_eq!(actual_header.n_serial_no, header.n_serial_no);
        assert_eq!(actual_header.n_body_len, header.n_body_len);
        assert_eq!(actual_body, body);
        assert!(decoded.is_empty());
    }

    #[test]
    fn codec_rejects_invalid_magic() {
        let mut codec = FutuCodec;
        let mut src = BytesMut::new();
        src.extend_from_slice(b"ZZ");
        src.resize(44, 0);

        let err = codec.decode(&mut src).unwrap_err();

        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
    }
}
