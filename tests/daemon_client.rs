use mentci::configuration::DaemonConfiguration;
use mentci::daemon::Daemon;
use mentci_egui::daemon_client::{DaemonClient, SocketKind};
use meta_signal_mentci::{
    ComponentKind, ComponentSocket, ComponentSocketKind, MentciDaemonConfiguration,
    NotificationClient, PersonaIdentity, PersonaKeyLabel, PersonaName, StandardSocket,
};
use signal_criome::{
    ParkedRequestIdentifier, RawSpiritOperationPayload, SpiritAuthorizationContext,
    SpiritOperationName, SpiritProcessKey,
};
use signal_mentci::{
    AnswerText, ApprovalSource, ContextBody, ContextLabel, ExplanationText, MentciReply,
    MentciRequest, PromptText, QuestionContext, QuestionProposal,
};

#[test]
fn daemon_client_observes_live_mentci_daemon_as_nota() {
    let directory = tempfile::tempdir().expect("tempdir");
    let mentci_socket = directory.path().join("mentci.socket");
    let criome_socket = directory.path().join("criome.socket");
    let configuration = DaemonConfiguration::new(MentciDaemonConfiguration::new(
        vec![
            ComponentSocket::new(
                ComponentSocketKind::Mentci,
                StandardSocket::unix(mentci_socket.display().to_string()),
            ),
            ComponentSocket::new(
                ComponentSocketKind::MetaCriome,
                StandardSocket::unix(criome_socket.display().to_string()),
            ),
        ],
        PersonaIdentity::new(
            PersonaName::new("psyche"),
            ComponentKind::Persona,
            PersonaKeyLabel::new("home-verdict"),
        ),
        vec![NotificationClient::StatusBar],
    ));
    let daemon = Daemon::from_configuration(configuration)
        .expect("daemon")
        .bind()
        .expect("bind daemon");
    let server = std::thread::spawn(move || daemon.serve_next().expect("serve one request"));

    let entry = DaemonClient::new(&mentci_socket)
        .observe_interface_state()
        .expect("observe interface state");

    server.join().expect("join server");
    assert_eq!(entry.socket_kind, SocketKind::Mentci);
    assert_eq!(entry.operation, "ObserveInterfaceState");
    assert!(entry.request_nota.contains("ObserveInterfaceState"));
    assert!(entry.reply_nota.contains("InterfaceObservationOpened"));
}

#[test]
fn daemon_client_exposes_raw_spirit_payload_in_observed_approval_state() {
    let directory = tempfile::tempdir().expect("tempdir");
    let mentci_socket = directory.path().join("mentci.socket");
    let criome_socket = directory.path().join("criome.socket");
    let configuration = DaemonConfiguration::new(MentciDaemonConfiguration::new(
        vec![
            ComponentSocket::new(
                ComponentSocketKind::Mentci,
                StandardSocket::unix(mentci_socket.display().to_string()),
            ),
            ComponentSocket::new(
                ComponentSocketKind::MetaCriome,
                StandardSocket::unix(criome_socket.display().to_string()),
            ),
        ],
        PersonaIdentity::new(
            PersonaName::new("psyche"),
            ComponentKind::Persona,
            PersonaKeyLabel::new("home-verdict"),
        ),
        vec![NotificationClient::StatusBar],
    ));
    let daemon = Daemon::from_configuration(configuration)
        .expect("daemon")
        .bind()
        .expect("bind daemon");
    let server = std::thread::spawn(move || {
        for _ in 0..2 {
            daemon.serve_next().expect("serve request");
        }
    });

    let spirit_context = SpiritAuthorizationContext {
        operation_name: SpiritOperationName::new("Record"),
        raw_payload: RawSpiritOperationPayload::new(
            "(Record (([(Technology Software)] Decision [payload shown] High Minimum Zero [])))",
        ),
        target_key: SpiritProcessKey::new("spirit-process-main"),
    };
    let proposal = QuestionProposal::new(
        ApprovalSource::CriomeInterception(ParkedRequestIdentifier::new("parked-request-1")),
        PromptText::new("Authorize component operation Record for target spirit-process-main"),
        Some(AnswerText::new("approve")),
        ExplanationText::new("criome parked a component operation matched by intercept policy"),
        vec![
            QuestionContext {
                label: ContextLabel::new("component-target"),
                body: ContextBody::new(spirit_context.target_key.as_str()),
            },
            QuestionContext {
                label: ContextLabel::new("component-operation"),
                body: ContextBody::new(spirit_context.operation_name.as_str()),
            },
            QuestionContext {
                label: ContextLabel::new("component-raw-payload"),
                body: ContextBody::new(spirit_context.raw_payload.as_str()),
            },
        ],
    );

    let client = DaemonClient::new(&mentci_socket);
    let presented = client
        .send_request_typed(MentciRequest::PresentQuestion(proposal))
        .expect("present criome interception question");
    assert!(matches!(presented, MentciReply::QuestionPresented(_)));

    let entry = client
        .observe_interface_state()
        .expect("observe interface state with raw payload");

    server.join().expect("join server");
    assert_eq!(entry.socket_kind, SocketKind::Mentci);
    assert_eq!(entry.operation, "ObserveInterfaceState");
    assert!(entry.reply_nota.contains("CriomeInterception"));
    assert!(entry.reply_nota.contains("component-target"));
    assert!(entry.reply_nota.contains("component-operation"));
    assert!(entry.reply_nota.contains("component-raw-payload"));
    assert!(entry.reply_nota.contains("payload shown"));
}
