use localsend_core::Server;

fn main() {
    let mut server = Server::new();
    // https://github.com/localsend/protocol/issues/1#issuecomment-1426998509
    for _ in 0..localsend_core::NUM_REPEAT {
        server.announce_multicast(true);
    }
    server.listen_multicast_annoucement();
}
