use super::*;

#[test]
fn test_session_creation() {
    let session = Session::new("secret".to_string());
    assert!(session.check_password("secret"));
    assert!(!session.check_password("wrong"));
}

#[test]
fn test_empty_password() {
    let session = Session::new(String::new());
    assert!(session.check_password(""));
    assert!(session.check_password("anything"));
}

#[test]
fn test_user_management() {
    let session = Session::new(String::new());

    let id1 = session.add_user("Alice".to_string());
    let id2 = session.add_user("Bob".to_string());

    assert_ne!(id1, id2);
    assert_eq!(session.get_users().len(), 2);

    session.update_cursor(id1, 10, 20);
    let user = session.get_user(id1).unwrap();
    assert_eq!(user.col, 10);
    assert_eq!(user.row, 20);

    session.remove_user(id1);
    assert_eq!(session.get_users().len(), 1);
    assert!(session.get_user(id1).is_none());
}

#[test]
fn test_chat_history() {
    let session = Session::new(String::new());

    session.add_chat_message(1, "Alice".to_string(), "Hello!".to_string());
    session.add_chat_message(2, "Bob".to_string(), "Hi there!".to_string());

    let history = session.get_chat_history();
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].nick, "Alice");
    assert_eq!(history[1].text, "Hi there!");
}
