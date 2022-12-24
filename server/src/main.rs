use std::{net::{SocketAddr}, sync::Arc, path::PathBuf, time::SystemTime};

use anyhow::Context;
use futures::{lock::Mutex, Future};
use structopt::StructOpt;
use tokio::net::{TcpListener, TcpStream};
use unishare_common::*;
use unishare_storage::FsStorage;
use remoc::rch::{self, oneshot};
use tracing::{error, warn, info, debug, trace};


#[derive(Debug, StructOpt)]
struct Options {
	#[structopt(short, long, default_value = "2048", help = "Port to listen on")]
	port: u16,
	root: PathBuf,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	let options = Options::from_args();
	
	tracing_subscriber::fmt()
		.with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
		.init();
	
	let listener = TcpListener::bind(SocketAddr::from(([0, 0, 0, 0], options.port))).await?;
	server(listener, &options).await
}

#[derive(Debug)]
struct State {
	storage: Mutex<FsStorage>,
}

impl State {
	pub fn new(options: &Options) -> anyhow::Result<Self> {
		let storage = FsStorage::load(options.root.clone())
			.context("Failed to open storage")?;
		Ok(Self {
			storage: Mutex::new(storage),
		})
	}
}

async fn server(listener: TcpListener, options: &Options) -> anyhow::Result<()> {
	let state = Arc::new(State::new(options)?);
	
	loop {
		let (stream, peer) = listener.accept().await?;
		let state = Arc::clone(&state);
		tokio::spawn(async move {
			match service(state, stream, peer).await {
				Ok(()) => info!("Service exited succesfully"),
				Err(e) => error!("Service failed: {}", e),
			}
		});
	}
}

#[tracing::instrument(skip(state, _peer))]
async fn service(state: Arc<State>, stream: TcpStream, _peer: SocketAddr) -> anyhow::Result<()> {
	let cfg = remoc::Cfg::default();
	let (socket_rx, socket_tx) = stream.into_split();
	let (conn, _tx, mut rx): (_, rch::base::Sender<ServerMessage>, rch::base::Receiver<ClientMessage>)
		= remoc::Connect::io(cfg, socket_rx, socket_tx).await?;
	tokio::spawn(conn);
	
	loop {
		let msg = match rx.recv().await {
			Ok(t) => t,
			Err(rch::base::RecvError::Receive(ch)) if ch.is_terminated() => {
				break;
			},
			Err(e) if e.is_final() => {
				return Err(e.into());
			},
			Err(e) => {
				warn!("Error occurred in service handler: {}", e);
				continue;
			},
		};
		
		match msg {
			Some(ClientMessage::GetMetadata(req, rep)) => handle(state.clone(), req, rep, get_metadata).await?,
			Some(ClientMessage::Lookup(req, rep)) => handle(state.clone(), req, rep, lookup).await?,
			Some(ClientMessage::Open(req, rep)) => handle(state.clone(), req, rep, open).await?,
			Some(ClientMessage::ReadFile(req, rep)) => handle(state.clone(), req, rep, read_file).await?,
			None => break,
			_ => unimplemented!()
		}
	}
	
	Ok(())
}

async fn handle<T, U, F: Future<Output=anyhow::Result<U>>>(state: Arc<State>, req: T, rep: oneshot::Sender<U>, f: fn(Arc<State>, req: T) -> F) -> anyhow::Result<()>
where
	T: std::fmt::Debug,
	U: remoc::RemoteSend + std::fmt::Debug + Send + Sync
{
	tracing::debug!(?req, "Handling request");
	let res = f(state, req).await?;
	tracing::debug!(?res, "Sending reply");
	rep.send(res)?;
	Ok(())
}

async fn get_metadata(state: Arc<State>, req: GetMetadata) -> anyhow::Result<Result<Metadata, GetMetadataError>> {
	match state.storage.lock().await.read_metadata(req.ino)? {
		Some(t) => Ok(Ok(t)),
		None => return Ok(Err(GetMetadataError::NotFound)),
	}
}

async fn lookup(state: Arc<State>, req: Lookup) -> anyhow::Result<Result<Metadata, LookupError>> {
	match state.storage.lock().await.lookup(req.parent, &req.name)? {
		Some(t) => Ok(Ok(t)),
		None => Ok(Err(LookupError::NotFound)),
	}
}

async fn open(state: Arc<State>, req: Open) -> anyhow::Result<Result<u64, OpenError>> {
	Ok(Ok(req.ino))
}

async fn read_file(state: Arc<State>, req: ReadFile) -> anyhow::Result<Result<Vec<u8>, ReadFileError>> {
	match state.storage.lock().await.read_file(req.ino, req.offset, req.size)? {
		Some(t) => Ok(Ok(t)),
		None => Ok(Err(ReadFileError::NotFound)),
	}
}