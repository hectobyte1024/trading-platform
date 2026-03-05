use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

/// User identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UserId(pub Uuid);

impl UserId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl fmt::Display for UserId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Default for UserId {
    fn default() -> Self {
        Self::new()
    }
}

/// Device identifier for device-bound sessions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DeviceId(pub Uuid);

impl DeviceId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl fmt::Display for DeviceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Default for DeviceId {
    fn default() -> Self {
        Self::new()
    }
}

/// Authentication domain - separates user types for security isolation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum UserDomain {
    /// Retail/individual traders
    Retail,
    /// Institutional clients (funds, family offices, etc.)
    Institutional,
    /// Platform administrators
    Admin,
    /// Compliance and audit personnel
    Compliance,
    /// System-to-system service accounts
    Service,
}

impl UserDomain {
    /// Check if domain has elevated privileges
    pub fn is_privileged(&self) -> bool {
        matches!(self, Self::Admin | Self::Compliance | Self::Service)
    }

    /// Check if domain can access administrative functions
    pub fn can_admin(&self) -> bool {
        matches!(self, Self::Admin)
    }

    /// Check if domain can access compliance functions
    pub fn can_audit(&self) -> bool {
        matches!(self, Self::Admin | Self::Compliance)
    }

    /// Token lifetime for this domain (in seconds)
    pub fn access_token_ttl(&self) -> i64 {
        match self {
            Self::Retail => 900,          // 15 minutes
            Self::Institutional => 1800,  // 30 minutes
            Self::Admin => 600,           // 10 minutes (shorter for security)
            Self::Compliance => 900,      // 15 minutes
            Self::Service => 3600,        // 1 hour
        }
    }

    /// Refresh token lifetime for this domain (in seconds)
    pub fn refresh_token_ttl(&self) -> i64 {
        match self {
            Self::Retail => 2592000,        // 30 days
            Self::Institutional => 604800,  // 7 days
            Self::Admin => 43200,           // 12 hours
            Self::Compliance => 86400,      // 24 hours
            Self::Service => 0,             // No refresh for services (use mTLS)
        }
    }
}

/// User type with granular categories
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UserType {
    // Retail domain
    RetailBasic,
    RetailPremium,
    RetailProfessional,

    // Institutional domain
    InstitutionalHedgeFund,
    InstitutionalFamilyOffice,
    InstitutionalMarketMaker,
    InstitutionalPropTrading,

    // Admin domain
    AdminSuperUser,
    AdminOperations,
    AdminSupport,

    // Compliance domain
    ComplianceOfficer,
    ComplianceAuditor,
    ComplianceAnalyst,

    // Service domain
    ServiceAPI,
    ServiceBackoffice,
    ServiceRiskEngine,
}

impl UserType {
    /// Get the domain this user type belongs to
    pub fn domain(&self) -> UserDomain {
        match self {
            Self::RetailBasic | Self::RetailPremium | Self::RetailProfessional => {
                UserDomain::Retail
            }
            Self::InstitutionalHedgeFund
            | Self::InstitutionalFamilyOffice
            | Self::InstitutionalMarketMaker
            | Self::InstitutionalPropTrading => UserDomain::Institutional,
            Self::AdminSuperUser | Self::AdminOperations | Self::AdminSupport => UserDomain::Admin,
            Self::ComplianceOfficer | Self::ComplianceAuditor | Self::ComplianceAnalyst => {
                UserDomain::Compliance
            }
            Self::ServiceAPI | Self::ServiceBackoffice | Self::ServiceRiskEngine => {
                UserDomain::Service
            }
        }
    }

    /// Check if this user type requires MFA
    pub fn requires_mfa(&self) -> bool {
        matches!(
            self,
            Self::RetailProfessional
                | Self::InstitutionalHedgeFund
                | Self::InstitutionalFamilyOffice
                | Self::InstitutionalMarketMaker
                | Self::InstitutionalPropTrading
                | Self::AdminSuperUser
                | Self::AdminOperations
                | Self::ComplianceOfficer
                | Self::ComplianceAuditor
        )
    }

