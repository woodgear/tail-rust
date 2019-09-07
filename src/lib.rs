use futures::{Async, Poll, Stream, try_ready};
#[cfg(unix)]
use inotify::{EventStream, Inotify, EventMask, WatchMask};
use failure::Error;
use std::{fs::{self, File}, path::*, io::SeekFrom, io::prelude::*};
use std::ffi::OsStr;
use std::collections::VecDeque;
use log::*;
use tokio::prelude::{Future, AsyncRead};
use tokio::io;
use tokio::codec::Decoder;
#[cfg(windows)]
use notify::{RecommendedWatcher, RecursiveMode, watcher};

// pub struct Tail {
//     buff: Vec<u8>,
//     lines: VecDeque<String>,
//     file: PathBuf,
//     buf_pos: usize,
//     inotify_event_stream: EventStream<[u8; 32]>,
// }

// fn get_file_size(path: &dyn AsRef<OsStr>) -> Result<usize, Error> {
//     let meta = fs::metadata(path.as_ref())?;
//     return Ok(meta.len() as usize);
// }

// fn read_file(path: &Path, from: usize, buf: &mut Vec<u8>) -> Result<usize, Error> {
//     let mut file = File::open(path)?;
//     let _start = file.seek(SeekFrom::Start(from as u64))?;
//     return Ok(file.read(buf)? as usize);
// }

// impl Tail {
//     pub fn new(path: &dyn AsRef<OsStr>) -> Result<Self, Error> {
//         let mut inotify = Inotify::init()?;
//         let path = path.as_ref();
//         inotify.add_watch(path, WatchMask::MODIFY | WatchMask::DELETE_SELF | WatchMask::DELETE)?;
//         let stream = inotify.event_stream([0; 32]);
//         let file = PathBuf::from(path);
//         Ok(Self {
//             buff: Default::default(),
//             lines: Default::default(),
//             buf_pos: get_file_size(&file)?,
//             file,
//             inotify_event_stream: stream,
//         })
//     }
//     fn fill_buffer(&mut self) -> Result<(), Error> {
//         if !self.file.exists() {
//             return Ok(());
//         }
//         let full_size = get_file_size(&self.file)? as usize;
//         let reside_size = full_size - self.buf_pos;
//         info!("fill_buffer {} {} {}", full_size, self.buf_pos, reside_size);
//         let mut buf = vec![0; reside_size];
//         let data_read = read_file(&self.file, self.buf_pos, &mut buf)?;

//         let b = &buf[0..data_read as usize];

//         self.buff.extend(b);
//         self.buf_pos += data_read;
//         return Ok(());
//     }

//     fn buffer_to_lines(&mut self) -> Result<(), Error> {
//         let index = match self.buff.iter().rev().position(|c| *c == 10 as u8) {
//             Some(index) => index,
//             None => return Ok(())
//         };

//         let line_buff = self.buff.split_off(index);
//         let lines = String::from_utf8(line_buff)?;
//         self.lines.extend(lines.lines().map(|s| s.to_string()));
//         Ok(())
//     }
// }

// impl Stream for Tail {
//     type Item = String;
//     type Error = Error;

//     fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
//         info!("poll");
//         if let Some(line) = self.lines.pop_front() {
//             info!("just pop");
//             return Ok(Async::Ready(Some(line)));
//         }
//         loop {
//             info!("loop");
//             match try_ready!(self.inotify_event_stream.poll()) {
//                 Some(event) => {
//                     info!("inotify_event_stream Ready {} {:?}", now(), event.mask);
//                     if event.mask == EventMask::DELETE || event.mask == EventMask::DELETE_SELF {
//                         return Ok(Async::Ready(None));
//                     } else {
//                         self.fill_buffer()?;
//                         self.buffer_to_lines()?;
//                     }

//                     if let Some(line) = self.lines.pop_front() {
//                         info!("ready some data");
//                         return Ok(Async::Ready(Some(line)));
//                     }
//                 }
//                 None => {
//                     info!("Async::Ready(None)");
//                     return Ok(Async::Ready(None));
//                 }
//             }
//         }
//     }
// }


fn now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let n = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    n.as_secs().to_string()
}

// pub struct FileStream {
//     path: PathBuf,
//     offset: usize,
//     inotify: EventStream<[u8; 32]>,

// }

// impl FileStream {
//     pub fn new(path: &dyn AsRef<OsStr>) -> Result<Self, Error> {
//         let mut inotify = Inotify::init()?;
//         let path = path.as_ref();
//         inotify.add_watch(path, WatchMask::MODIFY | WatchMask::DELETE_SELF | WatchMask::DELETE)?;
//         let stream = inotify.event_stream([0; 32]);
//         let path = PathBuf::from(path);
//         Ok(Self {
//             offset: get_file_size(&path)?,
//             path,
//             inotify: stream,
//         })
//     }
// }

