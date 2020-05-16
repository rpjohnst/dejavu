use std::{char, cmp};
use gml::symbol::Symbol;
use gml::{self, vm};

#[derive(Default)]
pub struct State;

#[gml::bind(Api)]
impl State {
    #[gml::function]
    pub fn chr(val: u32) -> Symbol {
        let c = char::from_u32(val).unwrap_or(char::REPLACEMENT_CHARACTER);
        Symbol::intern(c.encode_utf8(&mut [0; 4]))
    }

    #[gml::function]
    pub fn ord(str: Symbol) -> u32 {
        str.chars().next().unwrap_or('\0') as u32
    }

    #[gml::function]
    pub fn real(str: vm::ValueRef) -> f64 {
        match str.decode() {
            vm::Data::Real(real) => real,
            vm::Data::String(str) => str.parse().unwrap_or(0.0),
            _ => 0.0,
        }
    }

    #[gml::function]
    pub fn string(val: vm::ValueRef) -> Symbol {
        match val.decode() {
            vm::Data::Real(val) => Symbol::intern(&format!("{}", val)),
            vm::Data::String(val) => val,
            _ => Symbol::default(),
        }
    }

    #[gml::function]
    pub fn string_format(val: vm::ValueRef, tot: u32, dec: u32) -> Symbol {
        let tot = tot as usize;
        let dec = dec as usize;
        match val.decode() {
            vm::Data::Real(val) => Symbol::intern(&format!("{:1$.2$}", val, tot, dec)),
            vm::Data::String(val) => val,
            _ => Symbol::default(),
        }
    }

    #[gml::function]
    pub fn string_length(str: Symbol) -> u32 { str.chars().count() as u32 }

    #[gml::function]
    pub fn string_byte_length(str: Symbol) -> u32 { str.len() as u32 }

    #[gml::function]
    pub fn string_pos(substr: Symbol, str: Symbol) -> u32 {
        str.split(&substr[..]).next().map(|head| head.chars().count() + 1).unwrap_or(0) as u32
    }

    #[gml::function]
    pub fn string_copy(str: Symbol, index: u32, count: u32) -> Symbol {
        let index = cmp::max(index as usize, 1) - 1;
        let count = count as usize;

        let mut indices = str.char_indices().map(|(index, _)| index);
        let start = indices.nth(index).unwrap_or(0);
        let end = indices.take(count).last().unwrap_or(start);

        Symbol::intern(&str[start..end])
    }

    #[gml::function]
    pub fn string_char_at(str: Symbol, index: u32) -> Symbol {
        let index = cmp::max(index as usize, 1) - 1;
        let mut char = [0; 4];
        let mut empty = String::new();
        let str = str.chars().nth(index).map(|c| c.encode_utf8(&mut char)).unwrap_or(&mut empty);
        Symbol::intern(str)
    }

    #[gml::function]
    pub fn string_byte_at(str: Symbol, index: u32) -> u32 {
        let index = cmp::max(index as usize, 1) - 1;
        str.as_bytes().get(index).cloned().unwrap_or(0) as u32
    }

    #[gml::function]
    pub fn string_delete(str: Symbol, index: u32, count: u32) -> Symbol {
        let index = cmp::max(index as usize, 1) - 1;
        let count = count as usize;

        let mut indices = str.char_indices().map(|(index, _)| index);
        let start = indices.nth(index).unwrap_or(0);
        let end = indices.take(count).last().unwrap_or(start);

        let mut string = String::new();
        string.push_str(&str[..start]);
        string.push_str(&str[end..]);
        Symbol::intern(&string)
    }

    #[gml::function]
    pub fn string_insert(substr: Symbol, str: Symbol, index: u32) -> Symbol {
        let index = cmp::max(index as usize, 1) - 1;
        let index = str.char_indices().map(|(index, _)| index)
            .skip(index)
            .next()
            .unwrap_or(str.len());

        let mut string = String::new();
        string.push_str(&str[..index]);
        string.push_str(&substr[..]);
        string.push_str(&str[index..]);
        Symbol::intern(&string)
    }

    #[gml::function]
    pub fn string_replace(str: Symbol, substr: Symbol, newstr: Symbol) -> Symbol {
        let string = str.replacen(&substr[..], &newstr[..], 1);
        Symbol::intern(&string)
    }

    #[gml::function]
    pub fn string_replace_all(str: Symbol, substr: Symbol, newstr: Symbol) -> Symbol {
        let string = str.replace(&substr[..], &newstr[..]);
        Symbol::intern(&string)
    }

    #[gml::function]
    pub fn string_count(substr: Symbol, str: Symbol) -> u32 {
        str.matches(&substr[..]).count() as u32
    }

    #[gml::function]
    pub fn string_lower(str: Symbol) -> Symbol {
        Symbol::intern(&str.to_ascii_lowercase())
    }

    #[gml::function]
    pub fn string_upper(str: Symbol) -> Symbol {
        Symbol::intern(&str.to_ascii_uppercase())
    }

    #[gml::function]
    pub fn string_repeat(str: Symbol, count: u32) -> Symbol {
        Symbol::intern(&str.repeat(count as usize))
    }

    #[gml::function]
    pub fn string_letters(str: Symbol) -> Symbol {
        let string: String = str.matches(|char: char| char.is_ascii_alphabetic()).collect();
        Symbol::intern(&string)
    }

    #[gml::function]
    pub fn string_digits(str: Symbol) -> Symbol {
        let string: String = str.matches(|char: char| char.is_digit(10)).collect();
        Symbol::intern(&string)
    }

    #[gml::function]
    pub fn string_lettersdigits(str: Symbol) -> Symbol {
        let string: String = str.matches(|char: char| char.is_ascii_alphanumeric()).collect();
        Symbol::intern(&string)
    }
}
