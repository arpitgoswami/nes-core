use nes_core::settings::{default_keymap, Settings};

#[test]
fn default_controls_use_wasd() {
    let settings = Settings::default();
    assert_eq!(settings.keymap, default_keymap());
    for (button, key) in [
        ("Up", "W"),
        ("Left", "A"),
        ("Down", "S"),
        ("Right", "D"),
        ("A", "K"),
        ("B", "J"),
        ("Start", "Enter"),
        ("Select", "Backspace"),
    ] {
        assert_eq!(settings.keymap.get(button).map(String::as_str), Some(key));
    }
}
