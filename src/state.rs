use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthenticationPhase {
    Idle,
    Authenticating,
    Denied,
    Error,
    Cooldown,
    Authenticated,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthenticationOutcome {
    Authenticated,
    Denied,
    InfrastructureError,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompletionAction {
    KeepLocked,
    UnlockAuthorized,
    Ignored,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BeginAttemptError {
    Busy,
    CoolingDown,
    AlreadyAuthenticated,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AttemptToken(u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticationPolicy {
    pub feedback_duration: Duration,
    pub initial_cooldown: Duration,
    pub maximum_cooldown: Duration,
}

impl Default for AuthenticationPolicy {
    fn default() -> Self {
        Self {
            feedback_duration: Duration::from_millis(800),
            initial_cooldown: Duration::from_millis(500),
            maximum_cooldown: Duration::from_secs(8),
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum PhaseState {
    Idle,
    Authenticating {
        token: AttemptToken,
    },
    Denied {
        feedback_until: Instant,
        cooldown_until: Instant,
    },
    Error {
        feedback_until: Instant,
        cooldown_until: Instant,
    },
    Cooldown {
        until: Instant,
    },
    Authenticated,
}

#[derive(Debug)]
pub struct AuthenticationState {
    phase: PhaseState,
    policy: AuthenticationPolicy,
    next_attempt_id: u64,
    consecutive_failures: u32,
}

impl AuthenticationState {
    #[must_use]
    pub const fn new(policy: AuthenticationPolicy) -> Self {
        Self {
            phase: PhaseState::Idle,
            policy,
            next_attempt_id: 0,
            consecutive_failures: 0,
        }
    }

    #[must_use]
    pub const fn phase(&self) -> AuthenticationPhase {
        match self.phase {
            PhaseState::Idle => AuthenticationPhase::Idle,
            PhaseState::Authenticating { .. } => AuthenticationPhase::Authenticating,
            PhaseState::Denied { .. } => AuthenticationPhase::Denied,
            PhaseState::Error { .. } => AuthenticationPhase::Error,
            PhaseState::Cooldown { .. } => AuthenticationPhase::Cooldown,
            PhaseState::Authenticated => AuthenticationPhase::Authenticated,
        }
    }

    #[must_use]
    pub const fn accepts_input(&self) -> bool {
        matches!(self.phase, PhaseState::Idle)
    }

    #[must_use]
    pub const fn unlock_authorized(&self) -> bool {
        matches!(self.phase, PhaseState::Authenticated)
    }

    /// Starts one authentication attempt when the state is idle.
    ///
    /// # Errors
    ///
    /// Returns an error while another attempt, feedback, cooldown, or an authenticated terminal
    /// state is active.
    pub fn begin_attempt(&mut self) -> Result<AttemptToken, BeginAttemptError> {
        match self.phase {
            PhaseState::Idle => {
                let token = AttemptToken(self.next_attempt_id);
                self.next_attempt_id = self.next_attempt_id.wrapping_add(1);
                self.phase = PhaseState::Authenticating { token };
                Ok(token)
            }
            PhaseState::Authenticating { .. }
            | PhaseState::Denied { .. }
            | PhaseState::Error { .. } => Err(BeginAttemptError::Busy),
            PhaseState::Cooldown { .. } => Err(BeginAttemptError::CoolingDown),
            PhaseState::Authenticated => Err(BeginAttemptError::AlreadyAuthenticated),
        }
    }

    pub fn complete_attempt(
        &mut self,
        token: AttemptToken,
        outcome: AuthenticationOutcome,
        now: Instant,
    ) -> CompletionAction {
        let PhaseState::Authenticating {
            token: active_token,
        } = self.phase
        else {
            return CompletionAction::Ignored;
        };
        if active_token != token {
            return CompletionAction::Ignored;
        }

        if outcome == AuthenticationOutcome::Authenticated {
            self.consecutive_failures = 0;
            self.phase = PhaseState::Authenticated;
            return CompletionAction::UnlockAuthorized;
        }

        self.consecutive_failures = self.consecutive_failures.saturating_add(1);
        let feedback_until = now
            .checked_add(self.policy.feedback_duration)
            .unwrap_or(now);
        let cooldown_until = feedback_until
            .checked_add(self.cooldown_duration())
            .unwrap_or(feedback_until);
        self.phase = match outcome {
            AuthenticationOutcome::Denied => PhaseState::Denied {
                feedback_until,
                cooldown_until,
            },
            AuthenticationOutcome::InfrastructureError => PhaseState::Error {
                feedback_until,
                cooldown_until,
            },
            AuthenticationOutcome::Authenticated => unreachable!(),
        };
        CompletionAction::KeepLocked
    }

    /// Invalidates an in-flight attempt so a late worker result cannot affect the lock.
    pub fn cancel_attempt(&mut self, token: AttemptToken) -> bool {
        if matches!(self.phase, PhaseState::Authenticating { token: active } if active == token) {
            self.phase = PhaseState::Idle;
            true
        } else {
            false
        }
    }

    /// Advances feedback and cooldown states using a caller-provided monotonic time.
    pub fn advance(&mut self, now: Instant) {
        self.phase = match self.phase {
            PhaseState::Denied {
                feedback_until,
                cooldown_until,
            } if now >= feedback_until => next_after_feedback(now, cooldown_until),
            PhaseState::Error {
                feedback_until,
                cooldown_until,
            } if now >= feedback_until => next_after_feedback(now, cooldown_until),
            PhaseState::Cooldown { until } if now >= until => PhaseState::Idle,
            phase => phase,
        };
    }

    #[must_use]
    pub const fn next_deadline(&self) -> Option<Instant> {
        match self.phase {
            PhaseState::Denied { feedback_until, .. }
            | PhaseState::Error { feedback_until, .. } => Some(feedback_until),
            PhaseState::Cooldown { until } => Some(until),
            PhaseState::Idle | PhaseState::Authenticating { .. } | PhaseState::Authenticated => {
                None
            }
        }
    }

    fn cooldown_duration(&self) -> Duration {
        let exponent = self.consecutive_failures.saturating_sub(1).min(31);
        let factor = 1_u32.checked_shl(exponent).unwrap_or(u32::MAX);
        self.policy
            .initial_cooldown
            .checked_mul(factor)
            .unwrap_or(self.policy.maximum_cooldown)
            .min(self.policy.maximum_cooldown)
    }
}

impl Default for AuthenticationState {
    fn default() -> Self {
        Self::new(AuthenticationPolicy::default())
    }
}

fn next_after_feedback(now: Instant, cooldown_until: Instant) -> PhaseState {
    if now >= cooldown_until {
        PhaseState::Idle
    } else {
        PhaseState::Cooldown {
            until: cooldown_until,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::{
        AuthenticationOutcome, AuthenticationPhase, AuthenticationPolicy, AuthenticationState,
        BeginAttemptError, CompletionAction,
    };

    fn policy() -> AuthenticationPolicy {
        AuthenticationPolicy {
            feedback_duration: Duration::from_secs(1),
            initial_cooldown: Duration::from_secs(2),
            maximum_cooldown: Duration::from_secs(4),
        }
    }

    #[test]
    fn starts_only_one_attempt_at_a_time() {
        let mut state = AuthenticationState::new(policy());

        let _token = state.begin_attempt().expect("idle state should start");

        assert_eq!(state.phase(), AuthenticationPhase::Authenticating);
        assert_eq!(state.begin_attempt(), Err(BeginAttemptError::Busy));
        assert!(!state.accepts_input());
    }

    #[test]
    fn only_successful_completion_authorizes_unlock() {
        let now = Instant::now();
        let mut denied = AuthenticationState::new(policy());
        let denied_token = denied.begin_attempt().expect("attempt should start");
        let mut authenticated = AuthenticationState::new(policy());
        let authenticated_token = authenticated.begin_attempt().expect("attempt should start");

        assert_eq!(
            denied.complete_attempt(denied_token, AuthenticationOutcome::Denied, now),
            CompletionAction::KeepLocked
        );
        assert!(!denied.unlock_authorized());
        assert_eq!(
            authenticated.complete_attempt(
                authenticated_token,
                AuthenticationOutcome::Authenticated,
                now
            ),
            CompletionAction::UnlockAuthorized
        );
        assert!(authenticated.unlock_authorized());
        assert_eq!(
            authenticated.begin_attempt(),
            Err(BeginAttemptError::AlreadyAuthenticated)
        );
    }

    #[test]
    fn denial_moves_through_feedback_cooldown_and_idle() {
        let now = Instant::now();
        let mut state = AuthenticationState::new(policy());
        let token = state.begin_attempt().expect("attempt should start");

        state.complete_attempt(token, AuthenticationOutcome::Denied, now);
        assert_eq!(state.phase(), AuthenticationPhase::Denied);
        assert_eq!(
            state.next_deadline(),
            now.checked_add(Duration::from_secs(1))
        );

        state.advance(now + Duration::from_secs(1));
        assert_eq!(state.phase(), AuthenticationPhase::Cooldown);
        assert_eq!(state.begin_attempt(), Err(BeginAttemptError::CoolingDown));

        state.advance(now + Duration::from_secs(3));
        assert_eq!(state.phase(), AuthenticationPhase::Idle);
        assert!(state.accepts_input());
    }

    #[test]
    fn infrastructure_failure_uses_generic_error_feedback() {
        let now = Instant::now();
        let mut state = AuthenticationState::new(policy());
        let token = state.begin_attempt().expect("attempt should start");

        assert_eq!(
            state.complete_attempt(token, AuthenticationOutcome::InfrastructureError, now),
            CompletionAction::KeepLocked
        );
        assert_eq!(state.phase(), AuthenticationPhase::Error);
        assert!(!state.unlock_authorized());
    }

    #[test]
    fn late_completion_is_ignored_after_cancellation() {
        let now = Instant::now();
        let mut state = AuthenticationState::new(policy());
        let stale_token = state.begin_attempt().expect("attempt should start");
        assert!(state.cancel_attempt(stale_token));
        let active_token = state.begin_attempt().expect("new attempt should start");

        assert_eq!(
            state.complete_attempt(stale_token, AuthenticationOutcome::Authenticated, now),
            CompletionAction::Ignored
        );
        assert_eq!(state.phase(), AuthenticationPhase::Authenticating);
        assert!(!state.unlock_authorized());
        assert_eq!(
            state.complete_attempt(active_token, AuthenticationOutcome::Authenticated, now),
            CompletionAction::UnlockAuthorized
        );
    }

    #[test]
    fn progressive_cooldown_is_capped() {
        let mut state = AuthenticationState::new(policy());
        let mut now = Instant::now();

        for expected_cooldown in [2, 4, 4] {
            let token = state.begin_attempt().expect("attempt should start");
            state.complete_attempt(token, AuthenticationOutcome::Denied, now);
            state.advance(now + Duration::from_secs(1));
            let cooldown_deadline = state.next_deadline().expect("cooldown needs a deadline");
            assert_eq!(
                cooldown_deadline.duration_since(now + Duration::from_secs(1)),
                Duration::from_secs(expected_cooldown)
            );
            now = cooldown_deadline;
            state.advance(now);
        }
    }
}
