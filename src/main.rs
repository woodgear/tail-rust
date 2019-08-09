extern crate futures;
extern crate inotify;

use futures::{Async, Poll, Stream};
use inotify::{EventOwned, EventStream, Inotify, WatchMask};
use std::{
    fs::{self,File},
    io,
    path::*,
    thread,
    time::Duration,
    io::SeekFrom,
    io::prelude::*,
};

struct Tail<'a> {
    buff: Vec<u8>,
    lines: Vec<String>,
    file: PathBuf,
    buf_pos: u32,
    count: u32,
    inotify_event_stream: EventStream<&'a mut [u8; 32]>,
}

fn get_file_size(path: &Path) -> u32 {
    let meta = fs::metadata(path).unwrap();
    // println!("get_file_size {}",meta.len());
    return meta.len() as u32;
}

fn read_file(path: &Path, from: u32, buf: &mut Vec<u8>)->u32 {
    let mut file = File::open(path).unwrap();
    let start = file.seek(SeekFrom::Start(from as u64)).unwrap();
    return file.read(buf).unwrap() as u32;
}

impl<'a> Tail<'a> {
    fn new(path: &str, buff: &'a mut [u8; 32]) -> Result<Self, io::Error> {
        let mut inotify = Inotify::init().expect("Failed to initialize inotify");

        inotify.add_watch("./log", WatchMask::MODIFY | WatchMask::DELETE_SELF|WatchMask::DELETE)?;
        let stream = inotify.event_stream(buff);
        let file = PathBuf::from(path);
        Ok(Self {
            buff: vec![],
            lines: vec![],
            count: 0,
            buf_pos: get_file_size(file.as_path()),
            file,
            inotify_event_stream: stream,
        })
    }

    fn buffer_to_lines(&mut self) {
        let index = match self.buff.iter().rev().position(|c| *c==10 as u8) {
            Some(index)=> index,
            None=> return
        };

        let line_buff = self.buff.split_off(index);
        let lines = String::from_utf8(line_buff).unwrap();
        self.lines.extend(lines.lines().map(|s|s.to_string()));
    }

    fn fill_buffer(&mut self) {
        let full_size = get_file_size(&self.file.as_path());
        let mut buf = vec![0;1024];
        let data_readed = read_file(&self.file,self.buf_pos,&mut buf);

        let b = &buf[0..data_readed as usize];
        // println!("buff {:?} {}",b,data_readed);

        self.buff.extend(b);
        self.buf_pos+=data_readed;
        // println!("new buf_pos {} {}",self.buf_pos,data_readed);
    }
}

impl<'a> Stream for Tail<'a> {
    type Item = String;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        self.fill_buffer();
        self.buffer_to_lines();
        if !self.lines.is_empty() {
            return Ok(Async::Ready(Some(self.lines.pop().unwrap())));
        }
        loop {
            match self.inotify_event_stream.poll() {
                Ok(Async::Ready(Some(event))) => {
                    self.fill_buffer();
                    self.buffer_to_lines();
                    if !self.lines.is_empty() {
                        return Ok(Async::Ready(Some(self.lines.pop().unwrap())));
                    }
                }
                Ok(Async::Ready(None)) => {
                    println!("inotify_event_stream Ready None");
                    return Ok(Async::Ready(None));
                }
                Ok(Async::NotReady) => {
                    println!("inotify_event_stream NotReady");
                    return Ok(Async::NotReady);
                }
                Err(e) => {
                    println!("inotify_event_stream err {:?}", e);
                }
            }
        }
        return Ok(Async::NotReady);
    }
}

fn main() -> Result<(), io::Error> {
    let mut buff = [0; 32];
    let tail = Tail::new("./log", &mut buff)?;
    for line in tail.wait() {
        print!("event: {:?}\n", line);
    }
    Ok(())
}
