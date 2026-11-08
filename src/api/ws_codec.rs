//! RFC 6455 WebSocket codec: RFC 3174 SHA-1, `Sec-WebSocket-Accept`, frame encode/decode (`WEB-FE-01`).
//!
//! Pure and allocation-light so it is unit-testable without sockets. Client frames MUST be masked
//! (RFC 6455 §5.1); server frames are never masked. Payloads above `MAX_FRAME_BYTES` are refused.

/// GUID appended to `Sec-WebSocket-Key` before hashing (RFC 6455 §1.3).
pub const WS_GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";
/// Largest accepted inbound message (protocol `limits.maxFrameBytes`).
pub const MAX_FRAME_BYTES: usize = 1024 * 1024;

/// RFC 3174 SHA-1 digest.
pub fn sha1(data: &[u8]) -> [u8; 20] {
    let mut h: [u32; 5] = [0x6745_2301, 0xEFCD_AB89, 0x98BA_DCFE, 0x1032_5476, 0xC3D2_E1F0];
    let mut msg = data.to_vec();
    let bit_len = (data.len() as u64).wrapping_mul(8);
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());
    for block in msg.chunks_exact(64) {
        let mut w = [0u32; 80];
        for (i, word) in block.chunks_exact(4).enumerate() {
            w[i] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }
        let [mut a, mut b, mut c, mut d, mut e] = h;
        for (i, &wi) in w.iter().enumerate() {
            let (f, k) = match i {
                0..=19 => ((b & c) | (!b & d), 0x5A82_7999),
                20..=39 => (b ^ c ^ d, 0x6ED9_EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1B_BCDC),
                _ => (b ^ c ^ d, 0xCA62_C1D6),
            };
            let t = a.rotate_left(5).wrapping_add(f).wrapping_add(e).wrapping_add(k).wrapping_add(wi);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = t;
        }
        for (hv, v) in h.iter_mut().zip([a, b, c, d, e]) {
            *hv = hv.wrapping_add(v);
        }
    }
    let mut out = [0u8; 20];
    for (i, v) in h.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&v.to_be_bytes());
    }
    out
}

/// `Sec-WebSocket-Accept` for a client `Sec-WebSocket-Key`.
pub fn accept_key(client_key: &str) -> String {
    let mut s = client_key.trim().to_string();
    s.push_str(WS_GUID);
    crate::vt::codec::base64_encode(&sha1(s.as_bytes()))
}

