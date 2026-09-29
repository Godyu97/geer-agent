use std::sync::{
    Mutex,
    atomic::{AtomicU64, Ordering},
    mpsc::{self, Sender},
};

type AuthNotifier = dyn Fn(u64, &str) -> bool + Send + Sync;

pub(crate) struct AuthorizationGate {
    notify: Box<AuthNotifier>,
    next_id: AtomicU64,
    pending: Mutex<Option<(u64, Sender<bool>)>>,
}

impl AuthorizationGate {
    pub(crate) fn new(notify: impl Fn(u64, &str) -> bool + Send + Sync + 'static) -> Self {
        Self {
            notify: Box::new(notify),
            next_id: AtomicU64::new(1),
            pending: Mutex::new(None),
        }
    }

    pub(crate) fn request(&self, prompt: &str) -> bool {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (reply, answer) = mpsc::channel();
        *self.pending.lock().expect("授权锁损坏") = Some((id, reply));
        if !(self.notify)(id, prompt) {
            self.cancel();
            return false;
        }
        let allowed = answer.recv().unwrap_or(false);
        let mut pending = self.pending.lock().expect("授权锁损坏");
        if pending
            .as_ref()
            .is_some_and(|(pending_id, _)| *pending_id == id)
        {
            pending.take();
        }
        allowed
    }

    pub(crate) fn respond(&self, id: u64, allowed: bool) -> bool {
        let mut pending = self.pending.lock().expect("授权锁损坏");
        if pending
            .as_ref()
            .is_some_and(|(pending_id, _)| *pending_id == id)
            && let Some((_, reply)) = pending.take()
        {
            return reply.send(allowed).is_ok();
        }
        false
    }

    pub(crate) fn cancel(&self) {
        if let Some((_, reply)) = self.pending.lock().expect("授权锁损坏").take() {
            let _ = reply.send(false);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{sync::Arc, sync::mpsc, time::Duration};

    use super::AuthorizationGate;

    #[test]
    fn denied_and_late_replies_never_authorize_next_tool() {
        let (sent, received) = mpsc::channel();
        let gate = Arc::new(AuthorizationGate::new(move |id, prompt| {
            sent.send((id, prompt.to_owned())).is_ok()
        }));
        let pending = Arc::clone(&gate);
        let first = std::thread::spawn(move || pending.request("write /tmp/a"));
        let (id, prompt) = received.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(prompt, "write /tmp/a");
        assert!(gate.respond(id, false));
        assert!(!first.join().unwrap());
        assert!(!gate.respond(id, true));

        let pending = Arc::clone(&gate);
        let second = std::thread::spawn(move || pending.request("bash dangerous"));
        let (next_id, _) = received.recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(next_id > id);
        gate.cancel();
        assert!(!second.join().unwrap());
        assert!(!gate.respond(next_id, true));
    }

    #[test]
    fn frontend_disconnect_defaults_to_denial() {
        let gate = AuthorizationGate::new(|_, _| false);
        assert!(!gate.request("write /tmp/a"));
    }
}
