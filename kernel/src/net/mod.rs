//! Lunix OS Network Protocol Stack (Ethernet, ARP, IPv4, ICMP, UDP, TCP & Sockets)

use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU16, Ordering};
use spin::Mutex;
use crate::drivers::net::e1000;
use crate::drivers::timer;
use crate::lunix_println;

pub const ETHERTYPE_IPV4: u16 = 0x0800;
pub const ETHERTYPE_ARP: u16 = 0x0806;

pub const IP_PROTO_ICMP: u8 = 1;
pub const IP_PROTO_TCP: u8 = 6;
pub const IP_PROTO_UDP: u8 = 17;

pub const DEFAULT_IP: [u8; 4] = [10, 0, 2, 15];
pub const DEFAULT_GATEWAY: [u8; 4] = [10, 0, 2, 2];
pub const DEFAULT_NETMASK: [u8; 4] = [255, 255, 255, 0];
pub const BROADCAST_MAC: [u8; 6] = [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocketState {
    Closed,
    Listen,
    SynSent,
    Established,
    CloseWait,
}

pub struct Socket {
    pub id: usize,
    pub domain: i32,
    pub socket_type: i32,
    pub protocol: i32,
    pub local_ip: [u8; 4],
    pub local_port: u16,
    pub remote_ip: [u8; 4],
    pub remote_port: u16,
    pub state: SocketState,
    pub recv_queue: Vec<Vec<u8>>,
}

pub struct NetworkStack {
    pub ip: [u8; 4],
    pub gateway: [u8; 4],
    pub netmask: [u8; 4],
    pub arp_table: BTreeMap<[u8; 4], [u8; 6]>,
    pub sockets: BTreeMap<usize, Socket>,
    pub next_socket_id: usize,
    pub ping_replies: BTreeMap<u16, (u64, u8)>, // seq -> (timestamp_ms, ttl)
}

pub static NET_STACK: Mutex<Option<NetworkStack>> = Mutex::new(None);
static IP_IDENT: AtomicU16 = AtomicU16::new(100);

impl NetworkStack {
    pub fn new() -> Self {
        let mut arp_table = BTreeMap::new();
        // Pre-populate QEMU gateway ARP
        arp_table.insert(DEFAULT_GATEWAY, [0x52, 0x54, 0x00, 0x12, 0x34, 0x56]);

        Self {
            ip: DEFAULT_IP,
            gateway: DEFAULT_GATEWAY,
            netmask: DEFAULT_NETMASK,
            arp_table,
            sockets: BTreeMap::new(),
            next_socket_id: 1,
            ping_replies: BTreeMap::new(),
        }
    }
}

pub fn init() {
    let stack = NetworkStack::new();
    *NET_STACK.lock() = Some(stack);
    lunix_println!("[NET] Lunix TCP/IP Protocol Stack ready (IP: 10.0.2.15, Mask: 255.255.255.0, GW: 10.0.2.2).");
}

fn calculate_checksum(data: &[u8]) -> u16 {
    let mut sum: u32 = 0;
    let mut i = 0;
    while i < data.len() {
        let word = if i + 1 < data.len() {
            ((data[i] as u16) << 8) | (data[i + 1] as u16)
        } else {
            (data[i] as u16) << 8
        };
        sum += word as u32;
        i += 2;
    }
    while (sum >> 16) != 0 {
        sum = (sum & 0xFFFF) + (sum >> 16);
    }
    !(sum as u16)
}

pub fn send_ethernet_frame(dst_mac: [u8; 6], ethertype: u16, payload: &[u8]) -> Result<(), &'static str> {
    let src_mac = e1000::get_mac();
    let mut frame = Vec::with_capacity(14 + payload.len());
    frame.extend_from_slice(&dst_mac);
    frame.extend_from_slice(&src_mac);
    frame.extend_from_slice(&ethertype.to_be_bytes());
    frame.extend_from_slice(payload);

    e1000::send_packet(&frame)
}

