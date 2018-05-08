
use errors::*;
use std::sync::Arc;
use std::net::SocketAddr;
use std::fs::File;
use std::io::Read;
use std::time::{Instant, Duration};
use bincode::deserialize;
use ring::signature::Ed25519KeyPair;
use untrusted;
use chashmap::CHashMap;
use mio::{Events, Ready, Poll, PollOpt, Token};
use mio::net::UdpSocket;
use siege_net::Remote;
use siege_net::packets::{InitPacket, InitAckPacket, UpgradeRequiredPacket,
                         HeartbeatPacket, HeartbeatAckPacket};
use siege_example_net::{GamePacket, MAGIC, VERSION};
use state::State;
use config::Config;

pub mod packet_sender;
pub use self::packet_sender::PacketSender;

const INBOUND_READY: Token = Token(0);
const OUTBOUND_READY: Token = Token(1);

pub struct NetworkSystem {
    #[allow(dead_code)]
    config: Config,
    state: Arc<State>,
    remotes: CHashMap<SocketAddr, Remote>,
    key_pair: Ed25519KeyPair,
    socket: UdpSocket,
}

impl NetworkSystem {
    pub fn new(state: Arc<State>, config: Config)
               -> Result<NetworkSystem>
    {
        let key_pair: Ed25519KeyPair = {
            let mut pkcs8bytes = Vec::new();
            let mut pkcs8_file = File::open(&config.pkcs8_key_path)?;
            pkcs8_file.read_to_end(&mut pkcs8bytes)?;

            Ed25519KeyPair::from_pkcs8(untrusted::Input::from(&pkcs8bytes[..]))?
        };

        let remotes = CHashMap::with_capacity(config.num_expected_clients);

        // Bind a UDP socket
        let socket = UdpSocket::bind(&config.local_socket_addr)?;
        trace!("local socket bound.");

        let ns = NetworkSystem {
            config: config,
            state: state,
            remotes: remotes,
            key_pair: key_pair,
            socket: socket,
        };

        Ok(ns)
    }

    pub fn run(&mut self) -> Result<()>
    {
        // Setup the mio system for polling
        let poll = Poll::new()?;
        poll.register(&self.socket, INBOUND_READY, Ready::readable(), PollOpt::edge())?;
        poll.register(&self.state.packet_sender, OUTBOUND_READY,
                      Ready::readable(), PollOpt::edge())?;

        let mut events = Events::with_capacity(256);
        let mut buffer: [u8; 2000] = [0; 2000];
        loop {
            poll.poll(&mut events, None)?;
            for event in events.iter() {
                match event.token() {
                    INBOUND_READY => loop {
                        let (len,addr) = match self.socket.recv_from(&mut buffer) {
                            Err(e) => {
                                if e.kind() == ::std::io::ErrorKind::WouldBlock {
                                    break; // we have handled all events
                                }
                                return Err(From::from(e));
                            },
                            Ok(stuff) => stuff
                        };
                        if let Err(e) = self.handle_incoming_packet(&mut buffer[..len], addr) {
                            error!("{}", e);
                        }
                    },
                    OUTBOUND_READY => loop {
                        if let Some((packet,addr,irt)) = self.state.packet_sender.outbound.try_pop() {
                            if let Err(e) = self.handle_outgoing_packet(packet, addr, irt) {
                                error!("{}", e);
                            }
                        } else {
                            break; // we have handled all outbound messages
                        }
                    },
                    _ => unreachable!()
                }
            }
        }
    }

    fn handle_outgoing_packet(&self, packet: GamePacket, addr: SocketAddr,
                              in_reply_to: Option<u32>) -> Result<()>
    {
        let packet_bytes = {
            let mut guard = self.remotes.get_mut(&addr).ok_or(
                Error::from_kind(ErrorKind::General(
                    "Unknown Packet Recipient".to_owned()))
            )?;
            use std::ops::DerefMut;
            let remote: &mut Remote = guard.deref_mut();
            match in_reply_to {
                Some(seq) => remote.serialize_reply_packet(&packet, MAGIC, VERSION, seq)?,
                None => remote.serialize_packet(&packet, MAGIC, VERSION)?,
            }
        };
        let _len = self.socket.send_to(&packet_bytes, &addr)?;
        Ok(())
    }

