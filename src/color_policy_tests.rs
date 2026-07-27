use super::{ColorChoice, resolve_policy};

#[test]
fn explicit_color_choice_overrides_environment_and_terminal_detection() {
    assert!(!resolve_policy(ColorChoice::Never, true, false, true, true));
    assert!(resolve_policy(
        ColorChoice::Always,
        false,
        true,
        false,
        false
    ));
}

#[test]
fn automatic_color_choice_applies_environment_precedence() {
    assert!(!resolve_policy(ColorChoice::Auto, true, true, true, true));
    assert!(resolve_policy(ColorChoice::Auto, false, false, true, false));
    assert!(resolve_policy(ColorChoice::Auto, false, false, false, true));
    assert!(resolve_policy(ColorChoice::Auto, true, false, false, false));
    assert!(!resolve_policy(
        ColorChoice::Auto,
        false,
        false,
        false,
        false
    ));
}
