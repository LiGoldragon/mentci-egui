use mentci::configuration::DaemonConfiguration;
use mentci::daemon::Daemon;
use mentci_egui::daemon_client::{DaemonClient, SocketKind};
use meta_signal_mentci::{
    ComponentKind, ComponentSocket, ComponentSocketKind, MentciDaemonConfiguration,
    NotificationClient, PersonaIdentity, PersonaKeyLabel, PersonaName, StandardSocket,
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
