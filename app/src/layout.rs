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

    fn pages(entries: &[(u32, Option<&str>)]) -> ComicInfo {
        ComicInfo {
            pages: Some(
                entries
                    .iter()
                    .map(|(image, page_type)| ComicPage {
                        image: *image,
                        page_type: page_type.map(str::to_owned),
                    })
                    .collect(),
            ),
            ..Default::default()
        }
    }

    #[test]
    fn step_index_moves_and_stops_at_the_bounds() {
        assert_eq!(step_index(2, 5, 1, false, true), Some(3));
        assert_eq!(step_index(0, 5, 1, false, false), None);
        assert_eq!(step_index(1, 5, 2, false, false), None);
        assert_eq!(step_index(0, 5, 2, true, false), Some(3));
        assert_eq!(step_index(3, 5, 2, true, true), Some(0));
        // 끝에서 멈춤 모드: 마지막 장 앞에서 크게 건너뛰면 마지막 장에 붙고,
        // 마지막 장에서는 더 나아가지 않는다.
        assert_eq!(step_index(2, 5, 3, false, true), Some(4));
        assert_eq!(step_index(4, 5, 1, false, true), None);
        assert_eq!(step_index(0, 0, 1, true, true), None);
        assert_eq!(step_index(0, 1, 1, true, true), None);
    }

    #[test]
    fn offset_index_wraps_or_clamps() {
        assert_eq!(offset_index(1, 5, -3, true), 3);
        assert_eq!(offset_index(1, 5, -3, false), 0);
        assert_eq!(offset_index(3, 5, 3, true), 1);
        assert_eq!(offset_index(3, 5, 3, false), 4);
        assert_eq!(offset_index(0, 0, 1, true), 0);
    }

    #[test]
    fn screen_starts_pair_pages_two_by_two() {
        assert_eq!(screen_starts(5, &Solo::new(), None), vec![0, 2, 4]);
        assert_eq!(screen_starts(6, &Solo::new(), None), vec![0, 2, 4]);
        // 단독 페이지는 자기 화면을 갖고, 다음 쌍은 그 뒤에서 시작한다.
        let solo: Solo = [0].into_iter().collect();
        assert_eq!(screen_starts(5, &solo, None), vec![0, 1, 3]);
    }

    #[test]
    fn screen_starts_keep_the_anchor_as_a_screen_start() {
        // anchor 바로 앞 페이지는 짝을 잃고 혼자 남는다.
        assert_eq!(screen_starts(5, &Solo::new(), Some(1)), vec![0, 1, 3]);
    }

    #[test]
    fn pair_start_snaps_to_the_screen_that_shows_the_page() {
        let solo = Solo::new();
        assert_eq!(pair_start(3, 6, &solo, None), 2);
        assert_eq!(pair_start(5, 6, &solo, None), 4);
        assert_eq!(pair_start(0, 6, &solo, None), 0);
        // 범위를 벗어난 인덱스는 마지막 페이지로 취급한다.
        assert_eq!(pair_start(99, 6, &solo, None), 4);
        assert_eq!(pair_start(0, 0, &solo, None), 0);
    }

    #[test]
    fn dual_step_walks_screens_and_wraps_only_when_looped() {
        let solo = Solo::new();
        // total 3, anchor 0 → [0, 2]
        assert_eq!(dual_step(0, 3, false, true, &solo), Some(2));
        assert_eq!(dual_step(0, 3, false, false, &solo), None);
        assert_eq!(dual_step(0, 3, true, false, &solo), Some(2));
        assert_eq!(dual_step(2, 3, false, true, &solo), None);
        assert_eq!(dual_step(2, 3, true, true, &solo), Some(0));
        assert_eq!(dual_step(0, 1, true, true, &solo), None);
    }

    #[test]
    fn screen_indices_include_the_paired_page() {
        let solo = Solo::new();
        assert_eq!(screen_indices(0, 5, false, &solo), vec![0, 1]);
        assert_eq!(screen_indices(2, 5, false, &solo), vec![2, 3]);
        assert_eq!(screen_indices(4, 5, false, &solo), vec![4]);
        // 루프가 켜지면 마지막 홀수 장이 첫 장과 짝이 된다.
        assert_eq!(screen_indices(4, 5, true, &solo), vec![4, 0]);
        assert_eq!(screen_indices(0, 0, true, &solo), Vec::<usize>::new());
    }

    #[test]
    fn explicit_cover_pages_keep_only_the_leading_run() {
        let comic = pages(&[
            (0, Some("FrontCover")),
            (1, Some("frontcover")),
            (2, Some("Story")),
            (3, Some("FrontCover")),
            (99, Some("FrontCover")),
        ]);
        assert_eq!(explicit_cover_pages(Some(&comic), 5), vec![0, 1]);
        assert_eq!(explicit_cover_pages(None, 5), Vec::<usize>::new());
        assert_eq!(
            explicit_cover_pages(Some(&ComicInfo::default()), 5),
            Vec::<usize>::new()
        );
    }

    #[test]
    fn cover_layout_prefers_archive_covers_over_the_default() {
        assert_eq!(
            cover_layout(None, 5, true),
            CoverLayout {
                alone: true,
                index: 0,
                count: 1
            }
        );
        assert_eq!(
            cover_layout(None, 0, false),
            CoverLayout {
                alone: false,
                index: 0,
                count: 1
            }
        );

        let comic = pages(&[(0, Some("Story")), (1, Some("FrontCover"))]);
        assert_eq!(
            cover_layout(Some(&comic), 5, true),
            CoverLayout {
                alone: true,
                index: 1,
                count: 1
            }
        );

        // 0번에 다른 Type이 명시돼 있으면 표지 없음이다.
        let no_cover = pages(&[(0, Some("Story"))]);
        assert_eq!(
            cover_layout(Some(&no_cover), 5, true),
            CoverLayout {
                alone: false,
                index: 0,
                count: 1
            }
        );

        // 범위 밖 FrontCover만 있으면 지정이 없는 것으로 본다.
        let out_of_range = pages(&[(99, Some("FrontCover"))]);
        assert!(cover_layout(Some(&out_of_range), 5, true).alone);
    }

    #[test]
    fn cover_pages_render_only_while_alone() {
        let layout = CoverLayout {
            alone: true,
            index: 2,
            count: 2,
        };
        assert_eq!(layout.pages().collect::<Vec<_>>(), vec![2, 3]);
        let paired = CoverLayout {
            alone: false,
            index: 2,
            count: 2,
        };
        assert!(paired.pages().next().is_none());
    }

    #[test]
    fn toggle_cover_page_keeps_the_run_contiguous() {
        assert_eq!(toggle_cover_page(&[], 3), vec![3]);
        assert_eq!(toggle_cover_page(&[3], 4), vec![3, 4]);
        assert_eq!(toggle_cover_page(&[3, 4], 5), vec![3, 4, 5]);
        // 떨어진 페이지를 켜면 기존 구간을 대체한다.
        assert_eq!(toggle_cover_page(&[3, 4], 0), vec![0]);
        // 구간 가운데를 끄면 남은 구간만 유지한다.
        assert_eq!(toggle_cover_page(&[3, 4, 5], 4), vec![3]);
        assert_eq!(toggle_cover_page(&[3], 3), Vec::<usize>::new());
    }

    #[test]
    fn screen_layout_partitions_every_small_solo_and_anchor_combination() {
        // 모든 단독 페이지 조합에서 페이지가 누락되거나 겹치지 않아야 한다.
        for total in 0..=8 {
            for mask in 0..(1usize << total) {
                let solo: Solo = (0..total)
                    .filter(|index| mask & (1 << index) != 0)
                    .collect();
                for anchor in std::iter::once(None).chain((0..total).map(Some)) {
                    let starts = screen_starts(total, &solo, anchor);
                    if let Some(anchor) = anchor {
                        assert!(starts.contains(&anchor));
                    }
                    let mut shown = Vec::new();
                    for (position, &start) in starts.iter().enumerate() {
                        let end = starts.get(position + 1).copied().unwrap_or(total);
                        let indices: Vec<_> = (start..end).collect();
                        // screen_indices는 현재 화면을 새 anchor로 삼는다.
                        // 기존 anchor 바로 앞의 단독 화면은 새 anchor에서 다시 짝이 될 수 있다.
                        if anchor.is_none() || anchor == Some(start) {
                            assert_eq!(screen_indices(start, total, false, &solo), indices);
                        }
                        assert!((1..=2).contains(&indices.len()));
                        if indices.len() == 2 {
                            assert!(indices.iter().all(|index| !solo.contains(index)));
                        }
                        for &index in &indices {
                            assert_eq!(pair_start(index, total, &solo, anchor), start);
                        }
                        shown.extend(indices);
                    }
                    assert_eq!(shown, (0..total).collect::<Vec<_>>());
                }
            }
        }
    }

    #[test]
    fn dual_step_keeps_solo_pages_and_their_unpaired_neighbors_reachable() {
        let solo: Solo = [2, 3, 6].into_iter().collect();
        let starts = [0, 2, 3, 4, 6, 7];
        assert_eq!(screen_starts(9, &solo, None), starts);
        for (position, &current) in starts.iter().enumerate() {
            assert_eq!(
                dual_step(current, 9, false, true, &solo),
                starts.get(position + 1).copied()
            );
            assert_eq!(
                dual_step(current, 9, false, false, &solo),
                position.checked_sub(1).map(|previous| starts[previous])
            );
        }
        assert_eq!(dual_step(7, 9, true, true, &solo), Some(0));
        assert_eq!(dual_step(0, 9, true, false, &solo), Some(7));
        assert_eq!(dual_step(9, 9, true, true, &solo), None);
        for total in 0..=1 {
            for forward in [false, true] {
                assert_eq!(dual_step(0, total, true, forward, &solo), None);
                assert_eq!(step_index(0, total, 10, true, forward), None);
            }
        }
    }

    #[test]
    fn screen_indices_only_wrap_without_solo_pages() {
        for solo in [Solo::from([0]), Solo::from([4])] {
            assert_eq!(screen_indices(4, 5, true, &solo), vec![4]);
        }
        assert_eq!(screen_indices(99, 5, false, &Solo::new()), vec![4]);
        assert_eq!(screen_indices(0, 1, true, &Solo::new()), vec![0]);
    }

    #[test]
    fn navigation_handles_offsets_larger_than_the_page_count() {
        assert_eq!(step_index(1, 5, 12, true, true), Some(3));
        assert_eq!(step_index(1, 5, 12, true, false), Some(4));
        assert_eq!(step_index(1, 5, 12, false, true), Some(4));
        assert_eq!(step_index(1, 5, 12, false, false), None);
        for (offset, expected) in [(-12, 4), (-10, 1), (0, 1), (10, 1), (12, 3)] {
            assert_eq!(offset_index(1, 5, offset, true), expected);
        }
    }

    #[test]
    fn explicit_cover_pages_normalize_types_and_ignore_order_and_duplicates() {
        let comic = pages(&[
            (4, Some("FrontCover")),
            (2, Some(" FRONTcover ")),
            (1, Some("FrontCover")),
            (2, Some("FrontCover")),
            (0, Some("Story")),
        ]);
        assert_eq!(explicit_cover_pages(Some(&comic), 5), vec![1, 2]);
        assert_eq!(explicit_cover_pages(Some(&comic), 0), Vec::<usize>::new());
        for default_alone in [false, true] {
            assert_eq!(
                cover_layout(Some(&comic), 5, default_alone),
                CoverLayout {
                    alone: true,
                    index: 1,
                    count: 2
                }
            );
            let unspecified = pages(&[(0, Some(" \t "))]);
            assert_eq!(
                cover_layout(Some(&unspecified), 5, default_alone).alone,
                default_alone
            );
        }
        assert_eq!(toggle_cover_page(&[2, 3, 4], 2), vec![3, 4]);
        assert_eq!(toggle_cover_page(&[2, 3, 4], 4), vec![2, 3]);
        assert_eq!(toggle_cover_page(&[2, 3], 1), vec![1, 2, 3]);
    }

    #[test]
    fn comic_view_mode_respects_opt_out_and_direction_precedence_for_every_base() {
        for base in [
            ViewMode::Single,
            ViewMode::LeftToRight,
            ViewMode::RightToLeft,
            ViewMode::Webtoon,
        ] {
            let fallback = if base == ViewMode::RightToLeft {
                ViewMode::RightToLeft
            } else {
                ViewMode::LeftToRight
            };
            for (manga, direction) in [
                ("  yEsAnDrIgHtToLeFt \t", ViewMode::RightToLeft),
                (" NO ", ViewMode::LeftToRight),
                ("Yes", fallback),
                ("Unknown", fallback),
            ] {
                let comic = ComicInfo {
                    manga: Some(manga.into()),
                    ..Default::default()
                };
                assert_eq!(comic_view_mode(base, false, Some(&comic)), None);
                assert_eq!(
                    comic_view_mode(base, true, Some(&comic)),
                    (base != ViewMode::Webtoon).then_some(direction)
                );
            }
        }
    }

    #[test]
    fn comic_view_mode_follows_auto_dual_and_metadata() {
        assert_eq!(comic_view_mode(ViewMode::Single, false, None), None);
        assert_eq!(comic_view_mode(ViewMode::Webtoon, true, None), None);
        assert_eq!(
            comic_view_mode(ViewMode::Single, true, None),
            Some(ViewMode::LeftToRight)
        );

        let rtl = ComicInfo {
            manga: Some("YesAndRightToLeft".into()),
            ..Default::default()
        };
        assert_eq!(
            comic_view_mode(ViewMode::Single, true, Some(&rtl)),
            Some(ViewMode::RightToLeft)
        );

        let ltr = ComicInfo {
            manga: Some("no".into()),
            ..Default::default()
        };
        assert_eq!(
            comic_view_mode(ViewMode::RightToLeft, true, Some(&ltr)),
            Some(ViewMode::LeftToRight)
        );

        // 지정이 없으면 설정의 양쪽 방향을 따른다.
        let unknown = ComicInfo {
            manga: Some("unknown".into()),
            ..Default::default()
        };
        assert_eq!(
            comic_view_mode(ViewMode::RightToLeft, true, Some(&unknown)),
            Some(ViewMode::RightToLeft)
        );
    }
}
