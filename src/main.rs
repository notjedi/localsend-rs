use localsend_core::Server;
use std::{
    io::Read,
    net::{TcpListener, TcpStream},
};

fn main() {
    let mut server = Server::new();
    // https://github.com/localsend/protocol/issues/1#issuecomment-1426998509
    for _ in 0..localsend_core::NUM_REPEAT {
        server.announce_multicast(true);
    }
    // server.listen_and_announce_multicast();

    // /api/localsend/v1/send-request
    let listener = TcpListener::bind("192.168.1.2:53317").unwrap();
    for stream in listener.incoming() {
        let mut buf = [0u8; 4096];
        let mut stream = stream.unwrap();
        match stream.read(&mut buf) {
            Ok(size) => {
                println!("{:?}", &buf[..size]);
                // println!("{:?}", std::str::from_utf8(&buf[..size]));
                println!("{:?}", String::from_utf8_lossy(&buf[..]));
                println!("{:?}", size);
            }
            Err(e) => {
                println!("{:?}", e);
            }
        }
    }

    // let mut stream = TcpStream::connect("192.168.1.8:53317").unwrap();
    // match stream.read(&mut buf) {
    //     Ok(size) => {}
    //     Err(_) => {}
    // }
}
