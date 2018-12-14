use std::env;

use cancellation::{CancellationTokenSource};

use hshchk_lib::{
    HashType, block_hasher::{BlockHasher}, hash_file::{HashFile},
    hash_file_process::{HashFileProcessor, HashFileProcessType}
};

fn main() {
    let mut args = env::args().collect::<Vec<_>>();

    let _xyz = match args.iter().position(|a| a == "--xyz") {
        Some(i) => {
            args.remove(i);
            true
        }
        None => false,
    };
    
    if args.len() == 1 {
        let mut hash_file = HashFile::new();
        hash_file.load(&args[1]);
        hash_file.save("/home/mac/Temp/hc.test");
    } else if args.len() == 2 {
        let cts = CancellationTokenSource::new();
        let mut file_hasher = hshchk_lib::get_sha1_file_hasher(&args[1]);
        file_hasher.set_bytes_processed_event_handler(
            Box::new(|args| println!("processed {} bytes", args.bytes_processed)));
        file_hasher.compute(&cts);
        println!("SHA-1: {}", file_hasher.digest());
    } else if args.len() == 3 {
        let hfp = HashFileProcessor::new(
            HashFileProcessType::Create,
            HashType::SHA256,
            "hash_file_name",
            "app_file_name",
            &args[1],
            None
        );
        hfp.process();
    } else {
        println!("usage: hshchk [path]");
    }
}