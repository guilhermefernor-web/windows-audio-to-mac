//! mDNS (Bonjour) advertising and browsing. Works on a plain LAN with no internet.

use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, PartialEq)]
pub struct Peer {
    pub fullname: String,
    pub name: String,
    pub addr: SocketAddr,
}

pub struct Discovery {
    daemon: ServiceDaemon,
    pub peers: Arc<Mutex<Vec<Peer>>>,
}

impl Discovery {
    /// Advertise `advertise_type` (named `name`, on `port`) and browse for `browse_type`.
    pub fn start(
        name: &str,
        advertise_type: &str,
        port: u16,
        browse_type: &str,
    ) -> anyhow::Result<Self> {
        let daemon = ServiceDaemon::new()?;
        let host = format!("{}.local.", name.replace([' ', '.'], "-"));
        let mut props = std::collections::HashMap::new();
        props.insert("name".to_string(), name.to_string());
        let info = ServiceInfo::new(advertise_type, name, &host, "", port, props)?
            .enable_addr_auto();
        daemon.register(info)?;

        let peers: Arc<Mutex<Vec<Peer>>> = Arc::default();
        let rx = daemon.browse(browse_type)?;
        let p = peers.clone();
        std::thread::spawn(move || {
            while let Ok(ev) = rx.recv() {
                match ev {
                    ServiceEvent::ServiceResolved(info) => {
                        let name = info
                            .get_property_val_str("name")
                            .unwrap_or_else(|| info.get_fullname())
                            .to_string();
                        // Prefer IPv4 (simpler, always routable on home LANs).
                        let Some(ip) = info.get_addresses_v4().into_iter().next() else {
                            continue;
                        };
                        let peer = Peer {
                            fullname: info.get_fullname().to_string(),
                            name,
                            addr: SocketAddr::new(IpAddr::V4(ip), info.get_port()),
                        };
                        let mut g = p.lock().unwrap();
                        g.retain(|x| x.fullname != peer.fullname);
                        g.push(peer);
                    }
                    ServiceEvent::ServiceRemoved(_, full) => {
                        p.lock().unwrap().retain(|x| x.fullname != full);
                    }
                    _ => {}
                }
            }
        });
        Ok(Self { daemon, peers })
    }

    pub fn list(&self) -> Vec<Peer> {
        self.peers.lock().unwrap().clone()
    }
}

impl Drop for Discovery {
    fn drop(&mut self) {
        let _ = self.daemon.shutdown();
    }
}
