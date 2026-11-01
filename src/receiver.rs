//! Mac side: receive Opus over UDP, decode, and play with an adaptive jitter buffer.

use crate::proto::{self, Kind, CHANNELS, FRAME, RATE};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample};
use std::collections::VecDeque;
use std::net::{SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const BASE_TARGET_MS: u32 = 40;
const MAX_TARGET_MS: u32 = 160;
const ACK_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Link {
    Idle,
    /// Subscribed but the sender isn't answering (yet).
    Searching,
    /// Sender answers, but its owner hasn't pressed Start.
    Waiting,
    Playing,
}

#[derive(Clone, Debug)]
pub struct Status {
    pub link: Link,
    pub sender_name: String,
    pub buffer_ms: u32,
    pub target_ms: u32,
    pub lost: u64,
    pub underruns: u64,
    pub error: Option<String>,
}

struct Shared {
    name: String,
    ring: Mutex<VecDeque<f32>>, // interleaved stereo @ 48 kHz
    target_ms: AtomicU32,
    volume: AtomicU32,
    rebuffer: AtomicBool,
    generation: AtomicU32,
    peer: Mutex<Option<SocketAddr>>,
    ack: Mutex<Option<(Instant, bool, String)>>,
    lost: AtomicU64,
    underruns: AtomicU64,
    error: Mutex<Option<String>>,
}

pub struct Receiver {
    shared: Arc<Shared>,
    socket: Arc<UdpSocket>,
    pub port: u16,
}

impl Receiver {
    pub fn new() -> anyhow::Result<Self> {
        let socket = Arc::new(UdpSocket::bind(("0.0.0.0", 0))?);
        socket.set_read_timeout(Some(Duration::from_millis(20)))?;
        let port = socket.local_addr()?.port();
        let shared = Arc::new(Shared {
            name: proto::host_name(),
            ring: Mutex::new(VecDeque::with_capacity(RATE as usize)),
            target_ms: AtomicU32::new(BASE_TARGET_MS),
            volume: AtomicU32::new(1.0f32.to_bits()),
            rebuffer: AtomicBool::new(true),
            generation: AtomicU32::new(0),
            peer: Mutex::new(None),
            ack: Mutex::new(None),
            lost: AtomicU64::new(0),
            underruns: AtomicU64::new(0),
            error: Mutex::new(None),
        });
        {
            let (s, sock) = (shared.clone(), socket.clone());
            std::thread::spawn(move || net_loop(s, sock));
        }
        {
            let s = shared.clone();
            std::thread::spawn(move || output_loop(s));
        }
        Ok(Self { shared, socket, port })
    }

    pub fn name(&self) -> &str {
        &self.shared.name
    }

    pub fn connect(&self, addr: SocketAddr) {
        self.disconnect();
        *self.shared.peer.lock().unwrap() = Some(addr);
        self.shared.generation.fetch_add(1, Ordering::Relaxed);
        // Subscribe immediately instead of waiting for the next 500 ms tick.
        let _ = self.socket.send_to(&proto::build(Kind::Subscribe, 0, self.shared.name.as_bytes()), addr);
    }

    pub fn disconnect(&self) {
        if let Some(addr) = self.shared.peer.lock().unwrap().take() {
            let _ = self.socket.send_to(&proto::build(Kind::Bye, 0, &[]), addr);
        }
        *self.shared.ack.lock().unwrap() = None;
        self.shared.generation.fetch_add(1, Ordering::Relaxed);
        self.shared.ring.lock().unwrap().clear();
        self.shared.rebuffer.store(true, Ordering::Relaxed);
    }

    pub fn set_volume(&self, v: f32) {
        self.shared.volume.store(v.clamp(0.0, 1.0).to_bits(), Ordering::Relaxed);
    }

    pub fn status(&self) -> Status {
        let sh = &self.shared;
        let ack = sh.ack.lock().unwrap().clone();
        let connected = sh.peer.lock().unwrap().is_some();
        let (link, sender_name) = match (connected, ack) {
            (false, _) => (Link::Idle, String::new()),
            (true, Some((t, streaming, name))) if t.elapsed() < ACK_TIMEOUT => {
                (if streaming { Link::Playing } else { Link::Waiting }, name)
            }
            (true, Some((_, _, name))) => (Link::Searching, name),
            (true, None) => (Link::Searching, String::new()),
        };
        Status {
            link,
            sender_name,
            buffer_ms: (sh.ring.lock().unwrap().len() / CHANNELS) as u32 * 1000 / RATE,
            target_ms: sh.target_ms.load(Ordering::Relaxed),
            lost: sh.lost.load(Ordering::Relaxed),
            underruns: sh.underruns.load(Ordering::Relaxed),
            error: sh.error.lock().unwrap().clone(),
        }
    }
}

impl Drop for Receiver {
    fn drop(&mut self) {
        self.disconnect();
    }
}

fn net_loop(sh: Arc<Shared>, sock: Arc<UdpSocket>) {
    let mut dec = opus::Decoder::new(RATE, opus::Channels::Stereo).expect("opus decoder");
    let mut buf = [0u8; 2048];
    let mut pcm = vec![0f32; FRAME * CHANNELS * 6];
    let mut expected: Option<u16> = None;
    let mut seen_gen = sh.generation.load(Ordering::Relaxed);
    let mut last_sub = Instant::now() - Duration::from_secs(1);
    let mut last_adapt = Instant::now();
    let mut last_underruns = 0u64;
    let mut last_underrun_at = Instant::now();

    loop {
        let gen_now = sh.generation.load(Ordering::Relaxed);
        if gen_now != seen_gen {
            seen_gen = gen_now;
            expected = None;
        }
        let peer = *sh.peer.lock().unwrap();
        if let Some(addr) = peer {
            if last_sub.elapsed() >= Duration::from_millis(500) {
                last_sub = Instant::now();
                let _ = sock.send_to(&proto::build(Kind::Subscribe, 0, sh.name.as_bytes()), addr);
            }
        }

        // Adapt buffer depth: grow fast after glitches, shrink slowly when stable.
        if last_adapt.elapsed() >= Duration::from_secs(1) {
            last_adapt = Instant::now();
            let u = sh.underruns.load(Ordering::Relaxed);
            let t = sh.target_ms.load(Ordering::Relaxed);
            if u > last_underruns {
                sh.target_ms.store((t + 10).min(MAX_TARGET_MS), Ordering::Relaxed);
                last_underrun_at = Instant::now();
            } else if t > BASE_TARGET_MS && last_underrun_at.elapsed() > Duration::from_secs(20) {
                sh.target_ms.store(t - 5, Ordering::Relaxed);
                last_underrun_at = Instant::now();
            }
            last_underruns = u;
        }

        let Ok((n, from)) = sock.recv_from(&mut buf) else { continue };
        if peer != Some(from) {
            continue;
        }
        match proto::parse(&buf[..n]) {
            Some((Kind::Ack, _, p)) if !p.is_empty() => {
                *sh.ack.lock().unwrap() =
                    Some((Instant::now(), p[0] != 0, String::from_utf8_lossy(&p[1..]).to_string()));
            }
            Some((Kind::Audio, seq, payload)) => {
                // Audio also proves the link is alive.
                if let Some(a) = sh.ack.lock().unwrap().as_mut() {
                    a.0 = Instant::now();
                    a.1 = true;
                }
                let mut produced: Vec<f32> = Vec::new();
                match expected {
                    Some(e) => {
                        let diff = seq.wrapping_sub(e) as i16;
                        if diff < 0 && diff > -100 {
                            continue; // late or duplicate
                        }
                        if diff > 0 && diff < 100 {
                            // Conceal the gap with Opus PLC (cap so a long outage doesn't stall).
                            for _ in 0..diff.min(6) {
                                if let Ok(k) = dec.decode_float(&[], &mut pcm[..FRAME * CHANNELS], false) {
                                    produced.extend_from_slice(&pcm[..k * CHANNELS]);
                                }
                            }
                            sh.lost.fetch_add(diff as u64, Ordering::Relaxed);
                        } else if diff != 0 {
                            // Sender restarted or very long gap: resync.
                            sh.ring.lock().unwrap().clear();
                            sh.rebuffer.store(true, Ordering::Relaxed);
                        }
                    }
                    None => {}
                }
                if let Ok(k) = dec.decode_float(payload, &mut pcm, false) {
                    produced.extend_from_slice(&pcm[..k * CHANNELS]);
                }
                expected = Some(seq.wrapping_add(1));

                let target = (sh.target_ms.load(Ordering::Relaxed) as usize * RATE as usize / 1000) * CHANNELS;
                let mut ring = sh.ring.lock().unwrap();
                ring.extend(produced);
                // Too far ahead (burst, clock drift): drop the oldest audio down to target.
                let max = target + (RATE as usize / 10) * CHANNELS;
                if ring.len() > max {
                    let drop = (ring.len() - target) & !1;
                    ring.drain(..drop);
                }
            }
            _ => {}
        }
    }
}

fn output_loop(sh: Arc<Shared>) {
    let host = cpal::default_host();
    loop {
        let Some(dev) = host.default_output_device() else {
            *sh.error.lock().unwrap() = Some("Nenhuma saída de áudio no Mac.".into());
            std::thread::sleep(Duration::from_secs(1));
            continue;
        };
        let id = dev.id().map(|i| i.to_string()).unwrap_or_default();
        let broken = Arc::new(AtomicBool::new(false));
        match build_output(&dev, sh.clone(), broken.clone()) {
            Ok(stream) => {
                *sh.error.lock().unwrap() = None;
                loop {
                    std::thread::sleep(Duration::from_millis(300));
                    if broken.load(Ordering::Relaxed) {
                        break;
                    }
                    let now = host.default_output_device().and_then(|d| d.id().ok()).map(|i| i.to_string());
                    if now.is_some_and(|n| n != id) {
                        break; // user switched speakers/headphones: follow them
                    }
                }
                drop(stream);
            }
            Err(e) => {
                *sh.error.lock().unwrap() = Some(format!("Não consegui abrir a saída de áudio: {e}"));
                std::thread::sleep(Duration::from_secs(1));
            }
        }
    }
}

fn build_output(dev: &cpal::Device, sh: Arc<Shared>, broken: Arc<AtomicBool>) -> anyhow::Result<cpal::Stream> {
    let cfg = dev.default_output_config()?;
    let fmt = cfg.sample_format();
    let mut conf: cpal::StreamConfig = cfg.clone().into();
    // Ask for a small device buffer (≈5 ms) when the device allows it.
    if let cpal::SupportedBufferSize::Range { min, max } = cfg.buffer_size() {
        conf.buffer_size = cpal::BufferSize::Fixed(256u32.clamp(*min, *max));
    }
    let stream = match fmt {
        SampleFormat::F32 => out::<f32>(dev, conf.clone(), sh.clone(), broken.clone()),
        SampleFormat::I16 => out::<i16>(dev, conf.clone(), sh.clone(), broken.clone()),
        SampleFormat::I32 => out::<i32>(dev, conf.clone(), sh.clone(), broken.clone()),
        other => anyhow::bail!("formato de saída não suportado: {other}"),
    };
    let stream = match stream {
        Ok(s) => s,
        Err(_) => {
            // Fall back to the default buffer size.
            conf.buffer_size = cpal::BufferSize::Default;
            match fmt {
                SampleFormat::F32 => out::<f32>(dev, conf, sh, broken)?,
                SampleFormat::I16 => out::<i16>(dev, conf, sh, broken)?,
                _ => out::<i32>(dev, conf, sh, broken)?,
            }
        }
    };
    stream.play()?;
    Ok(stream)
}

fn out<T>(
    dev: &cpal::Device,
    conf: cpal::StreamConfig,
    sh: Arc<Shared>,
    broken: Arc<AtomicBool>,
) -> anyhow::Result<cpal::Stream>
where
    T: SizedSample + FromSample<f32>,
{
    let ch = conf.channels as usize;
    let base = RATE as f64 / conf.sample_rate as f64; // source frames per output frame
    let (mut prev, mut next) = ([0f32; 2], [0f32; 2]);
    let mut frac = 0.0f64;
    let s = dev.build_output_stream(
        conf,
        move |data: &mut [T], _| {
            let gain = f32::from_bits(sh.volume.load(Ordering::Relaxed));
            let target = (sh.target_ms.load(Ordering::Relaxed) as usize * RATE as usize / 1000).max(1);
            let mut ring = sh.ring.lock().unwrap();
            let fill = ring.len() / CHANNELS;
            let mut rebuf = sh.rebuffer.load(Ordering::Relaxed);
            if rebuf && fill >= target {
                rebuf = false;
                sh.rebuffer.store(false, Ordering::Relaxed);
            }
            // Slightly speed up / slow down playback to hold the buffer at target (absorbs clock drift).
            let err = (fill as f64 - target as f64) / target as f64;
            let step = base * (1.0 + (err * 0.002).clamp(-0.003, 0.003));
            for frame in data.chunks_mut(ch) {
                let (mut l, mut r) = (0.0f32, 0.0f32);
                if !rebuf {
                    l = prev[0] + (next[0] - prev[0]) * frac as f32;
                    r = prev[1] + (next[1] - prev[1]) * frac as f32;
                    frac += step;
                    while frac >= 1.0 {
                        frac -= 1.0;
                        prev = next;
                        match (ring.pop_front(), ring.pop_front()) {
                            (Some(a), Some(b)) => next = [a, b],
                            _ => {
                                next = [0.0; 2];
                                rebuf = true;
                                sh.rebuffer.store(true, Ordering::Relaxed);
                                sh.underruns.fetch_add(1, Ordering::Relaxed);
                                break;
                            }
                        }
                    }
                }
                l *= gain;
                r *= gain;
                match ch {
                    1 => frame[0] = T::from_sample((l + r) * 0.5),
                    _ => {
                        frame[0] = T::from_sample(l);
                        frame[1] = T::from_sample(r);
                        for x in frame.iter_mut().skip(2) {
                            *x = T::from_sample(0.0);
                        }
                    }
                }
            }
        },
        move |_e| broken.store(true, Ordering::Relaxed),
        None,
    )?;
    Ok(s)
}
