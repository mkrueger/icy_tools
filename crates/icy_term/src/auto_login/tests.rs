pub use super::*;

#[test]
fn test_parse_delay() {
    let result = AutoLoginParser::parse("@D5").unwrap();
    assert_eq!(result, vec![AutoLoginCommand::Delay(5)]);

    let result = AutoLoginParser::parse("@D").unwrap();
    assert_eq!(result, vec![AutoLoginCommand::Delay(1)]);
}

#[test]
fn test_parse_commands() {
    let result = AutoLoginParser::parse("@N@P").unwrap();
    assert_eq!(result, vec![AutoLoginCommand::SendFullName, AutoLoginCommand::SendPassword,]);
}

#[test]
fn test_parse_control_codes() {
    let result = AutoLoginParser::parse("@13").unwrap();
    assert_eq!(result, vec![AutoLoginCommand::SendControlCode(13)]);

    let result = AutoLoginParser::parse("!27").unwrap();
    assert_eq!(result, vec![AutoLoginCommand::SendControlCode(27)]);
}

#[test]
fn test_parse_script() {
    let result = AutoLoginParser::parse("!login.scr").unwrap();
    assert_eq!(result, vec![AutoLoginCommand::RunScript("login.scr".to_string())]);
}

#[test]
fn test_parse_mixed() {
    let result = AutoLoginParser::parse("Hello @N, password: @P@13").unwrap();
    assert_eq!(
        result,
        vec![
            AutoLoginCommand::SendText("Hello ".to_string()),
            AutoLoginCommand::SendFullName,
            AutoLoginCommand::SendText(", password: ".to_string()),
            AutoLoginCommand::SendPassword,
            AutoLoginCommand::SendControlCode(13),
        ]
    );
}

#[test]
fn test_parse_all_commands() {
    let script = "@E@W@N@F@L@P@I@D3@13!script.txt";
    let result = AutoLoginParser::parse(script).unwrap();
    assert_eq!(
        result,
        vec![
            AutoLoginCommand::EmulateMailerAccess,
            AutoLoginCommand::WaitForNamePrompt,
            AutoLoginCommand::SendFullName,
            AutoLoginCommand::SendFirstName,
            AutoLoginCommand::SendLastName,
            AutoLoginCommand::SendPassword,
            AutoLoginCommand::DisableIEMSI,
            AutoLoginCommand::Delay(3),
            AutoLoginCommand::SendControlCode(13),
            AutoLoginCommand::RunScript("script.txt".to_string()),
        ]
    );
}
