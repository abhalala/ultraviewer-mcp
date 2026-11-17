use crate::input::{Encoder, KeyCode, ALT, CONTROL, SHIFT, SUPER};
use crate::wire::Result;

#[derive(Clone, Debug, PartialEq)]
pub struct Chord {
    modifiers: Vec<KeyCode>,
    key: KeyCode,
    text: Option<String>,
}

pub fn parse(text: &str) -> Result<Vec<Chord>> {
    let chords = text
        .split_whitespace()
        .map(parse_chord)
        .collect::<Result<Vec<_>>>()?;
    if chords.is_empty() {
        return Err("No keys were given.".into());
    }
    Ok(chords)
}

fn parse_chord(text: &str) -> Result<Chord> {
    let mut parts: Vec<&str> = text.split('+').filter(|part| !part.is_empty()).collect();
    if text.ends_with("++") || text == "+" {
        parts.push("plus");
    }
    if parts.is_empty() {
        return Err("No keys were given.".into());
    }
    let mut modifiers = Vec::new();
    let mut key = None;
    let mut character = None;
    for (index, part) in parts.iter().enumerate() {
        let last = index + 1 == parts.len();
        let lower = part.to_lowercase();
        if let Some(modifier) = modifier(&lower) {
            if last {
                key = Some(modifier);
            } else if !modifiers.contains(&modifier) {
                modifiers.push(modifier);
            }
            continue;
        }
        if !last {
            return Err(unknown_key(part).into());
        }
        let (code, text, needs_shift) = named(&lower)
            .map(|code| {
                let plus = lower == "plus";
                (code, plus.then(|| "+".to_string()), plus)
            })
            .or_else(|| character_key(part))
            .ok_or_else(|| unknown_key(part))?;
        key = Some(code);
        character = text;
        if needs_shift && !modifiers.contains(&KeyCode::ShiftLeft) {
            modifiers.push(KeyCode::ShiftLeft);
        }
    }
    Ok(Chord {
        modifiers,
        key: key.ok_or("No keys were given.")?,
        text: character,
    })
}

fn unknown_key(key: &str) -> String {
    format!("Unknown key \"{key}\". Use names like enter, tab, esc, a, f5, ctrl, alt, shift, win.")
}

fn modifier(name: &str) -> Option<KeyCode> {
    match name {
        "ctrl" | "control" => Some(KeyCode::ControlLeft),
        "alt" | "option" | "opt" => Some(KeyCode::AltLeft),
        "shift" => Some(KeyCode::ShiftLeft),
        "win" | "windows" | "cmd" | "command" | "meta" | "super" => Some(KeyCode::SuperLeft),
        _ => None,
    }
}

fn modifier_flag(code: KeyCode) -> u8 {
    match code {
        KeyCode::ControlLeft | KeyCode::ControlRight => CONTROL,
        KeyCode::AltLeft | KeyCode::AltRight => ALT,
        KeyCode::ShiftLeft | KeyCode::ShiftRight => SHIFT,
        _ => SUPER,
    }
}

fn named(name: &str) -> Option<KeyCode> {
    use KeyCode::*;
    Some(match name {
        "enter" | "return" => Enter,
        "tab" => Tab,
        "space" => Space,
        "backspace" => Backspace,
        "esc" | "escape" => Escape,
        "delete" | "del" | "forwarddelete" => Delete,
        "insert" | "ins" => Insert,
        "home" => Home,
        "end" => End,
        "pageup" | "pgup" => PageUp,
        "pagedown" | "pgdn" => PageDown,
        "left" => ArrowLeft,
        "right" => ArrowRight,
        "down" => ArrowDown,
        "up" => ArrowUp,
        "capslock" => CapsLock,
        "plus" => Equal,
        "minus" => Minus,
        "f1" => F1,
        "f2" => F2,
        "f3" => F3,
        "f4" => F4,
        "f5" => F5,
        "f6" => F6,
        "f7" => F7,
        "f8" => F8,
        "f9" => F9,
        "f10" => F10,
        "f11" => F11,
        "f12" => F12,
        "f13" => F13,
        "f14" => F14,
        "f15" => F15,
        "f16" => F16,
        "f17" => F17,
        "f18" => F18,
        "f19" => F19,
        "f20" => F20,
        _ => return None,
    })
}

fn character_key(text: &str) -> Option<(KeyCode, Option<String>, bool)> {
    let mut chars = text.chars();
    let character = chars.next()?;
    if chars.next().is_some() {
        return None;
    }
    use KeyCode::*;
    let lower = character.to_ascii_lowercase();
    let code = match lower {
        'a' => KeyA,
        'b' => KeyB,
        'c' => KeyC,
        'd' => KeyD,
        'e' => KeyE,
        'f' => KeyF,
        'g' => KeyG,
        'h' => KeyH,
        'i' => KeyI,
        'j' => KeyJ,
        'k' => KeyK,
        'l' => KeyL,
        'm' => KeyM,
        'n' => KeyN,
        'o' => KeyO,
        'p' => KeyP,
        'q' => KeyQ,
        'r' => KeyR,
        's' => KeyS,
        't' => KeyT,
        'u' => KeyU,
        'v' => KeyV,
        'w' => KeyW,
        'x' => KeyX,
        'y' => KeyY,
        'z' => KeyZ,
        '0' | ')' => Digit0,
        '1' | '!' => Digit1,
        '2' | '@' => Digit2,
        '3' | '#' => Digit3,
        '4' | '$' => Digit4,
        '5' | '%' => Digit5,
        '6' | '^' => Digit6,
        '7' | '&' => Digit7,
        '8' | '*' => Digit8,
        '9' | '(' => Digit9,
        '-' | '_' => Minus,
        '=' | '+' => Equal,
        '[' | '{' => BracketLeft,
        ']' | '}' => BracketRight,
        '\\' | '|' => Backslash,
        ';' | ':' => Semicolon,
        '\'' | '"' => Quote,
        '`' | '~' => Backquote,
        ',' | '<' => Comma,
        '.' | '>' => Period,
        '/' | '?' => Slash,
        _ => return None,
    };
    let shifted = character.is_uppercase() || "!@#$%^&*()_+{}|:\"<>?~".contains(character);
    Some((code, Some(character.to_string()), shifted))
}

