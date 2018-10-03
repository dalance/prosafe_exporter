use failure::Error;
use hyper::header::ContentType;
use hyper::mime::{Mime, SubLevel, TopLevel};
use hyper::server::{Request, Response, Server};
use hyper::uri::RequestUri;
use prometheus;
use prometheus::{Encoder, GaugeVec, TextEncoder};
use prosafe_switch::ProSafeSwitch;

lazy_static! {
    static ref UP: GaugeVec =
        register_gauge_vec!("prosafe_up", "The last query is successful.", &["instance"]).unwrap();
    static ref RECEIVE_BYTES: GaugeVec = register_gauge_vec!(
        "prosafe_receive_bytes_total",
        "Incoming transfer in bytes.",
        &["switch", "port"]
    ).unwrap();
    static ref TRANSMIT_BYTES: GaugeVec = register_gauge_vec!(
        "prosafe_transmit_bytes_total",
        "Outgoing transfer in bytes.",
        &["switch", "port"]
    ).unwrap();
    static ref ERROR_PACKETS: GaugeVec = register_gauge_vec!(
        "prosafe_error_packets_total",
        "Transfer error in packets.",
        &["switch", "port"]
    ).unwrap();
    static ref BUILD_INFO: GaugeVec = register_gauge_vec!(
        "prosafe_build_info",
        "A metric with a constant '1' value labeled by version, revision and rustversion",
        &["version", "revision", "rustversion"]
    ).unwrap();
}

static LANDING_PAGE: &'static str = "<html>
<head><title>ProSAFE Exporter</title></head>
<body>
<h1>ProSAFE Exporter</h1>
<p><a href=\"/metrics\">Metrics</a></p>
</body>
";

static VERSION: &'static str = env!("CARGO_PKG_VERSION");
static GIT_REVISION: Option<&'static str> = option_env!("GIT_REVISION");
static RUST_VERSION: Option<&'static str> = option_env!("RUST_VERSION");

#[derive(Debug, Deserialize)]
pub struct Config {
    pub listen_port: Option<u32>,
    pub if_name: String,
    pub switches: Vec<String>,
}

pub struct Exporter;

impl Exporter {
    pub fn start(config: Config, verbose: bool) -> Result<(), Error> {
        let encoder = TextEncoder::new();
        let addr = format!("0.0.0.0:{}", config.listen_port.unwrap_or(9493));

        if verbose {
            println!("Server started: {}", addr);
        }

        Server::http(addr)?.handle(move |req: Request, mut res: Response| {
            if req.uri == RequestUri::AbsolutePath("/metrics".to_string()) {
                for sw_hostname in &config.switches {
                    if verbose {
                        println!("Access to switch: {}", sw_hostname);
                    }
                    let sw = ProSafeSwitch::new(&sw_hostname, &config.if_name);
                    match sw.port_stat() {
                        Ok(stats) => {
                            for s in stats.stats {
                                RECEIVE_BYTES
                                    .with_label_values(&[&sw_hostname, &format!("{}", s.port_no)])
                                    .set(s.recv_bytes as f64);
                                TRANSMIT_BYTES
                                    .with_label_values(&[&sw_hostname, &format!("{}", s.port_no)])
                                    .set(s.send_bytes as f64);
                                ERROR_PACKETS
                                    .with_label_values(&[&sw_hostname, &format!("{}", s.port_no)])
                                    .set(s.error_pkts as f64);
                            }

                            UP.with_label_values(&[&sw_hostname]).set(1.0);
                        }
                        Err(x) => {
                            UP.with_label_values(&[&sw_hostname]).set(0.0);

                            if verbose {
                                println!("Fail to access: {}", x);
                            }
                        }
                    }
                }

                let git_revision = GIT_REVISION.unwrap_or("");
                let rust_version = RUST_VERSION.unwrap_or("");
                BUILD_INFO
                    .with_label_values(&[&VERSION, &git_revision, &rust_version])
                    .set(1.0);

                let metric_familys = prometheus::gather();
                let mut buffer = vec![];
                encoder.encode(&metric_familys, &mut buffer).unwrap();
                res.headers_mut()
                    .set(ContentType(encoder.format_type().parse::<Mime>().unwrap()));
                res.send(&buffer).unwrap();
            } else {
                res.headers_mut()
                    .set(ContentType(Mime(TopLevel::Text, SubLevel::Html, vec![])));
                res.send(LANDING_PAGE.as_bytes()).unwrap();
            }
        })?;

        Ok(())
    }
}
