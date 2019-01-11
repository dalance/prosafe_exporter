use bincode::config;
use failure::format_err;
use combine::byte::bytes;
use combine::byte::num::{be_u16, be_u64};
use combine::combinator::*;
use combine::{ParseError, Parser, Stream};
use failure::Error;
use interfaces2::{HardwareAddr, Interface, Kind};
use rand;
use serde_derive::{Deserialize, Serialize};
use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::time::Duration;

// ---------------------------------------------------------------------------------------------------------------------
// QueryRequest
// ---------------------------------------------------------------------------------------------------------------------

#[repr(u32)]
enum Cmd {
    PortStat = 0x10000000,
    SpeedStat = 0x0c000000,
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
// Parser
// ---------------------------------------------------------------------------------------------------------------------

struct ResponseParser;

impl ResponseParser {
    fn header<'a, I>() -> impl Parser<Input = I, Output = (u16, u16)>
    where
        I: Stream<Item = u8, Range = &'a [u8]>,
        I::Error: ParseError<I::Item, I::Range, I::Position>,
    {
        bytes(&[0x01, 0x02])
            .and(be_u16())
            .and(be_u16())
            .and(skip_count(26, any()))
            .map(|(((_a, b), c), _d)| (b, c))
    }

    fn payload_header<'a, I>() -> impl Parser<Input = I, Output = (u16, u16)>
    where
        I: Stream<Item = u8, Range = &'a [u8]>,
        I::Error: ParseError<I::Item, I::Range, I::Position>,
    {
        be_u16().and(be_u16())
    }

    fn payload_body<'a, I>(len: u16) -> impl Parser<Input = I, Output = Vec<u8>>
    where
        I: Stream<Item = u8, Range = &'a [u8]>,
        I::Error: ParseError<I::Item, I::Range, I::Position>,
    {
        count::<Vec<_>, _>(len as usize, any())
    }

    fn port_stats<'a, I>() -> impl Parser<Input = I, Output = (u8, Vec<u64>)>
    where
        I: Stream<Item = u8, Range = &'a [u8]>,
        I::Error: ParseError<I::Item, I::Range, I::Position>,
    {
        any().and(count::<Vec<_>, _>(6, be_u64()))
    }

