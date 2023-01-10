use serde::{Deserialize, Serialize};
use unishare_transport::{ReplyToken};

use crate::storage::*;

#[derive(Debug, Serialize, Deserialize, derive_more::From)]
pub enum ServerMessage {
	
}

/// Big request enum
#[derive(Debug, Serialize, Deserialize, derive_more::From)]
pub enum ClientMessage {
	GetMetadata(GetMetadata, ReplyToken<Result<Metadata, GetMetadataError>>),
	Lookup(Lookup, ReplyToken<Result<Metadata, LookupError>>),
	Open(Open, ReplyToken<Result<u64, OpenError>>),
	CreateFile(CreateFile, ReplyToken<Result<(), ()>>),
	ReadFile(ReadFile, ReplyToken<Result<Vec<u8>, ReadFileError>>),
	WriteFile(WriteFile, ReplyToken<Result<u64, ()>>),
	DeleteFile(DeleteFile, ReplyToken<Result<(), ()>>),
	ReadDir(ReadDir, ReplyToken<Result<Vec<DirEntry>, ReadDirError>>)
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadDir {
	pub ino: u64,
	pub offset: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReadDirError {
	NotFound,
}