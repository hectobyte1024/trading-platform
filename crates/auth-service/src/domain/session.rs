use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::net::IpAddr;
use uuid::Uuid;

use super::{DeviceId, TokenId, UserId};

/// Session identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId(pub Uuid);

impl SessionId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

/// Device fingerprint for device identification and tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceFingerprint {
    /// User agent string
    pub user_agent: String,
    /// Screen resolution
    pub screen_resolution: Option<String>,
    /// Timezone offset
    pub timezone_offset: Option<i32>,
    /// Browser language
    pub language: Option<String>,
    /// Platform (OS)
    pub platform: Option<String>,
    /// Canvas fingerprint (for browser uniqueness)
    pub canvas_hash: Option<String>,
    /// WebGL renderer
    pub webgl_renderer: Option<String>,
    /// Installed fonts hash
    pub fonts_hash: Option<String>,
    /// Hardware concurrency (CPU cores)
    pub hardware_concurrency: Option<u32>,
    /// Device memory (GB)
    pub device_memory: Option<u32>,
    /// Additional custom attributes
    pub custom_attributes: HashMap<String, String>,
}

impl DeviceFingerprint {
    /// Calculate a hash of this fingerprint for comparison
    pub fn hash(&self) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        self.user_agent.hash(&mut hasher);
        
        if let Some(ref screen) = self.screen_resolution {
            screen.hash(&mut hasher);
        }
        if let Some(tz) = self.timezone_offset {
            tz.hash(&mut hasher);
        }
        if let Some(ref lang) = self.language {
            lang.hash(&mut hasher);
        }
        if let Some(ref platform) = self.platform {
            platform.hash(&mut hasher);
        }
        if let Some(ref canvas) = self.canvas_hash {
            canvas.hash(&mut hasher);
        }
        
        format!("{:x}", hasher.finish())
    }

    /// Check if this fingerprint matches another (fuzzy matching)
    pub fn matches(&self, other: &DeviceFingerprint, threshold: f32) -> bool {
        let mut matches = 0;
        let mut total = 0;

        // User agent is critical
        total += 3;
        if self.user_agent == other.user_agent {
            matches += 3;
        }

        // Screen resolution
        total += 2;
        if self.screen_resolution == other.screen_resolution {
            matches += 2;
        }

        // Platform
        total += 2;
        if self.platform == other.platform {
            matches += 2;
        }

        // Canvas hash (very distinctive)
        total += 3;
        if self.canvas_hash.is_some() && self.canvas_hash == other.canvas_hash {
            matches += 3;
        }

        // Timezone
        total += 1;
        if self.timezone_offset == other.timezone_offset {
            matches += 1;
        }

        (matches as f32 / total as f32) >= threshold
    }
}

/// Session state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionState {
    /// Session is active and valid
    Active,
    /// Session is pending MFA verification
    PendingMfa,
    /// Session is pending WebAuthn verification
    PendingWebAuthn,
    /// Session is pending additional risk checks
    PendingRiskCheck,
    /// Session was revoked by user
    RevokedByUser,
    /// Session was revoked by admin
    RevokedByAdmin,
    /// Session expired naturally
    Expired,
    /// Session invalidated due to suspicious activity
    Suspicious,
}

impl SessionState {
    /// Check if session is usable
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Active)
    }

    /// Check if session is in a pending state
    pub fn is_pending(&self) -> bool {
        matches!(
            self,
            Self::PendingMfa | Self::PendingWebAuthn | Self::PendingRiskCheck
        )
    }

    /// Check if session was terminated
    pub fn is_terminated(&self) -> bool {
        matches!(
            self,
            Self::RevokedByUser
                | Self::RevokedByAdmin
                | Self::Expired
                | Self::Suspicious
        )
    }
}

/// Risk score for session
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct RiskScore(f32);

impl RiskScore {
    /// Create a new risk score (0.0 = no risk, 1.0 = maximum risk)
    pub fn new(score: f32) -> Self {
        Self(score.clamp(0.0, 1.0))
    }

    /// Get the raw score value
    pub fn value(&self) -> f32 {
        self.0
    }

    /// Check if risk is low (< 0.3)
    pub fn is_low(&self) -> bool {
        self.0 < 0.3
    }

    /// Check if risk is medium (0.3 - 0.7)
    pub fn is_medium(&self) -> bool {
        self.0 >= 0.3 && self.0 < 0.7
    }

    /// Check if risk is high (>= 0.7)
    pub fn is_high(&self) -> bool {
        self.0 >= 0.7
    }
}

impl Default for RiskScore {
    fn default() -> Self {
        Self(0.5) // Medium risk by default
    }
}

/// Complete session object
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    /// Unique session identifier
    pub id: SessionId,
    /// User this session belongs to
    pub user_id: UserId,
    /// Device this session is bound to
    pub device_id: DeviceId,
    /// Current session state
    pub state: SessionState,
    /// When the session was created
    pub created_at: DateTime<Utc>,
    /// When the session was last accessed
    pub last_accessed_at: DateTime<Utc>,
    /// When the session expires (absolute timeout)
    pub expires_at: DateTime<Utc>,
    /// Last IP address used
    pub ip_address: IpAddr,
    /// Device fingerprint
    pub device_fingerprint: DeviceFingerprint,
    /// Current risk score
    pub risk_score: RiskScore,
    /// Current access token JTI
    pub current_access_token: Option<TokenId>,
    /// Current refresh token JTI
    pub current_refresh_token: Option<TokenId>,
    /// MFA verified for this session
    pub mfa_verified: bool,
    /// WebAuthn verified for this session
    pub webauthn_verified: bool,
    /// Number of tokens issued in this session
    pub tokens_issued: u32,
    /// Number of failed authentication attempts
    pub failed_auth_attempts: u32,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

