//! Local form state for Mentci intercept-policy controls.
//!
//! This module owns only view-state and typed request construction. The Mentci
//! daemon and criome remain the policy source of truth.

use signal_criome::{
    ActiveInterceptPolicies, ExpiryAction, InterceptPolicy, InterceptPolicyCancellation,
    InterceptPolicyIdentifier, InterceptPolicyProposal, InterceptTargetSelector, MentciSessionSlot,
    PolicyDurationNanos, PolicyOverlapMode, PolicyPriority, SpiritOperationName,
    SpiritOperationNames, SpiritProcessKey,
};
use signal_mentci::{InterceptPolicyObservation, MentciReply, MentciRequest};
use thiserror::Error;

const NANOS_PER_SECOND: u64 = 1_000_000_000;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PolicyForm {
    pub session_slot: String,
    pub target_key: String,
    pub operation_names: String,
    pub duration_seconds: String,
    pub expiry_action: ExpiryAction,
    pub priority: String,
    pub overlap_mode: PolicyOverlapMode,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PolicyEditorState {
    pub form: PolicyForm,
    policies: Vec<InterceptPolicy>,
    selected_policy: Option<InterceptPolicyIdentifier>,
    feedback: Option<String>,
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum PolicyEditorError {
    #[error("session slot is required")]
    MissingSessionSlot,
    #[error("target key is required")]
    MissingTargetKey,
    #[error("at least one operation name is required")]
    MissingOperationName,
    #[error("duration seconds must be a positive integer")]
    InvalidDurationSeconds,
    #[error("duration seconds is too large")]
    DurationTooLarge,
    #[error("priority must be a non-negative integer")]
    InvalidPriority,
}

impl Default for PolicyForm {
    fn default() -> Self {
        Self {
            session_slot: "mentci-egui".to_string(),
            target_key: "spirit-process-main".to_string(),
            operation_names: "Record\nObserve".to_string(),
            duration_seconds: "300".to_string(),
            expiry_action: ExpiryAction::LeaveParked,
            priority: "10".to_string(),
            overlap_mode: PolicyOverlapMode::ReplaceSamePriorityOverlap,
        }
    }
}

impl PolicyForm {
    pub fn proposal(&self) -> Result<InterceptPolicyProposal, PolicyEditorError> {
        let session_slot = self.trimmed_session_slot()?;
        let target_key = self.trimmed_target_key()?;
        let duration = self.duration_nanos()?;
        let priority = self.priority()?;
        Ok(InterceptPolicyProposal {
            session_slot: MentciSessionSlot::new(session_slot),
            target: InterceptTargetSelector::new(SpiritProcessKey::new(target_key)),
            spirit_operation_names: SpiritOperationNames::from_names(self.operation_names()?),
            duration,
            expiry_action: self.expiry_action,
            priority,
            overlap_mode: self.overlap_mode,
        })
    }

    pub fn absorb_policy(&mut self, policy: &InterceptPolicy) {
        self.session_slot = policy.session_slot.as_str().to_string();
        self.target_key = policy.target.payload().as_str().to_string();
        self.operation_names = policy
            .spirit_operation_names
            .names()
            .iter()
            .map(SpiritOperationName::as_str)
            .collect::<Vec<_>>()
            .join("\n");
        self.duration_seconds = policy.window.duration_seconds().to_string();
        self.expiry_action = policy.expiry_action;
        self.priority = policy.priority.into_u64().to_string();
        self.overlap_mode = PolicyOverlapMode::ReplaceSamePriorityOverlap;
    }

    pub fn expiry_action_label(&self) -> &'static str {
        match self.expiry_action {
            ExpiryAction::AutoApprove => "auto-approve",
            ExpiryAction::AutoReject => "auto-reject",
            ExpiryAction::LeaveParked => "leave-parked",
        }
    }

    pub fn overlap_mode_label(&self) -> &'static str {
        match self.overlap_mode {
            PolicyOverlapMode::RejectSamePriorityOverlap => "reject overlap",
            PolicyOverlapMode::ReplaceSamePriorityOverlap => "replace overlap",
        }
    }

    fn trimmed_session_slot(&self) -> Result<String, PolicyEditorError> {
        self.trimmed_text(&self.session_slot)
            .ok_or(PolicyEditorError::MissingSessionSlot)
    }

    fn trimmed_target_key(&self) -> Result<String, PolicyEditorError> {
        self.trimmed_text(&self.target_key)
            .ok_or(PolicyEditorError::MissingTargetKey)
    }

    fn trimmed_text(&self, text: &str) -> Option<String> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    }

    fn operation_names(&self) -> Result<Vec<SpiritOperationName>, PolicyEditorError> {
        let names: Vec<SpiritOperationName> = self
            .operation_names
            .split(|character| matches!(character, '\n' | ',' | '\r'))
            .filter_map(|name| self.trimmed_text(name))
            .map(SpiritOperationName::new)
            .collect();
        if names.is_empty() {
            Err(PolicyEditorError::MissingOperationName)
        } else {
            Ok(names)
        }
    }

    fn duration_nanos(&self) -> Result<PolicyDurationNanos, PolicyEditorError> {
        let seconds = self
            .duration_seconds
            .trim()
            .parse::<u64>()
            .map_err(|_| PolicyEditorError::InvalidDurationSeconds)?;
        if seconds == 0 {
            return Err(PolicyEditorError::InvalidDurationSeconds);
        }
        seconds
            .checked_mul(NANOS_PER_SECOND)
            .map(PolicyDurationNanos::new)
            .ok_or(PolicyEditorError::DurationTooLarge)
    }

    fn priority(&self) -> Result<PolicyPriority, PolicyEditorError> {
        self.priority
            .trim()
            .parse::<u64>()
            .map(PolicyPriority::new)
            .map_err(|_| PolicyEditorError::InvalidPriority)
    }
}

