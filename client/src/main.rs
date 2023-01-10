use std::{path::PathBuf, sync::Arc, collections::HashMap, time::Duration};

use serde::{Serialize, Deserialize};
use fuse::{FuseResult, FuseError};
use libc::ENOENT;
use remoc::rch::{self, oneshot};
use structopt::StructOpt;
use tokio::net::TcpStream;
use unishare_common::*;
use unishare_transport::ReplyToken;
pub mod fuse;

#[derive(Debug, StructOpt)]
struct Options {
	#[structopt(help = "Remote host to connect to")]
	host: String,
	#[structopt(help = "Mountpoint for remote filesystem")]
	mount: PathBuf,
}

pub struct InoInfo {
	path: String,
}

pub struct Fuse {
	conn: Connection,
	//fhs: HashMap<u64, OpenedFile>,
}

impl Fuse {
	pub fn new(conn: Connection) -> Self {
		//let inos = HashMap::new();
		Self {
			conn,
			//inos,
		}
	}
	
	/// Synchronously send a request to the server and expect a reply of type T. This function must
	/// be called from within a tokio context.
	pub fn request<R, T>(&mut self, request: R) -> Result<T, RequestError>
	where
		T: Serialize + for<'de> Deserialize<'de> + Send + 'static + std::fmt::Debug,
		ClientMessage: From<(R, ReplyToken<T>)>,
	{
		let rt = tokio::runtime::Handle::current();
		let (token, receiver) = self.conn.tx.create_request();
		let full_request = ClientMessage::from((request, token));
		rt.block_on(async move {
			let span = tracing::span!(tracing::Level::DEBUG, "Request");
			let _guard = span.enter();
			tracing::debug!(?full_request, "Sending request");
			self.conn.tx.send(full_request).await;
			let reply = receiver.recv().await.ok_or(RequestError::NoReply)?;
			tracing::debug!(?reply, "Got reply");
			
			Ok(reply)
		})
	}
}

/// Possible transport-level errors that can occur when sending a request
#[derive(Debug, thiserror::Error)]
pub enum RequestError {
	#[error("Reply not received")]
	NoReply,
}

impl From<RequestError> for fuse::FuseError {
    fn from(_: RequestError) -> Self {
        fuse::FuseError::Transport
    }
}

impl fuse::Filesystem for Fuse {
	fn getattr(&mut self, ino: u64) -> FuseResult<fuse::Attr> {
		match self.request(GetMetadata { ino })? {
			Ok(t) => Ok(t.into()),
			Err(GetMetadataError::NotFound) => return Err(FuseError::NotFound),
		}
	}
	
	fn lookup(&mut self, parent: u64, name: &str) -> FuseResult<fuse::Attr> {
		match self.request(Lookup { parent, name: name.to_string() })? {
			Ok(t) => Ok(t.into()),
			Err(LookupError::NotFound) => return Err(FuseError::NotFound),
		}
	}
	
	fn open(&mut self, ino: u64, flags: i32) -> FuseResult<fuse::Opened> {
		match self.request(Open { ino })? {
			Ok(t) => Ok(fuse::Opened { fh: t, flags: flags.try_into().unwrap() }),
			Err(OpenError::NotFound) => return Err(FuseError::NotFound),
		}
	}
	
	fn read(&mut self, ino: u64, _fh: u64, offset: i64, size: u32) -> FuseResult<Vec<u8>> {
		let req = ReadFile { ino, offset: offset.try_into().unwrap(), size: size as u64 };
		match self.request(req)? {
			Ok(t) => Ok(t),
			Err(ReadFileError::NotFound) => Err(FuseError::NotFound),
		}
	}

	fn readdir(&mut self, ino: u64, _fh: u64, offset: i64) -> FuseResult<Vec<DirEntry>> {
		let req = ReadDir { ino, offset };
		match self.request(req)? {
			Ok(t) => Ok(t),
			Err(ReadDirError::NotFound) => Err(FuseError::NotFound),
		}
	}
}

pub struct Connection {
	tx: unishare_transport::Sender<ClientMessage>,
	rx: unishare_transport::Receiver<ServerMessage>,
}

#[tokio::main(worker_threads = 4)]
async fn main() -> anyhow::Result<()> {
	let options = Options::from_args();
	
	tracing_subscriber::fmt()
		.with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
		.init();
	
	// Setup signal handler to print debug info on receiving SIGHUP
	// TODO: setup async-backtrace framing calls
	tokio::task::spawn(async {
		let mut stream = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::hangup())
			.expect("Failed to setup SIGHUP handler");
		loop {
			stream.recv().await;
			println!("{}", async_backtrace::taskdump_tree(false));
		}
	});
	
	let stream = TcpStream::connect(options.host).await?;
	let (connection, sender, receiver) = unishare_transport::Connection::new(stream);
	tokio::spawn(connection.run());
	
	let fuse = Fuse::new(Connection { tx: sender, rx: receiver });
	
	tokio::task::spawn_blocking(move || fuser::mount2(fuse::FilesystemImpl(fuse), &options.mount, &[fuser::MountOption::Sync, fuser::MountOption::CUSTOM(String::from("debug"))])).await??;
	
	Ok(())
}
