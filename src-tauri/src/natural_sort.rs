//! 탐색기와 같은 이름순(자연 정렬) 정렬 키.
//!
//! 숫자 구간을 값으로 비교해 `2.jpg`가 `10.jpg`보다 앞에 오고, 대소문자는
//! 무시하며, 기호와 비ASCII 문자는 사용자 로캘의 언어 규칙을 따른다.
//! 비교 함수 대신 `LCMapStringEx`가 만든 바이트 정렬 키를 쓰는 이유는 둘이다:
//! 항목당 한 번만 계산해 `sort_by_cached_key`에 그대로 넣을 수 있고, 바이트
//! 비교는 항상 전순서라 `StrCmpLogicalW`처럼 비교 결과가 어긋나 정렬이
//! panic할 여지가 없다.

/// 경로 하나의 정렬 키. 구분자로 나눈 구성 요소별로 비교하므로 재귀 목록과
/// 아카이브에서 같은 폴더의 항목이 흩어지지 않는다. 대소문자만 다른 이름은
/// 정렬 키가 같아서 원본 문자열로 순서를 확정한다.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct NaturalKey {
    parts: Vec<Vec<u8>>,
    raw: String,
}

pub(crate) fn natural_key(path: &str) -> NaturalKey {
    NaturalKey {
        parts: path.split(['/', '\\']).map(sort_key).collect(),
        raw: path.to_string(),
    }
}

fn fallback_key(part: &str) -> Vec<u8> {
    part.to_lowercase().into_bytes()
}

#[cfg(not(windows))]
fn sort_key(part: &str) -> Vec<u8> {
    fallback_key(part)
}

#[cfg(windows)]
fn sort_key(part: &str) -> Vec<u8> {
    use std::ptr::{null, null_mut};

    let wide: Vec<u16> = part.encode_utf16().collect();
    let len = match i32::try_from(wide.len()) {
        Ok(0) => return Vec::new(),
        Ok(len) => len,
        Err(_) => return fallback_key(part),
    };

    // SAFETY: `wide`는 호출 동안 살아 있고 길이를 `len`으로 명시한다. 출력
    // 버퍼 크기 0은 필요한 바이트 수만 돌려받는 조회 호출이라 쓰기가 없다.
    let needed = unsafe {
        LCMapStringEx(
            null(),
            SORT_KEY_FLAGS,
            wide.as_ptr(),
            len,
            null_mut(),
            0,
            null_mut(),
            null_mut(),
            0,
        )
    };
    let Ok(capacity) = usize::try_from(needed) else {
        return fallback_key(part);
    };
    if capacity == 0 {
        return fallback_key(part);
    }

    let mut key = vec![0u8; capacity];
    // SAFETY: `key`는 `needed` 바이트를 가진 쓰기 가능한 버퍼이고,
    // LCMAP_SORTKEY에서 출력 크기는 바이트 단위다(MSDN).
    let written = unsafe {
        LCMapStringEx(
            null(),
            SORT_KEY_FLAGS,
            wide.as_ptr(),
            len,
            key.as_mut_ptr().cast(),
            needed,
            null_mut(),
            null_mut(),
            0,
        )
    };
    match usize::try_from(written) {
        Ok(written) if written > 0 => {
            key.truncate(written);
            key
        }
        _ => fallback_key(part),
    }
}

#[cfg(windows)]
const LCMAP_SORTKEY: u32 = 0x0000_0400;
#[cfg(windows)]
const SORT_DIGITSASNUMBERS: u32 = 0x0000_0008;
#[cfg(windows)]
const NORM_IGNORECASE: u32 = 0x0000_0001;
#[cfg(windows)]
const SORT_KEY_FLAGS: u32 = LCMAP_SORTKEY | SORT_DIGITSASNUMBERS | NORM_IGNORECASE;

