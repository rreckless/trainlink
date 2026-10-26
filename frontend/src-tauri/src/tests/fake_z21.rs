//! A fake Z21 on this machine, for tests of anything that talks to a command station over UDP. It records every
//! frame the app sends, and sends back the frames a test gives it, as the Z21 would

use std::net::UdpSocket;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Frames the fake Z21 received, and a way to reply to the app
pub struct FakeZ21 {
    pub port: u16,
    pub received: Arc<Mutex<Vec<Vec<u8>>>>,
    reply: Arc<Mutex<Option<std::net::SocketAddr>>>,
    socket: Arc<UdpSocket>,
}

pub fn fake_z21() -> FakeZ21 {
    let socket = Arc::new(UdpSocket::bind("127.0.0.1:0").expect("bind fake Z21"));
    socket.set_read_timeout(Some(Duration::from_millis(200))).ok();
    let port = socket.local_addr().expect("address").port();
    let received = Arc::new(Mutex::new(Vec::new()));
    let reply = Arc::new(Mutex::new(None));
    let (socket_clone, received_clone, reply_clone) = (socket.clone(), received.clone(), reply.clone());
    std::thread::spawn(move || {
        let mut buffer = [0u8; 1024];
        loop {
            match socket_clone.recv_from(&mut buffer) {
                Ok((length, from)) => {
                    *reply_clone.lock().unwrap() = Some(from);
                    received_clone.lock().unwrap().push(buffer[..length].to_vec());
                }
                Err(_) => {
                    if Arc::strong_count(&socket_clone) <= 1 {
                        break;
                    }
                }
            }
        }
    });
    FakeZ21 { port, received, reply, socket }
}

impl FakeZ21 {
    /// Send a frame to the app as the Z21 would
    pub fn say(&self, frame: &[u8]) {
        // The app's address is known once it has sent its first frame, so wait for that
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            let to = *self.reply.lock().unwrap();
            if let Some(to) = to {
                self.socket.send_to(frame, to).expect("send to app");
                return;
            }
            assert!(Instant::now() < deadline, "the app never sent a frame to the fake Z21");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    pub fn saw(&self, expected: &[u8]) -> bool {
        self.received.lock().unwrap().iter().any(|frame| frame == expected)
    }

    /// Wait until the fake has received this frame, or give up after two seconds
    pub fn wait_for(&self, expected: &[u8]) -> bool {
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline {
            if self.saw(expected) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        false
    }
}
