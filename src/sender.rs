//! Windows side: capture system audio (WASAPI loopback), encode with Opus, send over UDP.

use crate::convert::ToStereo48k;
use crate::discovery::Discovery;
use crate::proto::{self, Kind, CHANNELS, FRAME, RATE};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, SizedSample};
use std::net::{SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const SUB_TIMEOUT: Duration = Duration::from_secs(3);

struct Sub {
    addr: SocketAddr,
    name: String,
    last: Instant,
}

struct Shared {
    socket: UdpSocket,
    name: String,
    sub: Mutex<Option<Sub>>,
    streaming: AtomicBool,
    error: Mutex<Option<String>>,
}

pub struct Sender {
    shared: Arc<Shared>,
    discovery: Option<Discovery>,
    stop: Mutex<Option<Arc<AtomicBool>>>,
    pub port: u16,
}

impl Sender {
    pub fn new() -> anyhow::Result<Self> {
        let name = proto::host_name();
        let socket = UdpSocket::bind(("0.0.0.0", proto::DEFAULT_PORT))
            .or_else(|_| UdpSocket::bind(("0.0.0.0", 0)))?;
        let port = socket.local_addr()?.port();
        socket.set_read_timeout(Some(Duration::from_millis(200)))?;
        let shared = Arc::new(Shared {
            socket,
            name: name.clone(),
            sub: Mutex::new(None),
            streaming: AtomicBool::new(false),
            error: Mutex::new(None),
        });
        let s = shared.clone();
        std::thread::spawn(move || control_loop(s));
        let discovery = Discovery::start(&name, proto::SERVICE_SRC, port, proto::SERVICE_RX).ok();
        Ok(Self { shared, discovery, stop: Mutex::new(None), port })
    }

    /// Name of the Mac currently connected, if any.
    pub fn connected_to(&self) -> Option<String> {
        let g = self.shared.sub.lock().unwrap();
        g.as_ref().filter(|s| s.last.elapsed() < SUB_TIMEOUT).map(|s| s.name.clone())
    }

    /// Names of receivers visible on the network (even if not connected yet).
    pub fn receivers_found(&self) -> Vec<String> {
        self.discovery.as_ref().map(|d| d.list().into_iter().map(|p| p.name).collect()).unwrap_or_default()
    }

    pub fn is_streaming(&self) -> bool {
        self.shared.streaming.load(Ordering::Relaxed)
    }

    pub fn error(&self) -> Option<String> {
        self.shared.error.lock().unwrap().clone()
    }

    pub fn start(&self) {
        let mut g = self.stop.lock().unwrap();
        if g.is_some() {
            return;
        }
        let stop = Arc::new(AtomicBool::new(false));
        *g = Some(stop.clone());
        self.shared.streaming.store(true, Ordering::Relaxed);
        *self.shared.error.lock().unwrap() = None;
        let (tx, rx) = mpsc::channel::<Vec<f32>>();
        let sh = self.shared.clone();
        std::thread::spawn(move || encoder_loop(sh, rx));
        let sh = self.shared.clone();
        std::thread::spawn(move || capture_loop(sh, tx, stop));
    }

    pub fn stop(&self) {
        if let Some(s) = self.stop.lock().unwrap().take() {
            s.store(true, Ordering::Relaxed);
        }
        self.shared.streaming.store(false, Ordering::Relaxed);
    }

    /// Feed already-captured interleaved PCM (used by the test tool instead of WASAPI).
    pub fn start_with_source(&self, rx: mpsc::Receiver<Vec<f32>>) {
        self.shared.streaming.store(true, Ordering::Relaxed);
        let sh = self.shared.clone();
        std::thread::spawn(move || encoder_loop(sh, rx));
    }
}

impl Drop for Sender {
    fn drop(&mut self) {
        self.stop();
    }
}

fn control_loop(sh: Arc<Shared>) {
    let mut buf = [0u8; 1500];
    loop {
        let Ok((n, from)) = sh.socket.recv_from(&mut buf) else { continue };
        match proto::parse(&buf[..n]) {
            Some((Kind::Subscribe, _, payload)) => {
                let name = String::from_utf8_lossy(payload).to_string();
                *sh.sub.lock().unwrap() = Some(Sub { addr: from, name, last: Instant::now() });
                let mut p = vec![sh.streaming.load(Ordering::Relaxed) as u8];
                p.extend_from_slice(sh.name.as_bytes());
                let _ = sh.socket.send_to(&proto::build(Kind::Ack, 0, &p), from);
            }
            Some((Kind::Bye, _, _)) => {
                let mut g = sh.sub.lock().unwrap();
                if g.as_ref().is_some_and(|s| s.addr == from) {
                    *g = None;
                }
            }
            _ => {}
        }
    }
}

fn encoder_loop(sh: Arc<Shared>, rx: mpsc::Receiver<Vec<f32>>) {
    let mut enc = match opus::Encoder::new(RATE, opus::Channels::Stereo, opus::Application::LowDelay) {
        Ok(e) => e,
        Err(e) => {
            *sh.error.lock().unwrap() = Some(format!("Opus: {e}"));
            return;
        }
    };
    let _ = enc.set_bitrate(opus::Bitrate::Bits(160_000));
    let _ = enc.set_vbr(false);
    let _ = enc.set_signal(opus::Signal::Music);
    let _ = enc.set_complexity(7);

    let mut pending: Vec<f32> = Vec::with_capacity(FRAME * CHANNELS * 4);
    let mut out = [0u8; 1400];
    let mut seq: u16 = 0;
    // Exits when the capture side drops its Sender.
    while let Ok(chunk) = rx.recv() {
        pending.extend_from_slice(&chunk);
        let step = FRAME * CHANNELS;
        let mut off = 0;
        while pending.len() - off >= step {
            let frame = &pending[off..off + step];
            off += step;
            let target = {
                let g = sh.sub.lock().unwrap();
                g.as_ref().filter(|s| s.last.elapsed() < SUB_TIMEOUT).map(|s| s.addr)
            };
            let Some(addr) = target else { continue };
            if let Ok(n) = enc.encode_float(frame, &mut out) {
                let _ = sh.socket.send_to(&proto::build(Kind::Audio, seq, &out[..n]), addr);
                seq = seq.wrapping_add(1);
            }
        }
        pending.drain(..off);
        // Never let a stalled consumer build latency.
        if pending.len() > RATE as usize * CHANNELS / 4 {
            pending.clear();
        }
    }
}

fn capture_loop(sh: Arc<Shared>, tx: mpsc::Sender<Vec<f32>>, stop: Arc<AtomicBool>) {
    let host = cpal::default_host();
    while !stop.load(Ordering::Relaxed) {
        let Some(dev) = host.default_output_device() else {
            *sh.error.lock().unwrap() = Some("Nenhuma saída de áudio encontrada no Windows.".into());
            std::thread::sleep(Duration::from_secs(1));
            continue;
        };
        let dev_id = dev.id().map(|i| i.to_string()).unwrap_or_default();
        let broken = Arc::new(AtomicBool::new(false));
        match build_loopback(&dev, tx.clone(), broken.clone()) {
            Ok(stream) => {
                *sh.error.lock().unwrap() = None;
                // Keep the stream alive until stopped, the stream breaks, or the default device changes.
                while !stop.load(Ordering::Relaxed) && !broken.load(Ordering::Relaxed) {
                    std::thread::sleep(Duration::from_millis(250));
                    let now = host.default_output_device().and_then(|d| d.id().ok()).map(|i| i.to_string());
                    if now.is_some_and(|n| n != dev_id) {
                        break;
                    }
                }
                drop(stream);
            }
            Err(e) => {
                *sh.error.lock().unwrap() = Some(format!("Não consegui capturar o áudio: {e}"));
                std::thread::sleep(Duration::from_secs(1));
            }
        }
    }
    // dropping `tx` here ends the encoder thread
}

fn build_loopback(
    dev: &cpal::Device,
    tx: mpsc::Sender<Vec<f32>>,
    broken: Arc<AtomicBool>,
) -> anyhow::Result<cpal::Stream> {
    // On WASAPI, opening an *output* device as input enables loopback (captures everything it plays).
    let cfg = dev.default_output_config()?;
    let fmt = cfg.sample_format();
    let channels = cfg.channels() as usize;
    let rate = cfg.sample_rate();
    let conf: cpal::StreamConfig = cfg.into();
    let stream = match fmt {
        SampleFormat::F32 => input::<f32>(dev, conf, rate, channels, tx, broken)?,
        SampleFormat::I16 => input::<i16>(dev, conf, rate, channels, tx, broken)?,
        SampleFormat::I32 => input::<i32>(dev, conf, rate, channels, tx, broken)?,
        other => anyhow::bail!("formato de áudio não suportado: {other}"),
    };
    stream.play()?;
    Ok(stream)
}

fn input<T>(
    dev: &cpal::Device,
    conf: cpal::StreamConfig,
    rate: u32,
    channels: usize,
    tx: mpsc::Sender<Vec<f32>>,
    broken: Arc<AtomicBool>,
) -> anyhow::Result<cpal::Stream>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    let mut conv = ToStereo48k::new(rate, channels);
    let mut tmp: Vec<f32> = Vec::new();
    let s = dev.build_input_stream(
        conf,
        move |data: &[T], _| {
            tmp.clear();
            tmp.extend(data.iter().map(|&s| f32::from_sample(s)));
            let mut out = Vec::with_capacity(data.len() * 2);
            conv.process(&tmp, &mut out);
            let _ = tx.send(out);
        },
        move |_e| broken.store(true, Ordering::Relaxed),
        None,
    )?;
    Ok(s)
}