    /// Check if this user type requires phishing-resistant authentication (WebAuthn)
    pub fn requires_webauthn(&self) -> bool {
        matches!(
            self,
            Self::InstitutionalHedgeFund
                | Self::InstitutionalFamilyOffice
                | Self::InstitutionalMarketMaker
                | Self::InstitutionalPropTrading
                | Self::AdminSuperUser
                | Self::AdminOperations
                | Self::ComplianceOfficer
        )
    }

    /// Maximum session lifetime (in seconds)
    pub fn max_session_lifetime(&self) -> i64 {
        match self.domain() {
            UserDomain::Retail => 86400,         // 24 hours
            UserDomain::Institutional => 43200,  // 12 hours
            UserDomain::Admin => 28800,          // 8 hours
            UserDomain::Compliance => 28800,     // 8 hours
            UserDomain::Service => 0,            // No sessions for services
        }
    }
}

/// Complete user identity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: UserId,
    pub user_type: UserType,
    pub email: String,
    pub created_at: DateTime<Utc>,
    pub last_login: Option<DateTime<Utc>>,
    pub mfa_enabled: bool,
    pub webauthn_enabled: bool,
    pub account_locked: bool,
    pub lockout_until: Option<DateTime<Utc>>,
    pub failed_login_attempts: u32,
    pub password_changed_at: DateTime<Utc>,
    pub must_change_password: bool,
}

impl User {
    /// Check if user account is active and not locked
    pub fn is_active(&self) -> bool {
        if self.account_locked {
            return false;
        }

        if let Some(lockout) = self.lockout_until {
            if lockout > Utc::now() {
                return false;
            }
        }

        true
    }

    /// Check if user should be forced through MFA
    pub fn should_require_mfa(&self) -> bool {
        self.mfa_enabled || self.user_type.requires_mfa()
    }

    /// Check if user should be forced through WebAuthn
    pub fn should_require_webauthn(&self) -> bool {
        self.webauthn_enabled || self.user_type.requires_webauthn()
    }

    /// Get the authentication domain
    pub fn domain(&self) -> UserDomain {
        self.user_type.domain()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_domain_privileges() {
        assert!(UserDomain::Admin.is_privileged());
        assert!(UserDomain::Compliance.is_privileged());
        assert!(!UserDomain::Retail.is_privileged());
        assert!(!UserDomain::Institutional.is_privileged());
    }

    #[test]
    fn test_user_domain_ttl() {
        assert_eq!(UserDomain::Retail.access_token_ttl(), 900);
        assert_eq!(UserDomain::Institutional.access_token_ttl(), 1800);
        assert_eq!(UserDomain::Admin.access_token_ttl(), 600);
    }

    #[test]
    fn test_user_type_domain_mapping() {
        assert_eq!(UserType::RetailBasic.domain(), UserDomain::Retail);
        assert_eq!(
            UserType::InstitutionalHedgeFund.domain(),
            UserDomain::Institutional
        );
        assert_eq!(UserType::AdminSuperUser.domain(), UserDomain::Admin);
        assert_eq!(
            UserType::ComplianceOfficer.domain(),
            UserDomain::Compliance
        );
    }

    #[test]
    fn test_user_type_webauthn_requirements() {
        assert!(UserType::InstitutionalHedgeFund.requires_webauthn());
        assert!(UserType::AdminSuperUser.requires_webauthn());
        assert!(!UserType::RetailBasic.requires_webauthn());
    }

    #[test]
    fn test_user_active_status() {
        let mut user = User {
            id: UserId::new(),
            user_type: UserType::RetailBasic,
            email: "test@example.com".to_string(),
            created_at: Utc::now(),
            last_login: None,
            mfa_enabled: false,
            webauthn_enabled: false,
            account_locked: false,
            lockout_until: None,
            failed_login_attempts: 0,
            password_changed_at: Utc::now(),
            must_change_password: false,
        };

        assert!(user.is_active());

        user.account_locked = true;
        assert!(!user.is_active());
    }
}
