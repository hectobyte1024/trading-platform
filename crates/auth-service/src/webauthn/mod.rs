/// WebAuthn (FIDO2) passwordless authentication
/// 
/// Implements W3C WebAuthn specification for secure, phishing-resistant
/// authentication using biometrics, security keys, and platform authenticators.

mod types;
mod registration;
mod authentication;
mod credential_store;
mod challenge;

pub use types::{
    WebAuthnError, AuthenticatorType, UserVerification, AttestationType,
    PublicKeyCredential, AuthenticatorData, ClientData, CredentialDescriptor,
};

pub use registration::{
    RegistrationOptions, RegistrationChallenge, RegistrationResponse,
    RegistrationVerifier,
};

pub use authentication::{
    AuthenticationOptions, AuthenticationChallenge, AuthenticationResponse,
    AuthenticationVerifier,
};

pub use credential_store::{
    CredentialStore, StoredCredential, CredentialMetadata,
};

pub use challenge::{
    ChallengeGenerator, ChallengeStore, InMemoryChallengeStore,
};
