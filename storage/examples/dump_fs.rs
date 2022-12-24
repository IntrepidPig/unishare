use std::{path::PathBuf};

use unishare_common::*;
use unishare_storage::*;

fn main() {
	let path = std::env::args().nth(1)
		.expect("Storage path not specified");
	let storage = FsStorage::load(PathBuf::from(path))
		.expect("Failed to load storage");
	print_directory(&storage, 0, 1);
}

fn print_directory(storage: &FsStorage, level: i32, ino: u64) {
	for entry in storage.read_directory(ino).unwrap().unwrap() {
		let meta = storage.read_metadata(entry).unwrap().unwrap();
		for _ in 0..level {
			print!("  ");
		}
		match meta.kind {
			FileType::Directory => {
				println!(" - {}/", meta.name);
				print_directory(storage, level + 1, meta.ino);
			},
			FileType::Regular => {
				println!(" - {}", meta.name);
			},
		}
	}
}