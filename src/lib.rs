#![deny(warnings)]
#![deny(clippy::all)]

use bytes::BytesMut;
use failure::Error;
use futures::{try_ready, Async, Poll, Stream};
#[cfg(unix)]
use inotify::{EventMask, EventStream, Inotify, WatchMask};
use log::*;
#[cfg(windows)]
use notify::{watcher, RecursiveMode};

// use tokio::prelude::*;


use std::{
    ffi::OsStr,
    fs::{self},
    io::SeekFrom,
    path::*,
};

use tokio::{
    codec::{Decoder, LinesCodec},
    prelude::Future,
};

fn get_file_size(path: &dyn AsRef<OsStr>) -> Result<usize, Error> {
    let meta = fs::metadata(path.as_ref())?;
    return Ok(meta.len() as usize);
}

pub struct Tail {
    _file: PathBuf,
    s: StreamFarmedRead<FileStream, LinesCodec>,
}

impl Tail {
    pub fn new<P: AsRef<OsStr> + ?Sized>(path: &P) -> Result<Self, Error> {
        // let path = path.as_ref().to_os_string();
        let s = StreamFarmedRead::new(FileStream::new(path)?, LinesCodec::new());
        Ok(Self {
            _file: PathBuf::from(path),
            s,
        })
    }
}

impl Stream for Tail {
    type Item = String;
    type Error = Error;
    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        return self.s.poll();
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum FileEvent {
    Modify,
    Delete,
}


#[cfg(windows)]
mod file_watcher_impl {
    use super::*;
    use notify::DebouncedEvent;
    use tokio::sync::mpsc::{Sender,Receiver};
    use std::{thread::{self,JoinHandle},time::Duration};
    use futures::sink::Sink;
    use notify::Watcher;
    type FileEventWrapper = Result<Option<FileEvent>, Error>;

    fn send_event(tx: Sender<FileEventWrapper>, event: FileEventWrapper) {
        tokio::run(futures::lazy(move || {
            tokio::spawn(futures::lazy(move || {
                tx.clone().send(event).and_then(|_| Ok(())).map_err(|_| ())
            }))
        }));
    }

    pub struct FileWatcher {
        _file: PathBuf,
        _thread_handle: JoinHandle<()>,
        rx: Receiver<FileEventWrapper>,
    }

    impl FileWatcher {
        pub fn new<P: AsRef<OsStr> + ?Sized>(path: &P) -> Self {
            let path = path.as_ref().to_os_string();
            let (event_tx, event_rx) = tokio::sync::mpsc::channel(1);
            let file = PathBuf::from(path);
            let file_clone = file.clone();
            let notify_thread = thread::spawn(move || {
                let (notify_tx, notify_rx) = std::sync::mpsc::channel();
                let mut watcher = watcher(notify_tx, Duration::from_nanos(1)).unwrap();
                info!("file exist {}", file_clone.exists());
                watcher
                    .watch(&file_clone, RecursiveMode::NonRecursive)
                    .unwrap();
                loop {
                    info!("FileWatcher loop");
                    let res = notify_rx.recv();
                    info!("FileWatcher event {:?}", res);

                    match res {
                        Ok(event) => {
                            info!("notify event {:?}", event);
                            match event {
                                DebouncedEvent::Write(_) => {
                                    send_event(event_tx.clone(), Ok(Some(FileEvent::Modify)));
                                }
                                DebouncedEvent::Remove(_) => {
                                    send_event(event_tx.clone(), Ok(Some(FileEvent::Delete)));
                                    break;
                                }
                                DebouncedEvent::Error(e, _) => {
                                    send_event(
                                        event_tx.clone(),
                                        Err(failure::err_msg(format!("{:?}", e))),
                                    );
                                    break;
                                }
                                _ => {}
                            }
                        }
                        Err(e) => {
                            send_event(event_tx.clone(), Err(failure::err_msg(format!("{:?}", e))));
                            break;
                        }
                    }
                }
            });

            Self {
                _file: file,
                _thread_handle: notify_thread,
                rx: event_rx,
            }
        }
    }

    impl Stream for FileWatcher {
        type Item = FileEvent;
        type Error = Error;
        fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
            info!("FileWatcher poll");
            match try_ready!(self.rx.poll()) {
                Some(e) => match e {
                    Ok(Some(e)) => {
                        return Ok(Async::Ready(Some(e)));
                    }
                    Ok(None) => {
                        info!("get a none");
                        return Ok(Async::Ready(None));
                    }
                    Err(e) => {
                        info!("e {:?}", e);
                        return Err(e);
                    }
                },
                None => {
                    return Ok(Async::Ready(None));
                }
            }
        }
    }
}