    fn handle_incoming_packet(&self, bytes: &mut [u8], addr: SocketAddr) -> Result<()>
    {
        trace!("handling incoming packet.");

        // We check magic and version here to discard wayward packets early.
        if ! ::siege_net::packets::validate_magic_and_version(MAGIC, VERSION, &bytes)? {
            // Packet has old version. Reply with UpgradeRequired
            let packet = GamePacket::UpgradeRequired(UpgradeRequiredPacket::new(VERSION));
            self.state.packet_sender.send(packet, addr, None)?;
            return Ok(());
        }

        let (body_bytes, in_seq, _stale) = {
            // Look up the remote (or create one)
            let mut guard = {
                if !self.remotes.contains_key(&addr) {
                    // Create a new remote for this addr.
                    let _ = self.remotes.insert(
                        addr,
                        Remote::new(addr, self.state.rng.clone())?);
                }
                // NOTE: this may block, hopefully briefly (lock aquisition)
                self.remotes.get_mut(&addr)
                    .ok_or(Error::from_kind(ErrorKind::General(
                        "Failed to get remote from CHashMap".to_owned())))?
            };
            use std::ops::DerefMut;
            let remote: &mut Remote = guard.deref_mut();
            remote.deserialize_packet_header::<GamePacket>(&mut bytes[..])?
        };
        let packet: GamePacket = deserialize(body_bytes)?;

        // Print the packet
        debug!("PACKET RECEIVED: [{}] {:?}", in_seq, packet);

        // Handle the packet
        match packet {
            GamePacket::Init(init) => self.handle_init(in_seq, init, addr)?,
            GamePacket::InitAck(_) => (),
            GamePacket::UpgradeRequired(_) => (), // invalid from clients
            GamePacket::Heartbeat(_) => self.handle_heartbeat(in_seq, addr)?,
            GamePacket::HeartbeatAck(_) => (),
            GamePacket::Shutdown(_) => (),
            GamePacket::ShutdownComplete(_) => (),
            GamePacket::Login(_) => (),
            GamePacket::LoginSuccess(_) => (), // invalid from clients
            GamePacket::LoginFailure(_) => (), // invalid from clients
        };

        Ok(())
    }

    fn handle_init(&self, in_seq: u32, init: InitPacket, addr: SocketAddr)
                   -> Result<()>
    {
        // Sign the nonce
        let nonce_response: Vec<u8> = self.key_pair.sign(&init.nonce).as_ref().to_owned();

        let mut guard = {
            self.remotes.get_mut(&addr)
                .ok_or(Error::from_kind(ErrorKind::General(
                    "Failed to get remote from CHashMap".to_owned())))?
        };
        use std::ops::DerefMut;
        let remote: &mut Remote = guard.deref_mut();

        // Respond with InitAck
        {
            // Build response (while session key is still zeroed, and ephemeral key is still unused)
            let init_ack_packet = try!(InitAckPacket::new(remote, &*nonce_response));
            let packet = GamePacket::InitAck(init_ack_packet);

            // We must respond NOW, before computing the session key, while our session key is
            // still zeroed. So we can't push this packet onto the packet_sender, we have to
            // handle it directly, here and now.
            {
                let packet_bytes = remote.serialize_reply_packet(&packet, MAGIC, VERSION, in_seq)?;
                let _len = self.socket.send_to(&packet_bytes, &addr)?;
            }

            // Compute session key
            try!(remote.compute_session_key(&init.public_key));
            trace!("InitAck to {}, Session key is {:?}", addr, remote.session_key);
        }

        Ok(())
    }

    fn handle_heartbeat(&self, in_seq: u32, addr: SocketAddr) -> Result<()>
    {
        // Send back a HeartbeatAck immediately
        let packet = GamePacket::HeartbeatAck(HeartbeatAckPacket::new());
        self.state.packet_sender.send(packet, addr, Some(in_seq))?;

        // Also send back a heartbeat after 10 seconds (this continues the heartbeat chain)
        self.state.packet_sender.send_at_future_time(
            GamePacket::Heartbeat(HeartbeatPacket::new()), addr, None,
            Instant::now() + Duration::new(10,0)
        )?;

        Ok(())
    }
}
