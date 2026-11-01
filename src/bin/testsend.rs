//! Headless test sender: streams a stereo tone (440 Hz left, 660 Hz right) through the real
//! network + Opus path. Stands in for WASAPI capture when testing on a Mac.
use audiobridge::sender::Sender;
use std::sync::mpsc;
use std::time::{Duration, Instant};

fn main() -> anyhow::Result<()> {
    let s = Sender::new()?;
    println!("sender '{}' on UDP {}", audiobridge::proto::host_name(), s.port);
    let (tx, rx) = mpsc::channel();
    s.start_with_source(rx);
    let t0 = Instant::now();
    let mut n: u64 = 0;
    let secs: u64 = std::env::args().nth(1).and_then(|a| a.parse().ok()).unwrap_or(30);
    while t0.elapsed() < Duration::from_secs(secs) {
        // Real-time pacing, 10 ms at a time (like a capture callback).
        let due = (t0.elapsed().as_secs_f64() * 48000.0) as u64;
        while n < due {
            let t = n as f32 / 48000.0;
            let _ = tx.send(vec![
                (t * 440.0 * std::f32::consts::TAU).sin() * 0.2,
                (t * 660.0 * std::f32::consts::TAU).sin() * 0.2,
            ]);
            n += 1;
        }
        if n % 48000 < 240 {
            println!("connected to: {:?}", s.connected_to());
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    Ok(())
}
