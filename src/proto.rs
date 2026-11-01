//! Wire format. Everything is a single UDP datagram:
//! `[MAGIC][kind][seq:u16 BE][payload...]`

pub const MAGIC: u8 = 0xAB;
pub const SERVICE_SRC: &str = "_abridge._udp.local.";
pub const SERVICE_RX: &str = "_abridgerx._udp.local.";
pub const DEFAULT_PORT: u16 = 47800;
pub const RATE: u32 = 48_000;
pub const FRAME: usize = 480; // 10 ms @ 48 kHz
pub const CHANNELS: usize = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Receiver -> sender, every 500 ms. Payload: receiver name.
    Subscribe = 1,
    /// Sender -> receiver, reply to Subscribe. Payload: [streaming:u8] + sender name.
    Ack = 2,
    /// Sender -> receiver. Payload: one Opus packet (10 ms).
    Audio = 3,
    /// Receiver -> sender, on disconnect.
    Bye = 4,
}

pub fn build(kind: Kind, seq: u16, payload: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(4 + payload.len());
    v.push(MAGIC);
    v.push(kind as u8);
    v.extend_from_slice(&seq.to_be_bytes());
    v.extend_from_slice(payload);
    v
}

pub fn parse(buf: &[u8]) -> Option<(Kind, u16, &[u8])> {
    if buf.len() < 4 || buf[0] != MAGIC {
        return None;
    }
    let kind = match buf[1] {
        1 => Kind::Subscribe,
        2 => Kind::Ack,
        3 => Kind::Audio,
        4 => Kind::Bye,
        _ => return None,
    };
    Some((kind, u16::from_be_bytes([buf[2], buf[3]]), &buf[4..]))
}

pub fn host_name() -> String {
    let n = gethostname::gethostname().to_string_lossy().to_string();
    let n = n.trim_end_matches(".local").to_string();
    if n.is_empty() { "Computador".into() } else { n }
}

/// Best-effort local LAN address (no packets are sent).
pub fn local_ip() -> Option<String> {
    let s = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    s.connect("192.0.2.1:9").ok()?;
    Some(s.local_addr().ok()?.ip().to_string())
}
