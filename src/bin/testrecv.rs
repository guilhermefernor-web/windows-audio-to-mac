//! Headless receiver for tests: auto-connects to the first sender found and prints stats.
use audiobridge::{discovery::Discovery, proto, receiver::Receiver};
use std::time::Duration;

fn main() -> anyhow::Result<()> {
    let r = Receiver::new()?;
    let d = Discovery::start(r.name(), proto::SERVICE_RX, r.port, proto::SERVICE_SRC)?;
    let secs: u64 = std::env::args().nth(1).and_then(|a| a.parse().ok()).unwrap_or(15);
    let mut connected = false;
    for _ in 0..secs {
        if !connected {
            if let Some(p) = d.list().first() {
                println!("found '{}' at {}", p.name, p.addr);
                r.connect(p.addr);
                connected = true;
            }
        }
        let s = r.status();
        println!("{:?} from '{}' buf={}ms target={}ms lost={} underruns={} err={:?}",
            s.link, s.sender_name, s.buffer_ms, s.target_ms, s.lost, s.underruns, s.error);
        std::thread::sleep(Duration::from_secs(1));
    }
    Ok(())
}
