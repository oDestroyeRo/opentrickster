use opentrickster_engine::{Mode, SessionKey};
use opentrickster_net::{NativeSession, NetCredentials, NetError, VerifiedProfile};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};

pub enum Prepared {
    Embedded(VerifiedProfile),
    Native(NativeSession),
}
pub enum Completion {
    Prepared {
        session: SessionKey,
        result: Result<Prepared, NetError>,
    },
    Closed {
        session: SessionKey,
    },
}
pub struct Background {
    sender: mpsc::SyncSender<Completion>,
    receiver: mpsc::Receiver<Completion>,
}
impl Background {
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::sync_channel(4);
        Self { sender, receiver }
    }
    pub fn prepare(
        &self,
        session: SessionKey,
        mode: Mode,
        credentials: Option<NetCredentials>,
        cancel: Arc<AtomicBool>,
        cache: PathBuf,
    ) {
        let sender = self.sender.clone();
        thread::spawn(move || {
            let result = (|| {
                let profile = VerifiedProfile::fetch_public(&cache)?;
                if cancel.load(Ordering::Acquire) {
                    return Err(NetError::Cancelled);
                }
                match mode {
                    Mode::Assisted => Ok(Prepared::Embedded(profile)),
                    Mode::Clientless => NativeSession::open(
                        session,
                        profile,
                        credentials.ok_or(NetError::InvalidCredentials)?,
                    )
                    .map(Prepared::Native),
                }
            })();
            if cancel.load(Ordering::Acquire) {
                close_prepared(result.ok());
                post(&sender, Completion::Closed { session });
            } else {
                post(&sender, Completion::Prepared { session, result });
            }
        });
    }
    pub fn close(&self, session: SessionKey, native: NativeSession) {
        let sender = self.sender.clone();
        thread::spawn(move || {
            close_prepared(Some(Prepared::Native(native)));
            post(&sender, Completion::Closed { session });
        });
    }
    pub fn next(&self) -> Option<Completion> {
        self.receiver.try_recv().ok()
    }
}
fn post(sender: &mpsc::SyncSender<Completion>, event: Completion) {
    // Only background workers send. Backpressure preserves the cleanup completion.
    if let Err(mpsc::SendError(Completion::Prepared {
        result: Ok(prepared),
        ..
    })) = sender.send(event)
    {
        close_prepared(Some(prepared));
    }
}
fn close_prepared(prepared: Option<Prepared>) {
    if let Some(Prepared::Native(mut native)) = prepared {
        while native.close().is_err() {
            // A timeout or joined panic is not the definitive cleanup receipt.
            thread::park_timeout(Duration::from_millis(100));
        }
    }
}
pub fn public_cache() -> Result<PathBuf, NetError> {
    let directory = std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or(NetError::Io)?;
    Ok(directory
        .join("Library/Caches/org.opentrickster.desktop/public")
        .join(opentrickster_net::BUILD))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completion_backpressure_preserves_all_closes() {
        use opentrickster_engine::{At, Command, Effect, Engine, Input, RequestKey};
        let session = Engine::new(Mode::Assisted)
            .handle(Input::User {
                at: At(0),
                request: RequestKey(1),
                command: Command::Open,
            })
            .io
            .into_iter()
            .find_map(|effect| match effect {
                Effect::Open { session, .. } => Some(session),
                _ => None,
            })
            .unwrap();
        let (sender, receiver) = mpsc::sync_channel(1);
        post(
            &sender,
            Completion::Prepared {
                session,
                result: Err(NetError::Cancelled),
            },
        );
        let worker = thread::spawn(move || {
            post(&sender, Completion::Closed { session });
        });
        assert!(matches!(
            receiver.recv().unwrap(),
            Completion::Prepared { .. }
        ));
        worker.join().unwrap();
        assert!(
            matches!(receiver.recv().unwrap(), Completion::Closed { session: closed } if closed == session)
        );
    }
}
