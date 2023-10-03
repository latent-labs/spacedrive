use crate::{library::Library, prisma::location, util::db::maybe_missing, Node};

use std::{
	collections::HashSet,
	path::{Path, PathBuf},
	sync::Arc,
	time::Duration,
};

use async_trait::async_trait;
use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::{
	runtime::Handle,
	select,
	sync::{mpsc, oneshot},
	task::{block_in_place, JoinHandle},
	time::{interval_at, Instant, MissedTickBehavior},
};
use tracing::{debug, error, warn};
use uuid::Uuid;

use super::LocationManagerError;

mod linux;
mod macos;
mod windows;

mod utils;

use utils::check_event;

#[cfg(target_os = "linux")]
type Handler<'lib> = linux::LinuxEventHandler<'lib>;

#[cfg(target_os = "macos")]
type Handler<'lib> = macos::MacOsEventHandler<'lib>;

#[cfg(target_os = "windows")]
type Handler<'lib> = windows::WindowsEventHandler<'lib>;

pub(super) type IgnorePath = (PathBuf, bool);

type INode = u64;
type InstantAndPath = (Instant, PathBuf);

const ONE_SECOND: Duration = Duration::from_secs(1);
const HUNDRED_MILLIS: Duration = Duration::from_millis(100);

#[async_trait]
trait EventHandler<'lib> {
	fn new(
		location_id: location::id::Type,
		library: &'lib Arc<Library>,
		node: &'lib Arc<Node>,
	) -> Self
	where
		Self: Sized;

	/// Handle a file system event.
	async fn handle_event(&mut self, event: Event) -> Result<(), LocationManagerError>;

	/// As Event Handlers have some inner state, from time to time we need to call this tick method
	/// so the event handler can update its state.
	async fn tick(&mut self);
}

#[derive(Debug)]
pub(super) struct LocationWatcher {
	id: i32,
	path: String,
	watcher: RecommendedWatcher,
	ignore_path_tx: mpsc::UnboundedSender<IgnorePath>,
	handle: Option<JoinHandle<()>>,
	stop_tx: Option<oneshot::Sender<()>>,
}

impl LocationWatcher {
	pub(super) async fn new(
		location: location::Data,
		library: Arc<Library>,
		node: Arc<Node>,
	) -> Result<Self, LocationManagerError> {
		let (events_tx, events_rx) = mpsc::unbounded_channel();
		let (ignore_path_tx, ignore_path_rx) = mpsc::unbounded_channel();
		let (stop_tx, stop_rx) = oneshot::channel();

		let watcher = RecommendedWatcher::new(
			move |result| {
				if !events_tx.is_closed() {
					if events_tx.send(result).is_err() {
						error!(
						"Unable to send watcher event to location manager for location: <id='{}'>",
						location.id
					);
					}
				} else {
					error!(
						"Tried to send location file system events to a closed channel: <id='{}'",
						location.id
					);
				}
			},
			Config::default(),
		)?;

		let handle = tokio::spawn(Self::handle_watch_events(
			location.id,
			Uuid::from_slice(&location.pub_id)?,
			node,
			library,
			events_rx,
			ignore_path_rx,
			stop_rx,
		));

		Ok(Self {
			id: location.id,
			path: maybe_missing(location.path, "location.path")?,
			watcher,
			ignore_path_tx,
			handle: Some(handle),
			stop_tx: Some(stop_tx),
		})
	}

