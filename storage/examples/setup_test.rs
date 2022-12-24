use std::path::PathBuf;

use unishare_storage::*;

fn main() {
	let path = std::env::args().nth(1)
		.expect("Storage path not specified");
	let storage = FsStorage::load(PathBuf::from(path))
		.expect("Failed to load storage");
	let children = storage.read_directory(1).unwrap();
	dbg!(&children);
	let ino = storage.create_file(1, "test.txt").unwrap();
	storage.write_file(ino, 0, b"test data").unwrap();
	let children = storage.read_directory(1).unwrap();
	dbg!(&children);
}