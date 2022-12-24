use std::{
	fs::{self, File},
	io::{Read, Seek, SeekFrom, Write},
	path::PathBuf, convert::TryInto,
};

use guard::guard;
use byteorder::BigEndian;
use serde::{Serialize, Deserialize};
use sled::{Tree, IVec};
use thiserror::Error;
use zerocopy::{FromBytes, AsBytes, Unaligned, U64};
use unishare_common::*;

pub struct FsStorage {
	root: PathBuf,
	sled: sled::Db,
	directory: Tree,
	metadata: Tree,
}

impl FsStorage {
	pub fn load(root: PathBuf) -> Result<Self, sled::Error> {
		fs::create_dir_all(&root)?;
		let mut sled_path = root.clone();
		sled_path.push("data.db");
		let sled = sled::open(&sled_path)?;
		let directory = sled.open_tree("directory")?;
		let metadata = sled.open_tree("metadata")?;
		
		// Initialize root directory if not already done
		directory.fetch_and_update(InoKey::from(1).as_bytes(), |data| {
			Some(data.unwrap_or(&[]).to_owned())
		})?;
		metadata.fetch_and_update(InoKey::from(1).as_bytes(), |data| {
			Some(data.map(|d| d.to_owned()).unwrap_or_else(|| {
				bincode::serialize(&Metadata {
					ino: 1,
					size: 0,
					name: String::new(),
					kind: FileType::Directory,
				}).unwrap()
			}))
		})?;
		
		let mut data_path = root.clone();
		data_path.push("data");
		fs::create_dir_all(&data_path)?;
		
		Ok(Self {
			root,
			sled,
			directory,
			metadata,
		})
	}
	
	pub fn next_ino(&self) -> sled::Result<u64> {
		let key: Option<IVec> = self.sled.fetch_and_update(b"current_ino", |opt| {
			let opt: Option<u64> = opt.map(|data| InoKey::read_from(data).unwrap().into());
			let ino = opt.unwrap_or(3);
			Some(IVec::from(InoKey::from(ino + 1).as_bytes()))
		})?;
		Ok(key.map(|data| InoKey::read_from(&*data).unwrap().into()).unwrap_or(2))
	}
	
	pub fn read_metadata(&self, ino: u64) -> sled::Result<Option<Metadata>> {
		let key = InoKey::from(ino);
		guard!(let Some(raw) = self.metadata.get(key.as_bytes())? else { return Ok(None) });
		let metadata = bincode::deserialize(&raw).unwrap();
		Ok(Some(metadata))
	}
	
	pub fn write_metadata(&self, ino: u64, metadata: &Metadata) -> sled::Result<()> {
		let key = InoKey::from(ino);
		let raw = bincode::serialize(metadata).unwrap();
		self.metadata.insert(key.as_bytes(), raw.as_bytes())?;
		Ok(())
	}
	
	pub fn read_directory(&self, ino: u64) -> sled::Result<Option<Vec<u64>>> {
		let key = InoKey::from(ino);
		guard!(let Some(raw) = self.directory.get(key.as_bytes())? else { return Ok(None) });
		let directory = parse_directory(&raw);
		Ok(Some(directory))
	}
	
	pub fn add_to_directory(&self, parent: u64, ino: u64) -> sled::Result<()> {
		let parent_key = InoKey::from(parent);
		self.directory.fetch_and_update(parent_key.as_bytes(), |dir| {
			if let Some(dir) = dir {
				let mut dir = parse_directory(dir);
				if !dir.contains(&ino) {
					dir.push(ino);
				}
				Some(IVec::from(serialize_directory(&dir)))
			} else {
				Some(IVec::from(&ino.to_be_bytes()))
			}
		})?;
		Ok(())
	}
	
	pub fn lookup(&self, parent: u64, name: &str) -> sled::Result<Option<Metadata>> {
		// TODO: implementation that is not O(<number of files in directory>)
		guard!(let Some(read_dir) = self.read_directory(parent)? else { return Ok(None) });
		for entry in read_dir {
			// TODO: metadata not found is an error? or is it a sign of a race that can be ignored...
			guard!(let Some(metadata) = self.read_metadata(entry)? else { return Ok(None) });
			if metadata.name == name {
				return Ok(Some(metadata))
			}
		}
		Ok(None)
	}

