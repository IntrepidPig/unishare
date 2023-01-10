use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicI64;
use std::{marker::PhantomData};

use serde::{Serialize, Deserialize};
use tokio::net::TcpStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{self, Mutex};
use futures::FutureExt;

#[cfg(test)]
mod tests;
//mod queuebuf;

#[derive(Debug)]
struct Message {
	id: i64,
	data: Vec<u8>,
}

pub struct Connection {
	/// The bidirectional underlying TcpStream
	stream: TcpStream,
	/// State shared across Sender objects
	inner: Arc<SharedState>,
	/// The buffer of inbound data that may not contain a complete message yet
	inbound: Vec<u8>,
	/// A clone of the Sender used locally to send messages to this connection. It is never used
	/// here directly, but it is still held onto to prevent the Receiver below from closing even
	/// when all Senders are dropped. This simplifies the implementation slightly.
	_outgoing_message_sender: sync::mpsc::Sender<Message>,
	/// Receives outgoing messages that shall then be sent across the network to the remote endpoint
	outgoing_messages: sync::mpsc::Receiver<Message>,
	/// Used to send incoming messages that are not replies to the Receiver
	incoming_message_sender: sync::mpsc::Sender<Message>,
}

/// State that is synchronized and shared with Sender objects, primarily to facilitate
/// dispatching replies to the proper ReplyReceiver and properly generating increasing message
/// ids.
struct SharedState {
	/// Mutexed map that relates the id's of replies that are expected to be received to a oneshot
	/// Sender that will send the message to it's awaiting endpoint once it is received.
	reply_map: std::sync::Mutex<HashMap<i64, sync::oneshot::Sender<Vec<u8>>>>,
	/// An incrementing id counter
	current_id: AtomicI64,
}

impl SharedState {
	fn next_id(&self) -> i64 {
		self.current_id.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
	}
}

impl Connection {
	pub fn new<T, U>(stream: TcpStream) -> (Self, Sender<T>, Receiver<U>) {
		let inner = Arc::new(SharedState {
			current_id: AtomicI64::from(1),
			reply_map: std::sync::Mutex::new(HashMap::new()),
		});
		
		// Channels are bounded fairly low to prevent large buildup of requests/replies
		let (incoming_message_sender, incoming_messages) = sync::mpsc::channel(4);
		let (outgoing_message_sender, outgoing_messages) = sync::mpsc::channel(4);
		let sender = Sender {
			inner: inner.clone(),
			outgoing_message_sender: outgoing_message_sender.clone(),
			_phantom: PhantomData,
		};
		let receiver = Receiver {
			incoming_messages,
			_phantom: PhantomData,
		};
		let this = Self {
			stream,
			inbound: Vec::new(),
			_outgoing_message_sender: outgoing_message_sender,
			outgoing_messages,
			incoming_message_sender,
			inner,
		};
		(this, sender, receiver)
	}
	
	pub async fn run(mut self) {
		let mut buf = [0u8; 1024];
		
		loop {
			futures::select! {
				read_res = self.stream.read(&mut buf).fuse() => {
					match read_res {
						Ok(n) => {
							if n == 0 {
								tracing::debug!("Connection closed");
								break;
							}
							
							tracing::trace!("Read {n} bytes");
							self.handle_incoming_bytes(&buf[..n]).await;
						},
						Err(_e) => {
							tracing::debug!("Connection closed");
							break;
						}
					}
				},
				message = self.outgoing_messages.recv().fuse() => {
					let message = message.expect("Channel should not be closed");
					if let Err(_e) = self.send_message(message).await {
						tracing::debug!("Connection closed");
						break;
					}
				},
			}
		}
	}
	
	async fn send_message(&mut self, message: Message) -> std::io::Result<()> {
		self.stream.write_all(&(message.data.len() as u64).to_be_bytes()).await?;
		self.stream.write_all(&message.id.to_be_bytes()).await?;
		self.stream.write_all(&message.data).await?;
		Ok(())
	}
	
