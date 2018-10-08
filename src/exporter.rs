use failure::Error;
use hyper::rt::{self, Future};
use hyper::service::service_fn_ok;
use hyper::{Body, Response, Server, Uri};
use prometheus::{Encoder, Gauge, GaugeVec, Opts, Registry, TextEncoder};
use prosafe_switch::{Link, ProSafeSwitch};
use url::form_urlencoded;

// ---------------------------------------------------------------------------------------------------------------------
// Metrics
// ---------------------------------------------------------------------------------------------------------------------

lazy_static! {
    static ref UP_OPT: Opts = Opts::new("prosafe_up", "The last query is successful.");
    static ref RECEIVE_BYTES_OPT: Opts =
        Opts::new("prosafe_receive_bytes_total", "Incoming transfer in bytes.");
    static ref TRANSMIT_BYTES_OPT: Opts = Opts::new(
        "prosafe_transmit_bytes_total",
        "Outgoing transfer in bytes."
    );
    static ref ERROR_PACKETS_OPT: Opts =
        Opts::new("prosafe_error_packets_total", "Transfer error in packets.");
    static ref LINK_SPEED_OPT: Opts = Opts::new("prosafe_link_speed", "Link speed in Mbps.");
    static ref BUILD_INFO_OPT: Opts = Opts::new(
        "prosafe_build_info",
        "A metric with a constant '1' value labeled by version, revision and rustversion."
    );
}

// ---------------------------------------------------------------------------------------------------------------------
// Landing Page HTML
// ---------------------------------------------------------------------------------------------------------------------

static LANDING_PAGE: &'static str = r#"<html>
<head><title>ProSAFE Exporter</title></head>
<body>
<h1>ProSAFE Exporter</h1>
<form action="/probe">
<label>Target:</label> <input type="text" name="target" placeholder="1.2.3.4:eth0"><br>
<input type="submit" value="Submit">
</form>
</body>
"#;

// ---------------------------------------------------------------------------------------------------------------------
// Build info
// ---------------------------------------------------------------------------------------------------------------------

static VERSION: &'static str = env!("CARGO_PKG_VERSION");
static GIT_REVISION: Option<&'static str> = option_env!("GIT_REVISION");
static RUST_VERSION: Option<&'static str> = option_env!("RUST_VERSION");

// ---------------------------------------------------------------------------------------------------------------------
// Exporter
// ---------------------------------------------------------------------------------------------------------------------

pub struct Exporter;

impl Exporter {
    pub fn start(listen_address: &str, verbose: bool) -> Result<(), Error> {
        let addr = format!("0.0.0.0{}", listen_address).parse()?;

        if verbose {
            println!("Server started: {:?}", addr);
        }

        let service = move || {
            service_fn_ok(move |req| {
                let uri = req.uri();
                if uri.path() == "/probe" {
                    Exporter::probe(uri, verbose)
                } else {
                    Response::new(Body::from(LANDING_PAGE))
                }
            })
        };

        let server = Server::bind(&addr)
            .serve(service)
            .map_err(|e| eprintln!("Server error: {}", e));

        rt::run(server);

        Ok(())
    }

    fn probe(uri: &Uri, verbose: bool) -> Response<Body> {
        let registry = Registry::new();

        let build_info = GaugeVec::new(
            BUILD_INFO_OPT.clone(),
            &["version", "revision", "rustversion"],
        ).unwrap();

        let up = Gauge::with_opts(UP_OPT.clone()).unwrap();
        let receive_bytes = GaugeVec::new(RECEIVE_BYTES_OPT.clone(), &["port"]).unwrap();
        let transmit_bytes = GaugeVec::new(TRANSMIT_BYTES_OPT.clone(), &["port"]).unwrap();
        let error_packets = GaugeVec::new(ERROR_PACKETS_OPT.clone(), &["port"]).unwrap();
        let link_speed = GaugeVec::new(LINK_SPEED_OPT.clone(), &["port"]).unwrap();

        let _ = registry.register(Box::new(build_info.clone()));
        let _ = registry.register(Box::new(up.clone()));
        let _ = registry.register(Box::new(receive_bytes.clone()));
        let _ = registry.register(Box::new(transmit_bytes.clone()));
        let _ = registry.register(Box::new(error_packets.clone()));
        let _ = registry.register(Box::new(link_speed.clone()));

        let git_revision = GIT_REVISION.unwrap_or("");
        let rust_version = RUST_VERSION.unwrap_or("");
        build_info
            .with_label_values(&[&VERSION, &git_revision, &rust_version])
            .set(1.0);

        if let Some(query) = uri.query() {
            let mut target = None;
            let query = form_urlencoded::parse(query.as_bytes());
            for (k, v) in query {
                if k == "target" && v.contains(":") {
                    target = Some(v);
                }
            }
            if let Some(target) = target {
                let target: Vec<&str> = target.split(":").collect();

                let host = &target[0];
                let if_name = &target[1];

                if verbose {
                    println!("Access to switch: {} though {}", host, if_name);
                }
                let sw = ProSafeSwitch::new(&host, &if_name);
                match sw.port_stat() {
                    Ok(stats) => {
                        for s in stats.stats {
                            receive_bytes
                                .with_label_values(&[&format!("{}", s.port_no)])
                                .set(s.recv_bytes as f64);
                            transmit_bytes
                                .with_label_values(&[&format!("{}", s.port_no)])
                                .set(s.send_bytes as f64);
                            error_packets
                                .with_label_values(&[&format!("{}", s.port_no)])
                                .set(s.error_pkts as f64);
                        }

                        up.set(1.0);
                    }
                    Err(x) => {
                        up.set(0.0);
                        eprintln!("Fail to access: {}", x);
                    }
                }
                match sw.speed_stat() {
                    Ok(stats) => {
                        for s in stats.stats {
                            let speed = match s.link {
                                Link::None => 0,
                                Link::Speed10Mbps => 10,
                                Link::Speed100Mbps => 100,
                                Link::Speed1Gbps => 1000,
                                Link::Speed10Gbps => 10000,
                                Link::Unknown => 0,
                            };
                            link_speed
                                .with_label_values(&[&format!("{}", s.port_no)])
                                .set(speed as f64);
                        }
                    }
                    Err(x) => {
                        eprintln!("Fail to access: {}", x);
                    }
                }
            }
        }

        let metric_familys = registry.gather();
        let mut buffer = vec![];
        let encoder = TextEncoder::new();
        encoder.encode(&metric_familys, &mut buffer).unwrap();
        Response::new(Body::from(buffer))
    }
}
