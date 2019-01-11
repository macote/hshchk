use std::env;
use std::path::Path;

use cancellation::{CancellationTokenSource};

use hshchk_lib::{
    HashType, hash_file::HashFile,
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
    
    let app_file_path = Path::new(&args[0]);
    let app_file_name = app_file_path.file_name().unwrap().to_str().unwrap();
    let cts = CancellationTokenSource::new();

    if args.len() == 1 {
        let mut hash_file = HashFile::new();
        hash_file.load(&args[1]);
        hash_file.save("/home/mac/Temp/hc.test");
    } else if args.len() == 2 {
        let mut file_hasher = hshchk_lib::get_file_hasher(HashType::SHA1, &args[1]);
        file_hasher.set_bytes_processed_event_handler(
            Box::new(|args| println!("processed {} bytes", args.bytes_processed)));
        file_hasher.compute(&cts);
        println!("SHA-1: {}", file_hasher.digest());
    } else if args.len() == 3 {
        let mut hfp = HashFileProcessor::new(
            HashFileProcessType::Create,
            HashType::SHA1,
            "checksum.sha1",
            &app_file_name,
            &args[1]
        );
        hfp.set_progress_event_handler(
            Box::new(|args| println!("processing {}", args.relative_file_path)));
        let result = hfp.process(&cts);
        println!("create result: {:?}", result);
    } else if args.len() == 4 {
        let mut hfp = HashFileProcessor::new(
            HashFileProcessType::Verify,
            HashType::SHA1,
            "checksum.sha1",
            &app_file_name,
            &args[1]
        );
        hfp.set_progress_event_handler(
            Box::new(|args| println!("processing {}", args.relative_file_path)));
        let result = hfp.process(&cts);
        println!("verify result: {:?}", result);
    } else {
        println!("usage: hshchk [path]");
    }
}