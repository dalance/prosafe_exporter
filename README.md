# prosafe_exporter
[Prometheus](https://prometheus.io) exporter for NETGEAR switches supported by ProSAFE Plus utility.

[![Build Status](https://travis-ci.org/dalance/prosafe_exporter.svg?branch=master)](https://travis-ci.org/dalance/prosafe_exporter)
[![Crates.io](https://img.shields.io/crates/v/prosafe_exporter.svg)](https://crates.io/crates/prosafe_exporter)
[![codecov](https://codecov.io/gh/dalance/prosafe_exporter/branch/master/graph/badge.svg)](https://codecov.io/gh/dalance/prosafe_exporter)

## Exported Metrics

| metric                       | description                        | labels                         |
| ---------------------------- | ---------------------------------- | ------------------------------ |
| prosafe_up                   | The last query is successful       |                                |
| prosafe_receive_bytes_total  | Incoming transfer in bytes         | port                           |
| prosafe_transmit_bytes_total | Outgoing transfer in bytes         | port                           |
| prosafe_error_packets_total  | Transfer error in packets          | port                           |
| prosafe_link_speed           | Link speed in Mbps                 | port                           |
| prosafe_build_info           | prosafe_exporter Build information | version, revision, rustversion |

## Tested Switches

- XS708E
- GS116Ev2
- GS108Ev3
- GS105Ev2
- JGS524PE

## Install
Download from [release page](https://github.com/dalance/prosafe_exporter/releases/latest), and extract to any directory ( e.g. `/usr/local/bin` ).
See the example files in `example` directory as below.

| File                             | Description                  |
| -------------------------------- | ---------------------------- |
| example/prosafe_exporter.service | systemd unit file            |


If the release build doesn't fit your environment, you can build and install from source code.

```
cargo install prosafe_exporter
```

## Usage

```
prosafe_exporter --web.listen-address=":9493"
```

The default listen port is 9493.
It can be changed by `--web.listen-address` option.

The ProSAFE switches need to have the Switch Management Mode set to "Web browser and Plus Utility" for the exporter to work correctly.

## Prometheus Server Configuration

The target switches of prosafe_exporter can be configured by the pair of hostname and network interface name ( e.g. `switch1:eth0` ).
The network interface must be belonged to the same subnet as the switch.

The Prometheus server configuration is like [SNMP exporter](https://github.com/prometheus/snmp_exporter).
The example of a configuration is below:

```yaml
- job_name: 'prosafe'
  static_configs:
      - targets: ['switch1:eth0', '192.128.0.100:enp1s0'] # target switches by hostname:if_name.
  metrics_path: /probe
  relabel_configs:
    - source_labels: [__address__]
      target_label: __param_target
    - source_labels: [__param_target]
      target_label: instance
    - target_label: __address__
      replacement: 127.0.0.1:9493 # The prosafe_exporter's real hostname:port.
```

## Query Example

Outgoing data rate of `port1` on `switch1:eth0` is below.

```
rate(prosafe_transmit_bytes_total{instance="switch1:eth0", port="1"}[1m])
```
