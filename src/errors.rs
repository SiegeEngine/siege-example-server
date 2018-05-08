
error_chain! {
    types {
        Error, ErrorKind, ResultExt, Result;
    }

    links {
        Net(::siege_net::Error, ::siege_net::ErrorKind);
    }

    foreign_links {
        Io(::std::io::Error);
        Toml(::toml::de::Error);
        Crypto(::ring::error::Unspecified);
        Bincode(::bincode::Error);
    }

    errors {
        General(s: String) {
            description("General Error"),
            display("General Error: '{}'", s),
        }
        ConfigNotFound {
            description("Configuration not found (pass as first argument)"),
        }
    }
}
