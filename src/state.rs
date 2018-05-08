
use std::sync::Arc;
use ring::rand::SystemRandom;
use errors::*;
use network::PacketSender;

pub struct State {
    pub rng: Arc<SystemRandom>,
    pub packet_sender: PacketSender,
}

impl State {
    pub fn new() -> Result<State>
    {
        Ok(State {
            rng: Arc::new(SystemRandom::new()),
            packet_sender: PacketSender::new(),
        })
    }
}
