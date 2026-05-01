//! Cloud type definitions.
//!
//! Derived from `packages/types/src/cloud.ts`.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// JWTPayload
// ---------------------------------------------------------------------------

/// JWT payload for cloud authentication.
///
/// Source: `packages/types/src/cloud.ts` — `JWTPayload`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JWTPayload {
    pub iss: Option<String>,
    pub sub: Option<String>,
    pub exp: Option<f64>,
    pub iat: Option<f64>,
    pub nbf: Option<f64>,
    pub v: Option<u64>,
    pub r: Option<JwtResource>,
}

/// JWT resource claims.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtResource {
    pub u: Option<String>,
    pub o: Option<String>,
    pub t: Option<String>,
}

// ---------------------------------------------------------------------------
// CloudUserInfo
// ---------------------------------------------------------------------------

/// Cloud user information.
///
/// Source: `packages/types/src/cloud.ts` — `CloudUserInfo`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudUserInfo {
    pub id: Option<String>,
    pub name: Option<String>,
    pub email: Option<String>,
    pub picture: Option<String>,
    pub organization_id: Option<String>,
    pub organization_name: Option<String>,
    pub organization_role: Option<String>,
    pub organization_image_url: Option<String>,
}

// ---------------------------------------------------------------------------
// CloudOrganization
// ---------------------------------------------------------------------------

/// Cloud organization.
///
/// Source: `packages/types/src/cloud.ts` — `CloudOrganization`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudOrganization {
    pub id: String,
    pub name: String,
    pub slug: Option<String>,
    pub image_url: Option<String>,
    pub has_image: Option<bool>,
    pub created_at: Option<f64>,
    pub updated_at: Option<f64>,
}

// ---------------------------------------------------------------------------
// CloudOrganizationMembership
// ---------------------------------------------------------------------------

/// Cloud organization membership.
///
/// Source: `packages/types/src/cloud.ts` — `CloudOrganizationMembership`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudOrganizationMembership {
    pub id: String,
    pub organization: CloudOrganization,
    pub role: String,
    pub permissions: Option<Vec<String>>,
    pub created_at: Option<f64>,
    pub updated_at: Option<f64>,
}
