//! How phones find each other. The phone's own system finds them where it can (Bonjour on iOS, Android's service
//! discovery) and hands each address to `probe`; what is here covers the rest: LocalSend's multicast hello, a walk
//! through the Wi-Fi's addresses, and the address typed in by hand.

use super::receive::Events;
use super::send::{Peer, probe};
use super::{Device, MULTICAST, PORT};
use futures::StreamExt;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

/// How long one phone has to answer when a whole network is being walked.
const SCAN_WAIT: Duration = Duration::from_millis(800);

/// This phone's address on the Wi-Fi, if it has one: asked of the routing table, no packet is sent.
pub fn local_ip() -> Option<Ipv4Addr> {
    let socket = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("192.168.0.1:9").ok()?;
    match socket.local_addr().ok()?.ip() {
        std::net::IpAddr::V4(ip) if !ip.is_loopback() && !ip.is_unspecified() => Some(ip),
        _ => None,
    }
}

/// Says hello to every address next to `own`, this phone's address on the Wi-Fi (the last number 1 to 254) and tells `events` who answered.
/// Slow, so it is only for when nothing else found the other phone.
pub async fn scan(me: &Device, own: Ipv4Addr, events: Arc<dyn Events>) {
    let [a, b, c, _] = own.octets();
    futures::stream::iter(1..=254u8)
        .map(|last| Ipv4Addr::new(a, b, c, last))
        .filter(|ip| std::future::ready(*ip != own))
        .for_each_concurrent(48, |ip| {
            let (me, events) = (me.clone(), events.clone());
            async move {
                let peer = Peer {
                    ip: ip.to_string(),
                    port: PORT,
                    protocol: "https".into(),
                };
                if let Ok(device) = probe(&peer, &me, SCAN_WAIT).await
                    && device.fingerprint != me.fingerprint
                {
                    events.seen(&peer.ip, &device);
                }
            }
        })
        .await;
}

/// Says the LocalSend hello to the multicast group, then listens for the same from others and answers each with a
/// `register` call. Runs until the task is dropped. Not for iOS, which blocks multicast without an entitlement.
pub async fn listen(me: Device, events: Arc<dyn Events>) -> anyhow::Result<()> {
    let socket = tokio::net::UdpSocket::bind(("0.0.0.0", PORT)).await?;
    socket.join_multicast_v4(MULTICAST, Ipv4Addr::UNSPECIFIED)?;
    let hello = serde_json::to_vec(&Device {
        announce: Some(true),
        ..me.clone()
    })?;
    socket
        .send_to(&hello, SocketAddr::from((MULTICAST, PORT)))
        .await?;
    let mut buffer = vec![0u8; 4096];
    loop {
        let (read, from) = socket.recv_from(&mut buffer).await?;
        let Ok(device) = serde_json::from_slice::<Device>(&buffer[..read]) else {
            continue;
        };
        if device.fingerprint == me.fingerprint || device.announce != Some(true) {
            continue;
        }
        let (me, events) = (me.clone(), events.clone());
        tokio::spawn(async move {
            let peer = Peer {
                ip: from.ip().to_string(),
                port: device.port,
                protocol: device.protocol.clone(),
            };
            if let Ok(device) = probe(&peer, &me, Duration::from_secs(3)).await
                && device.fingerprint != me.fingerprint
            {
                events.seen(&peer.ip, &device);
            }
        });
    }
}
