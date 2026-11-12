//! Helpers for tests of the `Conn` machine.

use estado_core::Machine;

use super::{Conn, Connected, Log};

pub const SESSION: u32 = 7;

/// `Idle/Waiting`, the log cleared.
pub fn started() -> Machine<Conn> {
    let mut machine = Machine::<Conn>::new(Log::default());
    machine.start().expect("start");
    machine.ctx_mut().lines.clear();
    machine
}

/// Takes the log.
pub fn lines(machine: &mut Machine<Conn>) -> Vec<String> {
    std::mem::take(&mut machine.ctx_mut().lines)
}

/// `Connected/link:Opening,keepalive:Pinging`, the log cleared.
pub fn connected() -> Machine<Conn> {
    let mut machine = started();
    machine.send(super::Event::Ring(SESSION)).unwrap();
    assert_eq!(
        machine.path().to_string(),
        "Connected/link:Opening,keepalive:Pinging"
    );
    lines(&mut machine);
    machine
}

pub fn connected_data(machine: &Machine<Conn>) -> &Connected {
    match machine.state() {
        Conn::Connected(connected) => connected,
        other => panic!("expected Connected, got {other:?}"),
    }
}

/// Handler calls, then leaving `Connected` for `Idle` from the given leaves:
/// regions exit in reverse declaration order (`keepalive` first), then the
/// parallel state.
pub fn handled_then_leave(handled: &[&str], link_leaf: &str, keepalive_leaf: &str) -> Vec<String> {
    let mut log: Vec<String> = handled.iter().map(ToString::to_string).collect();
    log.extend([
        format!("exit {keepalive_leaf}"),
        format!("exit {link_leaf}"),
        "exit Connected".into(),
        "entry Idle".into(),
        "entry Waiting".into(),
    ]);
    log
}