	pub fn create_file(&self, parent: u64, name: &str) -> Result<u64, StorageError> {
		let ino = self.next_ino()?;
		guard!(let Some(parent_dir) = self.read_directory(parent)? else { return Err(StorageError::DoesNotExist) });
		for i in parent_dir {
			let metadata = self.read_metadata(i)?.unwrap();
			if metadata.name == *name {
				return Err(StorageError::AlreadyExists);
			}
		}
		let storage_path = self.ino_to_storage_path(ino);
		dbg!(&storage_path);
		File::create(&storage_path).unwrap();
		
		let metadata = Metadata {
			ino,
			size: 0,
			kind: FileType::Regular,
			name: name.to_owned(),
		};
		self.write_metadata(ino, &metadata)?;
		self.add_to_directory(parent, ino)?;
		
		Ok(ino)
	}

	pub fn read_file(&self, ino: u64, start: u64, len: u64) -> sled::Result<Option<Vec<u8>>> {
		guard!(let Some(metadata) = self.read_metadata(ino)? else { return Ok(None) });
		let storage_path = self.ino_to_storage_path(ino);
		let mut file = File::open(&storage_path).unwrap();
		file.seek(SeekFrom::Start(start)).unwrap();
		let mut buf = Vec::new();
		buf.resize(len.try_into().unwrap(), 0);
		let mut read = 0;
		while read < len {
			let n = file.read(&mut buf[(read as usize)..])?;
			read += n as u64;
			if n == 0 || read >= len {
				break;
			}
		}
		Ok(Some(buf))
	}

	pub fn write_file(&self, ino: u64, start: u64, data: &[u8]) -> sled::Result<Option<()>> {
		guard!(let Some(mut metadata) = self.read_metadata(ino)? else { return Ok(None) });
		let storage_path = self.ino_to_storage_path(ino);
		let mut file = File::options().write(true).open(&storage_path).unwrap();
		file.seek(SeekFrom::Start(start)).unwrap();
		file.write_all(&data).unwrap();
		if start + data.len() as u64 > metadata.size{
			metadata.size = start + data.len() as u64;
			self.write_metadata(ino, &metadata)?;
		}
		Ok(Some(()))
	}

	pub fn delete_file(&self, path: &str) {
		unimplemented!()
	}

	pub fn ino_to_storage_path(&self, ino: u64) -> PathBuf {
		let mut full_path = self.root.clone();
		full_path.push("data");
		full_path.push(&format!("{}", ino));
		full_path
	}
}

fn path_to_storage_path(path: &str) -> String {
	base64::encode_config(path, base64::URL_SAFE)
}

#[derive(FromBytes, AsBytes, Unaligned, derive_more::Deref)]
#[repr(C)]
pub struct InoKey {
	pub ino: U64<BigEndian>,
}

impl From<u64> for InoKey {
    fn from(ino: u64) -> Self {
        Self {
			ino: ino.into(),
		}
    }
}

impl From<InoKey> for u64 {
	fn from(ino: InoKey) -> Self {
		ino.ino.get()
	}
}

fn parse_directory(data: &[u8]) -> Vec<u64> {
	data.chunks_exact(8)
		.map(|v| {
			U64::<BigEndian>::read_from(v).unwrap().get()
		})
		.collect()
}

fn serialize_directory(dir: &[u64]) -> Vec<u8> {
	dir.iter().cloned().map(|v| v.to_be_bytes()).flatten().collect()
}

#[derive(Debug, Error)]
pub enum StorageError {
	#[error("The file already exists")]
	AlreadyExists,
	#[error("The file does not exist")]
	DoesNotExist,
	#[error("An unknown internal error occurred")]
	InternalError,
	#[error(transparent)]
	Sled(#[from] sled::Error),
}