	async fn handle_watch_events(
		location_id: location::id::Type,
		location_pub_id: Uuid,
		node: Arc<Node>,
		library: Arc<Library>,
		mut events_rx: mpsc::UnboundedReceiver<notify::Result<Event>>,
		mut ignore_path_rx: mpsc::UnboundedReceiver<IgnorePath>,
		mut stop_rx: oneshot::Receiver<()>,
	) {
		let mut event_handler = Handler::new(location_id, &library, &node);

		let mut paths_to_ignore = HashSet::new();

		let mut handler_interval = interval_at(Instant::now() + HUNDRED_MILLIS, HUNDRED_MILLIS);
		// In case of doubt check: https://docs.rs/tokio/latest/tokio/time/enum.MissedTickBehavior.html
		handler_interval.set_missed_tick_behavior(MissedTickBehavior::Delay);

		loop {
			select! {
				Some(event) = events_rx.recv() => {
					match event {
						Ok(event) => {
							if let Err(e) = Self::handle_single_event(
								location_id,
								location_pub_id,
								event,
								&mut event_handler,
								&node,
								&library,
								&paths_to_ignore,
							).await {
								error!("Failed to handle location file system event: \
									<id='{location_id}', error='{e:#?}'>",
								);
							}
						}
						Err(e) => {
							error!("watch error: {:#?}", e);
						}
					}
				}

				Some((path, ignore)) = ignore_path_rx.recv() => {
					if ignore {
						paths_to_ignore.insert(path);
					} else {
						paths_to_ignore.remove(&path);
					}
				}

				_ = handler_interval.tick() => {
					event_handler.tick().await;
				}

				_ = &mut stop_rx => {
					debug!("Stop Location Manager event handler for location: <id='{}'>", location_id);
					break
				}
			}
		}
	}

	async fn handle_single_event<'lib>(
		location_id: location::id::Type,
		location_pub_id: Uuid,
		event: Event,
		event_handler: &mut impl EventHandler<'lib>,
		node: &'lib Node,
		_library: &'lib Library,
		ignore_paths: &HashSet<PathBuf>,
	) -> Result<(), LocationManagerError> {
		if !check_event(&event, ignore_paths) {
			return Ok(());
		}

		// let Some(location) = find_location(library, location_id)
		// 	.include(location_with_indexer_rules::include())
		// 	.exec()
		// 	.await?
		// else {
		// 	warn!("Tried to handle event for unknown location: <id='{location_id}'>");
		//     return Ok(());
		// };

		if !node.locations.is_online(&location_pub_id).await {
			warn!("Tried to handle event for offline location: <id='{location_id}'>");
			return Ok(());
		}

		event_handler.handle_event(event).await
	}

	pub(super) fn ignore_path(
		&self,
		path: PathBuf,
		ignore: bool,
	) -> Result<(), LocationManagerError> {
		self.ignore_path_tx.send((path, ignore)).map_err(Into::into)
	}

	pub(super) fn check_path(&self, path: impl AsRef<Path>) -> bool {
		Path::new(&self.path) == path.as_ref()
	}

	pub(super) fn watch(&mut self) {
		let path = &self.path;

		if let Err(e) = self
			.watcher
			.watch(Path::new(path), RecursiveMode::Recursive)
		{
			error!("Unable to watch location: (path: {path}, error: {e:#?})");
		} else {
			debug!("Now watching location: (path: {path})");
		}
	}

	pub(super) fn unwatch(&mut self) {
		let path = &self.path;
		if let Err(e) = self.watcher.unwatch(Path::new(path)) {
			/**************************************** TODO: ****************************************
			 * According to an unit test, this error may occur when a subdirectory is removed	   *
			 * and we try to unwatch the parent directory then we have to check the implications   *
			 * of unwatch error for this case.   												   *
			 **************************************************************************************/
			error!("Unable to unwatch location: (path: {path}, error: {e:#?})",);
		} else {
			debug!("Stop watching location: (path: {path})");
		}
	}
}

impl Drop for LocationWatcher {
	fn drop(&mut self) {
		if let Some(stop_tx) = self.stop_tx.take() {
			if stop_tx.send(()).is_err() {
				error!(
					"Failed to send stop signal to location watcher: <id='{}'>",
					self.id
				);
			}

			// FIXME: change this Drop to async drop in the future
			if let Some(handle) = self.handle.take() {
				if let Err(e) = block_in_place(move || Handle::current().block_on(handle)) {
					error!("Failed to join watcher task: {e:#?}")
				}
			}
		}
	}
}

