//! Native spell checking backed by the Windows Spell Checking API
//! (`ISpellChecker`) — the very same engine that powers Notepad's spell
//! check, suggestions, and "Add to dictionary" action. On platforms without
//! that service every operation degrades to a harmless no-op, so callers
//! never need their own `cfg` guards.

use std::ops::Range;

/// A handle to the OS spell-checking service. Construct one and keep it
/// around; it holds the underlying COM checker instance so repeated calls
/// don't reconnect every time.
pub struct SpellChecker {
    backend: backend::Backend,
}

impl SpellChecker {
    /// Connects to the system spell checker. Always succeeds: if the OS
    /// service can't be reached (unsupported platform, COM failure, missing
    /// language pack, …) the checker is still returned, and every method
    /// below simply reports nothing.
    pub fn new() -> Self {
        Self {
            backend: backend::Backend::new(),
        }
    }

    /// Whether a live OS spell-checking backend is available. Used only to
    /// skip unnecessary work; every method already degrades gracefully.
    pub fn is_available(&self) -> bool {
        self.backend.is_available()
    }

    /// Returns the UTF-8 byte ranges of misspelled words in `text`.
    pub fn check(&self, text: &str) -> Vec<Range<usize>> {
        self.backend.check(text)
    }

    /// Returns replacement suggestions for a single (presumably misspelled)
    /// word, best first.
    pub fn suggestions(&self, word: &str) -> Vec<String> {
        self.backend.suggestions(word)
    }

    /// Permanently adds `word` to the user's Windows dictionary, matching
    /// Notepad's "Add to dictionary" context-menu action. The word will no
    /// longer be flagged by this or any other application using the same
    /// system dictionary.
    pub fn add_to_dictionary(&self, word: &str) -> bool {
        self.backend.add_to_dictionary(word)
    }

    /// Ignores `word` for the remainder of the process, without persisting
    /// it to the user dictionary.
    pub fn ignore(&self, word: &str) -> bool {
        self.backend.ignore(word)
    }
}

impl Default for SpellChecker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(windows)]
mod backend {
    use super::Range;
    use windows::Win32::Globalization::{ISpellChecker, ISpellCheckerFactory, SpellCheckerFactory};
    use windows::Win32::System::Com::{
        CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
        CoTaskMemFree,
    };
    use windows::core::{HSTRING, PWSTR};

    pub struct Backend {
        checker: Option<ISpellChecker>,
    }

    impl Backend {
        pub fn new() -> Self {
            // COM may already be initialized (e.g. by the windowing backend);
            // failures here are ignored and simply leave spell checking off.
            unsafe {
                let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
            }
            Self {
                checker: Self::create_checker().ok(),
            }
        }

        fn create_checker() -> windows::core::Result<ISpellChecker> {
            unsafe {
                let factory: ISpellCheckerFactory =
                    CoCreateInstance(&SpellCheckerFactory, None, CLSCTX_INPROC_SERVER)?;
                factory.CreateSpellChecker(&HSTRING::from("en-US"))
            }
        }

        pub fn is_available(&self) -> bool {
            self.checker.is_some()
        }

        pub fn check(&self, text: &str) -> Vec<Range<usize>> {
            let Some(checker) = &self.checker else {
                return Vec::new();
            };
            if text.is_empty() {
                return Vec::new();
            }
            let wide_len = text.encode_utf16().count();
            let Ok(errors) = (unsafe { checker.Check(&HSTRING::from(text)) }) else {
                return Vec::new();
            };
            let mut ranges = Vec::new();
            loop {
                let mut error = None;
                let hr = unsafe { errors.Next(&mut error) };
                let Some(error) = error else { break };
                let indices = unsafe { (error.StartIndex(), error.Length()) };
                if let (Ok(start), Ok(length)) = indices {
                    let start = (start as usize).min(wide_len);
                    let end = start.saturating_add(length as usize).min(wide_len);
                    if end > start {
                        ranges.push(
                            utf16_offset_to_byte_offset(text, start)
                                ..utf16_offset_to_byte_offset(text, end),
                        );
                    }
                }
                if hr.is_err() {
                    break;
                }
            }
            ranges
        }

        pub fn suggestions(&self, word: &str) -> Vec<String> {
            const MAX_SUGGESTIONS: usize = 8;
            let Some(checker) = &self.checker else {
                return Vec::new();
            };
            if word.is_empty() {
                return Vec::new();
            }
            let Ok(suggestions) = (unsafe { checker.Suggest(&HSTRING::from(word)) }) else {
                return Vec::new();
            };
            let mut out = Vec::new();
            while out.len() < MAX_SUGGESTIONS {
                let mut item = [PWSTR::null()];
                let mut fetched = 0u32;
                let hr = unsafe { suggestions.Next(&mut item, Some(&mut fetched)) };
                if fetched == 0 || item[0].is_null() {
                    break;
                }
                if let Ok(text) = unsafe { item[0].to_string() }
                    && !text.is_empty()
                {
                    out.push(text);
                }
                unsafe {
                    CoTaskMemFree(Some(item[0].as_ptr() as *const _));
                }
                if hr.is_err() {
                    break;
                }
            }
            out
        }

        pub fn add_to_dictionary(&self, word: &str) -> bool {
            let Some(checker) = &self.checker else {
                return false;
            };
            if word.trim().is_empty() {
                return false;
            }
            unsafe { checker.Add(&HSTRING::from(word)) }.is_ok()
        }

        pub fn ignore(&self, word: &str) -> bool {
            let Some(checker) = &self.checker else {
                return false;
            };
            if word.trim().is_empty() {
                return false;
            }
            unsafe { checker.Ignore(&HSTRING::from(word)) }.is_ok()
        }
    }

    /// Converts a UTF-16 code-unit offset (as reported by the Win32 spell
    /// checker) into the matching UTF-8 byte offset within `text`.
    fn utf16_offset_to_byte_offset(text: &str, utf16_offset: usize) -> usize {
        let mut units = 0usize;
        for (byte_index, character) in text.char_indices() {
            if units >= utf16_offset {
                return byte_index;
            }
            units += character.len_utf16();
        }
        text.len()
    }
}

#[cfg(not(windows))]
mod backend {
    use super::Range;

    pub struct Backend;

    impl Backend {
        pub fn new() -> Self {
            Self
        }

        pub fn is_available(&self) -> bool {
            false
        }

        pub fn check(&self, _text: &str) -> Vec<Range<usize>> {
            Vec::new()
        }

        pub fn suggestions(&self, _word: &str) -> Vec<String> {
            Vec::new()
        }

        pub fn add_to_dictionary(&self, _word: &str) -> bool {
            false
        }

        pub fn ignore(&self, _word: &str) -> bool {
            false
        }
    }
}
