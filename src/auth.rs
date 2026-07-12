use std::{ffi::CString, fmt, fs::File, path::Path};

use pam_client2::{Context, ConversationHandler, ErrorCode, Flag};

use crate::input::PasswordAttempt;

const PAM_SERVICE: &str = "luma";
const PAM_SERVICE_PATH: &str = "/etc/pam.d/luma";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthenticationResult {
    Authenticated,
    Denied,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthenticationError {
    CurrentUserUnavailable,
    PamServiceUnavailable,
    Pam(ErrorCode),
}

impl fmt::Display for AuthenticationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CurrentUserUnavailable => {
                formatter.write_str("could not resolve the current user")
            }
            Self::PamServiceUnavailable => {
                formatter.write_str("the Luma PAM service is not installed or readable")
            }
            Self::Pam(_) => formatter.write_str("PAM authentication could not be completed"),
        }
    }
}

impl std::error::Error for AuthenticationError {}

/// Verifies that the PAM policy required for unlocking is available.
///
/// This check must run before requesting a session lock so a missing policy cannot trap the user.
///
/// # Errors
///
/// Returns an error if `/etc/pam.d/luma` is missing, not a regular file, or unreadable.
pub fn validate_service() -> Result<(), AuthenticationError> {
    validate_service_path(Path::new(PAM_SERVICE_PATH))
}

/// Authenticates the process owner through Luma's PAM policy.
///
/// The target username comes from the real UID rather than environment variables. Password
/// ownership remains inside the PAM conversation and is zeroized when the transaction ends.
///
/// # Errors
///
/// Returns an error when the process owner cannot be resolved or PAM infrastructure fails.
pub fn authenticate_current_user(
    password: PasswordAttempt,
) -> Result<AuthenticationResult, AuthenticationError> {
    let username = current_username()?;
    authenticate(PAM_SERVICE, &username, password)
}

fn authenticate(
    service: &str,
    username: &str,
    password: PasswordAttempt,
) -> Result<AuthenticationResult, AuthenticationError> {
    let conversation = PasswordConversation::new(username, password)
        .map_err(|_| AuthenticationError::CurrentUserUnavailable)?;
    let mut context = Context::new(service, Some(username), conversation)
        .map_err(|error| AuthenticationError::Pam(error.code()))?;
    let flags = Flag::SILENT | Flag::DISALLOW_NULL_AUTHTOK;

    if let Err(error) = context.authenticate(flags) {
        return classify_pam_error(error.code());
    }
    Ok(AuthenticationResult::Authenticated)
}

fn current_username() -> Result<String, AuthenticationError> {
    let user = uzers::get_user_by_uid(uzers::get_current_uid())
        .ok_or(AuthenticationError::CurrentUserUnavailable)?;
    user.name()
        .to_str()
        .map(str::to_owned)
        .ok_or(AuthenticationError::CurrentUserUnavailable)
}

fn validate_service_path(path: &Path) -> Result<(), AuthenticationError> {
    let metadata = path
        .metadata()
        .map_err(|_| AuthenticationError::PamServiceUnavailable)?;
    if !metadata.is_file() {
        return Err(AuthenticationError::PamServiceUnavailable);
    }
    File::open(path)
        .map(|_| ())
        .map_err(|_| AuthenticationError::PamServiceUnavailable)
}

fn classify_pam_error(code: ErrorCode) -> Result<AuthenticationResult, AuthenticationError> {
    if matches!(
        code,
        ErrorCode::AUTH_ERR
            | ErrorCode::CRED_INSUFFICIENT
            | ErrorCode::USER_UNKNOWN
            | ErrorCode::MAXTRIES
            | ErrorCode::PERM_DENIED
            | ErrorCode::ACCT_EXPIRED
            | ErrorCode::NEW_AUTHTOK_REQD
            | ErrorCode::CRED_EXPIRED
    ) {
        Ok(AuthenticationResult::Denied)
    } else {
        Err(AuthenticationError::Pam(code))
    }
}

struct PasswordConversation {
    username: CString,
    password: PasswordAttempt,
}

impl PasswordConversation {
    fn new(username: &str, password: PasswordAttempt) -> Result<Self, std::ffi::NulError> {
        Ok(Self {
            username: CString::new(username)?,
            password,
        })
    }
}

impl ConversationHandler for PasswordConversation {
    fn prompt_echo_on(&mut self, _prompt: &std::ffi::CStr) -> Result<CString, ErrorCode> {
        Ok(self.username.clone())
    }

    fn prompt_echo_off(&mut self, _prompt: &std::ffi::CStr) -> Result<CString, ErrorCode> {
        CString::new(self.password.as_bytes()).map_err(|_| ErrorCode::CONV_ERR)
    }

    fn text_info(&mut self, _message: &std::ffi::CStr) {}

    fn error_msg(&mut self, _message: &std::ffi::CStr) {}

    fn radio_prompt(&mut self, _prompt: &std::ffi::CStr) -> Result<bool, ErrorCode> {
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use pam_client2::ConversationHandler;

    use super::{
        AuthenticationError, AuthenticationResult, PasswordConversation, classify_pam_error,
        current_username, validate_service_path,
    };
    use crate::input::InputState;

    #[test]
    fn conversation_returns_fixed_credentials_without_recording_prompts() {
        let mut input = InputState::new(16);
        input.push_text("secret");
        let password = input.submit().expect("password should be present");
        let mut conversation =
            PasswordConversation::new("alice", password).expect("credentials contain no NUL");
        let prompt = c"Password: ";

        assert_eq!(
            conversation
                .prompt_echo_on(prompt)
                .expect("username should be returned")
                .as_bytes(),
            b"alice"
        );
        assert_eq!(
            conversation
                .prompt_echo_off(prompt)
                .expect("password should be returned")
                .as_bytes(),
            b"secret"
        );
    }

    #[test]
    fn authentication_failures_are_generic_denials() {
        for code in [
            pam_client2::ErrorCode::AUTH_ERR,
            pam_client2::ErrorCode::USER_UNKNOWN,
            pam_client2::ErrorCode::ACCT_EXPIRED,
        ] {
            assert_eq!(classify_pam_error(code), Ok(AuthenticationResult::Denied));
        }
    }

    #[test]
    fn infrastructure_failures_remain_distinguishable() {
        assert_eq!(
            classify_pam_error(pam_client2::ErrorCode::SERVICE_ERR),
            Err(AuthenticationError::Pam(
                pam_client2::ErrorCode::SERVICE_ERR
            ))
        );
    }

    #[test]
    fn resolves_the_process_owner_from_the_real_uid() {
        assert!(
            !current_username()
                .expect("current user should exist")
                .is_empty()
        );
    }

    #[test]
    fn rejects_a_missing_pam_policy_before_locking() {
        assert_eq!(
            validate_service_path(std::path::Path::new(
                "/path-that-must-not-exist/luma-pam-policy"
            )),
            Err(AuthenticationError::PamServiceUnavailable)
        );
    }
}