#[cfg(unix)]
mod file_watcher_impl {
    use super::*;
    pub struct FileWatcher {
        _file: PathBuf,
        inotify_event_stream: EventStream<[u8; 32]>,
        is_delete: bool,
    }
    impl FileWatcher {
        pub fn new<P: AsRef<OsStr> + ?Sized>(path: &P) -> Self {
            let mut inotify = Inotify::init().unwrap();
            let path = path.as_ref();
            inotify
                .add_watch(
                    path,
                    WatchMask::MODIFY | WatchMask::DELETE_SELF | WatchMask::DELETE,
                )
                .unwrap();
            let stream = inotify.event_stream([0; 32]);
            let file = PathBuf::from(path);
            Self {
                _file:file,
                is_delete: false,
                inotify_event_stream: stream,
            }
        }
    }

    impl Stream for FileWatcher {
        type Item = FileEvent;
        type Error = Error;

        fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
            info!("FileWatcher poll");
            if self.is_delete {
                return Ok(Async::Ready(None));
            }
            match try_ready!(self.inotify_event_stream.poll()) {
                Some(event) => {
                    info!("FileWatcher event {:?}", event.mask);
                    if event.mask == EventMask::DELETE || event.mask == EventMask::DELETE_SELF {
                        self.is_delete = true;
                        return Ok(Async::Ready(Some(FileEvent::Delete)));
                    }
                    return Ok(Async::Ready(Some(FileEvent::Modify)));
                }
                None => {
                    info!("Async::Ready(None)");
                    return Ok(Async::Ready(None));
                }
            }
        }
    }

}

use file_watcher_impl::*;

pub struct FileStream {
    path: PathBuf,
    offset: usize,
    file_watcher: FileWatcher,
}

impl FileStream {
    pub fn new<P: AsRef<OsStr> + ?Sized>(path: &P) -> Result<Self, Error> {
        Ok(Self {
            offset: get_file_size(&path).unwrap_or(0),
            path: PathBuf::from(path),
            file_watcher: FileWatcher::new(path),
        })
    }
}

impl Stream for FileStream {
    type Item = Vec<u8>;
    type Error = Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        info!("FileStream poll");
        loop {
            info!("FileStream loop");
            match try_ready!(self.file_watcher.poll()) {
                Some(event) => {
                    info!("FileStream event {:?}", event);
                    if event == FileEvent::Delete {
                        return Ok(Async::Ready(None));
                    } else if event == FileEvent::Modify {
                        let mut f = tokio::fs::File::open(self.path.clone())
                            .and_then(|f| f.seek(SeekFrom::Start(self.offset as u64)))
                            .and_then(|(f, _)| tokio::io::read_to_end(f, vec![]));
                        let (_, buff) = try_ready!(f.poll());

                        self.offset += buff.len();
                        return Ok(Async::Ready(Some(buff)));
                    }
                }
                None => {
                    info!("FileStream event noone");
                    return Ok(Async::Ready(None));
                }
            }
        }
    }
}

struct StreamFarmedRead<T, D> {
    is_readable: bool,
    stream: T,
    buffer: BytesMut,
    decoder: D,
    is_eof: bool,
}

impl<T, D> StreamFarmedRead<T, D>
where
    T: Stream<Item = Vec<u8>, Error = Error>,
    D: Decoder,
{
    pub fn new(stream: T, decoder: D) -> StreamFarmedRead<T, D> {
        StreamFarmedRead {
            is_readable: false, //could we start decode
            is_eof: false,      //is there more data
            stream,
            decoder: decoder,
            buffer: BytesMut::new(),
        }
    }
}

