//! Single instance via a loopback TCP port. A second launch asks the first one to show its window.

use std::io::Write;
use std::net::{TcpListener, TcpStream};
use std::time::Duration;

const PORT: u16 = 47801;

/// Returns true if this is the first instance (and then calls `on_show` whenever another
/// launch happens). Returns false if another instance already runs (it was told to show itself).
pub fn acquire(on_show: fn()) -> bool {
    match TcpListener::bind(("127.0.0.1", PORT)) {
        Ok(l) => {
            std::thread::spawn(move || {
                for c in l.incoming() {
                    if c.is_ok() {
                        on_show();
                    }
                }
            });
            true
        }
        Err(_) => {
            if let Ok(mut s) = TcpStream::connect_timeout(&([127, 0, 0, 1], PORT).into(), Duration::from_millis(500)) {
                let _ = s.write_all(b"show");
                false
            } else {
                true // port busy but nobody answering: don't lock the user out
            }
        }
    }
}
