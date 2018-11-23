use std::env;
use hshchk_lib::file_hash::{FileHasher, FileHashBytesProcessedEventArgs};
use cancellation::{CancellationTokenSource};

fn main() {
    //let mut args = env::args().skip(1).collect::<Vec<_>>();
    let mut args = env::args().collect::<Vec<_>>();

    let _xyz = match args.iter().position(|a| a == "--xyz") {
        Some(i) => {
            args.remove(i);
            false
        }
        None => true,
    };
    
    if args.len() == 1 {
        //let hash_file = hshchk_lib::hash_file::HashFile::new(args[1].clone());
        //hash_file.save("/home/mac/Temp/hc.test");
    } else if args.len() == 2 {
        let cts = CancellationTokenSource::new();
        // let mut file_hasher = hshchk_lib::file_hash::get_blake2b_file_hasher(&args[1]);
        // file_hasher.compute(&cts);
        // println!("BLAKE2B: {}", file_hasher.digest());
        // let mut file_hasher = hshchk_lib::file_hash::get_sha512_file_hasher(&args[1]);
        // file_hasher.compute(&cts);
        // println!("SHA-512: {}", file_hasher.digest());
        // let mut file_hasher = hshchk_lib::file_hash::get_blake2s_file_hasher(&args[1]);
        // file_hasher.compute(&cts);
        // println!("BLAKE2S: {}", file_hasher.digest());
        // let mut file_hasher = hshchk_lib::file_hash::get_sha256_file_hasher(&args[1]);
        // file_hasher.compute(&cts);
        // println!("SHA2-56: {}", file_hasher.digest());
        let mut file_hasher = hshchk_lib::file_hash::get_sha1_file_hasher(&args[1]);
        file_hasher.set_bytes_processed_event_handler(
            Box::new(|args: FileHashBytesProcessedEventArgs| println!("processed {} bytes", args.bytes_processed)));
        file_hasher.compute(&cts);
        println!("SHA-1: {}", file_hasher.digest());
    } else {
        println!("usage: hshchk [path]");
    }
}