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
///
/// 구성 요소 키를 0x00으로 끝내 한 버퍼에 이어 붙인다. 키 안에는 0x00이 없어
/// 이어 붙인 바이트 비교가 구성 요소별 비교와 같은 순서를 낸다.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct NaturalKey {
    key: Vec<u8>,
    raw: String,
}

pub(crate) fn natural_key(path: &str) -> NaturalKey {
    let mut key = Vec::with_capacity(path.len() * 4 + 8);
    let mut wide = Vec::new();
    for part in path.split(['/', '\\']) {
        push_sort_key(part, &mut wide, &mut key);
    }
    NaturalKey {
        key,
        raw: path.to_string(),
    }
}

/// 이 개수부터 키 계산과 정렬을 병렬로 돌린다. 작은 목록은 스레드 풀을
/// 깨우는 비용이 더 크다.
const PARALLEL_SORT_MIN: usize = 512;

/// `path_of`가 돌려주는 경로의 자연 정렬 순서로 `items`를 정렬한다.
/// 모든 항목이 공유하는 상위 폴더는 순서에 영향이 없으므로 키에서 뺀다.
pub(crate) fn sort_by_natural_path<T, F>(items: &mut [T], path_of: F)
where
    T: Send + Sync,
    F: for<'a> Fn(&'a T) -> &'a str + Sync,
{
    use rayon::prelude::*;

    if items.len() < 2 {
        return;
    }
    let skip = common_dir_prefix_len(items.iter().map(&path_of));
    let key_of = |(index, item): (usize, &T)| (natural_key(&path_of(item)[skip..]), index);
    let mut keyed: Vec<(NaturalKey, usize)> = if items.len() >= PARALLEL_SORT_MIN {
        let mut keyed: Vec<_> = items.par_iter().enumerate().map(key_of).collect();
        keyed.par_sort_unstable();
        keyed
    } else {
        let mut keyed: Vec<_> = items.iter().enumerate().map(key_of).collect();
        keyed.sort_unstable();
        keyed
    };
    // 정렬된 인덱스대로 제자리 치환한다(`sort_by_cached_key`와 같은 방식).
    for i in 0..keyed.len() {
        let mut index = keyed[i].1;
        while index < i {
            index = keyed[index].1;
        }
        keyed[i].1 = index;
        items.swap(i, index);
    }
}

/// 모든 경로가 공유하는 접두사 중 마지막 구분자까지의 바이트 길이.
fn common_dir_prefix_len<'a>(mut paths: impl Iterator<Item = &'a str>) -> usize {
    let Some(first) = paths.next() else {
        return 0;
    };
    let first = first.as_bytes();
    let mut common = first.len();
    for path in paths {
        common = first[..common]
            .iter()
            .zip(path.as_bytes())
            .take_while(|(a, b)| a == b)
            .count();
        if common == 0 {
            return 0;
        }
    }
    // 구분자는 ASCII라 그 다음 바이트는 항상 문자 경계다.
    first[..common]
        .iter()
        .rposition(|b| matches!(b, b'/' | b'\\'))
        .map_or(0, |pos| pos + 1)
}

/// 구성 요소 하나의 키를 0x00 종결자와 함께 `out`에 덧붙인다.
fn push_fallback_key(part: &str, out: &mut Vec<u8>) {
    out.extend(part.to_lowercase().bytes().filter(|b| *b != 0));
    out.push(0);
}

#[cfg(not(windows))]
fn push_sort_key(part: &str, _wide: &mut Vec<u16>, out: &mut Vec<u8>) {
    push_fallback_key(part, out);
}

#[cfg(windows)]
fn push_sort_key(part: &str, wide: &mut Vec<u16>, out: &mut Vec<u8>) {
    use std::ptr::{null, null_mut};

    wide.clear();
    wide.extend(part.encode_utf16());
    let len = match i32::try_from(wide.len()) {
        Ok(0) => return out.push(0),
        Ok(len) => len,
        Err(_) => return push_fallback_key(part, out),
    };

    let start = out.len();
    // 대부분의 이름은 이 추정 크기에 들어가 호출 1회로 끝난다. 모자라면
    // 필요한 크기를 조회해 한 번 더 부른다.
    let mut capacity = wide.len() * 6 + 16;
    for _ in 0..2 {
        let Ok(cb_dest) = i32::try_from(capacity) else {
            break;
        };
        out.resize(start + capacity, 0);
        // SAFETY: `wide`는 호출 동안 살아 있고 길이를 `len`으로 명시한다.
        // `out[start..]`는 `capacity` 바이트의 쓰기 가능한 버퍼이고,
        // LCMAP_SORTKEY에서 출력 크기는 바이트 단위다(MSDN).
        let written = unsafe {
            LCMapStringEx(
                null(),
                SORT_KEY_FLAGS,
                wide.as_ptr(),
                len,
                out[start..].as_mut_ptr().cast(),
                cb_dest,
                null_mut(),
                null_mut(),
                0,
            )
        };
        if let Some(written) = usize::try_from(written).ok().filter(|n| *n > 0) {
            out.truncate(start + written);
            // 키는 0x00으로 끝난다(MSDN). 어긋난 경우에도 종결자를 보장한다.
            if out.last() != Some(&0) {
                out.push(0);
            }
            return;
        }
        // SAFETY: 출력 버퍼 크기 0은 필요한 바이트 수만 돌려받는 조회 호출이라
        // 쓰기가 없다.
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
        match usize::try_from(needed) {
            Ok(needed) if needed > capacity => capacity = needed,
            _ => break,
        }
    }
    out.truncate(start);
    push_fallback_key(part, out);
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
                let ours = natural_key(a).key.cmp(&natural_key(b).key) as i32;
                assert_eq!(ours, explorer, "{a:?} vs {b:?}");
            }
        }
    }

    #[test]
    fn slice_sort_matches_per_item_keys() {
        // 병렬 경로(512개 이상)와 공통 폴더 제외가 항목별 키 정렬과 같은 순서를 낸다.
        let mut paths: Vec<String> = (0..1500)
            .map(|i| format!(r"C:\pics\set{}\Img{}.png", i % 7, (i * 37) % 1500))
            .collect();
        paths.push(r"C:\pics\cover.png".to_string());
        let mut expected = paths.clone();
        expected.sort_by_cached_key(|s| natural_key(s));
        sort_by_natural_path(&mut paths, |s| s.as_str());
        assert_eq!(paths, expected);

        let mut small = vec![
            "b/2.png".to_string(),
            "b/10.png".to_string(),
            "a.png".to_string(),
        ];
        let mut expected = small.clone();
        expected.sort_by_cached_key(|s| natural_key(s));
        sort_by_natural_path(&mut small, |s| s.as_str());
        assert_eq!(small, expected);
    }

    #[test]
    fn common_prefix_stops_at_a_separator() {
        let len = |paths: &[&str]| common_dir_prefix_len(paths.iter().copied());
        assert_eq!(len(&[r"C:\a\bc\1.png", r"C:\a\bd\2.png"]), r"C:\a\".len());
        assert_eq!(len(&["ch1/001.png", "ch1/002.png"]), "ch1/".len());
        assert_eq!(len(&["001.png", "002.png"]), 0);
        assert_eq!(len(&["only/one.png"]), "only/".len());
        assert_eq!(len(&[]), 0);
    }
}
