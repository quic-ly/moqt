use crate::moqt_messages::FullTrackName;

#[test]
fn test_full_track_name_constructors() {
    let name1 = FullTrackName::new_with_namespace_and_name("foo", "bar");
    let list = vec!["foo".to_string(), "bar".to_string()];
    let name2 = FullTrackName::new_with_elements(list);
    assert_eq!(name1, name2);
    //assert_eq!(HashOf(name1), HashOf(name2));
}

#[test]
fn test_full_track_name_order() {
    let name1 = FullTrackName::new_with_elements(vec!["a".to_string(), "b".to_string()]);
    let name2 =
        FullTrackName::new_with_elements(vec!["a".to_string(), "b".to_string(), "c".to_string()]);
    let name3 = FullTrackName::new_with_elements(vec!["b".to_string(), "a".to_string()]);
    assert!(name1 < name2);
    assert!(name2 < name3);
    assert!(name1 < name3);
}

#[test]
fn test_full_track_name_in_namespace() {
    let name1 = FullTrackName::new_with_elements(vec!["a".to_string(), "b".to_string()]);
    let name2 =
        FullTrackName::new_with_elements(vec!["a".to_string(), "b".to_string(), "c".to_string()]);
    let name3 = FullTrackName::new_with_elements(vec!["b".to_string(), "a".to_string()]);

    assert!(name2.in_namespace(&name1));
    assert!(!name1.in_namespace(&name2));
    assert!(name1.in_namespace(&name1));
    assert!(!name2.in_namespace(&name3));
}

#[test]
fn test_full_track_name_to_string() {
    let name1 = FullTrackName::new_with_elements(vec!["a".to_string(), "b".to_string()]);
    assert_eq!(name1.to_string(), r#"{"a", "b"}"#);

    //TODO: let name2 = FullTrackName::new_with_elements(vec!["\xff".to_string(), "\x61".to_string()]);
    // assert_eq!(name2.to_string(), r#"{"\xff", "a"}"#);
}