//https://github.com/tokio-rs/tokio/blob/master/tokio-codec/src/framed_read.rs
impl<T, D> Stream for StreamFarmedRead<T, D>
where
    T: Stream<Item = Vec<u8>, Error = Error>,
    D: Decoder,
{
    type Item = D::Item;
    type Error = Error;
    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        loop {
            if self.is_readable {
                if self.is_eof {
                    let frame = self
                        .decoder
                        .decode_eof(&mut self.buffer)
                        .map_err(|_| failure::err_msg("xxx"))?;
                    return Ok(Async::Ready(frame));
                }
                if let Some(frame) = self
                    .decoder
                    .decode(&mut self.buffer)
                    .map_err(|_| failure::err_msg("xxx"))?
                {
                    return Ok(Async::Ready(Some(frame)));
                }
                self.is_readable = false;
            }
            match try_ready!(self.stream.poll()) {
                Some(buff) => {
                    self.buffer.extend_from_slice(&buff);
                }
                None => {
                    info!("get eof");
                    self.is_eof = true;
                }
            }
            self.is_readable = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use simplelog::{ConfigBuilder, LevelFilter, TermLogger, TerminalMode};
    use std::str::FromStr;
    use std::{fs, thread, time::Duration};

    fn now() -> String {
        use std::time::{SystemTime, UNIX_EPOCH};
        let n = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        n.as_secs().to_string()
    }

    fn init_log() {
        let log_config = ConfigBuilder::new()
            .set_thread_level(LevelFilter::Error)
            .build();
        let _ = TermLogger::init(LevelFilter::Debug, log_config, TerminalMode::Mixed);
    }
    #[cfg(windows)]
    fn write_file(data: String, file: String) {
        use std::process::Command;
        let _res = Command::new("cmd")
            .args(&["/c", &format!("echo {} >> {}", data, file)])
            .spawn();
    }

    #[cfg(unix)]
    fn write_file(data: String, file: String) {
        use std::fs::OpenOptions;
        use std::io::Write;
        let mut file = OpenOptions::new().append(true).open(file).unwrap();
        let _ = file.write_all(format!("{}\n", data).as_bytes());
    }

    fn append_file(log_path_str: &'static str, total_count: usize, delay_secs: usize) {
        info!("append_file");
        let log_path = PathBuf::from(log_path_str);

        let delay = Duration::from_secs(delay_secs as u64);
        let _ = fs::remove_file(&log_path);
        let _ = fs::File::create(&log_path).unwrap();

        let long_content = "1".repeat(3);
        let _ = thread::spawn(move || {
            info!("append file thread start");
            thread::sleep(Duration::from_secs(3));
            for i in 0..total_count {
                info!("{}/{}", i, total_count);
                let now = now();
                let data = format!("{} {} {}", now, i, long_content);
                info!("write data {} {} len {}", now, i, now.as_bytes().len());
                write_file(data, log_path_str.to_string());
                thread::sleep(delay);
            }
            info!("over");
            thread::sleep(Duration::from_secs(1));

            let _ = fs::remove_file(log_path);
        });
    }

    fn abs_sub(l: u128, r: u128) -> u128 {
        if l > r {
            return l - r;
        }
        return r - l;
    }
    #[test]
    fn test_notify() {
        let log_config = ConfigBuilder::new()
            .set_thread_level(LevelFilter::Error)
            .build();
        let _ = TermLogger::init(LevelFilter::Debug, log_config, TerminalMode::Mixed);
        append_file("./data", 10, 5);
    }

    #[test]
    fn test_file_watcher_2() {
        init_log();
        let path_str = "./data";
        append_file(path_str, 5, 1);
        for i in FileWatcher::new(path_str).wait() {
            info!("i => {:?}", i);
        }
    }

    #[test]
    fn test_file_stream_1() {
        use tokio_threadpool::ThreadPool;
        init_log();
        let path_str = "./data";
        append_file(path_str, 5, 1);

        let thread_pool = ThreadPool::new();

        thread_pool.spawn(futures::lazy(move || {
            for i in FileStream::new(path_str).unwrap().wait() {
                info!("i {:?}", i);
            }
            Ok(())
        }));

        thread_pool.shutdown().wait().unwrap();
    }

    #[test]
    fn test_file_watcher_1() {
        init_log();
        info!("test_file_watcher");
        let path_str = "./data";
        append_file(path_str, 5, 1);
        use std::sync::mpsc::channel;
        let (tx, rx) = channel();
        let _path = PathBuf::from(path_str);
        let f = FileWatcher::new(path_str)
            .for_each(move |e| {
                info!("===========> get event e {:?}", e.clone());
                let _ = tx.clone().send(e);
                Ok(())
            })
            .map_err(|_e| ());

        tokio::run(f);
        let res: Vec<FileEvent> = rx.iter().collect();
        assert_eq!(
            res,
            vec![
                FileEvent::Modify,
                FileEvent::Modify,
                FileEvent::Modify,
                FileEvent::Modify,
                FileEvent::Modify,
                FileEvent::Delete,
            ]
        );
        info!("res {:?}", res);
    }

    #[test]
    fn test_file_stream_2() {
        init_log();
        info!("test_file_watcher");
        let path_str = "./data";
        append_file(path_str, 5, 1);
        use std::sync::mpsc::channel;
        let (tx, _rx) = channel();
        let _path = PathBuf::from(path_str);
        let f = FileStream::new(path_str)
            .unwrap()
            .for_each(move |e| {
                let _ = tx.clone().send(e.clone());
                info!("{:?}", String::from_utf8(e));
                Ok(())
            })
            .map_err(|_e| ());

        tokio::run(f);
    }

    #[test]
    fn test_tail() {
        init_log();
        let path_str = "./data";
        let total_count = 5;
        append_file(path_str, total_count, 0);

        let _expect_count = 0;
        let f = Tail::new(path_str)
            .unwrap()
            .for_each(|line| {
                let current_time = u128::from_str(&now()).unwrap();
                let items: Vec<String> = line.split_whitespace().map(|s| s.to_string()).collect();
                let (t, count) = (items[0].clone(), items[1].clone());
                let real_time = u128::from_str(&t).unwrap();
                print!(
                    "tail {} {} {} {:?}\n",
                    count,
                    real_time,
                    current_time,
                    abs_sub(current_time, real_time),
                );
                Ok(())
            })
            .map_err(|_| ());
        tokio::run(f);
    }
}