impl Session {
    /// Create a new session
    pub fn new(
        user_id: UserId,
        device_id: DeviceId,
        ip_address: IpAddr,
        fingerprint: DeviceFingerprint,
        ttl: i64,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: SessionId::new(),
            user_id,
            device_id,
            state: SessionState::Active,
            created_at: now,
            last_accessed_at: now,
            expires_at: now + chrono::Duration::seconds(ttl),
            ip_address,
            device_fingerprint: fingerprint,
            risk_score: RiskScore::default(),
            current_access_token: None,
            current_refresh_token: None,
            mfa_verified: false,
            webauthn_verified: false,
            tokens_issued: 0,
            failed_auth_attempts: 0,
            metadata: HashMap::new(),
        }
    }

    /// Check if session is valid
    pub fn is_valid(&self) -> bool {
        self.state.is_active() && !self.is_expired()
    }

    /// Check if session is expired
    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }

    /// Update last accessed time
    pub fn touch(&mut self) {
        self.last_accessed_at = Utc::now();
    }

    /// Check if session is idle for too long
    pub fn is_idle(&self, idle_timeout: i64) -> bool {
        let idle_duration = Utc::now() - self.last_accessed_at;
        idle_duration.num_seconds() > idle_timeout
    }

    /// Terminate session with reason
    pub fn terminate(&mut self, reason: SessionState) {
        if reason.is_terminated() {
            self.state = reason;
        }
    }

    /// Check if IP address changed
    pub fn ip_changed(&self, new_ip: IpAddr) -> bool {
        self.ip_address != new_ip
    }

    /// Update risk score
    pub fn update_risk_score(&mut self, score: RiskScore) {
        self.risk_score = score;
        
        // If risk is high, require additional verification
        if score.is_high() && self.state == SessionState::Active {
            self.state = SessionState::PendingRiskCheck;
        }
    }

    /// Record failed authentication attempt
    pub fn record_failed_auth(&mut self) {
        self.failed_auth_attempts += 1;
        
        // After 3 failed attempts, mark as suspicious
        if self.failed_auth_attempts >= 3 {
            self.state = SessionState::Suspicious;
        }
    }

    /// Check if session requires step-up authentication
    pub fn requires_stepup(&self, required_mfa: bool, required_webauthn: bool) -> bool {
        if required_mfa && !self.mfa_verified {
            return true;
        }
        if required_webauthn && !self.webauthn_verified {
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    fn mock_fingerprint() -> DeviceFingerprint {
        DeviceFingerprint {
            user_agent: "Mozilla/5.0".to_string(),
            screen_resolution: Some("1920x1080".to_string()),
            timezone_offset: Some(-300),
            language: Some("en-US".to_string()),
            platform: Some("Linux".to_string()),
            canvas_hash: Some("abc123".to_string()),
            webgl_renderer: None,
            fonts_hash: None,
            hardware_concurrency: Some(8),
            device_memory: Some(16),
            custom_attributes: HashMap::new(),
        }
    }

    #[test]
    fn test_session_creation() {
        let user_id = UserId::new();
        let device_id = DeviceId::new();
        let ip = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
        let fingerprint = mock_fingerprint();

        let session = Session::new(user_id, device_id, ip, fingerprint, 3600);

        assert_eq!(session.user_id, user_id);
        assert_eq!(session.device_id, device_id);
        assert_eq!(session.state, SessionState::Active);
        assert!(session.is_valid());
    }

    #[test]
    fn test_session_expiration() {
        let session = Session::new(
            UserId::new(),
            DeviceId::new(),
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            mock_fingerprint(),
            -100, // Already expired
        );

        assert!(session.is_expired());
        assert!(!session.is_valid());
    }

    #[test]
    fn test_session_touch() {
        let mut session = Session::new(
            UserId::new(),
            DeviceId::new(),
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            mock_fingerprint(),
            3600,
        );

        let original = session.last_accessed_at;
        std::thread::sleep(std::time::Duration::from_millis(10));
        session.touch();

        assert!(session.last_accessed_at > original);
    }

    #[test]
    fn test_session_risk_score() {
        let mut session = Session::new(
            UserId::new(),
            DeviceId::new(),
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            mock_fingerprint(),
            3600,
        );

        assert!(!session.risk_score.is_high());

        session.update_risk_score(RiskScore::new(0.9));
        assert!(session.risk_score.is_high());
        assert_eq!(session.state, SessionState::PendingRiskCheck);
    }

    #[test]
    fn test_session_failed_auth_attempts() {
        let mut session = Session::new(
            UserId::new(),
            DeviceId::new(),
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            mock_fingerprint(),
            3600,
        );

        session.record_failed_auth();
        session.record_failed_auth();
        assert_eq!(session.state, SessionState::Active);

        session.record_failed_auth();
        assert_eq!(session.state, SessionState::Suspicious);
    }

    #[test]
    fn test_fingerprint_matching() {
        let fp1 = mock_fingerprint();
        let fp2 = mock_fingerprint();

        assert!(fp1.matches(&fp2, 0.8));
    }

    #[test]
    fn test_risk_score_levels() {
        assert!(RiskScore::new(0.2).is_low());
        assert!(RiskScore::new(0.5).is_medium());
        assert!(RiskScore::new(0.8).is_high());
    }

    #[test]
    fn test_session_state_checks() {
        assert!(SessionState::Active.is_active());
        assert!(SessionState::PendingMfa.is_pending());
        assert!(SessionState::Expired.is_terminated());
    }
}
