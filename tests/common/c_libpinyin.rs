//! Runtime dlopen wrapper for system libpinyin (libpinyin.so.15 + glib).

#![allow(dead_code)]

use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::ptr;

use libloading::Library;

#[repr(C)]
struct PinyinContext {
    _private: [u8; 0],
}
#[repr(C)]
struct PinyinInstance {
    _private: [u8; 0],
}
#[repr(C)]
struct ChewingKey {
    _private: [u8; 0],
}
#[repr(C)]
struct ChewingKeyRest {
    _private: [u8; 0],
}
#[repr(C)]
struct LookupCandidate {
    _private: [u8; 0],
}

/// `SORT_BY_PHRASE_LENGTH_AND_FREQUENCY` from libpinyin 2.10.
pub const SORT_BY_PHRASE_LENGTH_AND_FREQUENCY: u32 = 0x2 | 0x4 | 0x10;
/// Upstream `DOUBLE_PINYIN_MS`.
pub const DOUBLE_PINYIN_MS: u32 = 2;

const SYSTEM_DATA_DIR: &str = "/usr/lib64/libpinyin/data";

fn system_data_dir() -> String {
    std::env::var("LIBPINYIN_DATA_DIR").unwrap_or_else(|_| SYSTEM_DATA_DIR.to_string())
}

fn open_libpinyin() -> Option<Library> {
    unsafe {
        if let Ok(path) = std::env::var("LIBPINYIN_LIB") {
            return Library::new(&path).ok();
        }
        Library::new("libpinyin.so.15").ok()
    }
}

/// Wrapper around system libpinyin loaded at runtime.
pub struct CLibPinyin {
    _lib: Library,
    _glib: Library,
    context: *mut PinyinContext,
    instance: *mut PinyinInstance,
    fn_set_options: unsafe extern "C" fn(*mut PinyinContext, u32) -> u8,
    fn_set_double_scheme: unsafe extern "C" fn(*mut PinyinContext, u32) -> u8,
    fn_parse_more: unsafe extern "C" fn(*mut PinyinInstance, *const c_char) -> usize,
    fn_parse_more_double: unsafe extern "C" fn(*mut PinyinInstance, *const c_char) -> usize,
    fn_get_key: unsafe extern "C" fn(*mut PinyinInstance, usize, *mut *mut ChewingKey) -> u8,
    fn_get_string:
        unsafe extern "C" fn(*mut PinyinInstance, *mut ChewingKey, *mut *mut c_char) -> u8,
    fn_get_key_rest:
        unsafe extern "C" fn(*mut PinyinInstance, usize, *mut *mut ChewingKeyRest) -> u8,
    fn_get_key_rest_positions:
        unsafe extern "C" fn(*mut PinyinInstance, *mut ChewingKeyRest, *mut u16, *mut u16) -> u8,
    fn_guess_candidates: unsafe extern "C" fn(*mut PinyinInstance, usize, u32) -> u8,
    fn_get_n_candidate: unsafe extern "C" fn(*mut PinyinInstance, *mut u32) -> u8,
    fn_get_candidate:
        unsafe extern "C" fn(*mut PinyinInstance, u32, *mut *mut LookupCandidate) -> u8,
    fn_get_candidate_string:
        unsafe extern "C" fn(*mut PinyinInstance, *mut LookupCandidate, *mut *const c_char) -> u8,
    fn_guess_sentence: unsafe extern "C" fn(*mut PinyinInstance) -> u8,
    fn_get_sentence: unsafe extern "C" fn(*mut PinyinInstance, u32, *mut *mut c_char) -> u8,
    fn_free_instance: unsafe extern "C" fn(*mut PinyinInstance),
    fn_fini: unsafe extern "C" fn(*mut PinyinContext),
    fn_g_free: unsafe extern "C" fn(*mut std::ffi::c_void),
}

