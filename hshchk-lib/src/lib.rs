use std::error::Error;
use std::fs::File;
use std::path::Path;

pub mod file_hash;
pub mod hash_file;
pub mod hash_file_process;

pub enum HashType {
	SHA1,
	SHA256,
	SHA512,
    BLAKE2B,
    BLAKE2S,
	Undefined
}

fn open_file(file_path: &str) -> File {
    let path = Path::new(file_path);
    let path_displayable = path.display();
    match File::open(&path) {
        Err(why) => panic!("couldn't open {}: {}", 
            path_displayable,
            why.description()),
        Ok(file) => file,
    }
}

fn create_file(file_path: &str) -> File {
    let path = Path::new(file_path);
    let path_displayable = path.display();
    match File::create(&path) {
        Err(why) => panic!("couldn't create {}: {}", 
            path_displayable,
            why.description()),
        Ok(file) => file,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
