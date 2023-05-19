# localsend-rs

a cli for localsend

<div align="center">
  <video src="https://github.com/notjedi/localsend-rs/assets/30691152/6bedeb44-1dd8-4f72-8a8d-c1c2be715a26" type="video/mp4"></video>
</div>

the current idea for sending files is to make the server an Arc type in the bin
and spawn 2 tokio tasks - one to listen for messages from client and call
corresponding methods to send files and another one to do what we do now which
is to recv files.

a small todo: `use mem::take` when ever possible, to avoid clones.

## Roadmap

- [x] receive files
- [ ] send files
- [ ] handle connection reset errors and cancel requests when sending and receiving files
- [ ] progress for sending files
- [x] pass config from bin to lib
- [ ] config file for device name, default port, etc
- [ ] Support protocol `v2`
- [ ] fix `Illegal SNI hostname received` from dart side