impl CLibPinyin {
    /// Load libpinyin and initialize with `user_dir` for writable state.
    pub fn new(user_dir: &str) -> Option<Self> {
        unsafe {
            let lib = open_libpinyin()?;
            let glib = Library::new("libglib-2.0.so.0").ok()?;

            type FnInit = unsafe extern "C" fn(*const c_char, *const c_char) -> *mut PinyinContext;
            type FnAlloc = unsafe extern "C" fn(*mut PinyinContext) -> *mut PinyinInstance;
            type FnSetOptions = unsafe extern "C" fn(*mut PinyinContext, u32) -> u8;
            type FnSetDouble = unsafe extern "C" fn(*mut PinyinContext, u32) -> u8;
            type FnParseMore = unsafe extern "C" fn(*mut PinyinInstance, *const c_char) -> usize;
            type FnGetKey =
                unsafe extern "C" fn(*mut PinyinInstance, usize, *mut *mut ChewingKey) -> u8;
            type FnGetString =
                unsafe extern "C" fn(*mut PinyinInstance, *mut ChewingKey, *mut *mut c_char) -> u8;
            type FnGetKeyRest =
                unsafe extern "C" fn(*mut PinyinInstance, usize, *mut *mut ChewingKeyRest) -> u8;
            type FnGetKeyRestPositions = unsafe extern "C" fn(
                *mut PinyinInstance,
                *mut ChewingKeyRest,
                *mut u16,
                *mut u16,
            ) -> u8;
            type FnGuessCandidates = unsafe extern "C" fn(*mut PinyinInstance, usize, u32) -> u8;
            type FnGetNCandidate = unsafe extern "C" fn(*mut PinyinInstance, *mut u32) -> u8;
            type FnGetCandidate =
                unsafe extern "C" fn(*mut PinyinInstance, u32, *mut *mut LookupCandidate) -> u8;
            type FnGetCandidateString = unsafe extern "C" fn(
                *mut PinyinInstance,
                *mut LookupCandidate,
                *mut *const c_char,
            ) -> u8;
            type FnGuessSentence = unsafe extern "C" fn(*mut PinyinInstance) -> u8;
            type FnGetSentence =
                unsafe extern "C" fn(*mut PinyinInstance, u32, *mut *mut c_char) -> u8;
            type FnFreeInstance = unsafe extern "C" fn(*mut PinyinInstance);
            type FnFini = unsafe extern "C" fn(*mut PinyinContext);
            type FnGFree = unsafe extern "C" fn(*mut std::ffi::c_void);

            let fn_init = *lib.get::<FnInit>(b"pinyin_init").ok()?;
            let fn_alloc = *lib.get::<FnAlloc>(b"pinyin_alloc_instance").ok()?;
            let fn_set_options = *lib.get::<FnSetOptions>(b"pinyin_set_options").ok()?;
            let fn_set_double_scheme = *lib
                .get::<FnSetDouble>(b"pinyin_set_double_pinyin_scheme")
                .ok()?;
            let fn_parse_more = *lib
                .get::<FnParseMore>(b"pinyin_parse_more_full_pinyins")
                .ok()?;
            let fn_parse_more_double = *lib
                .get::<FnParseMore>(b"pinyin_parse_more_double_pinyins")
                .ok()?;
            let fn_get_key = *lib.get::<FnGetKey>(b"pinyin_get_pinyin_key").ok()?;
            let fn_get_string = *lib.get::<FnGetString>(b"pinyin_get_pinyin_string").ok()?;
            let fn_get_key_rest = *lib
                .get::<FnGetKeyRest>(b"pinyin_get_pinyin_key_rest")
                .ok()?;
            let fn_get_key_rest_positions = *lib
                .get::<FnGetKeyRestPositions>(b"pinyin_get_pinyin_key_rest_positions")
                .ok()?;
            let fn_guess_candidates = *lib
                .get::<FnGuessCandidates>(b"pinyin_guess_candidates")
                .ok()?;
            let fn_get_n_candidate = *lib.get::<FnGetNCandidate>(b"pinyin_get_n_candidate").ok()?;
            let fn_get_candidate = *lib.get::<FnGetCandidate>(b"pinyin_get_candidate").ok()?;
            let fn_get_candidate_string = *lib
                .get::<FnGetCandidateString>(b"pinyin_get_candidate_string")
                .ok()?;
            let fn_guess_sentence = *lib.get::<FnGuessSentence>(b"pinyin_guess_sentence").ok()?;
            let fn_get_sentence = *lib.get::<FnGetSentence>(b"pinyin_get_sentence").ok()?;
            let fn_free_instance = *lib.get::<FnFreeInstance>(b"pinyin_free_instance").ok()?;
            let fn_fini = *lib.get::<FnFini>(b"pinyin_fini").ok()?;
            let fn_g_free = *glib.get::<FnGFree>(b"g_free").ok()?;

            let system_dir = CString::new(system_data_dir()).ok()?;
            let user_dir = CString::new(user_dir).ok()?;
            let _ = std::fs::create_dir_all(user_dir.to_str().unwrap_or("/tmp"));

            let context = fn_init(system_dir.as_ptr(), user_dir.as_ptr());
            if context.is_null() {
                return None;
            }
            let instance = fn_alloc(context);
            if instance.is_null() {
                fn_fini(context);
                return None;
            }

            Some(Self {
                _lib: lib,
                _glib: glib,
                context,
                instance,
                fn_set_options,
                fn_set_double_scheme,
                fn_parse_more,
                fn_parse_more_double,
                fn_get_key,
                fn_get_string,
                fn_get_key_rest,
                fn_get_key_rest_positions,
                fn_guess_candidates,
                fn_get_n_candidate,
                fn_get_candidate,
                fn_get_candidate_string,
                fn_guess_sentence,
                fn_get_sentence,
                fn_free_instance,
                fn_fini,
                fn_g_free,
            })
        }
    }

    pub fn set_options(&self, options: u32) {
        unsafe {
            (self.fn_set_options)(self.context, options);
        }
    }

    pub fn set_double_pinyin_microsoft(&self) {
        unsafe {
            (self.fn_set_double_scheme)(self.context, DOUBLE_PINYIN_MS);
        }
    }

