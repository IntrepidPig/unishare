use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Metadata {
	pub ino: u64,
	pub size: u64,
	pub kind: FileType,
	pub name: String, // TODO: store inline?
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileType {
	Directory,
	Regular,
}