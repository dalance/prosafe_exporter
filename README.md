# prosafe_exporter
[Prometheus](https://prometheus.io) exporter for NETGEAR switches supported by ProSAFE Plus utility.

[![Build Status](https://travis-ci.org/dalance/prosafe_exporter.svg?branch=master)](https://travis-ci.org/dalance/prosafe_exporter)
[![Crates.io](https://img.shields.io/crates/v/prosafe_exporter.svg)](https://crates.io/crates/prosafe_exporter)

## Exported Metrics

| metric                       | description                        | labels                         |
| ---------------------------- | ---------------------------------- | ------------------------------ |
| prosafe_up                   | The last query is successful       | switch                         |
| prosafe_receive_bytes_total  | Incoming transfer in bytes         | switch, port                   |
| prosafe_transmit_bytes_total | Outgoing transfer in bytes         | switch, port                   |
| prosafe_error_packets_total  | Transfer error in packets          | switch, port                   |
| prosafe_build_info           | prosafe_exporter Build information | version, revision, rustversion |

## Tested Switches

- XS708E
- GS108Ev3
- GS105Ev2

## Query Example

Outgoing data rate of `switch1:port1` is below.

```
rate(prosafe_transmit_bytes_total{switch="switch1", port="1"}[1m])
```

## Install
Download from [release page](https://github.com/dalance/prosafe_exporter/releases/latest), and extract to any directory ( e.g. `/usr/local/bin` ).
See the example files in `example` directory as below.

| File                             | Description                  |
| -------------------------------- | ---------------------------- |
| example/prosafe_exporter.service | systemd unit file            |
| example/config.toml              | prosafe_exporter config file |


If the release build doesn't fit your environment, you can build and install from source code.

```
cargo install prosafe_exporter
```

## Usage

```
prosafe_exporter --path.config [config_file]
```

The format of `config_file` is below.

```
scrape_interval = 15                          # interval of scraping by second ( default: 15s )
listen_port     = 9493                        # listen_port of expoter ( 9493 is the default port of prosafe_exporter )
if_name         = "eno1"                      # network interface name to access switches ( ex. eno1, eth0,,, )
switches        = ["switch1", "192.168.0.10"] # hostname or address of switches
```
