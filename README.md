# Echonet to Home Assistant bridge

This is an App for directly connecting ECHONET Lite to Home Assisant.

## Status

This is a work-in-progress and not ready for use.

## Goals

Integrate common household equipement that uses ECHONET Lite (a common
standard in Japan smart homes) directly to Home Assistant (HA). Devices found
on the network should directly appear as HA devices. This should be similar
to Matter or ESPHome devices.

This is different from a number of the existing ECHONET Lite integrations as
it is ground-up designed as a generic ECHONET Lite platform, rather than
supporting a specific device or category of devices (e.g. Air Conditioners).

The aim is to have HA act as a HEMS, with this project acting as an ECHONET
Lite controller.

Additionally, we aim to support the Route-B USB dongles that can connect
directly to the Japan elentricity meter network to read the smart meters
that are installed widely. This eliminates the extra equipment needed to 
monitor power usage at the switch box. Also, the power meters are usually
bi-directional aware, which the external monitors are not.

## Network requirements

For ECHONET Lite on standard UDP/IP networks, port 3610 is used for all communication, both inbound and outbound. The link-local multicast addresses `ff02::1` and `224.0.23.0` are used for discovery. ECHONET Lite prefers IPv6. After discovery, direct communication between the nodes is performed. Please ensure these network addresses/ports are open.

Note that the multicast addresses are link-local, meaning, that this bridge will only work on the same network.

## Future thoughts

* Use the stack as an ESPHome component to expose ESP devices
* Use ESPHome devices as a bridge, similar to the Bluetooth proxies.

## Key documents

In the specification documents, the most relevant information is in [Part 2](#spec-part2).

* [ECHONET Lite v1.14 standards](https://echonet.jp/spec_v114_lite_en/)
  * [Table of Contents](https://echonet.jp/wp/wp-content/uploads/pdf/General/Standard/ECHONET_lite_V1_14_en/ECHONET-Lite_Ver.1.14(00)_E.pdf)
  * [Part 1 ECHONET Lite Overview](https://echonet.jp/wp/wp-content/uploads/pdf/General/Standard/ECHONET_lite_V1_14_en/ECHONET-Lite_Ver.1.14(01)_E.pdf)
  * <span id="spec-part2">[Part 2 ECHONET Lite Communications Middleware Specifications](https://echonet.jp/wp/wp-content/uploads/pdf/General/Standard/ECHONET_lite_V1_14_en/ECHONET-Lite_Ver.1.14(02)_E.pdf)</span>
  * [Part 3 ECHONET Lite Communications Equipment Specification](https://echonet.jp/wp/wp-content/uploads/pdf/General/Standard/ECHONET_lite_V1_14_en/ECHONET-Lite_Ver.1.14(03)_E.pdf)
  * [Part 4 ECHONET Lite Gateway Specification](https://echonet.jp/wp/wp-content/uploads/pdf/General/Standard/ECHONET_lite_V1_14_en/ECHONET-Lite_Ver.1.14(04)_E.pdf)
  * [Part 5 ECHONET Lite System Design Guidelines](https://echonet.jp/wp/wp-content/uploads/pdf/General/Standard/ECHONET_lite_V1_14_en/ECHONET-Lite_Ver.1.14(05)_E.pdf)
* <span id="spec-appendix">[APPENDIX Detailed Requirements for ECHONET Device Objects](https://echonet.jp/spec_object_rr4_en/)</span>

Additionally, each group of equipment has specific protocols and examples on
the ECHONET Lite specification page. However, most of the information can be
determined from the [appendix](#spec-appendix).

