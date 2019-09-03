use futures::{Async, Poll, Stream, try_ready};
use inotify::{EventStream, Inotify, EventMask, WatchMask};
use failure::Error;
use std::{fs::{self, File}, path::*, io::SeekFrom, io::prelude::*, thread};
use std::ffi::OsStr;
use std::collections::VecDeque;
use std::thread::JoinHandle;
use tokio::sync::mpsc::{channel, Receiver};
use failure::_core::time::Duration;
use tokio::prelude::{Sink, Future};
use std::sync::mpsc::SendError;
use tokio_core::reactor::Core;

pub struct Tail {
    buff: Vec<u8>,
    lines: VecDeque<String>,
    file: PathBuf,
    buf_pos: usize,
    inotify_event_stream: EventStream<[u8; 32]>,
}

fn get_file_size(path: &dyn AsRef<OsStr>) -> Result<usize, Error> {
    let meta = fs::metadata(path.as_ref())?;
    return Ok(meta.len() as usize);
}

fn read_file(path: &Path, from: usize, buf: &mut Vec<u8>) -> Result<usize, Error> {
    let mut file = File::open(path)?;
    let _start = file.seek(SeekFrom::Start(from as u64))?;
    return Ok(file.read(buf)? as usize);
}

impl<'a> Tail {
    pub fn new(path: &dyn AsRef<OsStr>) -> Result<Self, Error> {
        let mut inotify = Inotify::init()?;
        let path = path.as_ref();
        inotify.add_watch(path, WatchMask::MODIFY | WatchMask::DELETE_SELF | WatchMask::DELETE)?;
        let stream = inotify.event_stream([0; 32]);
        let file = PathBuf::from(path);
        Ok(Self {
            buff: Default::default(),
            lines: Default::default(),
            buf_pos: get_file_size(&file)?,
            file,
            inotify_event_stream: stream,
        })
    }
    fn fill_buffer(&mut self) -> Result<(), Error> {
        if !self.file.exists() {
            return Ok(());
        }
        let full_size = get_file_size(&self.file)? as usize;
        let reside_size = full_size - self.buf_pos;
        println!("fill_buffer {} {} {}", full_size, self.buf_pos, reside_size);
        let mut buf = vec![0; reside_size];
        let data_read = read_file(&self.file, self.buf_pos, &mut buf)?;

        let b = &buf[0..data_read as usize];

        self.buff.extend(b);
        self.buf_pos += data_read;
        return Ok(());
    }

    fn buffer_to_lines(&mut self) -> Result<(), Error> {
        let index = match self.buff.iter().rev().position(|c| *c == 10 as u8) {
            Some(index) => index,
            None => return Ok(())
        };

        let line_buff = self.buff.split_off(index);
        let lines = String::from_utf8(line_buff)?;
        self.lines.extend(lines.lines().map(|s| s.to_string()));
        Ok(())
    }
}

impl<'a> Stream for Tail {
    type Item = String;
    type Error = Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        println!("poll");
        if let Some(line) = self.lines.pop_front() {
            println!("just pop");

            return Ok(Async::Ready(Some(line)));
        }
        loop {
            println!("loop");
            match self.inotify_event_stream.poll() {
                Ok(Async::Ready(Some(event))) => {
                    println!("inotify_event_stream Ready {} {:?}", now(), event.mask);
                    if event.mask == EventMask::DELETE || event.mask == EventMask::DELETE_SELF {
                        return Ok(Async::Ready(None));
                    } else {
                        self.fill_buffer()?;
                        self.buffer_to_lines()?;
                    }

                    if let Some(line) = self.lines.pop_front() {
                        println!("ready some data");
                        return Ok(Async::Ready(Some(line)));
                    }
                }
                Ok(Async::Ready(None)) => {
                    println!("Async::Ready(None)");
                    return Ok(Async::Ready(None));
                }
                Ok(Async::NotReady) => {
                    println!("Async::NotReady");
                    return Ok(Async::NotReady);
                }
                Err(e) => {
                    return Err(failure::err_msg(format!("inotify_event_stream err {:?}", e)));
                }
            }
        }
    }
}

fn now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let n = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    n.as_micros().to_string()
}

struct Test {
    thread_handle: JoinHandle<()>,
    rx: Receiver<String>,
}

impl Test {
    fn new() -> Self {
        let (tx, rx) = channel(1);
        let thread_handle = thread::spawn(move || {
            let mut core = Core::new().unwrap();
            let handle = core.handle();
            let f = tokio::timer::Interval::new_interval(Duration::from_secs(3))
                .for_each(move|_| {
                    let tx = tx.clone();
                    println!("interval foreach");
                    handle.spawn_fn(move || {
                        let count = 0;

                        let msg = format!("{}", count);
                        println!("thread:{:?} start send ", thread::current().id());
                        tx.clone().send(msg).then(|res| {
                            match res {
                                Ok(_) => {
                                    Ok((()))
                                }
                                Err(e) => {
                                    Err(())
                                }
                            }
                        })
                    });
                    Ok(())
                });
            core.run(f);
        });
        return Test { thread_handle, rx };
    }
}

impl Stream for Test {
    type Item = String;
    type Error = Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        println!("poll");
        loop {
            match self.rx.poll() {
                Ok(Async::Ready(Some(msg))) => {
                    println!("ready {}", msg);
                    return Ok(Async::Ready(Some(msg)));
                }
                Err(e) => {
                    println!("err {:?}", e);
                    return Err(failure::err_msg(format!("{:?}", e)));
                }
                Ok(Async::NotReady) => {
                    println!("notready");
                    return Ok(Async::NotReady);
                }
                Ok(Async::Ready(None)) => {
                    println!("none");
                    return Ok(Async::Ready(None));
                }
            }
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;


    fn abs_sub(l: u128, r: u128) -> u128 {
        if l > r {
            return l - r;
        }
        return r - l;
    }

    #[test]
    fn test_tokio() {
        let mut t = Test::new();
        for line in t.wait() {
            println!("thread:{:?} {} {:?}", thread::current().id(), now(), line);
        }
    }

    #[test]
    fn test_tail() {
        use std::{thread, fs, time::Duration, io::Write};
        let total_count = 10;
        let log_path = Path::new("./log");
        let _ = fs::remove_file(log_path);
        let mut file = fs::File::create(log_path).unwrap();
        let long_content = "1".repeat(1024 * 100);
        let _ = thread::spawn(move || {
            for i in 0..total_count {
                let now = now();
                let data = format!("{} {} {}\n", now, i, long_content);
                println!("write data {} {}", now, i);
                let _ = file.write_all(data.as_bytes());
                thread::sleep(Duration::from_micros(1000));
            }
            thread::sleep(Duration::from_secs(1));

            let _ = fs::remove_file(log_path);
        });
        let tail = Tail::new(&log_path).unwrap();
        let mut expect_count = 0;
        for line in tail.wait() {
            let current_time = u128::from_str(&now()).unwrap();
            let items: Vec<String> = line.unwrap().split_whitespace().map(|s| s.to_string()).collect();
            let (t, count) = (items[0].clone(), items[1].clone());
            let real_time = u128::from_str(&t).unwrap();
            print!("tail {} {} {} {:?}\n",
                   count,
                   real_time,
                   current_time,
                   abs_sub(current_time, real_time),
            );
            assert_eq!(count, format!("{}", expect_count));
            expect_count = expect_count + 1;
//            assert!(abs_sub(current_time, real_time) < 1000);
        }
        assert_eq!(expect_count, total_count);
    }
}