pub fn send_arp_request(target_ip: [u8; 4]) -> Result<(), &'static str> {
    let src_mac = e1000::get_mac();
    let src_ip = DEFAULT_IP;

    let mut arp_packet = Vec::with_capacity(28);
    arp_packet.extend_from_slice(&1u16.to_be_bytes()); // Hardware Type: Ethernet (1)
    arp_packet.extend_from_slice(&0x0800u16.to_be_bytes()); // Protocol: IPv4
    arp_packet.push(6); // HW Size: 6
    arp_packet.push(4); // Proto Size: 4
    arp_packet.extend_from_slice(&1u16.to_be_bytes()); // Opcode: Request (1)
    arp_packet.extend_from_slice(&src_mac);
    arp_packet.extend_from_slice(&src_ip);
    arp_packet.extend_from_slice(&[0u8; 6]); // Target MAC (unknown)
    arp_packet.extend_from_slice(&target_ip);

    send_ethernet_frame(BROADCAST_MAC, ETHERTYPE_ARP, &arp_packet)
}

pub fn send_ipv4_packet(dst_ip: [u8; 4], protocol: u8, payload: &[u8]) -> Result<(), &'static str> {
    let src_ip = DEFAULT_IP;
    let mut dst_mac = [0x52, 0x54, 0x00, 0x12, 0x34, 0x56]; // Default gateway MAC

    {
        let lock = NET_STACK.lock();
        if let Some(ref stack) = *lock {
            if let Some(mac) = stack.arp_table.get(&dst_ip) {
                dst_mac = *mac;
            }
        }
    }

    let total_len = (20 + payload.len()) as u16;
    let ident = IP_IDENT.fetch_add(1, Ordering::Relaxed);

    let mut header = [0u8; 20];
    header[0] = 0x45; // Version 4, IHL 5 (20 bytes)
    header[1] = 0;    // DSCP / ECN
    header[2..4].copy_from_slice(&total_len.to_be_bytes());
    header[4..6].copy_from_slice(&ident.to_be_bytes());
    header[6..8].copy_from_slice(&0x4000u16.to_be_bytes()); // Don't Fragment
    header[8] = 64;   // TTL
    header[9] = protocol;
    header[10] = 0;   // Checksum placeholder
    header[11] = 0;
    header[12..16].copy_from_slice(&src_ip);
    header[16..20].copy_from_slice(&dst_ip);

    let csum = calculate_checksum(&header);
    header[10..12].copy_from_slice(&csum.to_be_bytes());

    let mut packet = Vec::with_capacity(20 + payload.len());
    packet.extend_from_slice(&header);
    packet.extend_from_slice(payload);

    send_ethernet_frame(dst_mac, ETHERTYPE_IPV4, &packet)
}

pub fn send_icmp_ping(dst_ip: [u8; 4], seq: u16) -> Result<(), &'static str> {
    let mut icmp_payload = Vec::with_capacity(64);
    icmp_payload.push(8); // Type: Echo Request (8)
    icmp_payload.push(0); // Code: 0
    icmp_payload.extend_from_slice(&0u16.to_be_bytes()); // Checksum placeholder
    icmp_payload.extend_from_slice(&0x1234u16.to_be_bytes()); // Identifier
    icmp_payload.extend_from_slice(&seq.to_be_bytes()); // Sequence Number

    // Payload: timestamp (8 bytes) + padding
    let now = timer::get_ticks();
    icmp_payload.extend_from_slice(&now.to_le_bytes());
    for i in 0..32 {
        icmp_payload.push(i as u8);
    }

    let csum = calculate_checksum(&icmp_payload);
    icmp_payload[2..4].copy_from_slice(&csum.to_be_bytes());

    send_ipv4_packet(dst_ip, IP_PROTO_ICMP, &icmp_payload)
}