/// `true` when `key` is base64 of exactly 16 bytes (24 chars ending in `==`).
pub fn is_valid_client_key(key: &str) -> bool {
    let k = key.trim();
    k.len() == 24
        && k.ends_with("==")
        && k[..22].bytes().all(|b| b.is_ascii_alphanumeric() || b == b'+' || b == b'/')
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Opcode {
    Continuation,
    Text,
    Binary,
    Close,
    Ping,
    Pong,
}

impl Opcode {
    fn from_u8(v: u8) -> Option<Self> {
        Some(match v {
            0x0 => Self::Continuation,
            0x1 => Self::Text,
            0x2 => Self::Binary,
            0x8 => Self::Close,
            0x9 => Self::Ping,
            0xA => Self::Pong,
            _ => return None,
        })
    }

    fn as_u8(self) -> u8 {
        match self {
            Self::Continuation => 0x0,
            Self::Text => 0x1,
            Self::Binary => 0x2,
            Self::Close => 0x8,
            Self::Ping => 0x9,
            Self::Pong => 0xA,
        }
    }

    pub fn is_control(self) -> bool {
        matches!(self, Self::Close | Self::Ping | Self::Pong)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frame {
    pub fin: bool,
    pub opcode: Opcode,
    pub payload: Vec<u8>,
}

/// Why a frame was refused, with the RFC 6455 close code to send.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrameError {
    pub close_code: u16,
    pub reason: &'static str,
}

const fn ferr(close_code: u16, reason: &'static str) -> FrameError {
    FrameError { close_code, reason }
}

/// Encodes an unmasked server frame with 7/16/64-bit length.
pub fn encode_frame(opcode: Opcode, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(payload.len() + 10);
    out.push(0x80 | opcode.as_u8());
    match payload.len() {
        n @ 0..=125 => out.push(n as u8),
        n @ 126..=0xFFFF => {
            out.push(126);
            out.extend_from_slice(&(n as u16).to_be_bytes());
        }
        n => {
            out.push(127);
            out.extend_from_slice(&(n as u64).to_be_bytes());
        }
    }
    out.extend_from_slice(payload);
    out
}

/// Encodes a client (masked) frame; used by tests and tooling.
pub fn encode_masked_frame(opcode: Opcode, payload: &[u8], mask: [u8; 4]) -> Vec<u8> {
    let mut out = encode_frame(opcode, &[]);
    out.truncate(1);
    match payload.len() {
        n @ 0..=125 => out.push(0x80 | n as u8),
        n @ 126..=0xFFFF => {
            out.push(0x80 | 126);
            out.extend_from_slice(&(n as u16).to_be_bytes());
        }
        n => {
            out.push(0x80 | 127);
            out.extend_from_slice(&(n as u64).to_be_bytes());
        }
    }
    out.extend_from_slice(&mask);
    out.extend(payload.iter().enumerate().map(|(i, b)| b ^ mask[i % 4]));
    out
}

/// Decodes one frame from `buf`. `Ok(None)` = need more bytes; `Ok(Some((frame, used)))` otherwise.
/// With `require_mask`, unmasked frames are a protocol error (client -> server direction).
pub fn decode_frame(buf: &[u8], require_mask: bool) -> Result<Option<(Frame, usize)>, FrameError> {
    if buf.len() < 2 {
        return Ok(None);
    }
    let (b0, b1) = (buf[0], buf[1]);
    if b0 & 0x70 != 0 {
        return Err(ferr(1002, "reserved bits set"));
    }
    let fin = b0 & 0x80 != 0;
    let opcode = Opcode::from_u8(b0 & 0x0F).ok_or(ferr(1002, "unknown opcode"))?;
    let masked = b1 & 0x80 != 0;
    if require_mask && !masked {
        return Err(ferr(1002, "client frames must be masked"));
    }
    let mut pos = 2usize;
    let len = match b1 & 0x7F {
        126 => {
            if buf.len() < 4 {
                return Ok(None);
            }
            pos = 4;
            u64::from(u16::from_be_bytes([buf[2], buf[3]]))
        }
        127 => {
            if buf.len() < 10 {
                return Ok(None);
            }
            pos = 10;
            let mut b = [0u8; 8];
            b.copy_from_slice(&buf[2..10]);
            u64::from_be_bytes(b)
        }
        n => u64::from(n),
    };
    if opcode.is_control() && (len > 125 || !fin) {
        return Err(ferr(1002, "invalid control frame"));
    }
    if len > MAX_FRAME_BYTES as u64 {
        return Err(ferr(1009, "frame too large"));
    }
    let len = len as usize;
    let mask = if masked {
        if buf.len() < pos + 4 {
            return Ok(None);
        }
        let m = [buf[pos], buf[pos + 1], buf[pos + 2], buf[pos + 3]];
        pos += 4;
        Some(m)
    } else {
        None
    };
    if buf.len() < pos + len {
        return Ok(None);
    }
    let mut payload = buf[pos..pos + len].to_vec();
    if let Some(m) = mask {
        for (i, b) in payload.iter_mut().enumerate() {
            *b ^= m[i % 4];
        }
    }
    Ok(Some((Frame { fin, opcode, payload }, pos + len)))
}

/// Close frame payload: 2-byte code + UTF-8 reason.
pub fn close_payload(code: u16, reason: &str) -> Vec<u8> {
    let mut p = code.to_be_bytes().to_vec();
    p.extend_from_slice(reason.as_bytes().get(..reason.len().min(120)).unwrap_or(&[]));
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(d: &[u8]) -> String {
        d.iter().map(|b| format!("{b:02x}")).collect()
    }

    #[test]
    fn sha1_rfc3174_vectors() {
        assert_eq!(hex(&sha1(b"abc")), "a9993e364706816aba3e25717850c26c9cd0d89d");
        assert_eq!(hex(&sha1(b"")), "da39a3ee5e6b4b0d3255bfef95601890afd80709");
        assert_eq!(
            hex(&sha1(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq")),
            "84983e441c3bd26ebaae4aa1f95129e5e54670f1"
        );
        assert_eq!(hex(&sha1(&vec![b'a'; 1_000_000])), "34aa973cd4c4daa4f61eeb2bdbad27316534016f");
    }

    #[test]
    fn rfc6455_accept_example() {
        assert_eq!(accept_key("dGhlIHNhbXBsZSBub25jZQ=="), "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=");
        assert!(is_valid_client_key("dGhlIHNhbXBsZSBub25jZQ=="));
        assert!(!is_valid_client_key("short"));
    }

    #[test]
    fn frames_round_trip_all_length_forms() {
        for n in [0usize, 5, 125, 126, 300, 65535, 65536, 200_000] {
            let data = vec![b'z'; n];
            let wire = encode_masked_frame(Opcode::Text, &data, [1, 2, 3, 4]);
            let (f, used) = decode_frame(&wire, true).unwrap().unwrap();
            assert_eq!((f.opcode, f.payload.len(), used), (Opcode::Text, n, wire.len()));
            assert_eq!(f.payload, data);
            let srv = encode_frame(Opcode::Text, &data);
            let (g, _) = decode_frame(&srv, false).unwrap().unwrap();
            assert_eq!(g.payload, data);
            assert_eq!(decode_frame(&wire[..wire.len() - 1], true).unwrap(), None);
        }
        assert_eq!(decode_frame(&encode_frame(Opcode::Text, b"x"), true).unwrap_err().close_code, 1002);
    }
}
