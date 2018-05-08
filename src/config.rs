use errors::*;
use std::env;
use std::default::Default;
use std::path::PathBuf;
use std::fs::File;
use std::io::Read;
use std::str::FromStr;
use std::net::SocketAddr;
use toml;

#[inline] fn default_pkcs8_key_path() -> PathBuf {
    PathBuf::from("keys/pkcs8.der")
}
#[inline] fn default_num_expected_clients() -> usize { 4 }
#[inline] fn default_local_socket_addr() -> SocketAddr {
    FromStr::from_str("0.0.0.0:5555").unwrap()
}
#[inline] fn default_packet_message_buffer_size() -> usize { 64 }

#[derive(Clone, Deserialize)]
pub struct Config {
    #[serde(default = "default_pkcs8_key_path")]
    pub pkcs8_key_path: PathBuf,
    #[serde(default = "default_num_expected_clients")]
    pub num_expected_clients: usize,
    #[serde(default = "default_local_socket_addr")]
    pub local_socket_addr: SocketAddr,
    #[serde(default = "default_packet_message_buffer_size")]
    pub packet_message_buffer_size: usize,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            pkcs8_key_path: default_pkcs8_key_path(),
            num_expected_clients: default_num_expected_clients(),
            local_socket_addr: default_local_socket_addr(),
            packet_message_buffer_size: default_packet_message_buffer_size(),
        }
    }
}

impl Config {
    // Get the path to the configuration file
    fn get_path() -> PathBuf {
        // Try first argument (this is supposed to be the config file)
        let args: Vec<String> = env::args().map(|e| e.to_owned()).collect();
        if args.len() >= 2 {
            return PathBuf::from(&args[1]);
        }

        // Try environment variable
        if let Ok(p) = env::var("SIEGE_EXAMPLE_CONFIG_FILE") {
            return PathBuf::from(p);
        }

        // Otherwise, look in the current directory for eob.toml
        PathBuf::from("./siege-example-server.toml")
    }

    pub fn load() -> Result<Config>
    {
        let path = Config::get_path();

        if ! path.is_file() {
            return Err(ErrorKind::ConfigNotFound.into());
        }

        let config = Config::from_file( path )?;

        Ok(config)
    }

    pub fn from_file(path: PathBuf) -> Result<Config>
    {
        let mut contents: String = String::new();
        let mut file = File::open(&path)?;
        file.read_to_string(&mut contents)?;
        Ok(toml::from_str(&*contents)?)
    }
}