pub fn poll_network() {
    while let Some(packet) = e1000::receive_packet() {
        if packet.len() < 14 {
            continue;
        }

        let ethertype = ((packet[12] as u16) << 8) | (packet[13] as u16);
        let payload = &packet[14..];

        match ethertype {
            ETHERTYPE_ARP => {
                if payload.len() >= 28 {
                    let opcode = ((payload[6] as u16) << 8) | (payload[7] as u16);
                    let mut sender_mac = [0u8; 6];
                    let mut sender_ip = [0u8; 4];
                    sender_mac.copy_from_slice(&payload[8..14]);
                    sender_ip.copy_from_slice(&payload[14..18]);

                    let mut lock = NET_STACK.lock();
                    if let Some(ref mut stack) = *lock {
                        stack.arp_table.insert(sender_ip, sender_mac);
                    }

                    // If it's an ARP request for our IP, reply!
                    if opcode == 1 && &payload[24..28] == &DEFAULT_IP {
                        let my_mac = e1000::get_mac();
                        let mut reply = Vec::with_capacity(28);
                        reply.extend_from_slice(&1u16.to_be_bytes());
                        reply.extend_from_slice(&0x0800u16.to_be_bytes());
                        reply.push(6);
                        reply.push(4);
                        reply.extend_from_slice(&2u16.to_be_bytes()); // Reply opcode
                        reply.extend_from_slice(&my_mac);
                        reply.extend_from_slice(&DEFAULT_IP);
                        reply.extend_from_slice(&sender_mac);
                        reply.extend_from_slice(&sender_ip);
                        let _ = send_ethernet_frame(sender_mac, ETHERTYPE_ARP, &reply);
                    }
                }
            }
            ETHERTYPE_IPV4 => {
                if payload.len() >= 20 {
                    let proto = payload[9];
                    let mut src_ip = [0u8; 4];
                    src_ip.copy_from_slice(&payload[12..16]);
                    let ttl = payload[8];

                    let ip_header_len = ((payload[0] & 0x0F) as usize) * 4;
                    if payload.len() >= ip_header_len {
                        let ip_payload = &payload[ip_header_len..];

                        if proto == IP_PROTO_ICMP && ip_payload.len() >= 8 {
                            let icmp_type = ip_payload[0];
                            let seq = ((ip_payload[6] as u16) << 8) | (ip_payload[7] as u16);

                            if icmp_type == 0 {
                                // Echo Reply!
                                let mut lock = NET_STACK.lock();
                                if let Some(ref mut stack) = *lock {
                                    stack.ping_replies.insert(seq, (timer::get_ticks(), ttl));
                                }
                            } else if icmp_type == 8 {
                                // Echo Request -> Send Echo Reply
                                let mut reply = ip_payload.to_vec();
                                reply[0] = 0; // Type 0 = Echo Reply
                                reply[2] = 0; // Clear checksum
                                reply[3] = 0;
                                let csum = calculate_checksum(&reply);
                                reply[2..4].copy_from_slice(&csum.to_be_bytes());
                                let _ = send_ipv4_packet(src_ip, IP_PROTO_ICMP, &reply);
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

pub fn ping_host(target_ip: [u8; 4], count: usize) {
    lunix_println!("PING {}.{}.{}.{} 56(84) bytes of data.",
        target_ip[0], target_ip[1], target_ip[2], target_ip[3]
    );

    let mut received = 0;
    let mut min_rtt = u64::MAX;
    let mut max_rtt = 0u64;
    let mut sum_rtt = 0u64;

    for seq in 1..=(count as u16) {
        let start_time = timer::get_ticks();
        let _ = send_icmp_ping(target_ip, seq);

        let mut got_reply = false;
        let mut reply_ttl = 64;
        let mut rtt = 0;

        // Poll for reply up to 1000ms (200 * 5ms intervals)
        for _ in 0..200 {
            poll_network();

            let mut lock = NET_STACK.lock();
            if let Some(ref mut stack) = *lock {
                if let Some(&(reply_time, ttl)) = stack.ping_replies.get(&seq) {
                    got_reply = true;
                    reply_ttl = ttl;
                    rtt = reply_time.saturating_sub(start_time).max(1);
                    stack.ping_replies.remove(&seq);
                    break;
                }
            }
            timer::busy_wait_ms(5);
        }

        if got_reply {
            received += 1;
            min_rtt = min_rtt.min(rtt);
            max_rtt = max_rtt.max(rtt);
            sum_rtt += rtt;
            lunix_println!("64 bytes from {}.{}.{}.{}: icmp_seq={} ttl={} time={} ms",
                target_ip[0], target_ip[1], target_ip[2], target_ip[3],
                seq, reply_ttl, rtt
            );
        } else {
            lunix_println!("Request timeout for icmp_seq {}", seq);
        }

        if seq < count as u16 {
            timer::busy_wait_ms(500);
        }
    }

    let loss = ((count - received) * 100) / count;
    lunix_println!("--- {}.{}.{}.{} ping statistics ---",
        target_ip[0], target_ip[1], target_ip[2], target_ip[3]
    );
    lunix_println!("{} packets transmitted, {} received, {}% packet loss",
        count, received, loss
    );
    if received > 0 {
        let avg_rtt = sum_rtt / (received as u64);
        lunix_println!("rtt min/avg/max = {}/{}/{} ms", min_rtt, avg_rtt, max_rtt);
    }
}
