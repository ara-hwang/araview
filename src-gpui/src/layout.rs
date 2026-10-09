//! 이동 인덱스와 양쪽 보기 화면 배치 계산(SPEC §5.3, §5.4, §6).
//!
//! 양쪽 보기는 앞에서부터 두 장씩 묶되, 단독 페이지(표지, 넓은 페이지, 폴더 안
//! 아카이브)와 그 바로 앞에 남는 페이지는 혼자 한 화면을 쓴다. 표지 단독도 단독
//! 집합에 표지를 넣는 것으로 표현한다.

use std::collections::HashSet;

use araview_core::comic_info::ComicInfo;

use crate::settings::ViewMode;

pub type Solo = HashSet<usize>;

/// 방향 이동의 다음 인덱스. 이동할 수 없으면 `None`(비루프 경계).
pub fn step_index(
    current: usize,
    total: usize,
    step: usize,
    looped: bool,
    forward: bool,
) -> Option<usize> {
    if total <= 1 {
        return None;
    }
    let last = total - 1;
    if !forward {
        if current >= step {
            return Some(current - step);
        }
        return looped.then(|| (current + total - step % total) % total);
    }
    let next = current + step;
    if next <= last {
        return Some(next);
    }
    if looped {
        return Some(next % total);
    }
    // 끝에서 멈춤 모드라도 마지막 장은 볼 수 있게 clamp한다.
    (current != last).then_some(last)
}

/// 오프셋 점프 인덱스. 루프면 wrap, 아니면 clamp.
pub fn offset_index(current: usize, total: usize, offset: isize, looped: bool) -> usize {
    if total == 0 {
        return 0;
    }
    let raw = current as isize + offset;
    if looped {
        raw.rem_euclid(total as isize) as usize
    } else {
        raw.clamp(0, total as isize - 1) as usize
    }
}

/// 화면 시작 인덱스 목록. `anchor`는 항상 화면 시작이 되고, 그 바로 앞에 남는
/// 페이지는 혼자 한 화면이 된다.
pub fn screen_starts(total: usize, solo: &Solo, anchor: Option<usize>) -> Vec<usize> {
    let mut starts = Vec::with_capacity(total / 2 + 1);
    let mut i = 0;
    while i < total {
        starts.push(i);
        let alone =
            solo.contains(&i) || i + 1 >= total || solo.contains(&(i + 1)) || Some(i + 1) == anchor;
        i += if alone { 1 } else { 2 };
    }
    starts
}

fn start_containing(starts: &[usize], index: usize) -> usize {
    let position = starts.partition_point(|start| *start <= index);
    starts[position
        .saturating_sub(1)
        .min(starts.len().saturating_sub(1))]
}

/// `index`를 담는 화면의 시작. 두 화면이 페이지를 겹쳐 보여주지 않게 점프를 스냅한다.
pub fn pair_start(index: usize, total: usize, solo: &Solo, anchor: Option<usize>) -> usize {
    if total == 0 {
        return 0;
    }
    let starts = screen_starts(total, solo, anchor);
    start_containing(&starts, index.min(total - 1))
}

/// 양쪽 보기의 다음/이전 화면 시작. 이동할 수 없으면 `None`.
/// `current`는 지금 화면의 시작이며 배치의 기준(`anchor`)이 된다.
pub fn dual_step(
    current: usize,
    total: usize,
    looped: bool,
    forward: bool,
    solo: &Solo,
) -> Option<usize> {
    if total <= 1 {
        return None;
    }
    let starts = screen_starts(total, solo, Some(current));
    let position = starts.iter().position(|start| *start == current)?;
    let next = if forward {
        position + 1
    } else {
        position.checked_sub(1).unwrap_or(usize::MAX)
    };
    if next < starts.len() {
        return Some(starts[next]);
    }
    if !looped {
        return None;
    }
    Some(if forward {
        starts[0]
    } else {
        starts[starts.len() - 1]
    })
}

/// 화면 시작이 `index`일 때 함께 보이는 인덱스. 단독 화면이면 한 장이다.
/// 단독 페이지가 없는 목록에서 루프가 켜져 있으면 마지막 홀수 장이 첫 장과 짝이 된다.
pub fn screen_indices(index: usize, total: usize, looped: bool, solo: &Solo) -> Vec<usize> {
    if total == 0 {
        return Vec::new();
    }
    let index = index.min(total - 1);
    let starts = screen_starts(total, solo, Some(index));
    let position = starts.iter().position(|start| *start == index).unwrap_or(0);
    let end = starts.get(position + 1).copied().unwrap_or(total);
    let mut indices: Vec<usize> = (index..end.min(index + 2)).collect();
    if indices.len() == 1 && index + 1 == total && looped && total > 1 && solo.is_empty() {
        indices.push(0);
    }
    indices
}

/// 양쪽 보기의 표지 배치. 표지는 `index`부터 `count`장 연속이다.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CoverLayout {
    pub alone: bool,
    pub index: usize,
    pub count: usize,
}

impl CoverLayout {
    /// 지금 단독 표지로 보이는 인덱스.
    pub fn pages(&self) -> std::ops::Range<usize> {
        if self.alone {
            self.index..self.index + self.count
        } else {
            0..0
        }
    }
}

