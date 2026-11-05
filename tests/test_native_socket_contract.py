"""Source contracts for Windows IPC handshake and terminal lifecycle."""
from pathlib import Path
import unittest
ROOT = Path(__file__).resolve().parents[1]

class SocketContractTests(unittest.TestCase):
    def test_listener_is_blocking_with_bounded_accept(self):
        code=(ROOT/'rust-core/src/bin/webcodex-native.rs').read_text(encoding='utf-8')
        self.assertIn('rx.recv_timeout(Duration::from_secs(10))',code)
        self.assertIn('std::thread::spawn(move || {let _=tx.send(listener.accept());});',code)
        self.assertNotIn('listener.set_nonblocking(true)',code)
        self.assertIn('socket.set_nonblocking(false)',code)
        self.assertIn('copy.set_nonblocking(false)',code)

    def test_transient_socket_read_is_retried(self):
        agent=(ROOT/'rust-core/src/bin/webcodex-native.rs').read_text(encoding='utf-8')
        tui=(ROOT/'rust-tui/src/main.rs').read_text(encoding='utf-8')
        self.assertIn('e.kind()==io::ErrorKind::WouldBlock',agent)
        self.assertIn('e.kind()==io::ErrorKind::WouldBlock',tui)

    def test_tui_restores_shell_and_ignores_release_events(self):
        code=(ROOT/'rust-tui/src/main.rs').read_text(encoding='utf-8')
        self.assertIn('let _=disable_raw_mode();',code)
        self.assertIn('event::DisableMouseCapture,LeaveAlternateScreen',code)
        self.assertIn('kind == KeyEventKind::Release',code)
        self.assertIn('kind:"exit".into()',code)

    def test_agent_shuts_down_tui_gracefully(self):
        code=(ROOT/'rust-core/src/bin/webcodex-native.rs').read_text(encoding='utf-8')
        self.assertIn('socket.write_all(b"{\\"kind\\":\\"exit',code)
        self.assertIn('process.try_wait()',code)

if __name__=='__main__':unittest.main()