// 시그니처를 바꿀 때는 MSDN의 원본 선언과 대조할 것(`file_assoc.rs` 참고).
//   LCMapStringEx(LPCWSTR, DWORD, LPCWSTR, int, LPWSTR, int,
//                 LPNLSVERSIONINFO, LPVOID, LPARAM) -> int
// 로캘 이름 NULL은 LOCALE_NAME_USER_DEFAULT다.
#[cfg(windows)]
#[link(name = "kernel32")]
extern "system" {
    fn LCMapStringEx(
        lp_locale_name: *const u16,
        dw_map_flags: u32,
        lp_src_str: *const u16,
        cch_src: i32,
        lp_dest_str: *mut u16,
        cch_dest: i32,
        lp_version_information: *mut std::ffi::c_void,
        lp_reserved: *mut std::ffi::c_void,
        sort_handle: isize,
    ) -> i32;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sorted(names: &[&str]) -> Vec<String> {
        let mut v: Vec<String> = names.iter().map(|s| s.to_string()).collect();
        v.sort_by_cached_key(|s| natural_key(s));
        v
    }

    #[cfg(windows)]
    #[test]
    fn digit_runs_compare_by_value() {
        assert_eq!(
            sorted(&["10.jpg", "2.jpg", "1.jpg", "page 100.png", "page 20.png"]),
            ["1.jpg", "2.jpg", "10.jpg", "page 20.png", "page 100.png"]
        );
    }

    #[test]
    fn case_is_ignored() {
        assert_eq!(
            sorted(&["c.jpg", "B.jpg", "a.jpg"]),
            ["a.jpg", "B.jpg", "c.jpg"]
        );
    }

    #[test]
    fn case_only_variants_get_a_stable_order() {
        let a = sorted(&["A.jpg", "a.jpg", "A.jpg"]);
        let b = sorted(&["a.jpg", "A.jpg", "A.jpg"]);
        assert_eq!(a, b);
        assert_eq!(a[0], a[1]);
    }

    #[cfg(windows)]
    #[test]
    fn folders_compare_component_by_component() {
        assert_eq!(
            sorted(&[
                r"C:\a\ch10\1.jpg",
                r"C:\a\ch2\9.jpg",
                "ch10/001.png",
                "ch2/010.png"
            ]),
            [
                r"C:\a\ch2\9.jpg",
                r"C:\a\ch10\1.jpg",
                "ch2/010.png",
                "ch10/001.png"
            ]
        );
    }

    /// 탐색기가 쓰는 `StrCmpLogicalW`와 같은 순서를 내는지 직접 대조한다.
    #[cfg(windows)]
    #[test]
    fn matches_explorer_ordering() {
        #[link(name = "shlwapi")]
        extern "system" {
            fn StrCmpLogicalW(psz1: *const u16, psz2: *const u16) -> i32;
        }

        fn wide(s: &str) -> Vec<u16> {
            s.encode_utf16().chain(std::iter::once(0)).collect()
        }

        let names = [
            "1.jpg",
            "01.jpg",
            "001.jpg",
            "2.jpg",
            "10.jpg",
            "100.jpg",
            "a1.png",
            "a_1.png",
            "a-1.png",
            "a 1.png",
            "a(1).png",
            "a10.png",
            "A2.png",
            "b.png",
            "_cover.jpg",
            "[01] cover.jpg",
            "img12b3.png",
            "img12b20.png",
            "img2b3.png",
            "가10.jpg",
            "가2.jpg",
            "나.jpg",
            "ß.png",
            "z.png",
            "1.2.3.png",
            "1.10.3.png",
        ];

        for a in names {
            for b in names {
                let (wa, wb) = (wide(a), wide(b));
                // SAFETY: 두 버퍼 모두 NUL로 끝나고 호출 동안 살아 있다.
                let explorer = unsafe { StrCmpLogicalW(wa.as_ptr(), wb.as_ptr()) }.signum();
                let ours = natural_key(a).parts.cmp(&natural_key(b).parts) as i32;
                assert_eq!(ours, explorer, "{a:?} vs {b:?}");
            }
        }
    }
}
