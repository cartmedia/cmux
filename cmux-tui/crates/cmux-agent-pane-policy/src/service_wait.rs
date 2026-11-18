//! What the pane's "waiting for the agent service" line says, and whether it offers Restart agent
//! service, from what the host saw of this app's acpmux while a handshake waits on it.
//!
//! Leo, 2026-11-18 ~23:10Z, on "Waiting for the agent service (1:10)  Restart agent service": "i
//! cant connect to agent service? something? :(". The daemon was alive and slow (`acpmux daemon
//! status` took 4.9 to 11.8 s), so a restart was the wrong thing to offer: it reloads every chat and
//! a daemon that answers late is not a crash (rule R5). The line now says which it is:
//!
//! - [`WaitState::Busy`]: a process listens on the socket and took the connection, but the reply is
//!   late. Restart is offered only after [`BUSY_RESTART_AFTER_MS`] of waiting, as quiet text.
//! - [`WaitState::Starting`]: the daemon's process runs but nothing accepts on its socket yet (a start
//!   that loads its chats). Restart only after the same long wait.
//! - [`WaitState::Down`]: no daemon process, and nothing accepts on the socket: Restart at once.
//!
//! The host probes (the pid file's process, a connect to the socket); this module only decides.
//! No I/O, no clock: `waited_ms` is the host's.

use serde_json::{Value, json};

/// How long a wait on a daemon that is alive (busy or starting) lasts before the line offers
/// Restart agent service. Status replies under load took up to 12 s (2026-11-18); minutes of no
/// answer from a live daemon is a wedge a person may want to end.
pub const BUSY_RESTART_AFTER_MS: u64 = 3 * 60 * 1000;

/// What the host saw of the daemon a handshake waits on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Probe {
    /// The home's `daemon.pid` names a running acpmux process.
    pub process_alive: bool,
    /// A connect to the home's socket succeeded (a process listens there).
    pub socket_accepts: bool,
}

/// What the waiting line says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WaitState {
    /// Alive and listening, the reply is late.
    Busy,
    /// The process runs, its socket does not accept yet.
    Starting,
    /// No process and no listener.
    Down,
}

impl WaitState {
    pub fn name(self) -> &'static str {
        match self {
            WaitState::Busy => "busy",
            WaitState::Starting => "starting",
            WaitState::Down => "down",
        }
    }
}

/// The line's state and whether it offers Restart agent service.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Verdict {
    pub state: WaitState,
    pub offers_restart: bool,
}

/// The verdict for `probe` after `waited_ms` of waiting. A listener on the socket is the daemon
/// answering slowly, whatever the pid file says (a pid file a start has not written yet).
pub fn classify(probe: Probe, waited_ms: u64) -> Verdict {
    let state = if probe.socket_accepts {
        WaitState::Busy
    } else if probe.process_alive {
        WaitState::Starting
    } else {
        WaitState::Down
    };
    let offers_restart = match state {
        WaitState::Down => true,
        WaitState::Busy | WaitState::Starting => waited_ms >= BUSY_RESTART_AFTER_MS,
    };
    Verdict { state, offers_restart }
}

/// The `service_wait` op: `{process_alive, socket_accepts, waited_ms}` to
/// `{"state": "busy"|"starting"|"down", "restart": bool, "restart_after_ms": n}`. None for a request
/// missing a key or with a key of another type.
pub fn call(request: &Value) -> Option<Value> {
    let flag = |key: &str| request.get(key).and_then(Value::as_bool);
    let probe =
        Probe { process_alive: flag("process_alive")?, socket_accepts: flag("socket_accepts")? };
    let waited_ms = request.get("waited_ms").and_then(Value::as_u64)?;
    let verdict = classify(probe, waited_ms);
    Some(json!({
        "state": verdict.state.name(),
        "restart": verdict.offers_restart,
        "restart_after_ms": BUSY_RESTART_AFTER_MS,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALIVE: Probe = Probe { process_alive: true, socket_accepts: true };
    const STARTING: Probe = Probe { process_alive: true, socket_accepts: false };
    const DOWN: Probe = Probe { process_alive: false, socket_accepts: false };

    #[test]
    fn a_slow_daemon_is_busy_and_offers_no_restart_at_first() {
        // Leo's case: 1:10 of waiting on a daemon that listens and answers late.
        assert_eq!(
            classify(ALIVE, 70_000),
            Verdict { state: WaitState::Busy, offers_restart: false }
        );
    }

    #[test]
    fn a_slow_daemon_offers_restart_only_after_a_long_wait() {
        assert!(!classify(ALIVE, BUSY_RESTART_AFTER_MS - 1).offers_restart);
        assert_eq!(
            classify(ALIVE, BUSY_RESTART_AFTER_MS),
            Verdict { state: WaitState::Busy, offers_restart: true }
        );
    }

    #[test]
    fn a_listener_without_a_pid_file_is_busy() {
        let probe = Probe { process_alive: false, socket_accepts: true };
        assert_eq!(classify(probe, 30_000).state, WaitState::Busy);
    }

    #[test]
    fn a_process_whose_socket_refuses_is_starting() {
        assert_eq!(
            classify(STARTING, 30_000),
            Verdict { state: WaitState::Starting, offers_restart: false }
        );
        assert!(classify(STARTING, BUSY_RESTART_AFTER_MS).offers_restart);
    }

    #[test]
    fn no_process_and_no_listener_is_down_and_offers_restart_at_once() {
        assert_eq!(classify(DOWN, 0), Verdict { state: WaitState::Down, offers_restart: true });
    }

    #[test]
    fn the_op_answers_json_and_refuses_a_partial_request() {
        let reply =
            call(&json!({"process_alive": true, "socket_accepts": true, "waited_ms": 70_000}))
                .unwrap();
        assert_eq!(
            reply,
            json!({"state": "busy", "restart": false, "restart_after_ms": BUSY_RESTART_AFTER_MS})
        );
        let down = call(&json!({"process_alive": false, "socket_accepts": false, "waited_ms": 1}))
            .unwrap();
        assert_eq!(down["state"], "down");
        assert_eq!(down["restart"], true);
        assert!(call(&json!({"process_alive": true, "waited_ms": 1})).is_none());
        assert!(
            call(&json!({"process_alive": true, "socket_accepts": "yes", "waited_ms": 1}))
                .is_none()
        );
        assert!(
            call(&json!({"process_alive": true, "socket_accepts": true, "waited_ms": -1}))
                .is_none()
        );
    }
}
