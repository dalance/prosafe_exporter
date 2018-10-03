use bincode::config;
use combine::byte::{bytes, num::be_u16, num::be_u64};
use combine::combinator::*;
use combine::Parser;
use failure::Error;
use interfaces::{HardwareAddr, Interface, Kind};
use rand;
use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::time::Duration;

// ---------------------------------------------------------------------------------------------------------------------
// QueryRequest
// ---------------------------------------------------------------------------------------------------------------------

#[repr(u32)]
enum Cmd {
    PortStat = 0x10000000,
    End = 0xffff0000,
}

#[derive(Serialize, Deserialize, Debug)]
struct QueryRequest {
    ctype: u16,
    padding1: [u8; 6],
    src_mac: [u8; 6],
    dst_mac: [u8; 6],
    padding2: [u8; 2],
    seq: u16,
    fix: [u8; 8],
    cmd: [u32; 2],
}

impl QueryRequest {
    fn new(cmd: Cmd, src_mac: &HardwareAddr, dst_mac: &HardwareAddr) -> Self {
        let mut src: [u8; 6] = Default::default();
        let mut dst: [u8; 6] = Default::default();
        src.copy_from_slice(src_mac.as_bytes());
        dst.copy_from_slice(dst_mac.as_bytes());
        QueryRequest {
            ctype: 0x0101u16,
            padding1: [0; 6],
            src_mac: src,
            dst_mac: dst,
            padding2: [0; 2],
            seq: rand::random(),
            fix: [b'N', b'S', b'D', b'P', 0, 0, 0, 0],
            cmd: [cmd as u32, Cmd::End as u32],
        }
    }

    fn encode(&self) -> Result<Vec<u8>, Error> {
        let mut config = config();
        config.big_endian();
        Ok(config.serialize(&self)?)
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// PortStats
// ---------------------------------------------------------------------------------------------------------------------

#[derive(Debug)]
pub struct PortStats {
    pub stats: Vec<PortStat>,
}

#[derive(Debug)]
pub struct PortStat {
    pub port_no: u8,
    pub recv_bytes: u64,
    pub send_bytes: u64,
    pub error_pkts: u64,
}

impl PortStats {
    fn decode(dat: &[u8]) -> Result<Self, Error> {
        let (_, rest) = bytes(&[0x01, 0x02])
            .and(be_u16())
            .and(be_u16())
            .and(skip_count(26, any()))
            .parse(dat)
            .map_err(|x| format_err!("failed to parse: {:?}", x))?;
        let mut stats = Vec::new();
        let mut buf = rest;
        while buf.len() != 0 {
            let ((cmd, len), rest) = be_u16()
                .and(be_u16())
                .parse(buf)
                .map_err(|x| format_err!("failed to parse: {:?}", x))?;
            buf = rest;

            if cmd == 0xffff {
                break;
            }

            let (dat, rest) = count::<Vec<_>, _>(len as usize, any())
                .parse(buf)
                .map_err(|x| format_err!("failed to parse: {:?}", x))?;
            buf = rest;

            let ((port_no, metrics), _rest) = any()
                .and(count::<Vec<_>, _>(6, be_u64()))
                .parse(&dat as &[u8])
                .map_err(|x| format_err!("failed to parse: {:?}", x))?;

            let stat = PortStat {
                port_no: port_no,
                recv_bytes: metrics[0],
                send_bytes: metrics[1],
                error_pkts: metrics[5],
            };
            stats.push(stat);
        }

        Ok(PortStats { stats: stats })
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// ProSafeSwitch
// ---------------------------------------------------------------------------------------------------------------------

pub struct ProSafeSwitch {
    hostname: String,
    if_name: String,
    timeout: Duration,
}

impl ProSafeSwitch {
    pub fn new(hostname: &str, if_name: &str) -> Self {
        ProSafeSwitch {
            hostname: String::from(hostname),
            if_name: String::from(if_name),
            timeout: Duration::new(1, 0),
        }
    }

    fn request(&self, cmd: Cmd) -> Result<Vec<u8>, Error> {
        let iface = Interface::get_by_name(&self.if_name)?.ok_or(format_err!(
            "failed to get network interface '{}'",
            self.if_name
        ))?;
        let mut iface_addr = None;
        for address in &iface.addresses {
            match address.kind {
                Kind::Ipv4 => iface_addr = Some(address.addr.unwrap().ip()),
                _ => (),
            }
        }
        let iface_addr = iface_addr.ok_or(format_err!(
            "failed to get IPv4 address of network interface '{}'",
            self.if_name
        ))?;

        let req = QueryRequest::new(cmd, &iface.hardware_addr()?, &HardwareAddr::zero());
        let req = req.encode()?;

        let ssocket = UdpSocket::bind(SocketAddr::new(iface_addr, 63321))?;
        let rsocket = UdpSocket::bind("255.255.255.255:63321")?;
        let _ = rsocket.set_read_timeout(Some(self.timeout));

        let sw_addr = format!("{}:{}", self.hostname, 63322)
            .to_socket_addrs()
            .unwrap()
            .next()
            .unwrap();
        ssocket.send_to(&req, sw_addr)?;

        let mut buf = [0; 1024];
        let (_len, _src_addr) = rsocket.recv_from(&mut buf)?;

        Ok(Vec::from(&buf as &[u8]))
    }

    pub fn port_stat(&self) -> Result<PortStats, Error> {
        let ret = self.request(Cmd::PortStat)?;
        Ok(PortStats::decode(&ret)?)
    }
}