// impl Stream for FileStream {
//     type Item = Vec<u8>;
//     type Error = Error;

//     fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
//         loop {
//             match try_ready!(self.inotify.poll()) {
//                 Some(event) => {
//                     if event.mask == EventMask::DELETE || event.mask == EventMask::DELETE_SELF {
//                         return Ok(Async::Ready(None));
//                     }
//                     let mut f = tokio::fs::File::open("./data")
//                         .and_then(|mut f| f.seek(SeekFrom::Start(self.offset as u64)))
//                         .and_then(|(f, offset)| tokio::io::read_to_end(f, vec![]));
//                     let (f, buff) = try_ready!(f.poll());
//                     return Ok(Async::Ready(Some(buff)));
//                 }
//                 None => {
//                     return Ok(Async::Ready(None));
//                 }
//             }
//         }
//     }
// }
#[cfg(windows)]
mod file_watcher_impl {
    use super::*;
    pub struct FileWatcher {
    }

    impl FileWatcher {
        fn new(path:&'static str) ->Result<Self,failure::Error> {

        }
    }
}

use file_watcher_impl::*;


#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;
    use simplelog::{TermLogger, LevelFilter, TerminalMode, ConfigBuilder};
    use std::{thread, fs, time::Duration, io::Write};

    fn init_log() {
        let log_config = ConfigBuilder::new()
        // .set_target_level(LevelFilter::max())
        .set_thread_level(LevelFilter::max())
        // .set_max_level(LevelFilter::max())
        .build();
        let _ = TermLogger::init(LevelFilter::Debug, log_config, TerminalMode::Mixed);
    }

    fn append_file(file: &'static str,total_count:usize) {
        let log_path = Path::new(file);
        let delay = Duration::from_secs(3);
        let _ = fs::remove_file(log_path);
        let mut file = fs::File::create(log_path).unwrap();
        let long_content = "1".repeat(3);
        let _ = thread::spawn(move || {
            for i in 0..total_count {
                info!("{}/{}",i,total_count);
                let now = now();
                let data = format!("{} {} {}\n", now, i, long_content);
                info!("write data {} {} len {}", now, i,now.as_bytes().len());
                let _ = file.write_all(data.as_bytes());
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
        // .set_target_level(LevelFilter::max())
        .set_thread_level(LevelFilter::Error)
        // .set_max_level(LevelFilter::max())
        .build();
        let _ = TermLogger::init(LevelFilter::Debug, log_config, TerminalMode::Mixed);

        append_file("./data",10);
        use notify::Watcher;

        thread::spawn(move || {
            let (notify_tx,notify_rx) = std::sync::mpsc::channel();
            let mut watcher = watcher(notify_tx,Duration::from_secs(2)).unwrap();
            watcher.watch("./data", RecursiveMode::Recursive).unwrap();
            loop {
                match notify_rx.recv() {
                    Ok(event)=>{
                        info!("notify event {} {:?}",now(),event);
                    }
                    Err(e)=> {
                        error!("e {:?}",e);
                    }
                }
            }
            
        });
    }

    #[test]
    fn test_simple() {
        init_log();
        append_file("./data",100);
        tokio::run(futures::lazy(|| {
            tokio::fs::File::open(Path::new("./data"))
                .and_then(|f| {
                    tokio::codec::Framed::new(f, tokio::codec::LinesCodec::new()).for_each(|line| {
                        info!("rece now {} line {} len size {}", now(), line,line.len());
                        Ok(())
                    })
                }).map_err(|e|()).map(|_|())
        }));
    }

    // #[test]
    // fn test_file_stream() {
    //     init_log();
    //     append_file("./data");
    //     info!("test_file_stream");
    //     let f = FileStream::new(&Path::new("./data")).unwrap();
    //     tokio::run(futures::lazy(|| {
    //         tokio::spawn(f.for_each(|buf| {
    //             println!("buf len {}", buf.len());
    //             Ok(())
    //         }).map_err(|_| ()))
    //     }))
    // }

    // #[test]
    // fn test_tail() {
    //     init_log();

    //     let log_path = "./data";
    //     append_file(log_path);

    //     let total_count = 10;
    //     let tail = Tail::new(&log_path).unwrap();
    //     let mut expect_count = 0;
    //     for line in tail.wait() {
    //         let current_time = u128::from_str(&now()).unwrap();
    //         let items: Vec<String> = line.unwrap().split_whitespace().map(|s| s.to_string()).collect();
    //         let (t, count) = (items[0].clone(), items[1].clone());
    //         let real_time = u128::from_str(&t).unwrap();
    //         print!("tail {} {} {} {:?}\n",
    //                count,
    //                real_time,
    //                current_time,
    //                abs_sub(current_time, real_time),
    //         );
    //         assert_eq!(count, format!("{}", expect_count));
    //         expect_count = expect_count + 1;
    //     }
    //     assert_eq!(expect_count, total_count);
    // }
}

