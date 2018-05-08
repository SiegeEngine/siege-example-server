
// Event handling, Network:
extern crate siege_net;
extern crate siege_example_net;
extern crate mio;

// Serialization:
#[macro_use]
extern crate serde_derive;
extern crate bincode;

// Cryptography:
extern crate rand;
extern crate ring;
extern crate untrusted;

// Logging
#[macro_use]
extern crate log;
extern crate env_logger;

// Configuration
extern crate toml;

// Data Structures
extern crate chashmap;
extern crate crossbeam;

// Time
extern crate chrono;

// Errors
#[macro_use]
extern crate error_chain;

mod errors;
mod config;
mod state;
mod network;

use std::sync::Arc;
use std::thread;
use config::Config;
use state::State;
use errors::*;

fn error_dump(e: &Error) {
    use std::io::Write;
    let stderr = &mut ::std::io::stderr();
    let errmsg = "Error writing to stderr";

    writeln!(stderr, "error: {}", e).expect(errmsg);

    for e in e.iter().skip(1) {
        writeln!(stderr, "caused by: {}", e).expect(errmsg);
    }

    if let Some(backtrace) = e.backtrace() {
        writeln!(stderr, "backtrace: {:?}", backtrace).expect(errmsg);
    }
}

fn main() {
    if let Err(ref e) = run() {
        error_dump(e);
        ::std::process::exit(1);
    }
}

fn run() -> Result<()>
{
    // Start logging
    env_logger::init().unwrap();

    // Load configuration
    let config = Config::load()?;

    // Create shared state
    let state: Arc<State> = {
        let state = state::State::new().unwrap();
        Arc::new(state)
    };

    // Setup the network
    let mut network = network::NetworkSystem::new(
        state.clone(), config.clone())?;

    // Fire off network listener in a separate thread
    let network_guard = thread::spawn(move|| {
        if let Err(ref e) = network.run() {
            error_dump(e);
        }
    });

    trace!("All systems go.");

    // Wait for the network system to end
    let _ = network_guard.join();
    trace!("Network system thread has completed.");

    Ok(())
}
