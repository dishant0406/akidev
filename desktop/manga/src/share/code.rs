//! The text inside the QR code. The phone that receives shows it, the other phone scans it. It says how to reach the phone:
//! `h` and `p` are an address on a Wi-Fi both phones are on, `n` and `k` are a temporary Wi-Fi the phone made for this send (its
//! name and its password), and `b` is the name an iPhone announces for a direct link to another iPhone.

use std::net::Ipv4Addr;
use url::Url;

const START: &str = "akidev://share";

/// What a scanned code says. At least an address (`ip` with `port`) or a `peer`; a network always comes with an address.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Code {
    pub ip: Option<Ipv4Addr>,
    pub port: Option<u16>,
    /// The temporary Wi-Fi to join first: its name and its password.
    pub network: Option<(String, String)>,
    pub peer: Option<String>,
}

impl Code {
    /// The text for the QR code.
    pub fn text(&self) -> String {
        let mut url = Url::parse(START).expect("the start is a fixed, valid address");
        {
            let mut query = url.query_pairs_mut();
            if let (Some(ip), Some(port)) = (self.ip, self.port) {
                query.append_pair("h", &ip.to_string());
                query.append_pair("p", &port.to_string());
            }
            if let Some((name, password)) = &self.network {
                query.append_pair("n", name);
                query.append_pair("k", password);
            }
            if let Some(peer) = &self.peer {
                query.append_pair("b", peer);
            }
        }
        url.to_string()
    }

    /// Reads scanned text. Fails for anything that is not one of our codes, or that cannot be used.
    pub fn read(text: &str) -> Option<Code> {
        let url = Url::parse(text).ok()?;
        if url.scheme() != "akidev" || url.host_str() != Some("share") {
            return None;
        }
        let find = |name: &str| {
            url.query_pairs()
                .find(|(key, _)| key == name)
                .map(|(_, value)| value.into_owned())
        };
        let ip = match find("h") {
            Some(ip) => Some(ip.parse().ok()?),
            None => None,
        };
        let port = match find("p") {
            Some(port) => Some(port.parse().ok().filter(|port| *port != 0)?),
            None => None,
        };
        let network = match (find("n"), find("k")) {
            (Some(name), Some(password)) if !name.is_empty() && !password.is_empty() => {
                Some((name, password))
            }
            (None, None) => None,
            _ => return None,
        };
        let peer = find("b").filter(|name| !name.is_empty() && name.len() <= 64);
        // An address needs both halves, and a network is useless without one.
        if ip.is_some() != port.is_some() || (network.is_some() && ip.is_none()) {
            return None;
        }
        if ip.is_none() && peer.is_none() {
            return None;
        }
        Some(Code {
            ip,
            port,
            network,
            peer,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wifi() -> Code {
        Code {
            ip: Some(Ipv4Addr::new(192, 168, 1, 7)),
            port: Some(53317),
            ..Code::default()
        }
    }

    #[test]
    fn a_wifi_code_keeps_its_old_shape() {
        assert_eq!(wifi().text(), "akidev://share?h=192.168.1.7&p=53317");
        assert_eq!(Code::read(&wifi().text()), Some(wifi()));
    }

    #[test]
    fn a_network_with_odd_letters_comes_back_the_same() {
        let code = Code {
            network: Some(("My Wi-Fi & Co=1".into(), "p@ss word+&=%é".into())),
            ..wifi()
        };
        assert_eq!(Code::read(&code.text()), Some(code));
    }

    #[test]
    fn a_peer_alone_is_enough() {
        let code = Code {
            peer: Some("akidev-1a2b3c4d".into()),
            ..Code::default()
        };
        assert_eq!(Code::read(&code.text()), Some(code));
        let both = Code {
            peer: Some("akidev-1a2b3c4d".into()),
            ..wifi()
        };
        assert_eq!(Code::read(&both.text()), Some(both));
    }

    #[test]
    fn what_is_not_ours_or_half_made_is_refused() {
        for text in [
            "",
            "https://share?h=1.2.3.4&p=5",
            "akidev://other?h=1.2.3.4&p=5",
            "akidev://share",
            "akidev://share?h=1.2.3.4",
            "akidev://share?p=5",
            "akidev://share?h=not-an-ip&p=5",
            "akidev://share?h=1.2.3.4&p=0",
            "akidev://share?h=1.2.3.4&p=70000",
            "akidev://share?h=1.2.3.4&p=5&n=name",
            "akidev://share?h=1.2.3.4&p=5&k=secret",
            "akidev://share?n=name&k=secret&b=x",
            "akidev://share?b=",
        ] {
            assert_eq!(Code::read(text), None, "{text}");
        }
    }
}
