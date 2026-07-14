use std::{
    io,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::mpsc,
    thread,
};

use calloop::channel;

use super::{AuthenticationError, AuthenticationResult, authenticate_current_user};
use crate::{
    input::PasswordAttempt,
    state::{AttemptToken, AuthenticationOutcome},
};

type Authenticator = fn(PasswordAttempt) -> Result<AuthenticationResult, AuthenticationError>;

struct AuthenticationRequest {
    token: AttemptToken,
    password: PasswordAttempt,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticationCompletion {
    pub(crate) token: AttemptToken,
    pub(crate) outcome: AuthenticationOutcome,
}

pub(crate) struct AuthenticationWorker {
    requests: mpsc::Sender<AuthenticationRequest>,
}

impl AuthenticationWorker {
    /// Starts the PAM worker before the session lock is requested.
    ///
    /// # Errors
    ///
    /// Returns an error when the operating system cannot create the worker thread.
    pub(crate) fn spawn(
        completions: channel::Sender<AuthenticationCompletion>,
    ) -> Result<Self, io::Error> {
        Self::spawn_with(completions, authenticate_current_user)
    }

    /// Transfers one zeroizing password attempt to the worker.
    ///
    /// # Errors
    ///
    /// Returns an error when the worker is no longer available. The rejected request is dropped
    /// inside this method so its password is cleared before control returns to the caller.
    pub(crate) fn submit(
        &self,
        token: AttemptToken,
        password: PasswordAttempt,
    ) -> Result<(), WorkerUnavailable> {
        self.requests
            .send(AuthenticationRequest { token, password })
            .map_err(|_| WorkerUnavailable)
    }

    fn spawn_with(
        completions: channel::Sender<AuthenticationCompletion>,
        authenticator: Authenticator,
    ) -> Result<Self, io::Error> {
        let (requests, receiver) = mpsc::channel::<AuthenticationRequest>();
        thread::Builder::new()
            .name("luma-pam".to_owned())
            .spawn(move || run_worker(&receiver, &completions, authenticator))?;
        Ok(Self { requests })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct WorkerUnavailable;

fn run_worker(
    requests: &mpsc::Receiver<AuthenticationRequest>,
    completions: &channel::Sender<AuthenticationCompletion>,
    authenticator: Authenticator,
) {
    while let Ok(request) = requests.recv() {
        let token = request.token;
        let result = catch_unwind(AssertUnwindSafe(|| authenticator(request.password)));
        let outcome = match result {
            Ok(Ok(AuthenticationResult::Authenticated)) => AuthenticationOutcome::Authenticated,
            Ok(Ok(AuthenticationResult::Denied)) => AuthenticationOutcome::Denied,
            Ok(Err(_)) | Err(_) => AuthenticationOutcome::InfrastructureError,
        };
        if completions
            .send(AuthenticationCompletion { token, outcome })
            .is_err()
        {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use calloop::{EventLoop, channel};

    use super::{AuthenticationCompletion, AuthenticationWorker};
    use crate::{
        auth::{AuthenticationError, AuthenticationResult},
        input::{InputState, PasswordAttempt},
        state::{AuthenticationOutcome, AuthenticationState},
    };

    fn password() -> PasswordAttempt {
        let mut input = InputState::new(16);
        input.push_text("test-password");
        input.submit().expect("test password should be present")
    }

    fn authenticated(
        password: PasswordAttempt,
    ) -> Result<AuthenticationResult, AuthenticationError> {
        let is_empty = password.as_bytes().is_empty();
        drop(password);
        if is_empty {
            Err(AuthenticationError::CurrentUserUnavailable)
        } else {
            Ok(AuthenticationResult::Authenticated)
        }
    }

    fn denied(password: PasswordAttempt) -> Result<AuthenticationResult, AuthenticationError> {
        let is_empty = password.as_bytes().is_empty();
        drop(password);
        if is_empty {
            Err(AuthenticationError::CurrentUserUnavailable)
        } else {
            Ok(AuthenticationResult::Denied)
        }
    }

    fn infrastructure_error(
        _password: PasswordAttempt,
    ) -> Result<AuthenticationResult, AuthenticationError> {
        Err(AuthenticationError::CurrentUserUnavailable)
    }

    fn panics(_password: PasswordAttempt) -> Result<AuthenticationResult, AuthenticationError> {
        panic!("synthetic authenticator panic")
    }

    fn run(
        authenticator: super::Authenticator,
    ) -> (crate::state::AttemptToken, AuthenticationCompletion) {
        let (sender, receiver) = channel::channel();
        let worker =
            AuthenticationWorker::spawn_with(sender, authenticator).expect("worker should start");
        let mut state = AuthenticationState::default();
        let token = state.begin_attempt().expect("attempt should start");

        worker
            .submit(token, password())
            .expect("worker should receive request");

        let completion = receiver.recv().expect("worker should return completion");
        (token, completion)
    }

    #[test]
    fn reports_authenticated_completion_with_attempt_token() {
        let (token, completion) = run(authenticated);

        assert_eq!(completion.token, token);
        assert_eq!(completion.outcome, AuthenticationOutcome::Authenticated);
    }

    #[test]
    fn maps_denial_without_exposing_authentication_details() {
        let (_, completion) = run(denied);

        assert_eq!(completion.outcome, AuthenticationOutcome::Denied);
    }

    #[test]
    fn maps_authentication_errors_to_infrastructure_failure() {
        let (_, completion) = run(infrastructure_error);

        assert_eq!(
            completion.outcome,
            AuthenticationOutcome::InfrastructureError
        );
    }

    #[test]
    fn contains_authenticator_panics_as_infrastructure_failure() {
        let (_, completion) = run(panics);

        assert_eq!(
            completion.outcome,
            AuthenticationOutcome::InfrastructureError
        );
    }

    #[test]
    fn completion_wakes_a_blocked_event_loop() {
        let (sender, receiver) = channel::channel();
        let worker = AuthenticationWorker::spawn_with(sender, denied).expect("worker should start");
        let mut state = AuthenticationState::default();
        let token = state.begin_attempt().expect("attempt should start");
        let mut event_loop: EventLoop<Vec<AuthenticationCompletion>> =
            EventLoop::try_new().expect("event loop should start");
        event_loop
            .handle()
            .insert_source(receiver, |event, (), completions| {
                if let channel::Event::Msg(completion) = event {
                    completions.push(completion);
                }
            })
            .expect("channel should register");

        worker
            .submit(token, password())
            .expect("worker should receive request");
        let mut completions = Vec::new();
        event_loop
            .dispatch(Duration::from_secs(1), &mut completions)
            .expect("completion should wake the event loop");

        assert_eq!(
            completions,
            vec![AuthenticationCompletion {
                token,
                outcome: AuthenticationOutcome::Denied,
            }]
        );
    }
}
