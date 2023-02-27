use localsend_core::Server;

fn main() {
    let server = Server::new();
    server.announce_multicast();
    server.listen_multicast_annoucement();
}
