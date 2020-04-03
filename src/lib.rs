// #![deny(warnings)]
// #![deny(clippy::all)]
use bytes::BytesMut;

use futures::{
    ready,
    task::{Context, Poll, Waker},
    Stream,
};
use notify::{watcher, DebouncedEvent, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::{
    mpsc::{self, channel, Receiver, Sender},
    Arc, Mutex,
};
use std::thread::JoinHandle;
use std::time::Duration;
use tokio_util::codec::{Decoder, FramedRead, LinesCodec};
pub fn test() {
    println!("{:?}", "ok");
}

pub enum FileEvent {
    Modify,
    Delete,
    Err,
}

struct SharedState {
    /// Whether or not the sleep time has elapsed
    completed: bool,

    /// The waker for the task that `TimerFuture` is running on.
    /// The thread can use this after setting `completed = true` to tell
    /// `TimerFuture`'s task to wake up, see that `completed = true`, and
    /// move forward.
    waker: Option<Waker>,
}

/// a stream of file event like modify/delete which impl by notify
pub struct FileEventWatcher {
    watcher: notify::RecommendedWatcher,
    shared_state: Arc<Mutex<SharedState>>,
    inotify_thread_handle: JoinHandle<()>,
    receiver: Receiver<FileEvent>,
    file: PathBuf,
}

impl From<DebouncedEvent> for FileEvent {
    fn from(o: DebouncedEvent) -> Self {
        match o {
            DebouncedEvent::NoticeWrite(_)
            | DebouncedEvent::NoticeRemove(_)
            | DebouncedEvent::Create(_)
            | DebouncedEvent::Write(_)
            | DebouncedEvent::Chmod(_)
            | DebouncedEvent::Remove(_)
            | DebouncedEvent::Rename(_, _)
            | DebouncedEvent::Rescan => FileEvent::Modify,
            DebouncedEvent::Error(_, _) => FileEvent::Err,
        }
    }
}

impl FileEventWatcher {
    fn new(path: PathBuf) -> Self {
        let file = path;
        println!("{:?}", file.exists());
        let shared_state = Arc::new(Mutex::new(SharedState {
            completed: false,
            waker: None,
        }));
        let (notify_tx, notify_rx) = mpsc::channel();
        let (event_tx, event_rx) = mpsc::channel();
        let mut watcher = watcher(notify_tx, Duration::from_secs(1)).unwrap();
        watcher.watch(&file, RecursiveMode::NonRecursive).unwrap();
        println!("{:?} init notify ok", file);
        let shared_state_clone = shared_state.clone();
        let handle = std::thread::spawn(move || {
            let shared_state_arc = shared_state_clone;
            loop {
                match notify_rx.recv() {
                    Ok(event) => {
                        event_tx.send(FileEvent::from(event));
                        let mut shared_state = shared_state_arc.lock().unwrap();
                        if let Some(waker) = &shared_state.waker {
                            waker.wake_by_ref();
                        }
                    }
                    Err(e) => {
                        println!("err {:?}", e);
                        let mut shared_state = shared_state_arc.lock().unwrap();
                        shared_state.completed = true;
                        event_tx.send(FileEvent::Err);
                    }
                }
            }
        });
        Self {
            watcher,
            shared_state,
            file,
            receiver: event_rx,
            inotify_thread_handle: handle,
        }
    }
}
use tokio::io::AsyncRead;
impl Stream for FileEventWatcher {
    type Item = FileEvent;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        {
            let mut shared_state = self.shared_state.lock().unwrap();
            if shared_state.waker.is_none() {
                shared_state.waker = Some(cx.waker().clone());
                return Poll::Pending;
            }
        }
        while let Ok(e) = self.receiver.try_recv() {
            match e {
                FileEvent::Delete => return Poll::Ready(Some(FileEvent::Delete)),
                FileEvent::Modify => return Poll::Ready(Some(FileEvent::Modify)),
                FileEvent::Err => return Poll::Ready(Some(FileEvent::Err)),
            }
        }

        {
            let mut shared_state = self.shared_state.lock().unwrap();
            if shared_state.completed {
                return Poll::Ready(None);
            } else {
                return Poll::Pending;
            }
        }
    }
}
pub struct FileStream {
    path: PathBuf,
    offset: usize, // TODO should open a file and seek to end
    file_watcher: FileEventWatcher,
}
fn get_file_size(path: &dyn AsRef<Path>) -> Result<usize, failure::Error> {
    let meta = std::fs::metadata(path.as_ref())?;
    return Ok(meta.len() as usize);
}

impl FileStream {
    fn new<P: AsRef<Path>>(path: P) -> Self {
        let path = path.as_ref().to_path_buf();
        let offset = get_file_size(&path).unwrap();
        println!("file stream {:?}", offset);
        let watcher = FileEventWatcher::new(path.clone());
        Self {
            path,
            offset,
            file_watcher: watcher,
        }
    }
}
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
impl Stream for FileStream {
    type Item = Vec<u8>;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        let file_watcher = unsafe { Pin::new_unchecked(&mut self.file_watcher) };
        match ready!(file_watcher.poll_next(cx)) {
            Some(event) => match event {
                FileEvent::Delete | FileEvent::Err => {
                    println!("{:?}", "fs event error");
                    return Poll::Ready(None);
                }
                FileEvent::Modify => {
                    println!("{:?}", "fs event modify");
                    let mut f = File::open(&self.path).unwrap();

                    f.seek(SeekFrom::Start(self.offset as u64)).unwrap();
                    let mut buff = vec![];
                    f.read_to_end(&mut buff).unwrap();
                    self.offset += buff.len();
                    return Poll::Ready(Some(buff));
                }
            },
            None => return Poll::Ready(None),
        }
    }
}
use std::io::Write;

impl AsyncRead for FileStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context,
        mut buf: &mut [u8],
    ) -> Poll<Result<usize, futures::io::Error>> {
        let mut cache = vec![];
        loop {
            match ready!(self.as_mut().poll_next(cx)) {
                Some(data) => {
                    cache.write_all(&data);
                    let cache_len = cache.len();
                    if cache_len!= 0 {
                        buf.write_all(&mut cache);
                        return Poll::Ready(Ok( cache_len ));
                    }
                }
                None => {
                    return Poll::Ready(Ok(0));
                }
            }
        }
    }
}

pub struct Tail {
    path: PathBuf,
    stream: FramedRead<FileStream, LinesCodec>,
}
impl Tail {
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        let file_stream = FileStream::new(&path);
        let stream = FramedRead::new(file_stream, LinesCodec::new());
        Self {
            path: path.as_ref().to_path_buf(),
            stream,
        }
    }
}

impl Stream for Tail {
    type Item = Result<String, failure::Error>;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        let stream = unsafe { Pin::new_unchecked(&mut self.stream) };

        match ready!(stream.poll_next(cx)) {
            Some(Ok(line)) => return Poll::Ready(Some(Ok(line))),
            Some(Err(e)) => {
                return Poll::Ready(Some(Err(failure::err_msg(e.to_string()))));
            }
            None => return Poll::Ready(None),
        }
    }
}
