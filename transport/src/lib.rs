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

#[derive(Debug)]
pub struct Message {
	id: i64,
	data: Vec<u8>,
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

pub struct Connection {
	stream: TcpStream,
	inbound: Vec<u8>,/* 
	/// Used to send incoming replies to the connection to be distributed to their handlers
	_incoming_reply_sender: sync::mpsc::Sender<Message>,
	/// Receives replies from copies of the above sender, then the connection will distribute them
	incoming_replies: sync::mpsc::Receiver<Message>, */
	/* /// Used from local sender to send replies to connection to be sent to remote
	_outgoing_reply_sender: sync::mpsc::Sender<Message>,
	/// Receives local outgoing replies to be sent across stream
	outgoing_replies: sync::mpsc::Receiver<Message>, */
	// This is not necessary to keep here? Except to prevent the channel from closing maybe...
	_outgoing_message_sender: sync::mpsc::Sender<Message>,
	/// Receive outgoing messages from the above sender, then the connection sends them off
	outgoing_messages: sync::mpsc::Receiver<Message>,
	/// Used to send incoming messages that are not replies to the receiver
	incoming_message_sender: sync::mpsc::Sender<Message>,
	inner: Arc<Conn>,
}

struct Conn {
	reply_map: std::sync::Mutex<HashMap<i64, sync::oneshot::Sender<Vec<u8>>>>,
	current_id: AtomicI64,
}

impl Conn {
	fn next_id(&self) -> i64 {
		self.current_id.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
	}
}

impl Connection {
	pub fn new<T, U>(stream: TcpStream) -> (Self, Sender<T>, Receiver<U>) {
		let inner = Arc::new(Conn {
			current_id: AtomicI64::from(1),
			reply_map: std::sync::Mutex::new(HashMap::new()),
		});
		
		let (incoming_message_sender, incoming_messages) = sync::mpsc::channel(1024);
		let (outgoing_message_sender, outgoing_messages) = sync::mpsc::channel(1024);
		/* let (incoming_reply_sender, incoming_replies) = sync::mpsc::channel::<Message>(1024);
		let (outgoing_reply_sender, outgoing_replies) = sync::mpsc::channel::<Message>(1024); */
		let sender = Sender {
			inner: inner.clone(),
			outgoing_message_sender: outgoing_message_sender.clone(),
			//outgoing_reply_sender: outgoing_reply_sender.clone(),
			_phantom: PhantomData,
		};
		let receiver = Receiver {
			incoming_messages,
			_phantom: PhantomData,
		};
		let this = Self {
			stream,
			inbound: Vec::new(),
			/* _incoming_reply_sender: incoming_reply_sender,
			incoming_replies, */
			/* _outgoing_reply_sender: outgoing_reply_sender,
			outgoing_replies, */
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
							tracing::trace!("Read {n} bytes");
							self.handle_incoming_bytes(&buf[..n]).await;
						},
						Err(_e) => {
							tracing::debug!("Connection closed");
							break;
						}
					}
				},
				/* reply = self.incoming_replies.recv().fuse() => {
					let message = reply.expect("Channel should not be closed");
					self.handle_incoming_reply(message).await;
				}, */
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
	
	pub async fn send_message(&mut self, message: Message) -> std::io::Result<()> {
		self.stream.write_all(&(message.data.len() as u64).to_be_bytes()).await?;
		self.stream.write_all(&message.id.to_be_bytes()).await?;
		self.stream.write_all(&message.data).await?;
		Ok(())
	}
	
	pub async fn handle_incoming_bytes(&mut self, data: &[u8]) {
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
	
	pub async fn handle_incoming_reply(&mut self, msg: Message) {
		if let Some(target) = self.inner.reply_map.lock().unwrap().remove(&msg.id) {
			if let Err(_) = target.send(msg.data) {
				tracing::debug!("Reply handler was dropped before receiving reply");
			}
		}
	}
}

#[derive(Clone)]
pub struct Sender<T> {
	inner: Arc<Conn>,
	outgoing_message_sender: sync::mpsc::Sender<Message>,
	//outgoing_reply_sender: sync::mpsc::Sender<Message>,
	_phantom: PhantomData<T>,
}

impl<T: Serialize> Sender<T> {
	pub async fn send(&self, msg: T) {
		let id = self.inner.next_id();
		let data = bincode::serialize(&msg)
			.expect("Failed to serialize value");
		self.outgoing_message_sender.send(Message { id, data }).await
			.expect("Connection closed");
	}
	
	pub async fn reply<U: Serialize>(&self, token: ReplyToken<U>, msg: U) {
		let data = bincode::serialize(&msg)
			.expect("Failed to serialize value");
		self.outgoing_message_sender.send(Message { id: token.id, data }).await
			.expect("Connection closed");
		/* self.outgoing_reply_sender.send(Message { id: token.id, data }).await
			.map_err(|_| ())
			.expect("Connection closed"); */
	}
	
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