use futures::{Async, Poll, Stream};
use inotify::{EventStream, Inotify, EventMask, WatchMask};
use failure::Error;
use std::{
    fs::{self, File},
    path::*,
    io::SeekFrom,
    io::prelude::*,
};
use std::ffi::OsStr;

pub struct Tail {
    buff: Vec<u8>,
    lines: Vec<String>,
    file: PathBuf,
    buf_pos: u32,
    inotify_event_stream: EventStream<[u8; 32]>,
}

fn get_file_size(path: &dyn AsRef<OsStr>) -> Result<u32, Error> {
    let meta = fs::metadata(path.as_ref())?;
    return Ok(meta.len() as u32);
}

fn read_file(path: &Path, from: u32, buf: &mut Vec<u8>) -> Result<u32, Error> {
    let mut file = File::open(path)?;
    let _start = file.seek(SeekFrom::Start(from as u64))?;
    return Ok(file.read(buf)? as u32);
}

impl<'a> Tail {
    pub fn new(path: &dyn AsRef<OsStr>) -> Result<Self, Error> {
        let mut inotify = Inotify::init()?;
        let path = path.as_ref();
        inotify.add_watch(path, WatchMask::MODIFY | WatchMask::DELETE_SELF | WatchMask::DELETE)?;
        let stream = inotify.event_stream([0; 32]);
        let file = PathBuf::from(path);
        Ok(Self {
            buff: vec![],
            lines: vec![],
            buf_pos: get_file_size(&file)?,
            file,
            inotify_event_stream: stream,
        })
    }

    fn buffer_to_lines(&mut self) {
        let index = match self.buff.iter().rev().position(|c| *c == 10 as u8) {
            Some(index) => index,
            None => return
        };

        let line_buff = self.buff.split_off(index);
        let lines = String::from_utf8(line_buff).unwrap();
        self.lines.extend(lines.lines().map(|s| s.to_string()));
    }

    fn fill_buffer(&mut self) -> Result<(), Error> {
        if !self.file.exists() {
            return Ok(());
        }
        let mut buf = vec![0; 1024];
        let data_read = read_file(&self.file, self.buf_pos, &mut buf)?;

        let b = &buf[0..data_read as usize];

        self.buff.extend(b);
        self.buf_pos += data_read;
        return Ok(());
    }
}

impl<'a> Stream for Tail {
    type Item = String;
    type Error = Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        if !self.lines.is_empty() {
            return Ok(Async::Ready(Some(self.lines.pop().unwrap())));
        }
        loop {
            match self.inotify_event_stream.poll() {
                Ok(Async::Ready(Some(event))) => {
                    println!("inotify_event_stream Ready {} {:?}", now(), event.mask);
                    if event.mask == EventMask::DELETE || event.mask == EventMask::DELETE_SELF {
                        return Ok(Async::Ready(None));
                    } else {
                        self.fill_buffer()?;
                        self.buffer_to_lines();
                    }

                    if !self.lines.is_empty() {
                        return Ok(Async::Ready(Some(self.lines.pop().unwrap())));
                    }
                }
                Ok(Async::Ready(None)) => {
                    return Ok(Async::Ready(None));
                }
                Ok(Async::NotReady) => {
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
    fn test_tail() {
        use std::{thread, fs, time::Duration, io::Write};

        let log_path = Path::new("./log");
        let _ = fs::remove_file(log_path);
        let mut file = fs::File::create(log_path).unwrap();

        let _ = thread::spawn(move || {
            for i in 0..10 {
                let data = format!("{} {}\n", now(), i);
                let _ = file.write_all(data.as_bytes());
                thread::sleep(Duration::from_micros(100));
            }

            let _ = fs::remove_file(log_path);
        });
        let tail = Tail::new(&log_path).unwrap();
        for line in tail.wait() {
            let current_time = u128::from_str(&now()).unwrap();
            let items: Vec<String> = line.unwrap().split_whitespace().map(|s| s.to_string()).collect();
            let (t,_) = (items[0].clone(), items[1].clone());
            let real_time = u128::from_str(&t).unwrap();
            print!("tail {} {} {:?}\n",
                   real_time,
                   current_time,
                   abs_sub(current_time, real_time),
            );
            assert!(abs_sub(current_time, real_time) < 1000);
        }
    }
}

