//! Domain types — single source of truth.
//! These structs serialise directly to the JSON schema consumed by the Svelte frontend.
//! Every field name and variant must match the TypeScript types in src/lib/types.ts.

use serde::{Deserialize, Serialize};

// ── Status / confidence enums ─────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum CheckStatus {
    Pass,
    Warn,
    Fail,
    Skip,
}

impl std::fmt::Display for CheckStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pass => write!(f, "PASS"),
            Self::Warn => write!(f, "WARN"),
            Self::Fail => write!(f, "FAIL"),
            Self::Skip => write!(f, "SKIP"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Confidence {
    High,
    Medium,
    Low,
}

impl std::fmt::Display for Confidence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::High   => write!(f, "HIGH"),
            Self::Medium => write!(f, "MEDIUM"),
            Self::Low    => write!(f, "LOW"),
        }
    }
}

// ── Layer labels ──────────────────────────────────────────────────────────

/// The eight diagnostic layers, matching the failure mode table in the foundation doc.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Layer {
    L0, // GPU / NVIDIA driver
    L1, // D-Bus / portal session
    L2, // Portal backend / ScreenCast interface
    L3, // Wayland compositor protocols
    L4, // PipeWire graph
    L5, // Flatpak permissions
    L6, // Environment variables
    L7, // KWin plugins
}

impl std::fmt::Display for Layer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

// ── Core result type ──────────────────────────────────────────────────────

/// A single check result. The `fix` field drives "Fix it" buttons in the GUI —
/// keep fix commands conservative (restarts only, never destructive).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticResult {
    pub layer: Layer,
    pub check: String,
    pub status: CheckStatus,
    pub detail: String,
    /// Complete, pasteable shell command. None if no automated fix exists.
    pub fix: Option<String>,
    pub confidence: Confidence,
}

impl DiagnosticResult {
    /// Convenience constructor for a passing check.
    pub fn pass(layer: Layer, check: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            layer,
            check: check.into(),
            status: CheckStatus::Pass,
            detail: detail.into(),
            fix: None,
            confidence: Confidence::High,
        }
    }

    /// Convenience constructor for a failing check with a fix command.
    pub fn fail(
        layer: Layer,
        check: impl Into<String>,
        detail: impl Into<String>,
        fix: impl Into<String>,
        confidence: Confidence,
    ) -> Self {
        Self {
            layer,
            check: check.into(),
            status: CheckStatus::Fail,
            detail: detail.into(),
            fix: Some(fix.into()),
            confidence,
        }
    }

    /// Convenience constructor for a warning.
    pub fn warn(
        layer: Layer,
        check: impl Into<String>,
        detail: impl Into<String>,
        fix: Option<String>,
        confidence: Confidence,
    ) -> Self {
        Self {
            layer,
            check: check.into(),
            status: CheckStatus::Warn,
            detail: detail.into(),
            fix,
            confidence,
        }
    }

    /// Convenience constructor for a skipped check (prereq missing).
    pub fn skip(layer: Layer, check: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            layer,
            check: check.into(),
            status: CheckStatus::Skip,
            detail: reason.into(),
            fix: None,
            confidence: Confidence::Low,
        }
    }
}

// ── Report types ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticSummary {
    pub pass: usize,
    pub warn: usize,
    pub fail: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    pub generated_at: String,
    pub hostname: String,
    pub kernel: String,
    pub nvidia_driver: Option<String>,
    pub bazzite_image: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticReport {
    pub schema_version: String,
    pub system: SystemInfo,
    pub results: Vec<DiagnosticResult>,
    pub summary: DiagnosticSummary,
}

// ── Capture test ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureTestResult {
    pub success: bool,
    pub node_id: Option<u32>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub format: Option<String>,
    pub error: Option<String>,
}