	async fn handle_incoming_bytes(&mut self, data: &[u8]) {
		self.inbound.extend_from_slice(data);
		loop {
			// If at least an id and length aren't present
			if self.inbound.len() < 16 {
				return;
			}
			
			let len = u64::from_be_bytes(self.inbound[..8].try_into().unwrap());
			let id = i64::from_be_bytes(self.inbound[8..16].try_into().unwrap());
			
			if (self.inbound.len() as u64) < len + 16 {
				return;
			}
			
			// TODO: optimize
			let data = self.inbound[16..(16 + usize::try_from(len).unwrap())].to_owned();
			let rest = self.inbound[(16 + usize::try_from(len).unwrap())..].to_owned();
			self.inbound = rest;
			
			if id > 0 {
				if let Err(_e) = self.incoming_message_sender.send(Message { id, data }).await {
					tracing::debug!("Receiver was closed, incoming message ignored");
				};
			} else {
				self.handle_incoming_reply(Message { id, data }).await;
			}
		}
	}
	
	async fn handle_incoming_reply(&mut self, msg: Message) {
		if let Some(target) = self.inner.reply_map.lock().unwrap().remove(&msg.id) {
			if let Err(_) = target.send(msg.data) {
				tracing::debug!("Reply handler was dropped before receiving reply");
			}
		}
	}
}

#[derive(Clone)]
pub struct Sender<T> {
	inner: Arc<SharedState>,
	outgoing_message_sender: sync::mpsc::Sender<Message>,
	_phantom: PhantomData<T>,
}

impl<T: Serialize> Sender<T> {
	/// Send a message across the network
	pub async fn send(&self, msg: T) {
		let id = self.inner.next_id();
		let data = bincode::serialize(&msg)
			.expect("Failed to serialize value");
		if let Err(e) = self.outgoing_message_sender.send(Message { id, data }).await {
			tracing::error!("Failed to send reply: {e}")
		}
			
	}
	
	/// Reply directly to the given ReplyToken
	pub async fn reply<U: Serialize>(&self, token: ReplyToken<U>, msg: U) {
		let data = bincode::serialize(&msg)
			.expect("Failed to serialize value");
		if let Err(e) = self.outgoing_message_sender.send(Message { id: token.id, data }).await {
			tracing::error!("Failed to send reply: {e}")
		}
	}
	
	/// Create a request that expects exactly one reply from the remote endpoint The `ReplyToken`
	/// should be sent to the remote endpoint, and when the remote endpoint sends a reply that
	/// consumes that token, the result will be sent to the same `ReplyReceiver` that was created
	/// with the token.
	pub fn create_request<U: Serialize + for<'de> Deserialize<'de>>(&self) -> (ReplyToken<U>, ReplyReceiver<U>) {
		let id = -self.inner.next_id();
		let token = ReplyToken {
			id,
			_phantom: PhantomData,
		};
		let (sender, receiver) = sync::oneshot::channel();
		self.inner.reply_map.lock().unwrap().insert(id, sender);
		let receiver = ReplyReceiver {
			reply_receiver: receiver,
			_phantom: PhantomData,
		};
		(token, receiver)
	}
}

pub struct Receiver<T> {
	incoming_messages: sync::mpsc::Receiver<Message>,
	_phantom: PhantomData<T>,
}

impl<T: for<'de> Deserialize<'de>> Receiver<T> {
	pub async fn recv(&mut self) -> Option<T> {
		if let Some(msg) = self.incoming_messages.recv().await {
			match bincode::deserialize(&msg.data) {
				Ok(t) => Some(t),
				Err(_e) => {
					tracing::warn!("Failed to deserialize response");
					None
				}
			}
		} else {
			None
		}
	}
}

/// Used in conjunction with a Sender to send a reply to a specific receiver
#[derive(Debug, Serialize, Deserialize)]
pub struct ReplyToken<T> {
	id: i64,
	_phantom: PhantomData<T>,
}

/// Used to await a reply from a certain token
pub struct ReplyReceiver<T> {
	reply_receiver: sync::oneshot::Receiver<Vec<u8>>,
	_phantom: PhantomData<T>,
}

impl<T: for<'de> Deserialize<'de>> ReplyReceiver<T> {
	pub async fn recv(self) -> Option<T> {
		if let Some(data) = self.reply_receiver.await.ok() {
			if let Ok(obj) = bincode::deserialize(&data) {
				Some(obj)
			} else {
				tracing::debug!("Deserialization of reply failed");
				None
			}
		} else {
			None
		}
	}
}