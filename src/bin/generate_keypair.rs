
extern crate ring;
extern crate untrusted;

use std::fs::File;
use std::io::Write;
use ring::{rand, signature};

fn main() {
    let rng = rand::SystemRandom::new();
    let pkcs8_bytes = signature::Ed25519KeyPair::generate_pkcs8(&rng).unwrap();

    let mut keyfile = File::create("/tmp/pkcs8.der").unwrap();
    keyfile.write(&pkcs8_bytes[..]).unwrap();

    println!("Private key (and public key) are in PKCS#8 format in /tmp/pkcs8.der");

    let key_pair =
        signature::Ed25519KeyPair::from_pkcs8(
            untrusted::Input::from(&pkcs8_bytes)).unwrap();
    let public_key_bytes = key_pair.public_key_bytes();

    println!("Public key is:\n{:?}", &public_key_bytes);
}