fn is_front_cover(page_type: Option<&str>) -> bool {
    page_type.is_some_and(|kind| kind.trim().eq_ignore_ascii_case("frontcover"))
}

/// `start`부터 끊기지 않고 이어지는 구간.
fn contiguous_run(marked: &[usize], start: usize) -> Vec<usize> {
    let mut run = Vec::new();
    let mut i = start;
    while marked.contains(&i) {
        run.push(i);
        i += 1;
    }
    run
}

/// ComicInfo에 명시된 표지 인덱스. 여러 장이면 가장 앞에서부터 이어지는 구간만
/// 표지로 본다. 목록 범위를 벗어난 값은 버린다.
pub fn explicit_cover_pages(comic: Option<&ComicInfo>, total: usize) -> Vec<usize> {
    let marked: Vec<usize> = comic
        .and_then(|comic| comic.pages.as_ref())
        .into_iter()
        .flatten()
        .filter(|page| is_front_cover(page.page_type.as_deref()))
        .filter_map(|page| usize::try_from(page.image).ok())
        .filter(|index| *index < total)
        .collect();
    match marked.iter().min() {
        Some(first) => contiguous_run(&marked, *first),
        None => Vec::new(),
    }
}

/// 아카이브의 표지 배치. 아카이브 지정이 전역 설정보다 우선한다.
pub fn cover_layout(comic: Option<&ComicInfo>, total: usize, default_alone: bool) -> CoverLayout {
    let fallback = CoverLayout {
        alone: default_alone,
        index: 0,
        count: 1,
    };
    if total == 0 {
        return fallback;
    }
    let covers = explicit_cover_pages(comic, total);
    if let Some(first) = covers.first() {
        return CoverLayout {
            alone: true,
            index: *first,
            count: covers.len(),
        };
    }
    let pages = comic
        .and_then(|comic| comic.pages.as_deref())
        .unwrap_or(&[]);
    // 범위 밖 FrontCover만 있으면 지정이 없는 것으로 본다.
    if pages
        .iter()
        .any(|page| is_front_cover(page.page_type.as_deref()))
    {
        return fallback;
    }
    let first_typed = pages
        .iter()
        .find(|page| page.image == 0)
        .and_then(|page| page.page_type.as_deref())
        .is_some_and(|kind| !kind.trim().is_empty());
    if first_typed {
        // 0번에 다른 Type이 명시돼 있으면 "표지 없음"이다.
        return CoverLayout {
            alone: false,
            index: 0,
            count: 1,
        };
    }
    fallback
}

/// `index` 페이지의 표지 지정을 뒤집은 새 표지 목록. 표지는 연속 구간만 허용한다.
pub fn toggle_cover_page(covers: &[usize], index: usize) -> Vec<usize> {
    if covers.contains(&index) {
        let rest: Vec<usize> = covers.iter().copied().filter(|c| *c != index).collect();
        return match rest.iter().min() {
            Some(first) => contiguous_run(&rest, *first),
            None => Vec::new(),
        };
    }
    let adjacent =
        covers.contains(&(index + 1)) || index.checked_sub(1).is_some_and(|p| covers.contains(&p));
    if adjacent {
        let mut next = covers.to_vec();
        next.push(index);
        next.sort_unstable();
        next
    } else {
        vec![index]
    }
}

