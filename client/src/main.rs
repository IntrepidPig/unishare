use std::{path::PathBuf, sync::Arc, collections::HashMap, time::Duration};

use fuse::{FuseResult, FuseError};
use libc::ENOENT;
use remoc::rch::{self, oneshot};
use structopt::StructOpt;
use tokio::net::TcpStream;
use unishare_common::*;

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
		T: remoc::rtc::Serialize + for<'de> remoc::rtc::Deserialize<'de> + Send + 'static + std::fmt::Debug,
		ClientMessage: From<(R, oneshot::Sender<T>)>,
	{
		let rt = tokio::runtime::Handle::current();
		let (tx, rx) = oneshot::channel();
		let full_request = ClientMessage::from((request, tx));
		rt.block_on(async move {
			let span = tracing::span!(tracing::Level::DEBUG, "Request");
			let _guard = span.enter();
			tracing::debug!(?full_request, "Sending request");
			self.conn.tx.send(full_request).await
				.map_err(rch::base::SendError::without_item)?;
			let reply = rx.await?;
			tracing::debug!(?reply, "Got reply");
			
			Ok(reply)
		})
	}
}

/// Possible transport-level errors that can occur when sending a request
#[derive(Debug, thiserror::Error)]
pub enum RequestError {
	#[error(transparent)]
	SendError(#[from] rch::base::SendError<()>),
	#[error(transparent)]
	RecvError(#[from] rch::oneshot::RecvError),
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
}

pub struct Connection {
	tx: rch::base::Sender<ClientMessage>,
	rx: rch::base::Receiver<ServerMessage>,
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
	let cfg = remoc::Cfg::default();
	let (socket_rx, socket_tx) = stream.into_split();
	let (conn, tx, rx): (_, rch::base::Sender<ClientMessage>, rch::base::Receiver<ServerMessage>)
		= remoc::Connect::io(cfg, socket_rx, socket_tx).await?;
	tokio::spawn(conn);
	
	let conn = Connection {
		tx,
		rx,
	};
	let fuse = Fuse::new(conn);
	
	tokio::task::spawn_blocking(move || fuser::mount2(fuse::FilesystemImpl(fuse), &options.mount, &[])).await??;
	
	Ok(())
}
