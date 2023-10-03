use futures::{future::join_all, StreamExt};
use futures_channel::mpsc;
use once_cell::sync::{Lazy, OnceCell};
use rspc::internal::jsonrpc::{self, *};
use sd_core::{api::Router, Node};
use serde_json::{from_str, from_value, to_string, Value};
use std::{
	borrow::Cow,
	collections::HashMap,
	future::{ready, Ready},
	marker::Send,
	sync::Arc,
};
use tokio::{
	runtime::Runtime,
	sync::{oneshot, Mutex},
};
use tracing::error;

pub static RUNTIME: Lazy<Runtime> = Lazy::new(|| Runtime::new().unwrap());

pub type NodeType = Lazy<Mutex<Option<(Arc<Node>, Arc<Router>)>>>;

pub static NODE: NodeType = Lazy::new(|| Mutex::new(None));

#[allow(clippy::type_complexity)]
pub static SUBSCRIPTIONS: Lazy<Arc<futures_locks::Mutex<HashMap<RequestId, oneshot::Sender<()>>>>> =
	Lazy::new(Default::default);

pub static EVENT_SENDER: OnceCell<mpsc::Sender<Response>> = OnceCell::new();

pub const CLIENT_ID: &str = "d068776a-05b6-4aaa-9001-4d01734e1944";
pub const CLIENT_SECRET: &str = "961cdf5c-9eb1-43dc-b921-5b1dd8bbf6a5";

pub struct MobileSender<'a> {
	resp: &'a mut Option<Response>,
}

impl<'a> Sender<'a> for MobileSender<'a> {
	type SendFut = Ready<()>;
	type SubscriptionMap = Arc<futures_locks::Mutex<HashMap<RequestId, oneshot::Sender<()>>>>;
	type OwnedSender = OwnedMpscSender;

	fn subscription(self) -> SubscriptionUpgrade<'a, Self> {
		SubscriptionUpgrade::Supported(
			OwnedMpscSender::new(
				EVENT_SENDER
					.get()
					.expect("Core was not started before making a request!")
					.clone(),
			),
			SUBSCRIPTIONS.clone(),
		)
	}

	fn send(self, resp: jsonrpc::Response) -> Self::SendFut {
		*self.resp = Some(resp);
		ready(())
	}
}

pub fn handle_core_msg(
	query: String,
	data_dir: String,
	callback: impl FnOnce(Result<String, String>) + Send + 'static,
) {
	RUNTIME.spawn(async move {
		let (node, router) = {
			let node = &mut *NODE.lock().await;
			match node {
				Some(node) => node.clone(),
				None => {
					let _guard = Node::init_logger(&data_dir);

					// TODO: probably don't unwrap
					let new_node = Node::new(
						data_dir,
						sd_core::Env {
							api_url: "https://app.spacedrive.com".to_string(),
							client_id: CLIENT_ID.to_string(),
							client_secret: CLIENT_SECRET.to_string(),
						},
					)
					.await
					.unwrap();
					node.replace(new_node.clone());
					new_node
				}
			}
		};

		let reqs = match from_str::<Value>(&query).and_then(|v| match v.is_array() {
			true => from_value::<Vec<Request>>(v),
			false => from_value::<Request>(v).map(|v| vec![v]),
		}) {
			Ok(v) => v,
			Err(err) => {
				error!("failed to decode JSON-RPC request: {}", err); // Don't use tracing here because it's before the `Node` is initialised which sets that config!
				callback(Err(query));
				return;
			}
		};

		let responses = join_all(reqs.into_iter().map(|request| {
			let node = node.clone();
			let router = router.clone();
			async move {
				let mut resp = Option::<Response>::None;
				handle_json_rpc(
					node.clone(),
					request,
					Cow::Borrowed(&router),
					MobileSender { resp: &mut resp },
				)
				.await;
				resp
			}
		}))
		.await;

		callback(Ok(serde_json::to_string(
			&responses.into_iter().flatten().collect::<Vec<_>>(),
		)
		.unwrap()));
	});
}

pub fn spawn_core_event_listener(callback: impl Fn(String) + Send + 'static) {
	let (tx, mut rx) = mpsc::channel(100);
	let _ = EVENT_SENDER.set(tx);

	RUNTIME.spawn(async move {
		while let Some(event) = rx.next().await {
			let data = match to_string(&event) {
				Ok(json) => json,
				Err(err) => {
					error!("Failed to serialize event: {err}");
					continue;
				}
			};

			callback(data);
		}
	});
}
