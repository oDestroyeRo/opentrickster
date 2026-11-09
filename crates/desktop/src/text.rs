use std::ops::Range;

pub fn from_utf16(text: &str, offset: usize) -> usize {
    let mut units = 0;
    for (index, character) in text.char_indices() {
        if units >= offset {
            return index;
        }
        units += character.len_utf16();
    }
    text.len()
}

pub fn to_utf16(text: &str, offset: usize) -> usize {
    text.char_indices()
        .take_while(|(index, _)| *index < offset)
        .map(|(_, c)| c.len_utf16())
        .sum()
}

pub fn composition_selection(
    inserted: &str,
    selection: Option<Range<usize>>,
    start: usize,
) -> Range<usize> {
    selection
        .map(|range| {
            start + from_utf16(inserted, range.start)..start + from_utf16(inserted, range.end)
        })
        .unwrap_or(start + inserted.len()..start + inserted.len())
}

pub fn masked_offset(text: &str, offset: usize) -> usize {
    text.char_indices()
        .take_while(|(index, _)| *index < offset)
        .count()
        * '•'.len_utf8()
}

pub fn unmasked_offset(text: &str, offset: usize) -> usize {
    text.char_indices()
        .nth(offset / '•'.len_utf8())
        .map(|(index, _)| index)
        .unwrap_or(text.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf16_selection_survives_thai_and_surrogate_pairs() {
        let text = "ปู่😀a";
        for (index, _) in text
            .char_indices()
            .chain(std::iter::once((text.len(), '\0')))
        {
            assert_eq!(from_utf16(text, to_utf16(text, index)), index);
            assert_eq!(unmasked_offset(text, masked_offset(text, index)), index);
        }
    }

    #[test]
    fn ime_selection_is_relative_to_inserted_text_not_old_range_end() {
        assert_eq!(composition_selection("ปู่😀", Some(3..5), 7), 16..20);
        assert_eq!(composition_selection("ปู่", None, 7), 16..16);
    }
}
