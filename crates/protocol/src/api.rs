//! Request and response bodies of the console's REST API (spec 0010): accounts,
//! invitations, networks, their devices, members and API keys.

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use crate::Backend;

/// The signed-in account.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Me {
    pub id: Uuid,
    pub email: String,
    pub name: String,
    pub is_admin: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Login {
    pub email: String,
    pub password: String,
}

/// What an invitation link offers, before it is accepted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct InviteInfo {
    pub email: Option<String>,
    pub network_name: Option<String>,
    pub role: Option<Role>,
    /// Whoever sent it.
    pub invited_by: Option<String>,
}

/// Accepting an invitation creates an account; a signed-in account that accepts one
/// only joins the network (both fields are then ignored).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AcceptInvite {
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub password: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CreateInvite {
    #[serde(default)]
    pub email: Option<String>,
    /// For a network invitation; the platform admin's account invitations have none.
    #[serde(default)]
    pub role: Option<Role>,
}

/// The link is shown once; only its hash is stored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CreatedInvite {
    pub token: String,
    #[ts(type = "number")]
    pub expires_at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum Role {
    Viewer,
    Admin,
    Owner,
}

impl Role {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Viewer => "viewer",
            Self::Admin => "admin",
            Self::Owner => "owner",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "viewer" => Self::Viewer,
            "admin" => Self::Admin,
            "owner" => Self::Owner,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum JoinMode {
    /// Anyone with the join link adds a device.
    Open,
    /// A device waits until an admin approves it.
    Approval,
}

/// When a network's devices may compute: windows in the network's time zone. A window
/// whose `to` is not after `from` runs past midnight (15:00–08:00).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Hours {
    /// IANA name, e.g. "Europe/Sofia".
    pub timezone: String,
    pub windows: Vec<HoursWindow>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct HoursWindow {
    /// ISO weekdays the window starts on: 1 = Monday … 7 = Sunday.
    pub days: Vec<u8>,
    /// "HH:MM".
    pub from: String,
    pub to: String,
}

/// A network as its members see it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct NetworkView {
    pub id: Uuid,
    pub name: String,
    pub join_code: String,
    pub role: Role,
    pub join_mode: JoinMode,
    pub max_power: u32,
    pub hours: Option<Hours>,
    pub paused: bool,
    pub lend_idle: bool,
    /// The public volunteer network.
    pub public: bool,
    pub devices_online: u32,
    pub devices_total: u32,
    #[ts(type = "number")]
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CreateNetwork {
    pub name: String,
}

/// Fields left out stay as they are; `hours: null` means always.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct UpdateNetwork {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub join_mode: Option<JoinMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub max_power: Option<u32>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "serde_with_double_option"
    )]
    #[ts(optional = nullable, as = "Option<Hours>")]
    pub hours: Option<Option<Hours>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub paused: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub lend_idle: Option<bool>,
    /// True issues a new join code; links and QR codes with the old one stop working.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub new_join_code: Option<bool>,
}

/// Tells a missing field (leave as is) from `null` (clear).
mod serde_with_double_option {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<T: Serialize, S: Serializer>(
        value: &Option<Option<T>>,
        s: S,
    ) -> Result<S::Ok, S::Error> {
        match value {
            Some(inner) => inner.serialize(s),
            None => s.serialize_none(),
        }
    }

    pub fn deserialize<'de, T: Deserialize<'de>, D: Deserializer<'de>>(
        d: D,
    ) -> Result<Option<Option<T>>, D::Error> {
        Option::<T>::deserialize(d).map(Some)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum DeviceStatus {
    Active,
    Pending,
    Removed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct DeviceView {
    pub id: Uuid,
    pub name: Option<String>,
    pub status: DeviceStatus,
    pub paused: bool,
    pub backend: Backend,
    pub vendor: String,
    pub architecture: String,
    pub units_done: u32,
    /// Connected right now.
    pub online: bool,
    #[ts(type = "number")]
    pub first_seen_at: i64,
    #[ts(type = "number")]
    pub last_seen_at: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct UpdateDevice {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub paused: Option<bool>,
    /// Admits a device waiting for approval.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub approve: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct MemberView {
    pub account_id: Uuid,
    pub name: String,
    pub email: String,
    pub role: Role,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct UpdateMember {
    pub role: Role,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ApiKeyView {
    pub id: Uuid,
    pub label: String,
    #[ts(type = "number")]
    pub created_at: i64,
    #[ts(type = "number | null")]
    pub last_used_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CreateApiKey {
    pub label: String,
}

/// The key is shown once; only its hash is stored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CreatedApiKey {
    pub id: Uuid,
    pub key: String,
}

/// An account, for the platform admin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AccountView {
    pub id: Uuid,
    pub email: String,
    pub name: String,
    pub is_admin: bool,
    #[ts(type = "number")]
    pub created_at: i64,
    #[ts(type = "number | null")]
    pub last_login_at: Option<i64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn update_network_tells_missing_from_null() {
        let keep: UpdateNetwork = serde_json::from_value(json!({})).unwrap();
        assert_eq!(keep.hours, None);
        let clear: UpdateNetwork = serde_json::from_value(json!({ "hours": null })).unwrap();
        assert_eq!(clear.hours, Some(None));
        let set: UpdateNetwork = serde_json::from_value(json!({
            "hours": { "timezone": "Europe/Sofia", "windows": [] }
        }))
        .unwrap();
        assert!(matches!(set.hours, Some(Some(_))));
    }

    #[test]
    fn roles_are_ordered_by_power() {
        assert!(Role::Viewer < Role::Admin && Role::Admin < Role::Owner);
        for r in [Role::Viewer, Role::Admin, Role::Owner] {
            assert_eq!(Role::parse(r.as_str()), Some(r));
        }
    }
}