    fn collect_syllables(&self, parsed_len: usize, raw: bool) -> Vec<String> {
        unsafe {
            let mut syllables = Vec::new();
            let mut offset: usize = 0;
            while offset < parsed_len {
                let mut key: *mut ChewingKey = ptr::null_mut();
                let ok = (self.fn_get_key)(self.instance, offset, &mut key);
                if ok == 0 || key.is_null() {
                    offset += 1;
                    continue;
                }
                let mut utf8_str: *mut c_char = ptr::null_mut();
                let ok = (self.fn_get_string)(self.instance, key, &mut utf8_str);
                if ok != 0 && !utf8_str.is_null() {
                    let s = CStr::from_ptr(utf8_str).to_string_lossy().into_owned();
                    syllables.push(if raw {
                        s
                    } else {
                        s.trim_end_matches(|c: char| c.is_ascii_digit())
                            .to_lowercase()
                    });
                    (self.fn_g_free)(utf8_str as *mut std::ffi::c_void);
                }
                let mut key_rest: *mut ChewingKeyRest = ptr::null_mut();
                let ok = (self.fn_get_key_rest)(self.instance, offset, &mut key_rest);
                if ok != 0 && !key_rest.is_null() {
                    let mut begin: u16 = 0;
                    let mut end: u16 = 0;
                    (self.fn_get_key_rest_positions)(self.instance, key_rest, &mut begin, &mut end);
                    offset = if end as usize > offset {
                        end as usize
                    } else {
                        offset + 1
                    };
                } else {
                    offset += 1;
                }
            }
            syllables
        }
    }

    /// Parse full pinyin; returns `(bytes parsed, syllable strings from C)`.
    pub fn parse(&self, input: &str) -> (usize, Vec<String>) {
        let c_input = CString::new(input).unwrap();
        unsafe {
            let parsed_len = (self.fn_parse_more)(self.instance, c_input.as_ptr());
            (parsed_len, self.collect_syllables(parsed_len, true))
        }
    }

    /// Normalized syllable texts (no tone digits, lowercase).
    pub fn parse_normalized(&self, input: &str) -> Vec<String> {
        let c_input = CString::new(input).unwrap();
        unsafe {
            let parsed_len = (self.fn_parse_more)(self.instance, c_input.as_ptr());
            self.collect_syllables(parsed_len, false)
        }
    }

    pub fn parse_double(&self, input: &str) -> Vec<String> {
        let c_input = CString::new(input).unwrap();
        unsafe {
            let parsed_len = (self.fn_parse_more_double)(self.instance, c_input.as_ptr());
            self.collect_syllables(parsed_len, false)
        }
    }

    pub fn candidates(&self, input: &str, limit: usize) -> Vec<String> {
        let c_input = CString::new(input).unwrap();
        unsafe {
            let parsed_len = (self.fn_parse_more)(self.instance, c_input.as_ptr());
            if parsed_len == 0 {
                return Vec::new();
            }
            if (self.fn_guess_candidates)(self.instance, 0, SORT_BY_PHRASE_LENGTH_AND_FREQUENCY)
                == 0
            {
                return Vec::new();
            }
            let mut num: u32 = 0;
            if (self.fn_get_n_candidate)(self.instance, &mut num) == 0 || num == 0 {
                return Vec::new();
            }
            let take = (num as usize).min(limit);
            let mut out = Vec::with_capacity(take);
            for i in 0..take {
                let mut cand: *mut LookupCandidate = ptr::null_mut();
                if (self.fn_get_candidate)(self.instance, i as u32, &mut cand) == 0
                    || cand.is_null()
                {
                    break;
                }
                let mut utf8: *const c_char = ptr::null();
                if (self.fn_get_candidate_string)(self.instance, cand, &mut utf8) != 0
                    && !utf8.is_null()
                {
                    out.push(CStr::from_ptr(utf8).to_string_lossy().into_owned());
                }
            }
            out
        }
    }

    /// Count top candidates fetched (warms candidate path for benchmarks).
    pub fn candidates_count(&self, input: &str, limit: usize) -> usize {
        self.candidates(input, limit).len()
    }

    /// C `pinyin_guess_sentence` + `pinyin_get_sentence` (index 0).
    pub fn sentence(&self, input: &str) -> Option<String> {
        let c_input = CString::new(input).unwrap();
        unsafe {
            let parsed_len = (self.fn_parse_more)(self.instance, c_input.as_ptr());
            if parsed_len == 0 {
                return None;
            }
            if (self.fn_guess_sentence)(self.instance) == 0 {
                return None;
            }
            let mut utf8: *mut c_char = ptr::null_mut();
            if (self.fn_get_sentence)(self.instance, 0, &mut utf8) == 0 || utf8.is_null() {
                return None;
            }
            let s = CStr::from_ptr(utf8).to_string_lossy().into_owned();
            (self.fn_g_free)(utf8 as *mut _);
            Some(s)
        }
    }

    /// Whether sentence generation succeeded (benchmark helper).
    pub fn sentence_ok(&self, input: &str) -> bool {
        self.sentence(input).is_some()
    }
}

impl Drop for CLibPinyin {
    fn drop(&mut self) {
        unsafe {
            (self.fn_free_instance)(self.instance);
            (self.fn_fini)(self.context);
        }
    }
}
