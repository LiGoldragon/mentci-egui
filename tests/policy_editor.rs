use mentci_egui::policy_editor::{PolicyEditorError, PolicyEditorState};
use signal_criome::{ExpiryAction, PolicyOverlapMode};
use signal_mentci::MentciRequest;

#[test]
fn policy_editor_builds_create_request_from_form_fields() {
    let mut editor = PolicyEditorState::new();
    editor.form.session_slot = "session-a".to_string();
    editor.form.target_key = "spirit-process-main".to_string();
    editor.form.operation_names = "Record\nObserve".to_string();
    editor.form.duration_seconds = "12".to_string();
    editor.form.expiry_action = ExpiryAction::AutoReject;
    editor.form.priority = "7".to_string();
    editor.form.overlap_mode = PolicyOverlapMode::RejectSamePriorityOverlap;

    let request = editor.create_request().expect("create request");
    let MentciRequest::CreateInterceptPolicy(proposal) = request else {
        panic!("expected create policy request");
    };

    assert_eq!(proposal.session_slot.as_str(), "session-a");
    assert_eq!(proposal.target.payload().as_str(), "spirit-process-main");
    assert_eq!(proposal.spirit_operation_names.names().len(), 2);
    assert_eq!(proposal.duration.into_u64(), 12_000_000_000);
    assert_eq!(proposal.expiry_action, ExpiryAction::AutoReject);
    assert_eq!(proposal.priority.into_u64(), 7);
    assert_eq!(
        proposal.overlap_mode,
        PolicyOverlapMode::RejectSamePriorityOverlap
    );
}

#[test]
fn policy_editor_requires_at_least_one_operation_name() {
    let mut editor = PolicyEditorState::new();
    editor.form.operation_names = "  \n , ".to_string();

    let error = editor.create_request().expect_err("operation required");

    assert_eq!(error, PolicyEditorError::MissingOperationName);
}

#[test]
fn policy_editor_rejects_zero_duration() {
    let mut editor = PolicyEditorState::new();
    editor.form.duration_seconds = "0".to_string();

    let error = editor.replace_request().expect_err("duration required");

    assert_eq!(error, PolicyEditorError::InvalidDurationSeconds);
}
