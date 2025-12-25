mod none;
mod traits;
mod types;

pub use none::*;
pub use traits::*;
pub use types::*;

use crate::config::AuthMethod;

/// Factory function to create authenticator from config
pub fn create_authenticator(method: &AuthMethod) -> Box<dyn Authenticator> {
    match method {
        AuthMethod::None => Box::new(NoneAuthenticator::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_authenticator_none() {
        let auth = create_authenticator(&AuthMethod::None);
        assert_eq!(auth.method_name(), "none");
    }
}