/***************************************************************************************************
* Some tests to validate our assumptions of events through different file systems				   *
****************************************************************************************************
*	Events dispatched on Linux:																	   *
*		Create File:																			   *
*			1) EventKind::Create(CreateKind::File)												   *
*			2) EventKind::Modify(ModifyKind::Metadata(MetadataKind::Any))						   *
*				or EventKind::Modify(ModifyKind::Data(DataChange::Any))							   *
*			3) EventKind::Access(AccessKind::Close(AccessMode::Write)))							   *
*		Create Directory:																		   *
*			1) EventKind::Create(CreateKind::Folder)											   *
*		Update File:																			   *
*			1) EventKind::Modify(ModifyKind::Data(DataChange::Any))								   *
*			2) EventKind::Access(AccessKind::Close(AccessMode::Write)))							   *
*		Update File (rename):																	   *
*			1) EventKind::Modify(ModifyKind::Name(RenameMode::From))							   *
*			2) EventKind::Modify(ModifyKind::Name(RenameMode::To))								   *
*			3) EventKind::Modify(ModifyKind::Name(RenameMode::Both))							   *
*		Update Directory (rename):																   *
*			1) EventKind::Modify(ModifyKind::Name(RenameMode::From))							   *
*			2) EventKind::Modify(ModifyKind::Name(RenameMode::To))								   *
*			3) EventKind::Modify(ModifyKind::Name(RenameMode::Both))							   *
*		Delete File:																			   *
*			1) EventKind::Remove(RemoveKind::File)												   *
*		Delete Directory:																		   *
*			1) EventKind::Remove(RemoveKind::Folder)											   *
*																								   *
*	Events dispatched on MacOS:																	   *
*		Create File:																			   *
*			1) EventKind::Create(CreateKind::File)												   *
*			2) EventKind::Modify(ModifyKind::Data(DataChange::Content))							   *
*		Create Directory:																		   *
*			1) EventKind::Create(CreateKind::Folder)											   *
*		Update File:																			   *
*			1) EventKind::Modify(ModifyKind::Data(DataChange::Content))							   *
*		Update File (rename):																	   *
*			1) EventKind::Modify(ModifyKind::Name(RenameMode::Any)) -- From						   *
*			2) EventKind::Modify(ModifyKind::Name(RenameMode::Any))	-- To						   *
*		Update Directory (rename):																   *
*			1) EventKind::Modify(ModifyKind::Name(RenameMode::Any)) -- From						   *
*			2) EventKind::Modify(ModifyKind::Name(RenameMode::Any))	-- To						   *
*		Delete File:																			   *
*			1) EventKind::Remove(RemoveKind::File)												   *
*		Delete Directory:																		   *
*			1) EventKind::Remove(RemoveKind::Folder)											   *
*																								   *
*	Events dispatched on Windows:																   *
*		Create File:																			   *
*			1) EventKind::Create(CreateKind::Any)												   *
*			2) EventKind::Modify(ModifyKind::Any)												   *
*		Create Directory:																		   *
*			1) EventKind::Create(CreateKind::Any)												   *
*		Update File:																			   *
*			1) EventKind::Modify(ModifyKind::Any)												   *
*		Update File (rename):																	   *
*			1) EventKind::Modify(ModifyKind::Name(RenameMode::From))							   *
*			2) EventKind::Modify(ModifyKind::Name(RenameMode::To))								   *
*		Update Directory (rename):																   *
*			1) EventKind::Modify(ModifyKind::Name(RenameMode::From))							   *
*			2) EventKind::Modify(ModifyKind::Name(RenameMode::To))								   *
*		Delete File:																			   *
*			1) EventKind::Remove(RemoveKind::Any)												   *
*		Delete Directory:																		   *
*			1) EventKind::Remove(RemoveKind::Any)												   *
*																								   *
*	Events dispatched on Android:																   *
*	TODO																						   *
*																								   *
*	Events dispatched on iOS:																	   *
*	TODO																						   *
*																								   *
***************************************************************************************************/
#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
	use std::{
		io::ErrorKind,
		path::{Path, PathBuf},
		time::Duration,
	};

	use notify::{
		event::{CreateKind, ModifyKind, RemoveKind, RenameMode},
		Config, Event, EventKind, RecommendedWatcher, Watcher,
	};
	use tempfile::{tempdir, TempDir};
	use tokio::{fs, io::AsyncWriteExt, sync::mpsc, time::sleep};
	use tracing::{debug, error};
	// use tracing_test::traced_test;

	#[cfg(target_os = "macos")]
	use notify::event::DataChange;

	#[cfg(target_os = "linux")]
	use notify::event::{AccessKind, AccessMode};

	async fn setup_watcher() -> (
		TempDir,
		RecommendedWatcher,
		mpsc::UnboundedReceiver<notify::Result<Event>>,
	) {
		let (events_tx, events_rx) = mpsc::unbounded_channel();

		let watcher = RecommendedWatcher::new(
			move |result| {
				events_tx
					.send(result)
					.expect("Unable to send watcher event");
			},
			Config::default(),
		)
		.expect("Failed to create watcher");

		(tempdir().unwrap(), watcher, events_rx)
	}

	async fn expect_event(
		mut events_rx: mpsc::UnboundedReceiver<notify::Result<Event>>,
		path: impl AsRef<Path>,
		expected_event: EventKind,
	) {
		let path = path.as_ref();
		debug!(
			"Expecting event: {expected_event:#?} at path: {}",
			path.display()
		);
		let mut tries = 0;
		loop {
			match events_rx.try_recv() {
				Ok(maybe_event) => {
					let event = maybe_event.expect("Failed to receive event");
					debug!("Received event: {event:#?}");
					// Using `ends_with` and removing root path here due to a weird edge case on CI tests at MacOS
					if event.paths[0].ends_with(path.iter().skip(1).collect::<PathBuf>())
						&& event.kind == expected_event
					{
						debug!("Received expected event: {expected_event:#?}");
						break;
					}
				}
				Err(e) => {
					debug!("No event yet: {e:#?}");
					tries += 1;
					sleep(Duration::from_millis(100)).await;
				}
			}

			if tries == 10 {
				panic!("No expected event received after 10 tries");
			}
		}
	}

	#[tokio::test]
	// #[traced_test]
	async fn create_file_event() {
		let (root_dir, mut watcher, events_rx) = setup_watcher().await;

		watcher
			.watch(root_dir.path(), notify::RecursiveMode::Recursive)
			.expect("Failed to watch root directory");
		debug!("Now watching {}", root_dir.path().display());

		let file_path = root_dir.path().join("test.txt");
		fs::write(&file_path, "test").await.unwrap();

		#[cfg(target_os = "windows")]
		expect_event(events_rx, &file_path, EventKind::Modify(ModifyKind::Any)).await;

		#[cfg(target_os = "macos")]
		expect_event(
			events_rx,
			&file_path,
			EventKind::Modify(ModifyKind::Data(DataChange::Content)),
		)
		.await;

		#[cfg(target_os = "linux")]
		expect_event(
			events_rx,
			&file_path,
			EventKind::Access(AccessKind::Close(AccessMode::Write)),
		)
		.await;

		debug!("Unwatching root directory: {}", root_dir.path().display());
		if let Err(e) = watcher.unwatch(root_dir.path()) {
			error!("Failed to unwatch root directory: {e:#?}");
		}
	}

	#[tokio::test]
	// #[traced_test]
	async fn create_dir_event() {
		let (root_dir, mut watcher, events_rx) = setup_watcher().await;

		watcher
			.watch(root_dir.path(), notify::RecursiveMode::Recursive)
			.expect("Failed to watch root directory");
		debug!("Now watching {}", root_dir.path().display());

		let dir_path = root_dir.path().join("inner");
		fs::create_dir(&dir_path)
			.await
			.expect("Failed to create directory");

		#[cfg(target_os = "windows")]
		expect_event(events_rx, &dir_path, EventKind::Create(CreateKind::Any)).await;

		#[cfg(target_os = "macos")]
		expect_event(events_rx, &dir_path, EventKind::Create(CreateKind::Folder)).await;

		#[cfg(target_os = "linux")]
		expect_event(events_rx, &dir_path, EventKind::Create(CreateKind::Folder)).await;

		debug!("Unwatching root directory: {}", root_dir.path().display());
		if let Err(e) = watcher.unwatch(root_dir.path()) {
			error!("Failed to unwatch root directory: {e:#?}");
		}
	}

	#[tokio::test]
	// #[traced_test]
	async fn update_file_event() {
		let (root_dir, mut watcher, events_rx) = setup_watcher().await;

		let file_path = root_dir.path().join("test.txt");
		fs::write(&file_path, "test").await.unwrap();

		watcher
			.watch(root_dir.path(), notify::RecursiveMode::Recursive)
			.expect("Failed to watch root directory");
		debug!("Now watching {}", root_dir.path().display());

		let mut file = fs::OpenOptions::new()
			.append(true)
			.open(&file_path)
			.await
			.expect("Failed to open file");

		// Writing then sync data before closing the file
		file.write_all(b"\nanother test")
			.await
			.expect("Failed to write to file");
		file.sync_all().await.expect("Failed to flush file");
		drop(file);

		#[cfg(target_os = "windows")]
		expect_event(events_rx, &file_path, EventKind::Modify(ModifyKind::Any)).await;

		#[cfg(target_os = "macos")]
		expect_event(
			events_rx,
			&file_path,
			EventKind::Modify(ModifyKind::Data(DataChange::Content)),
		)
		.await;

		#[cfg(target_os = "linux")]
		expect_event(
			events_rx,
			&file_path,
			EventKind::Access(AccessKind::Close(AccessMode::Write)),
		)
		.await;

		debug!("Unwatching root directory: {}", root_dir.path().display());
		if let Err(e) = watcher.unwatch(root_dir.path()) {
			error!("Failed to unwatch root directory: {e:#?}");
		}
	}

	#[tokio::test]
	// #[traced_test]
	async fn update_file_rename_event() {
		let (root_dir, mut watcher, events_rx) = setup_watcher().await;

		let file_path = root_dir.path().join("test.txt");
		fs::write(&file_path, "test").await.unwrap();

		watcher
			.watch(root_dir.path(), notify::RecursiveMode::Recursive)
			.expect("Failed to watch root directory");
		debug!("Now watching {}", root_dir.path().display());

		let new_file_name = root_dir.path().join("test2.txt");

		fs::rename(&file_path, &new_file_name)
			.await
			.expect("Failed to rename file");

		#[cfg(target_os = "windows")]
		expect_event(
			events_rx,
			&new_file_name,
			EventKind::Modify(ModifyKind::Name(RenameMode::To)),
		)
		.await;

		#[cfg(target_os = "macos")]
		expect_event(
			events_rx,
			&file_path,
			EventKind::Modify(ModifyKind::Name(RenameMode::Any)),
		)
		.await;

		#[cfg(target_os = "linux")]
		expect_event(
			events_rx,
			&file_path,
			EventKind::Modify(ModifyKind::Name(RenameMode::Both)),
		)
		.await;

		debug!("Unwatching root directory: {}", root_dir.path().display());
		if let Err(e) = watcher.unwatch(root_dir.path()) {
			error!("Failed to unwatch root directory: {e:#?}");
		}
	}

	#[tokio::test]
	// #[traced_test]
	async fn update_dir_event() {
		let (root_dir, mut watcher, events_rx) = setup_watcher().await;

		let dir_path = root_dir.path().join("inner");
		fs::create_dir(&dir_path)
			.await
			.expect("Failed to create directory");

		watcher
			.watch(root_dir.path(), notify::RecursiveMode::Recursive)
			.expect("Failed to watch root directory");
		debug!("Now watching {}", root_dir.path().display());

		let new_dir_name = root_dir.path().join("inner2");

		fs::rename(&dir_path, &new_dir_name)
			.await
			.expect("Failed to rename directory");

		#[cfg(target_os = "windows")]
		expect_event(
			events_rx,
			&new_dir_name,
			EventKind::Modify(ModifyKind::Name(RenameMode::To)),
		)
		.await;

		#[cfg(target_os = "macos")]
		expect_event(
			events_rx,
			&dir_path,
			EventKind::Modify(ModifyKind::Name(RenameMode::Any)),
		)
		.await;

		#[cfg(target_os = "linux")]
		expect_event(
			events_rx,
			&dir_path,
			EventKind::Modify(ModifyKind::Name(RenameMode::Both)),
		)
		.await;

		debug!("Unwatching root directory: {}", root_dir.path().display());
		if let Err(e) = watcher.unwatch(root_dir.path()) {
			error!("Failed to unwatch root directory: {e:#?}");
		}
	}

	#[tokio::test]
	// #[traced_test]
	async fn delete_file_event() {
		let (root_dir, mut watcher, events_rx) = setup_watcher().await;

		let file_path = root_dir.path().join("test.txt");
		fs::write(&file_path, "test").await.unwrap();

		watcher
			.watch(root_dir.path(), notify::RecursiveMode::Recursive)
			.expect("Failed to watch root directory");
		debug!("Now watching {}", root_dir.path().display());

		fs::remove_file(&file_path)
			.await
			.expect("Failed to remove file");

		#[cfg(target_os = "windows")]
		expect_event(events_rx, &file_path, EventKind::Remove(RemoveKind::Any)).await;

		#[cfg(target_os = "macos")]
		expect_event(events_rx, &file_path, EventKind::Remove(RemoveKind::File)).await;

		#[cfg(target_os = "linux")]
		expect_event(events_rx, &file_path, EventKind::Remove(RemoveKind::File)).await;

		debug!("Unwatching root directory: {}", root_dir.path().display());
		if let Err(e) = watcher.unwatch(root_dir.path()) {
			error!("Failed to unwatch root directory: {e:#?}");
		}
	}

	#[tokio::test]
	// #[traced_test]
	async fn delete_dir_event() {
		let (root_dir, mut watcher, events_rx) = setup_watcher().await;

		let dir_path = root_dir.path().join("inner");
		fs::create_dir(&dir_path)
			.await
			.expect("Failed to create directory");

		if let Err(e) = fs::metadata(&dir_path).await {
			if e.kind() == ErrorKind::NotFound {
				panic!("Directory not found");
			} else {
				panic!("{e}");
			}
		}

		watcher
			.watch(root_dir.path(), notify::RecursiveMode::Recursive)
			.expect("Failed to watch root directory");
		debug!("Now watching {}", root_dir.path().display());

		debug!("First unwatching the inner directory before removing it");
		if let Err(e) = watcher.unwatch(&dir_path) {
			error!("Failed to unwatch inner directory: {e:#?}");
		}

		fs::remove_dir(&dir_path)
			.await
			.expect("Failed to remove directory");

		#[cfg(target_os = "windows")]
		expect_event(events_rx, &dir_path, EventKind::Remove(RemoveKind::Any)).await;

		#[cfg(target_os = "macos")]
		expect_event(events_rx, &dir_path, EventKind::Remove(RemoveKind::Folder)).await;

		#[cfg(target_os = "linux")]
		expect_event(events_rx, &dir_path, EventKind::Remove(RemoveKind::Folder)).await;

		debug!("Unwatching root directory: {}", root_dir.path().display());
		if let Err(e) = watcher.unwatch(root_dir.path()) {
			error!("Failed to unwatch root directory: {e:#?}");
		}
	}
}