/// 아카이브를 열 때 쓸 보기 모드. 자동 양쪽 보기가 꺼져 있거나 웹툰이면 `None`
/// (설정값 그대로)이다. 방향은 ComicInfo > 설정의 양쪽 방향 > 좌→우 순이다.
pub fn comic_view_mode(
    base: ViewMode,
    auto_dual: bool,
    comic: Option<&ComicInfo>,
) -> Option<ViewMode> {
    if !auto_dual || base == ViewMode::Webtoon {
        return None;
    }
    let manga = comic
        .and_then(|comic| comic.manga.as_deref())
        .map(|value| value.trim().to_ascii_lowercase());
    Some(match manga.as_deref() {
        Some("yesandrighttoleft") => ViewMode::RightToLeft,
        Some("no") => ViewMode::LeftToRight,
        _ if base == ViewMode::RightToLeft => ViewMode::RightToLeft,
        _ => ViewMode::LeftToRight,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use araview_core::comic_info::ComicPage;

    fn solo(items: &[usize]) -> Solo {
        items.iter().copied().collect()
    }

    fn comic(pages: &[(u32, Option<&str>)], manga: Option<&str>) -> ComicInfo {
        ComicInfo {
            manga: manga.map(str::to_owned),
            pages: Some(
                pages
                    .iter()
                    .map(|(image, kind)| ComicPage {
                        image: *image,
                        page_type: kind.map(str::to_owned),
                    })
                    .collect(),
            ),
            ..ComicInfo::default()
        }
    }

    #[test]
    fn step_stops_or_wraps_at_the_ends() {
        assert_eq!(step_index(0, 5, 1, false, false), None);
        assert_eq!(step_index(0, 5, 1, true, false), Some(4));
        assert_eq!(step_index(4, 5, 1, false, true), None);
        assert_eq!(step_index(4, 5, 1, true, true), Some(0));
        // 끝에서 멈춤이어도 마지막 장은 clamp로 볼 수 있다.
        assert_eq!(step_index(3, 5, 2, false, true), Some(4));
        assert_eq!(step_index(2, 1, 1, true, true), None);
    }

    #[test]
    fn offset_clamps_or_wraps() {
        assert_eq!(offset_index(2, 20, -10, false), 0);
        assert_eq!(offset_index(2, 20, -10, true), 12);
        assert_eq!(offset_index(15, 20, 10, false), 19);
        assert_eq!(offset_index(15, 20, 10, true), 5);
    }

    #[test]
    fn plain_pairs_and_cover_alone() {
        assert_eq!(screen_starts(5, &solo(&[]), None), [0, 2, 4]);
        assert_eq!(screen_starts(6, &solo(&[0]), None), [0, 1, 3, 5]);
        // 표지가 0번이 아니면 표지 바로 앞에 남는 페이지도 단독이다.
        assert_eq!(screen_starts(8, &solo(&[3]), None), [0, 2, 3, 4, 6]);
    }

    #[test]
    fn wide_page_and_the_page_before_it_stand_alone() {
        assert_eq!(screen_starts(7, &solo(&[0, 4]), None), [0, 1, 3, 4, 5]);
    }

    #[test]
    fn anchor_is_always_a_screen_start() {
        assert_eq!(screen_starts(6, &solo(&[]), Some(3)), [0, 2, 3, 5]);
        assert_eq!(pair_start(3, 6, &solo(&[]), None), 2);
        assert_eq!(pair_start(3, 6, &solo(&[0]), None), 3);
    }

    #[test]
    fn dual_step_moves_by_screen() {
        let cover = solo(&[0]);
        assert_eq!(dual_step(0, 6, false, true, &cover), Some(1));
        assert_eq!(dual_step(1, 6, false, true, &cover), Some(3));
        assert_eq!(dual_step(1, 6, false, false, &cover), Some(0));
        assert_eq!(dual_step(0, 6, false, false, &cover), None);
        assert_eq!(dual_step(0, 6, true, false, &cover), Some(5));
        assert_eq!(dual_step(5, 6, true, true, &cover), Some(0));
    }

    #[test]
    fn screen_indices_show_one_or_two_pages() {
        let cover = solo(&[0]);
        assert_eq!(screen_indices(0, 6, false, &cover), [0]);
        assert_eq!(screen_indices(1, 6, false, &cover), [1, 2]);
        assert_eq!(screen_indices(5, 6, false, &cover), [5]);
        assert_eq!(screen_indices(4, 5, false, &solo(&[])), [4]);
        assert_eq!(screen_indices(4, 5, true, &solo(&[])), [4, 0]);
    }

    #[test]
    fn front_cover_run_wins_over_the_global_default() {
        let info = comic(
            &[
                (2, Some("FrontCover")),
                (3, Some(" frontcover ")),
                (5, Some("FrontCover")),
            ],
            None,
        );
        assert_eq!(explicit_cover_pages(Some(&info), 10), [2, 3]);
        assert_eq!(
            cover_layout(Some(&info), 10, false),
            CoverLayout {
                alone: true,
                index: 2,
                count: 2
            }
        );
    }

    #[test]
    fn typed_first_page_means_no_cover_and_out_of_range_is_ignored() {
        let story = comic(&[(0, Some("Story"))], None);
        assert!(!cover_layout(Some(&story), 4, true).alone);
        let out_of_range = comic(&[(9, Some("FrontCover"))], None);
        assert!(cover_layout(Some(&out_of_range), 4, true).alone);
        assert_eq!(cover_layout(None, 4, false).pages(), 0..0);
    }

    #[test]
    fn toggling_cover_keeps_a_contiguous_run() {
        assert_eq!(toggle_cover_page(&[0], 1), [0, 1]);
        assert_eq!(toggle_cover_page(&[0], 3), [3]);
        assert_eq!(toggle_cover_page(&[0, 1, 2], 1), [0]);
        assert_eq!(toggle_cover_page(&[0], 0), Vec::<usize>::new());
    }

    #[test]
    fn comic_mode_follows_manga_then_setting() {
        let rtl = comic(&[], Some("YesAndRightToLeft"));
        let ltr = comic(&[], Some("No"));
        assert_eq!(
            comic_view_mode(ViewMode::Single, true, Some(&rtl)),
            Some(ViewMode::RightToLeft)
        );
        assert_eq!(
            comic_view_mode(ViewMode::RightToLeft, true, Some(&ltr)),
            Some(ViewMode::LeftToRight)
        );
        assert_eq!(
            comic_view_mode(ViewMode::RightToLeft, true, None),
            Some(ViewMode::RightToLeft)
        );
        assert_eq!(comic_view_mode(ViewMode::Webtoon, true, Some(&rtl)), None);
        assert_eq!(comic_view_mode(ViewMode::Single, false, Some(&rtl)), None);
    }
}
