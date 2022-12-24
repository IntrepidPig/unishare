use serde::{Deserialize, Serialize};
use remoc::rch::oneshot::{Sender, Receiver};

use crate::storage::*;

#[derive(Debug, Serialize, Deserialize, derive_more::From)]
pub enum ServerMessage {
	
}

/// Big request enum
#[derive(Debug, Serialize, Deserialize, derive_more::From)]
pub enum ClientMessage {
	GetMetadata(GetMetadata, Sender<Result<Metadata, GetMetadataError>>),
	Lookup(Lookup, Sender<Result<Metadata, LookupError>>),
	Open(Open, Sender<Result<u64, OpenError>>),
	CreateFile(CreateFile, Sender<Result<(), ()>>),
	ReadFile(ReadFile, Sender<Result<Vec<u8>, ReadFileError>>),
	WriteFile(WriteFile, Sender<Result<u64, ()>>),
	DeleteFile(DeleteFile, Sender<Result<(), ()>>),
}

/// 
/// Individual requests and responses
/// 

#[derive(Debug, Serialize, Deserialize)]
pub struct GetMetadata {
	pub ino: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum GetMetadataError {
	NotFound,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Lookup {
	pub parent: u64,
	pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum LookupError {
	NotFound,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Open {
	pub ino: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum OpenError {
	NotFound,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateFile {
	pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadFile {
	pub ino: u64,
	pub offset: u64,
	pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReadFileError {
	NotFound,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WriteFile {
	pub path: String,
	pub start: u64,
	pub data: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteFile {
	pub path: String,
}
