use bytes::{Buf, BufMut, BytesMut};
use tokio_util::codec::{Decoder, Encoder};
use byteorder::{LittleEndian, ReadBytesExt};
use std::io::Cursor;

pub const FUTU_PROTO_MAGIC: [u8; 4] = [b'F', b'T', b'-', b'X'];

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
        
        let mut magic = [0u8; 4];
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

    fn encode(&mut self, item: (FutuHeader, Vec<u8>), dst: &mut BytesMut) -> Result<(), Self::Error> {
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