pub fn chord_payloads(encoder: &mut Encoder, chord: &Chord) -> Vec<Vec<u8>> {
    let mut payloads = Vec::new();
    let mut flags = 0;
    for modifier in &chord.modifiers {
        flags |= modifier_flag(*modifier);
        payloads.extend(encoder.key_with(*modifier, true, None, flags));
    }
    let key_flag = modifier_flag(chord.key);
    let is_modifier = matches!(
        chord.key,
        KeyCode::ControlLeft
            | KeyCode::ControlRight
            | KeyCode::AltLeft
            | KeyCode::AltRight
            | KeyCode::ShiftLeft
            | KeyCode::ShiftRight
            | KeyCode::SuperLeft
            | KeyCode::SuperRight
    );
    let key_flags = if is_modifier { flags | key_flag } else { flags };
    payloads.extend(encoder.key_with(chord.key, true, chord.text.as_deref(), key_flags));
    payloads.extend(encoder.key_with(chord.key, false, chord.text.as_deref(), flags));
    for modifier in chord.modifiers.iter().rev() {
        flags &= !modifier_flag(*modifier);
        payloads.extend(encoder.key_with(*modifier, false, None, flags));
    }
    payloads
}

pub fn payloads(encoder: &mut Encoder, chords: &[Chord], repeat: u32) -> Vec<Vec<u8>> {
    let mut output = Vec::new();
    for _ in 0..repeat {
        for chord in chords {
            output.extend(chord_payloads(encoder, chord));
        }
    }
    output
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct TypingSummary {
    pub total_units: usize,
    pub skipped_units: usize,
}

fn flush_text(encoder: &mut Encoder, pending: &mut String, payloads: &mut Vec<Vec<u8>>) {
    if !pending.is_empty() {
        payloads.extend(encoder.text(pending));
        pending.clear();
    }
}

pub fn typing_payloads(encoder: &mut Encoder, text: &str) -> (Vec<Vec<u8>>, TypingSummary) {
    let mut payloads = Vec::new();
    let mut pending = String::new();
    let mut summary = TypingSummary {
        total_units: text.encode_utf16().count(),
        skipped_units: 0,
    };
    let mut characters = text.chars().peekable();
    while let Some(character) = characters.next() {
        match character {
            '\n' | '\r' => {
                if character == '\r' && characters.peek() == Some(&'\n') {
                    characters.next();
                }
                flush_text(encoder, &mut pending, &mut payloads);
                payloads.extend(chord_payloads(encoder, &parse_chord("enter").unwrap()));
            }
            '\t' => {
                flush_text(encoder, &mut pending, &mut payloads);
                payloads.extend(chord_payloads(encoder, &parse_chord("tab").unwrap()));
            }
            _ => {
                pending.push(character);
                let mut units = [0u16; 2];
                summary.skipped_units += character
                    .encode_utf16(&mut units)
                    .iter()
                    .filter(|unit| **unit < 0x20 || (0xd800..=0xdfff).contains(*unit))
                    .count();
            }
        }
    }
    flush_text(encoder, &mut pending, &mut payloads);
    (payloads, summary)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_shortcuts_and_shifted_characters() {
        let chords = parse("ctrl+c alt+f4 win+r A + ctrl++").unwrap();
        assert_eq!(chords.len(), 6);
        assert_eq!(chords[0].modifiers, [KeyCode::ControlLeft]);
        assert_eq!(chords[0].key, KeyCode::KeyC);
        assert_eq!(chords[3].modifiers, [KeyCode::ShiftLeft]);
        assert_eq!(chords[4].modifiers, [KeyCode::ShiftLeft]);
        assert_eq!(chords[5].key, KeyCode::Equal);
        assert!(parse("ctrl+").is_ok());
        assert!(parse("not-a-key").is_err());
    }

    #[test]
    fn shortcut_generates_balanced_input() {
        let mut encoder = Encoder::new(true);
        let output = payloads(&mut encoder, &parse("ctrl+c").unwrap(), 1);
        assert_eq!(
            output.iter().map(|payload| payload[5]).collect::<Vec<_>>(),
            [0xa2, 0x43, 0x43, 0xa2]
        );
        assert!(encoder.release_all().is_empty());
    }

    #[test]
    fn typing_handles_newlines_tabs_and_unicode() {
        let mut encoder = Encoder::new(true);
        let (output, summary) = typing_payloads(&mut encoder, "a\nb\tcệ");
        assert!(output.iter().any(|payload| payload[5] == 0x0d));
        assert!(output.iter().any(|payload| payload[5] == 0x09));
        assert!(output.iter().any(|payload| payload[8..10] == [0xc7, 0x1e]));
        assert_eq!(summary.skipped_units, 0);
        let (_, emoji) = typing_payloads(&mut encoder, "😀");
        assert_eq!(emoji.skipped_units, 2);
    }
}