impl PolicyEditorState {
    pub fn new() -> Self {
        Self {
            form: PolicyForm::default(),
            policies: Vec::new(),
            selected_policy: None,
            feedback: None,
        }
    }

    pub fn policies(&self) -> &[InterceptPolicy] {
        &self.policies
    }

    pub fn selected_policy(&self) -> Option<&InterceptPolicyIdentifier> {
        self.selected_policy.as_ref()
    }

    pub fn feedback(&self) -> Option<&str> {
        self.feedback.as_deref()
    }

    pub fn list_request(&self) -> MentciRequest {
        MentciRequest::ListInterceptPolicies(InterceptPolicyObservation::new())
    }

    pub fn create_request(&self) -> Result<MentciRequest, PolicyEditorError> {
        Ok(MentciRequest::CreateInterceptPolicy(self.form.proposal()?))
    }

    pub fn replace_request(&self) -> Result<MentciRequest, PolicyEditorError> {
        Ok(MentciRequest::ReplaceInterceptPolicy(self.form.proposal()?))
    }

    pub fn cancel_request(&self) -> Option<MentciRequest> {
        self.selected_policy
            .clone()
            .map(InterceptPolicyCancellation::new)
            .map(MentciRequest::CancelInterceptPolicy)
    }

    pub fn select_policy(&mut self, identifier: InterceptPolicyIdentifier) {
        self.selected_policy = Some(identifier.clone());
        if let Some(policy) = self
            .policies
            .iter()
            .find(|policy| policy.identifier == identifier)
        {
            self.form.absorb_policy(policy);
            self.feedback = Some(format!("editing {}", policy.identifier.as_str()));
        }
    }

    pub fn clear_selection(&mut self) {
        self.selected_policy = None;
        self.form = PolicyForm::default();
        self.feedback = Some("new policy".to_string());
    }

    pub fn record_error(&mut self, error: &dyn std::error::Error) {
        self.feedback = Some(error.to_string());
    }

    pub fn absorb_reply(&mut self, reply: &MentciReply) {
        match reply {
            MentciReply::InterceptPoliciesListed(policies) => self.absorb_policy_list(policies),
            MentciReply::InterceptPolicyCreated(policy) => {
                self.upsert_policy(policy.clone());
                self.selected_policy = Some(policy.identifier.clone());
                self.feedback = Some(format!("created {}", policy.identifier.as_str()));
            }
            MentciReply::InterceptPolicyReplaced(policy) => {
                self.upsert_policy(policy.clone());
                self.selected_policy = Some(policy.identifier.clone());
                self.feedback = Some(format!("replaced {}", policy.identifier.as_str()));
            }
            MentciReply::InterceptPolicyCancelled(identifier) => {
                self.policies
                    .retain(|policy| policy.identifier.as_str() != identifier.as_str());
                if self
                    .selected_policy
                    .as_ref()
                    .is_some_and(|selected| selected.as_str() == identifier.as_str())
                {
                    self.selected_policy = None;
                }
                self.feedback = Some(format!("cancelled {}", identifier.as_str()));
            }
            MentciReply::Rejection(_) => {
                self.feedback = Some("daemon rejected policy request; see transcript".to_string());
            }
            _ => {}
        }
    }

    fn absorb_policy_list(&mut self, policies: &ActiveInterceptPolicies) {
        self.policies = policies.policies().to_vec();
        self.feedback = Some(format!("{} active policies", self.policies.len()));
        if self.selected_policy.as_ref().is_some_and(|identifier| {
            !self
                .policies
                .iter()
                .any(|policy| policy.identifier.as_str() == identifier.as_str())
        }) {
            self.selected_policy = None;
        }
    }

    fn upsert_policy(&mut self, policy: InterceptPolicy) {
        if let Some(existing) = self
            .policies
            .iter_mut()
            .find(|existing| existing.identifier.as_str() == policy.identifier.as_str())
        {
            *existing = policy;
        } else {
            self.policies.push(policy);
        }
    }
}

impl Default for PolicyEditorState {
    fn default() -> Self {
        Self::new()
    }
}

trait InterceptPolicyWindowDuration {
    fn duration_seconds(&self) -> u64;
}

impl InterceptPolicyWindowDuration for signal_criome::InterceptPolicyWindow {
    fn duration_seconds(&self) -> u64 {
        self.expires_at
            .into_u64()
            .saturating_sub(self.starts_at.into_u64())
            / NANOS_PER_SECOND
    }
}
