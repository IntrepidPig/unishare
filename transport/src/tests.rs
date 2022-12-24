
use tokio::net::TcpListener;

use super::*;
use serde::{Serialize, Deserialize};

fn start_tracing() {
	let _ = tracing_subscriber::fmt()
		.with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
		.try_init();
}

#[test]
fn basic() {
	#[derive(Debug, Serialize, Deserialize, PartialEq)]
	enum BasicMessageA {
		ASaidHi,
		ASaidBye,
	}
	
	#[derive(Debug, Serialize, Deserialize, PartialEq)]
	enum BasicMessageB {
		BSaidYo,
	}
	
	start_tracing();
	
	let rt = tokio::runtime::Runtime::new().unwrap();
	rt.block_on(async {
		let listener = TcpListener::bind("localhost:8951").await.unwrap();
		
		tokio::spawn(async {
			let stream = TcpStream::connect("localhost:8951").await.unwrap();
			let (conn, sender, mut receiver) = Connection::new::<BasicMessageA, BasicMessageB>(stream);
			tokio::spawn(conn.run());
			sender.send(BasicMessageA::ASaidHi).await;
			assert_eq!(receiver.recv().await, Some(BasicMessageB::BSaidYo));
			sender.send(BasicMessageA::ASaidBye).await;
		});
		
		let (stream, _addr) = listener.accept().await.unwrap();
		let (conn, sender, mut receiver) = Connection::new::<BasicMessageB, BasicMessageA>(stream);
		tokio::spawn(conn.run());
		assert_eq!(receiver.recv().await, Some(BasicMessageA::ASaidHi));
		sender.send(BasicMessageB::BSaidYo).await;
		assert_eq!(receiver.recv().await, Some(BasicMessageA::ASaidBye));
	});
}

#[test]
fn replies() {
	#[derive(Debug, Serialize, Deserialize, PartialEq)]
	enum BasicMessageA {
		ASaidHi,
		ASaidBye,
	}
	
	#[derive(Debug, Serialize, Deserialize)]
	enum BasicMessageB {
		BQuery(ReplyToken<i64>),
	}
	
	start_tracing();
	
	let rt = tokio::runtime::Runtime::new().unwrap();
	rt.block_on(async {
		let listener = TcpListener::bind("localhost:8951").await.unwrap();
		
		tokio::spawn(async {
			let stream = TcpStream::connect("localhost:8951").await.unwrap();
			let (conn, sender, mut receiver) = Connection::new::<BasicMessageA, BasicMessageB>(stream);
			tokio::spawn(conn.run());
			sender.send(BasicMessageA::ASaidHi).await;
			match receiver.recv().await.expect("No query received") {
				BasicMessageB::BQuery(token) => sender.reply(token, 17).await,
			};
			sender.send(BasicMessageA::ASaidBye).await;
		});
		
		let (stream, _addr) = listener.accept().await.unwrap();
		let (conn, sender, mut receiver) = Connection::new::<BasicMessageB, BasicMessageA>(stream);
		tokio::spawn(conn.run());
		assert_eq!(receiver.recv().await, Some(BasicMessageA::ASaidHi));
		let (token, reply) = sender.create_request();
		sender.send(BasicMessageB::BQuery(token)).await;
		let response = reply.recv().await.expect("Reply not received");
		assert_eq!(response, 17);
		assert_eq!(receiver.recv().await, Some(BasicMessageA::ASaidBye));
	});
}