    fn speed_stats<'a, I>() -> impl Parser<Input = I, Output = (u8, Vec<u8>)>
    where
        I: Stream<Item = u8, Range = &'a [u8]>,
        I::Error: ParseError<I::Item, I::Range, I::Position>,
    {
        any().and(count::<Vec<_>, _>(2, any()))
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// QueryResponse
// ---------------------------------------------------------------------------------------------------------------------

struct QueryResponse;

impl QueryResponse {
    fn decode(dat: &[u8]) -> Result<Vec<Vec<u8>>, Error> {
        let (_, rest) = ResponseParser::header()
            .parse(dat)
            .map_err(|x| format_err!("failed to parse: {:?}", x))?;
        let mut ret = Vec::new();
        let mut buf = rest;
        while buf.len() != 0 {
            let ((cmd, len), rest) = ResponseParser::payload_header()
                .parse(buf)
                .map_err(|x| format_err!("failed to parse: {:?}", x))?;
            buf = rest;

            let (dat, rest) = ResponseParser::payload_body(len)
                .parse(buf)
                .map_err(|x| format_err!("failed to parse: {:?}", x))?;
            buf = rest;

            if cmd == 0xffff {
                break;
            }

            ret.push(dat);
        }

        Ok(ret)
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// PortStats
// ---------------------------------------------------------------------------------------------------------------------

#[derive(Debug, PartialEq)]
pub struct PortStats {
    pub stats: Vec<PortStat>,
}

#[derive(Debug, PartialEq)]
pub struct PortStat {
    pub port_no: u8,
    pub recv_bytes: u64,
    pub send_bytes: u64,
    pub error_pkts: u64,
}

impl PortStats {
    fn decode(dat: &[u8]) -> Result<Self, Error> {
        let dat = QueryResponse::decode(dat)?;
        let mut stats = Vec::new();
        for d in dat {
            let ((port_no, metrics), _rest) = ResponseParser::port_stats()
                .parse(&d as &[u8])
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
// SpeedStats
// ---------------------------------------------------------------------------------------------------------------------

#[derive(Debug, PartialEq)]
pub struct SpeedStats {
    pub stats: Vec<SpeedStat>,
}

#[derive(Debug, PartialEq)]
pub struct SpeedStat {
    pub port_no: u8,
    pub link: Link,
}

#[derive(Debug, PartialEq)]
pub enum Link {
    None,
    Speed10Mbps,
    Speed100Mbps,
    Speed1Gbps,
    Speed10Gbps,
    Unknown,
}

impl SpeedStats {
    fn decode(dat: &[u8]) -> Result<Self, Error> {
        let dat = QueryResponse::decode(dat)?;
        let mut stats = Vec::new();
        for d in dat {
            let ((port_no, metrics), _rest) = ResponseParser::speed_stats()
                .parse(&d as &[u8])
                .map_err(|x| format_err!("failed to parse: {:?}", x))?;

            let link = match metrics[0] {
                0 => Link::None,
                1 => Link::Speed10Mbps,
                2 => Link::Speed10Mbps,
                3 => Link::Speed100Mbps,
                4 => Link::Speed100Mbps,
                5 => Link::Speed1Gbps,
                6 => Link::Speed10Gbps,
                _ => Link::Unknown,
            };

            let stat = SpeedStat {
                port_no: port_no,
                link: link,
            };
            stats.push(stat);
        }

        Ok(SpeedStats { stats: stats })
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
    #[cfg_attr(tarpaulin, skip)]
    pub fn new(hostname: &str, if_name: &str) -> Self {
        ProSafeSwitch {
            hostname: String::from(hostname),
            if_name: String::from(if_name),
            timeout: Duration::new(1, 0),
        }
    }

    #[cfg_attr(tarpaulin, skip)]
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

    #[cfg_attr(tarpaulin, skip)]
    pub fn port_stat(&self) -> Result<PortStats, Error> {
        let ret = self.request(Cmd::PortStat)?;
        Ok(PortStats::decode(&ret)?)
    }

    #[cfg_attr(tarpaulin, skip)]
    pub fn speed_stat(&self) -> Result<SpeedStats, Error> {
        let ret = self.request(Cmd::SpeedStat)?;
        Ok(SpeedStats::decode(&ret)?)
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Test
// ---------------------------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use hex_literal::*;

    #[test]
    fn test_query_encode() {
        let req = QueryRequest::new(Cmd::PortStat, &HardwareAddr::zero(), &HardwareAddr::zero());
        let dat = req.encode().unwrap();
        let expected = hex!(
            "010100000000000000000000000000000000000000000a0a4e5344500000000010000000ffff0000"
        );
        assert_eq!(dat[0..22], expected[0..22]);
        assert_eq!(dat[24..], expected[24..]);
    }

    #[test]
    fn test_port_stat_decode() {
        let dat = hex!(
            "01020000000000000cc47a3a39a808bd436a1596000000804e5344500000000010000031010000001c7e67379200000021fc85e1c40000000000000000000000000000000000000000000000000000000000000000100000310200000053ff78f7460000003581ed74c700000000000000000000000000000000000000000000000000000000000dce56100000310300000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000010000031040000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001000003105000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000100000310600000177e8658769000001cae4c262b90000000000000000000000000000000000000000000000000000000000000000100000310700000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000010000031080000000027f5c6f700000000450e67bd0000000000000000000000000000000000000000000000000000000000000000ffff0000"
        );
        let stat = PortStats::decode(&dat).unwrap();

        let expected = PortStats {
            stats: vec![
                PortStat {
                    port_no: 1,
                    recv_bytes: 122379777938,
                    send_bytes: 145970553284,
                    error_pkts: 0,
                },
                PortStat {
                    port_no: 2,
                    recv_bytes: 360768403270,
                    send_bytes: 229813089479,
                    error_pkts: 904790,
                },
                PortStat {
                    port_no: 3,
                    recv_bytes: 0,
                    send_bytes: 0,
                    error_pkts: 0,
                },
                PortStat {
                    port_no: 4,
                    recv_bytes: 0,
                    send_bytes: 0,
                    error_pkts: 0,
                },
                PortStat {
                    port_no: 5,
                    recv_bytes: 0,
                    send_bytes: 0,
                    error_pkts: 0,
                },
                PortStat {
                    port_no: 6,
                    recv_bytes: 1614511703913,
                    send_bytes: 1970932966073,
                    error_pkts: 0,
                },
                PortStat {
                    port_no: 7,
                    recv_bytes: 0,
                    send_bytes: 0,
                    error_pkts: 0,
                },
                PortStat {
                    port_no: 8,
                    recv_bytes: 670418679,
                    send_bytes: 1158571965,
                    error_pkts: 0,
                },
            ],
        };
        assert_eq!(stat, expected);
    }

    #[test]
    fn test_speed_stat_decode() {
        let dat = hex!(
            "01020000000000000cc47a3a39a828c68e6c2ebc000005ab4e534450000000000c0000030100010c0000030201010c0000030302010c0000030403010c0000030504010c0000030605010c0000030706010c000003080701ffff0000"
        );
        let stat = SpeedStats::decode(&dat).unwrap();

        let expected = SpeedStats {
            stats: vec![
                SpeedStat {
                    port_no: 1,
                    link: Link::None,
                },
                SpeedStat {
                    port_no: 2,
                    link: Link::Speed10Mbps,
                },
                SpeedStat {
                    port_no: 3,
                    link: Link::Speed10Mbps,
                },
                SpeedStat {
                    port_no: 4,
                    link: Link::Speed100Mbps,
                },
                SpeedStat {
                    port_no: 5,
                    link: Link::Speed100Mbps,
                },
                SpeedStat {
                    port_no: 6,
                    link: Link::Speed1Gbps,
                },
                SpeedStat {
                    port_no: 7,
                    link: Link::Speed10Gbps,
                },
                SpeedStat {
                    port_no: 8,
                    link: Link::Unknown,
                },
            ],
        };
        assert_eq!(stat, expected);
    }
}
