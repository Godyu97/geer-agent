use super::{Input, parse_input};

#[test]
fn delete_requires_exact_ids_and_explicit_confirmation() {
    let id = "11111111-1111-4111-8111-111111111111";
    assert!(
        matches!(parse_input(&format!("/delete {id} {id}")), Input::Delete { ids, confirmed: false } if ids == vec![id])
    );
    assert!(matches!(
        parse_input(&format!("/delete --yes {id}")),
        Input::Delete {
            confirmed: true,
            ..
        }
    ));
    for line in [
        "/delete",
        "/delete --yes",
        "/delete 11111111",
        "/delete *",
        "/delete --all",
        "/delete title",
    ] {
        assert!(matches!(parse_input(line), Input::Invalid(_)), "{line}");
    }
}

#[test]
fn parses_supported_commands() {
    assert!(matches!(parse_input("/help"), Input::Help));
    assert!(matches!(parse_input("/reset"), Input::Reset));
    assert!(matches!(parse_input("/new"), Input::New));
    assert!(matches!(parse_input("/save"), Input::Save));
    assert!(matches!(parse_input("/compact"), Input::Compact));
    assert!(matches!(
        parse_input("/sessions"),
        Input::Sessions(crate::interaction::SessionScope::Current)
    ));
    assert!(matches!(
        parse_input("/sessions --all"),
        Input::Sessions(crate::interaction::SessionScope::All)
    ));
    assert!(matches!(parse_input("/workspace"), Input::Workspace(None)));
    assert!(
        matches!(parse_input("/workspace ../other"), Input::Workspace(Some(path)) if path == "../other")
    );
    assert!(
        matches!(parse_input("/workspace \"/tmp/foo bar\""), Input::Workspace(Some(path)) if path == "\"/tmp/foo bar\"")
    );
    assert!(matches!(parse_input("/resume abc"), Input::Open(id) if id == "abc"));
    assert!(matches!(parse_input("/open abc"), Input::Open(id) if id == "abc"));
    assert!(matches!(parse_input("/exit"), Input::Exit));
}

#[test]
fn skips_blank_lines_and_reports_unknown_commands() {
    assert!(matches!(parse_input("  \n"), Input::Empty));
    assert!(matches!(parse_input("/other"), Input::Unknown(command) if command == "/other"));
}

#[test]
fn preserves_ordinary_user_input_without_outer_whitespace() {
    assert!(
        matches!(parse_input("  hello model  \n"), Input::Message(message) if message == "hello model")
    );
}
