//! CBZ/ZIP 안의 `ComicInfo.xml` 메타데이터 읽기와 표지 지정(`FrontCover`) 쓰기.
//!
//! ComicRack/Komga/Kavita가 쓰는 스키마를 그대로 따른다. 탐색은 엔트리
//! basename 대소문자 무시 일치이고, 루트(`ComicInfo.xml`)를 우선한 뒤
//! 없으면 첫 번째 중첩 경로를 쓴다. XML 부재는 에러가 아니라 `Ok(None)`이다.
//! CBZ/ZIP만 지원하고 그 밖의 확장자는 `Ok(None)`이다.
//!
//! 쓰기는 `Pages`의 `Type`만 바꾼다. 나머지 요소는 이벤트 단위로 그대로 옮겨
//! 이 모듈이 모르는 필드도 보존한다.

use std::collections::BTreeSet;
use std::path::Path;

use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use serde::{Deserialize, Serialize};

use crate::app_error::AppError;

/// ComicInfo.xml 읽기 상한 (zipbomb 가드). 실제 메타데이터는 수 KB 수준이다.
pub const MAX_COMICINFO_BYTES: u64 = 1024 * 1024;

/// `Pages` 배열 상한. 비정상적으로 긴 목록은 잘라서 표시용으로만 쓴다.
const MAX_COMIC_PAGES: usize = 1000;

/// 프론트와 공유하는 만화 메타데이터 (snake_case 직렬화).
#[derive(Serialize, Debug, Clone, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub struct ComicInfo {
    pub title: Option<String>,
    pub series: Option<String>,
    /// "1.5" 같은 값을 보존하기 위해 문자열로 둔다.
    pub number: Option<String>,
    pub count: Option<i32>,
    pub volume: Option<i32>,
    pub summary: Option<String>,
    pub writer: Option<String>,
    pub penciller: Option<String>,
    pub inker: Option<String>,
    pub colorist: Option<String>,
    pub letterer: Option<String>,
    pub cover_artist: Option<String>,
    pub editor: Option<String>,
    pub year: Option<i32>,
    pub month: Option<i32>,
    pub day: Option<i32>,
    pub publisher: Option<String>,
    pub genre: Option<String>,
    pub tags: Option<String>,
    pub language_iso: Option<String>,
    pub page_count: Option<i32>,
    pub age_rating: Option<String>,
    /// 소수 등급(`8.5`)을 보존하기 위해 문자열로 둔다.
    pub community_rating: Option<String>,
    /// `Unknown` / `No` / `Yes` / `YesAndRightToLeft` 원문. 읽기 방향 판정용.
    pub manga: Option<String>,
    /// 표지 판정용 페이지 목록. `image`는 ComicRack 스키마대로 0 기반이다.
    pub pages: Option<Vec<ComicPage>>,
}

#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct ComicPage {
    pub image: u32,
    /// `FrontCover` 등. 없으면 `None`.
    pub page_type: Option<String>,
}

/// XML 원문 역직렬화용 (PascalCase). 숫자 필드도 문자열로 받아
/// 값 하나가 이상해도 전체 파싱이 실패하지 않게 한다.
#[derive(Deserialize, Debug, Default)]
struct RawComicInfo {
    #[serde(rename = "Title")]
    title: Option<String>,
    #[serde(rename = "Series")]
    series: Option<String>,
    #[serde(rename = "Number")]
    number: Option<String>,
    #[serde(rename = "Count")]
    count: Option<String>,
    #[serde(rename = "Volume")]
    volume: Option<String>,
    #[serde(rename = "Summary")]
    summary: Option<String>,
    #[serde(rename = "Writer")]
    writer: Option<String>,
    #[serde(rename = "Penciller")]
    penciller: Option<String>,
    #[serde(rename = "Inker")]
    inker: Option<String>,
    #[serde(rename = "Colorist")]
    colorist: Option<String>,
    #[serde(rename = "Letterer")]
    letterer: Option<String>,
    #[serde(rename = "CoverArtist")]
    cover_artist: Option<String>,
    #[serde(rename = "Editor")]
    editor: Option<String>,
    #[serde(rename = "Year")]
    year: Option<String>,
    #[serde(rename = "Month")]
    month: Option<String>,
    #[serde(rename = "Day")]
    day: Option<String>,
    #[serde(rename = "Publisher")]
    publisher: Option<String>,
    #[serde(rename = "Genre")]
    genre: Option<String>,
    #[serde(rename = "Tags")]
    tags: Option<String>,
    #[serde(rename = "LanguageISO")]
    language_iso: Option<String>,
    #[serde(rename = "PageCount")]
    page_count: Option<String>,
    #[serde(rename = "AgeRating")]
    age_rating: Option<String>,
    #[serde(rename = "CommunityRating")]
    community_rating: Option<String>,
    #[serde(rename = "Manga")]
    manga: Option<String>,
    #[serde(rename = "Pages")]
    pages: Option<RawPages>,
}

#[derive(Deserialize, Debug, Default)]
struct RawPages {
    #[serde(rename = "Page", default)]
    page: Vec<RawPage>,
}

#[derive(Deserialize, Debug, Default)]
struct RawPage {
    #[serde(rename = "@Image")]
    image: Option<u32>,
    #[serde(rename = "@Type")]
    page_type: Option<String>,
}

/// CBZ/ZIP에서 ComicInfo.xml을 찾아 파싱한다.
/// 지원 확장자가 아니거나 XML이 없으면 `Ok(None)`, 깨진 XML은 `Corrupt`다.
pub fn read_comic_info(archive_path: &Path) -> Result<Option<ComicInfo>, AppError> {
    if !crate::image::is_archive_file(archive_path) {
        return Ok(None);
    }

    // 목록은 인덱스 캐시로 조회한다. 아카이브 오픈 흐름에서 `get_archive_images`
    // 가 먼저 캐시를 채우므로 여기서는 두 번째 전체 스캔이 아니라 히트다.
    let entries = crate::archive_index::get_archive_entries(archive_path)?;
    let Some(entry_name) = find_comic_info_in_entries(&entries.all) else {
        return Ok(None);
    };

    let bytes =
        crate::archive::read_archive_entry_bounded(archive_path, &entry_name, MAX_COMICINFO_BYTES)?;

    Ok(Some(parse_comic_info(&bytes)?))
}

/// 캐시된 전체 엔트리 목록에서 ComicInfo.xml 후보를 루트 우선으로 찾는다.
/// 챕터별로 여러 개가 들어 있으면 루트 다음 첫 중첩 하나만 쓴다.
fn find_comic_info_in_entries(entries: &[String]) -> Option<String> {
    let mut root: Option<&String> = None;
    let mut nested: Option<&String> = None;
    for name in entries {
        let base = Path::new(name)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        if !base.eq_ignore_ascii_case("comicinfo.xml") {
            continue;
        }
        let is_root = !name.contains('/') && !name.contains('\\');
        if is_root {
            if root.is_none() {
                root = Some(name);
            }
        } else if nested.is_none() {
            nested = Some(name);
        }
    }
    root.or(nested).cloned()
}

/// 바이트 → UTF-8 문자열. UTF-16 BOM이면 std만으로 디코딩한다.
fn decode_xml(bytes: &[u8]) -> Result<String, AppError> {
    if bytes.starts_with(&[0xFF, 0xFE]) {
        return decode_utf16(&bytes[2..], true);
    }
    if bytes.starts_with(&[0xFE, 0xFF]) {
        return decode_utf16(&bytes[2..], false);
    }
    let body = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    String::from_utf8(body.to_vec())
        .map_err(|_| AppError::corrupt("ComicInfo.xml is not valid UTF-8"))
}

fn decode_utf16(bytes: &[u8], little_endian: bool) -> Result<String, AppError> {
    if !bytes.len().is_multiple_of(2) {
        return Err(AppError::corrupt(
            "ComicInfo.xml has a truncated UTF-16 sequence",
        ));
    }
    let units: Vec<u16> = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| {
            if little_endian {
                u16::from_le_bytes(*pair)
            } else {
                u16::from_be_bytes(*pair)
            }
        })
        .collect();
    String::from_utf16(&units).map_err(|_| AppError::corrupt("ComicInfo.xml is not valid UTF-16"))
}

/// XML 본문을 파싱하고 표시용으로 정규화한다 (trim, 빈 값 → None).
pub fn parse_comic_info(bytes: &[u8]) -> Result<ComicInfo, AppError> {
    let text = decode_xml(bytes)?;
    let raw: RawComicInfo = quick_xml::de::from_str(&text)
        .map_err(|e| AppError::corrupt(format!("Failed to parse ComicInfo.xml: {e}")))?;

    Ok(ComicInfo {
        title: normalize(raw.title),
        series: normalize(raw.series),
        number: normalize(raw.number),
        count: normalize_number(raw.count),
        volume: normalize_number(raw.volume),
        summary: normalize(raw.summary),
        writer: normalize(raw.writer),
        penciller: normalize(raw.penciller),
        inker: normalize(raw.inker),
        colorist: normalize(raw.colorist),
        letterer: normalize(raw.letterer),
        cover_artist: normalize(raw.cover_artist),
        editor: normalize(raw.editor),
        year: normalize_number(raw.year),
        month: normalize_number(raw.month),
        day: normalize_number(raw.day),
        publisher: normalize(raw.publisher),
        genre: normalize(raw.genre),
        tags: normalize(raw.tags),
        language_iso: normalize(raw.language_iso),
        page_count: normalize_number(raw.page_count),
        age_rating: normalize(raw.age_rating),
        community_rating: normalize(raw.community_rating),
        manga: normalize(raw.manga),
        pages: normalize_pages(raw.pages),
    })
}

fn normalize(value: Option<String>) -> Option<String> {
    value
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

/// 숫자 필드는 값이 이상해도 전체를 실패시키지 않고 `None`으로 둔다.
fn normalize_number(value: Option<String>) -> Option<i32> {
    normalize(value).and_then(|v| v.parse::<i32>().ok())
}

fn normalize_pages(pages: Option<RawPages>) -> Option<Vec<ComicPage>> {
    let entries = pages?.page;
    if entries.is_empty() {
        return None;
    }
    let mut normalized: Vec<ComicPage> = entries
        .into_iter()
        .filter_map(|page| {
            // Image 속성이 없으면 페이지 순서를 알 수 없어 버린다.
            page.image.map(|image| ComicPage {
                image,
                page_type: normalize(page.page_type),
            })
        })
        .collect();
    normalized.sort_by_key(|page| page.image);
    normalized.truncate(MAX_COMIC_PAGES);
    if normalized.is_empty() {
        return None;
    }
    Some(normalized)
}

const FRONT_COVER: &str = "FrontCover";
const STORY: &str = "Story";

/// 표지 페이지 집합을 `ComicInfo.xml`에 쓰고 아카이브를 교체한다.
/// `covers`의 페이지는 `FrontCover`가 되고, 그 밖의 기존 `FrontCover`는 `Story`로
/// 바뀐다. `covers`가 비면 0번을 `Story`로 남겨 "표지 없음"을 명시한다.
/// XML이 없으면 루트에 새로 만든다. 쓴 결과를 다시 파싱해 돌려준다.
pub fn write_cover_pages(archive_path: &Path, covers: &[u32]) -> Result<ComicInfo, AppError> {
    if !crate::image::is_archive_file(archive_path) {
        return Err(AppError::unsupported("Not an archive file"));
    }

    let key = archive_path.to_string_lossy().into_owned();
    crate::sidecar::with_file_lock(&key, "comic info write lock", || {
        let entries = crate::archive_index::get_archive_entries(archive_path)?;
        let total = entries.images.len();
        if covers.iter().any(|&cover| cover as usize >= total) {
            return Err(AppError::invalid_input("Cover page is out of range"));
        }
        let covers: BTreeSet<u32> = covers.iter().copied().collect();

        let existing = find_comic_info_in_entries(&entries.all);
        let xml = match &existing {
            Some(entry_name) => {
                let bytes = crate::archive::read_archive_entry_bounded(
                    archive_path,
                    entry_name,
                    MAX_COMICINFO_BYTES,
                )?;
                apply_cover_pages(&decode_xml(&bytes)?, &covers)?
            }
            None => new_document(&covers),
        };
        // 쓰기 전에 결과가 읽히는지 확인해, 깨진 XML로 원본을 덮지 않는다.
        let info = parse_comic_info(xml.as_bytes())?;

        let entry_name = existing.as_deref().unwrap_or("ComicInfo.xml");
        crate::archive::replace_archive_entry(archive_path, entry_name, xml.as_bytes())?;
        Ok(info)
    })
}

fn page_element(image: u32, page_type: &str) -> BytesStart<'static> {
    let mut element = BytesStart::new("Page");
    element.push_attribute(("Image", image.to_string().as_str()));
    element.push_attribute(("Type", page_type));
    element
}

/// `Pages`에 아직 없는 표지(또는 "표지 없음" 표식) 페이지.
fn missing_pages(covers: &BTreeSet<u32>, seen: &BTreeSet<u32>) -> Vec<BytesStart<'static>> {
    if covers.is_empty() {
        return if seen.contains(&0) {
            Vec::new()
        } else {
            vec![page_element(0, STORY)]
        };
    }
    covers
        .difference(seen)
        .map(|&image| page_element(image, FRONT_COVER))
        .collect()
}

fn new_document(covers: &BTreeSet<u32>) -> String {
    let pages: String = missing_pages(covers, &BTreeSet::new())
        .iter()
        .map(|page| format!("    <{} />\n", &**page))
        .collect();
    format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<ComicInfo xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xmlns:xsd=\"http://www.w3.org/2001/XMLSchema\">\n  <Pages>\n{pages}  </Pages>\n</ComicInfo>\n"
    )
}

/// 기존 `Page` 요소의 `Type`을 표지 집합에 맞춘다. 다른 속성은 원문 그대로 둔다.
fn retype_page(
    page: &BytesStart<'_>,
    covers: &BTreeSet<u32>,
    seen: &mut BTreeSet<u32>,
) -> Result<BytesStart<'static>, AppError> {
    let attrs = page
        .attributes()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| AppError::corrupt(format!("Failed to parse ComicInfo.xml: {e}")))?;
    let value_of = |name: &str| {
        attrs
            .iter()
            .find(|attr| attr.key.0 == name)
            .map(|attr| attr.value.trim().to_string())
    };
    // Image가 없으면 어느 페이지인지 알 수 없어 건드리지 않는다.
    let Some(image) = value_of("Image").and_then(|v| v.parse::<u32>().ok()) else {
        return Ok(page.to_owned());
    };
    seen.insert(image);

    let current = value_of("Type").unwrap_or_default();
    let next = if covers.contains(&image) {
        FRONT_COVER
    } else if current.eq_ignore_ascii_case(FRONT_COVER)
        || (covers.is_empty() && image == 0 && current.is_empty())
    {
        STORY
    } else {
        return Ok(page.to_owned());
    };

    let name = page.name().0.to_string();
    let mut element = BytesStart::new(name);
    for attr in attrs {
        if attr.key.0 != "Type" {
            element.push_attribute(attr);
        }
    }
    element.push_attribute(("Type", next));
    Ok(element)
}

/// 기존 XML에 표지 집합을 반영한다. 출력은 항상 UTF-8이다.
fn apply_cover_pages(xml: &str, covers: &BTreeSet<u32>) -> Result<String, AppError> {
    let mut reader = quick_xml::Reader::from_str(xml);
    let mut writer = quick_xml::Writer::new(Vec::new());
    let mut emit = |event: Event<'_>| {
        writer
            .write_event(event)
            .map_err(|e| AppError::unknown(format!("Failed to write ComicInfo.xml: {e}")))
    };
    let pages_block = |seen: &BTreeSet<u32>| -> Vec<Event<'static>> {
        let mut events = vec![Event::Start(BytesStart::new("Pages"))];
        events.extend(missing_pages(covers, seen).into_iter().map(Event::Empty));
        events.push(Event::End(BytesEnd::new("Pages")));
        events
    };

    let mut seen = BTreeSet::new();
    let mut depth = 0usize;
    let mut in_pages = false;
    let mut wrote_pages = false;
    loop {
        let event = reader
            .read_event()
            .map_err(|e| AppError::corrupt(format!("Failed to parse ComicInfo.xml: {e}")))?;
        match event {
            Event::Eof => break,
            // UTF-16 원본도 UTF-8로 다시 쓰므로 선언의 encoding을 맞춘다.
            Event::Decl(_) => emit(Event::Decl(BytesDecl::new("1.0", Some("utf-8"), None)))?,
            Event::Start(element) => {
                let is_pages = depth == 1 && element.name().0 == "Pages";
                let is_page = in_pages && depth == 2 && element.name().0 == "Page";
                if is_page {
                    emit(Event::Start(retype_page(&element, covers, &mut seen)?))?;
                } else {
                    emit(Event::Start(element))?;
                }
                if is_pages {
                    in_pages = true;
                    wrote_pages = true;
                }
                depth += 1;
            }
            Event::Empty(element) => {
                if in_pages && depth == 2 && element.name().0 == "Page" {
                    emit(Event::Empty(retype_page(&element, covers, &mut seen)?))?;
                } else if depth == 1 && element.name().0 == "Pages" {
                    wrote_pages = true;
                    for event in pages_block(&seen) {
                        emit(event)?;
                    }
                } else if depth == 0 {
                    // `<ComicInfo/>`: 루트를 열어 Pages를 넣는다.
                    wrote_pages = true;
                    let end = element.to_end().into_owned();
                    emit(Event::Start(element))?;
                    for event in pages_block(&seen) {
                        emit(event)?;
                    }
                    emit(Event::End(end))?;
                } else {
                    emit(Event::Empty(element))?;
                }
            }
            Event::End(element) => {
                depth = depth.saturating_sub(1);
                if in_pages && depth == 1 {
                    in_pages = false;
                    for page in missing_pages(covers, &seen) {
                        emit(Event::Empty(page))?;
                    }
                } else if depth == 0 && !wrote_pages {
                    wrote_pages = true;
                    emit(Event::Text(BytesText::new("  ")))?;
                    for event in pages_block(&seen) {
                        emit(event)?;
                    }
                    emit(Event::Text(BytesText::new("\n")))?;
                }
                emit(Event::End(element))?;
            }
            other => emit(other)?,
        }
    }
    if !wrote_pages {
        return Err(AppError::corrupt("ComicInfo.xml has no root element"));
    }

    String::from_utf8(writer.into_inner())
        .map_err(|_| AppError::corrupt("ComicInfo.xml is not valid UTF-8"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_error::ErrorCode;
    use std::fs;
    use std::io::Write as _;
    use std::path::{Path, PathBuf};

    /// zip writer로 픽스처를 만든다. `entries`는 (엔트리 이름, 바이트).
    fn write_cbz(dir: &Path, file_name: &str, entries: &[(&str, &[u8])]) -> PathBuf {
        let archive_path = dir.join(file_name);
        let file = fs::File::create(&archive_path).expect("create cbz");
        let mut writer = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        for (name, data) in entries {
            writer.start_file(*name, options).expect("start entry");
            writer.write_all(data).expect("write entry");
        }
        writer.finish().expect("finish zip");
        archive_path
    }

    const SIMPLE_XML: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<ComicInfo xmlns:xsd="http://www.w3.org/2001/XMLSchema">
  <Title>첫 화</Title>
  <Series>테스트 시리즈</Series>
  <Number>1.5</Number>
  <Count>12</Count>
  <Volume>2024</Volume>
  <Summary>줄거리 요약</Summary>
  <Writer>글 작가</Writer>
  <Penciller>그림 작가</Penciller>
  <Publisher>테스트 출판사</Publisher>
  <Genre>판타지</Genre>
  <Tags>태그1,태그2</Tags>
  <LanguageISO>ko</LanguageISO>
  <PageCount>20</PageCount>
  <AgeRating>12+</AgeRating>
  <CommunityRating>8.5</CommunityRating>
  <Pages>
    <Page Image="3" Type="Story"/>
    <Page Image="0" Type="FrontCover"/>
    <Page Image="1"/>
  </Pages>
</ComicInfo>"#;

    #[test]
    fn parses_all_fields() {
        let dir = tempfile::tempdir().expect("tempdir");
        let archive = write_cbz(
            dir.path(),
            "comic.cbz",
            &[("ComicInfo.xml", SIMPLE_XML.as_bytes())],
        );

        let info = read_comic_info(&archive)
            .expect("read comic info")
            .expect("comic info present");

        assert_eq!(info.title.as_deref(), Some("첫 화"));
        assert_eq!(info.series.as_deref(), Some("테스트 시리즈"));
        assert_eq!(info.number.as_deref(), Some("1.5"));
        assert_eq!(info.count, Some(12));
        assert_eq!(info.volume, Some(2024));
        assert_eq!(info.summary.as_deref(), Some("줄거리 요약"));
        assert_eq!(info.writer.as_deref(), Some("글 작가"));
        assert_eq!(info.penciller.as_deref(), Some("그림 작가"));
        assert_eq!(info.publisher.as_deref(), Some("테스트 출판사"));
        assert_eq!(info.genre.as_deref(), Some("판타지"));
        assert_eq!(info.tags.as_deref(), Some("태그1,태그2"));
        assert_eq!(info.language_iso.as_deref(), Some("ko"));
        assert_eq!(info.page_count, Some(20));
        assert_eq!(info.age_rating.as_deref(), Some("12+"));
        assert_eq!(info.community_rating.as_deref(), Some("8.5"));

        let pages = info.pages.expect("pages");
        assert_eq!(pages.len(), 3);
        // image 오름차순 정렬
        assert_eq!(pages[0].image, 0);
        assert_eq!(pages[0].page_type.as_deref(), Some("FrontCover"));
        assert_eq!(pages[1].image, 1);
        assert_eq!(pages[1].page_type, None);
        assert_eq!(pages[2].image, 3);
    }

    #[test]
    fn parses_manga_direction() {
        let dir = tempfile::tempdir().expect("tempdir");
        let archive = write_cbz(
            dir.path(),
            "comic.cbz",
            &[(
                "ComicInfo.xml",
                b"<ComicInfo><Manga> YesAndRightToLeft </Manga></ComicInfo>",
            )],
        );
        let info = read_comic_info(&archive).expect("read").expect("present");
        assert_eq!(info.manga.as_deref(), Some("YesAndRightToLeft"));
    }

    #[test]
    fn missing_fields_and_empty_elements_become_none() {
        let xml = "<ComicInfo><Title>   </Title><Series></Series><Count>abc</Count></ComicInfo>";
        let dir = tempfile::tempdir().expect("tempdir");
        let archive = write_cbz(
            dir.path(),
            "comic.zip",
            &[("ComicInfo.xml", xml.as_bytes())],
        );

        let info = read_comic_info(&archive)
            .expect("read")
            .expect("comic info present");

        assert_eq!(info.title, None);
        assert_eq!(info.series, None);
        assert_eq!(info.count, None);
        assert_eq!(info.pages, None);
        assert_eq!(info, ComicInfo::default());
    }

    #[test]
    fn finds_entry_case_insensitively() {
        let dir = tempfile::tempdir().expect("tempdir");
        let archive = write_cbz(
            dir.path(),
            "comic.cbz",
            &[
                ("COMICINFO.XML", SIMPLE_XML.as_bytes()),
                ("001.png", b"fake-png-bytes"),
            ],
        );

        let info = read_comic_info(&archive).expect("read").expect("present");
        assert_eq!(info.series.as_deref(), Some("테스트 시리즈"));
    }

    #[test]
    fn root_entry_wins_over_nested() {
        let root_xml = "<ComicInfo><Title>루트</Title></ComicInfo>";
        let nested_xml = "<ComicInfo><Title>중첩</Title></ComicInfo>";
        let dir = tempfile::tempdir().expect("tempdir");
        let archive = write_cbz(
            dir.path(),
            "comic.cbz",
            &[
                ("ch1/ComicInfo.xml", nested_xml.as_bytes()),
                ("ComicInfo.xml", root_xml.as_bytes()),
                ("ch2/ComicInfo.xml", nested_xml.as_bytes()),
            ],
        );

        let info = read_comic_info(&archive).expect("read").expect("present");
        assert_eq!(info.title.as_deref(), Some("루트"));
    }

    #[test]
    fn nested_entry_is_used_when_root_is_missing() {
        let nested_xml = "<ComicInfo><Title>중첩</Title></ComicInfo>";
        let dir = tempfile::tempdir().expect("tempdir");
        let archive = write_cbz(
            dir.path(),
            "comic.cbz",
            &[("ch1/ComicInfo.xml", nested_xml.as_bytes())],
        );

        let info = read_comic_info(&archive).expect("read").expect("present");
        assert_eq!(info.title.as_deref(), Some("중첩"));
    }

    #[test]
    fn missing_xml_returns_none() {
        let dir = tempfile::tempdir().expect("tempdir");
        let archive = write_cbz(dir.path(), "comic.cbz", &[("001.png", b"fake-png-bytes")]);

        assert!(read_comic_info(&archive).expect("read").is_none());
    }

    #[test]
    fn broken_xml_is_corrupt() {
        let dir = tempfile::tempdir().expect("tempdir");
        let archive = write_cbz(
            dir.path(),
            "comic.cbz",
            &[("ComicInfo.xml", b"<ComicInfo><Title>oops</ComicInfo>")],
        );

        let err = read_comic_info(&archive).expect_err("expected error");
        assert_eq!(err.code, ErrorCode::Corrupt);
    }

    #[test]
    fn invalid_utf8_is_corrupt() {
        let dir = tempfile::tempdir().expect("tempdir");
        let archive = write_cbz(
            dir.path(),
            "comic.cbz",
            &[("ComicInfo.xml", &[0xFF, 0x00, 0xFE, 0x41])],
        );

        let err = read_comic_info(&archive).expect_err("expected error");
        assert_eq!(err.code, ErrorCode::Corrupt);
    }

    #[test]
    fn oversized_xml_is_too_large() {
        let mut xml = String::from("<ComicInfo><Summary>");
        xml.push_str(&"a".repeat(MAX_COMICINFO_BYTES as usize));
        xml.push_str("</Summary></ComicInfo>");

        let dir = tempfile::tempdir().expect("tempdir");
        let archive = write_cbz(
            dir.path(),
            "comic.cbz",
            &[("ComicInfo.xml", xml.as_bytes())],
        );

        let err = read_comic_info(&archive).expect_err("expected error");
        assert_eq!(err.code, ErrorCode::TooLarge);
    }

    #[test]
    fn reads_utf16_boms() {
        let xml = "<ComicInfo><Title>유니코드</Title><LanguageISO>ja</LanguageISO></ComicInfo>";

        let mut le: Vec<u8> = vec![0xFF, 0xFE];
        for unit in xml.encode_utf16() {
            le.extend_from_slice(&unit.to_le_bytes());
        }
        let mut be: Vec<u8> = vec![0xFE, 0xFF];
        for unit in xml.encode_utf16() {
            be.extend_from_slice(&unit.to_be_bytes());
        }
        let mut utf8_bom: Vec<u8> = vec![0xEF, 0xBB, 0xBF];
        utf8_bom.extend_from_slice(xml.as_bytes());

        let dir = tempfile::tempdir().expect("tempdir");
        for (file_name, bytes) in [
            ("le.cbz", le.as_slice()),
            ("be.cbz", be.as_slice()),
            ("bom.cbz", utf8_bom.as_slice()),
        ] {
            let archive = write_cbz(dir.path(), file_name, &[("ComicInfo.xml", bytes)]);
            let info = read_comic_info(&archive)
                .expect("read")
                .unwrap_or_else(|| panic!("{file_name}: comic info missing"));
            assert_eq!(info.title.as_deref(), Some("유니코드"), "{file_name}");
        }
    }

    #[test]
    fn truncated_utf16_is_corrupt() {
        // BOM 뒤 홀수 바이트(반쪽 코드 유닛)는 거부한다. as_chunks 교체가
        // 마지막 바이트를 조용히 버리고 성공하지 않는지 회귀 고정한다.
        let dir = tempfile::tempdir().expect("tempdir");
        let mut bytes: Vec<u8> = vec![0xFF, 0xFE];
        bytes.extend_from_slice(b"<C");
        bytes.push(0x00);
        let archive = write_cbz(
            dir.path(),
            "comic.cbz",
            &[("ComicInfo.xml", bytes.as_slice())],
        );

        let err = read_comic_info(&archive).expect_err("expected error");
        assert_eq!(err.code, ErrorCode::Corrupt);
    }

    #[test]
    fn non_zip_extensions_return_none() {
        let dir = tempfile::tempdir().expect("tempdir");
        for ext in ["cb7", "cbr", "cbt", "rar", "7z", "png"] {
            let path = dir.path().join(format!("comic.{ext}"));
            fs::write(&path, b"not an archive").expect("write");
            assert!(read_comic_info(&path).expect("read").is_none(), "{ext}");
        }
    }

    #[test]
    fn pages_without_image_attribute_are_skipped() {
        let xml =
            "<ComicInfo><Pages><Page Type=\"FrontCover\"/><Page Image=\"2\"/></Pages></ComicInfo>";
        let dir = tempfile::tempdir().expect("tempdir");
        let archive = write_cbz(
            dir.path(),
            "comic.cbz",
            &[("ComicInfo.xml", xml.as_bytes())],
        );

        let info = read_comic_info(&archive).expect("read").expect("present");
        let pages = info.pages.expect("pages");
        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].image, 2);
    }

    #[test]
    fn sample_cbz_fixture_when_present() {
        // 저장소 samples/sample-comicinfo.cbz(실제 CBZ)로 회귀 검증.
        let source = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../samples")
            .join("sample-comicinfo.cbz");
        if !source.is_file() {
            return;
        }
        let info = read_comic_info(&source)
            .expect("read sample")
            .expect("comic info present");
        assert_eq!(info.series.as_deref(), Some("AraView 샘플"));
        assert_eq!(info.number.as_deref(), Some("1"));
        assert_eq!(info.community_rating.as_deref(), Some("7.5"));
        assert_eq!(info.page_count, Some(5));
        let pages = info.pages.expect("pages");
        assert_eq!(pages[0].image, 0);
        assert_eq!(pages[0].page_type.as_deref(), Some("FrontCover"));
    }

    #[test]
    fn comic_info_serializes_snake_case() {
        let info = ComicInfo {
            series: Some("S".to_string()),
            language_iso: Some("ko".to_string()),
            page_count: Some(3),
            pages: Some(vec![ComicPage {
                image: 0,
                page_type: Some("FrontCover".to_string()),
            }]),
            ..ComicInfo::default()
        };
        let json = serde_json::to_value(&info).expect("serialize");
        assert_eq!(json["language_iso"], "ko");
        assert_eq!(json["page_count"], 3);
        assert_eq!(json["pages"][0]["page_type"], "FrontCover");
        assert!(json["title"].is_null());
    }

    #[test]
    fn parses_credits_and_release_date() {
        let xml = "<ComicInfo><Inker>잉커</Inker><Colorist>채색</Colorist><Letterer>식자</Letterer><CoverArtist>표지</CoverArtist><Editor>편집</Editor><Year>2024</Year><Month>3</Month><Day>x</Day></ComicInfo>";
        let info = parse_comic_info(xml.as_bytes()).expect("parse");
        assert_eq!(info.inker.as_deref(), Some("잉커"));
        assert_eq!(info.cover_artist.as_deref(), Some("표지"));
        assert_eq!(info.editor.as_deref(), Some("편집"));
        assert_eq!(
            (info.year, info.month, info.day),
            (Some(2024), Some(3), None)
        );
    }

    fn cover_set(covers: &[u32]) -> BTreeSet<u32> {
        covers.iter().copied().collect()
    }

    fn cover_types(xml: &str) -> Vec<(u32, Option<String>)> {
        parse_comic_info(xml.as_bytes())
            .expect("parse")
            .pages
            .unwrap_or_default()
            .into_iter()
            .map(|page| (page.image, page.page_type))
            .collect()
    }

    #[test]
    fn apply_cover_pages_moves_cover_and_keeps_other_fields() {
        let out = apply_cover_pages(SIMPLE_XML, &cover_set(&[1])).expect("apply");
        let info = parse_comic_info(out.as_bytes()).expect("parse");
        assert_eq!(info.series.as_deref(), Some("테스트 시리즈"));
        assert_eq!(info.number.as_deref(), Some("1.5"));
        assert_eq!(
            cover_types(&out),
            vec![
                (0, Some("Story".to_string())),
                (1, Some("FrontCover".to_string())),
                (3, Some("Story".to_string())),
            ]
        );
    }

    #[test]
    fn apply_cover_pages_adds_missing_pages_and_pages_element() {
        let out = apply_cover_pages(
            "<ComicInfo><Title>t</Title></ComicInfo>",
            &cover_set(&[0, 1]),
        )
        .expect("apply");
        assert_eq!(
            cover_types(&out),
            vec![
                (0, Some("FrontCover".to_string())),
                (1, Some("FrontCover".to_string())),
            ]
        );
        assert!(out.contains("<Title>t</Title>"));

        for xml in ["<ComicInfo/>", "<ComicInfo><Pages/></ComicInfo>"] {
            let out = apply_cover_pages(xml, &cover_set(&[2])).expect("apply");
            assert_eq!(cover_types(&out), vec![(2, Some("FrontCover".to_string()))]);
        }
    }

    #[test]
    fn apply_cover_pages_empty_set_marks_first_page_as_story() {
        let out = apply_cover_pages(SIMPLE_XML, &cover_set(&[])).expect("apply");
        assert_eq!(cover_types(&out)[0], (0, Some("Story".to_string())));

        let out = apply_cover_pages("<ComicInfo></ComicInfo>", &cover_set(&[])).expect("apply");
        assert_eq!(cover_types(&out), vec![(0, Some("Story".to_string()))]);

        // 0번에 이미 다른 Type이 있으면 그대로 둔다.
        let xml = "<ComicInfo><Pages><Page Image=\"0\" Type=\"Other\"/></Pages></ComicInfo>";
        let out = apply_cover_pages(xml, &cover_set(&[])).expect("apply");
        assert_eq!(cover_types(&out), vec![(0, Some("Other".to_string()))]);
    }

    #[test]
    fn apply_cover_pages_preserves_unknown_attributes_and_elements() {
        let xml = "<?xml version=\"1.0\" encoding=\"utf-16\"?><ComicInfo><Custom a=\"1\">x &amp; y</Custom><Pages><Page Image=\"0\" ImageSize=\"123\" Type=\"FrontCover\"/></Pages></ComicInfo>";
        let out = apply_cover_pages(xml, &cover_set(&[])).expect("apply");
        assert!(out.contains("<Custom a=\"1\">x &amp; y</Custom>"));
        assert!(out.contains("ImageSize=\"123\""));
        assert!(out.contains("encoding=\"utf-8\""));
        assert_eq!(cover_types(&out), vec![(0, Some("Story".to_string()))]);
    }

    #[test]
    fn write_cover_pages_creates_and_updates_comic_info() {
        let dir = tempfile::tempdir().unwrap();
        let archive = write_cbz(
            dir.path(),
            "plain.cbz",
            &[("001.png", b"a"), ("002.png", b"b"), ("003.png", b"c")],
        );

        let info = write_cover_pages(&archive, &[0, 1]).expect("create");
        let covers: Vec<u32> = info.pages.unwrap().iter().map(|p| p.image).collect();
        assert_eq!(covers, vec![0, 1]);

        write_cover_pages(&archive, &[]).expect("clear");
        let read = read_comic_info(&archive).expect("read").expect("some");
        assert_eq!(
            read.pages.unwrap(),
            vec![
                ComicPage {
                    image: 0,
                    page_type: Some("Story".to_string())
                },
                ComicPage {
                    image: 1,
                    page_type: Some("Story".to_string())
                },
            ]
        );
        // 페이지 목록은 그대로다.
        let entries = crate::archive_index::get_archive_entries(&archive).expect("entries");
        assert_eq!(entries.images.len(), 3);
    }

    #[test]
    fn write_cover_pages_rejects_out_of_range_and_non_archive() {
        let dir = tempfile::tempdir().unwrap();
        let archive = write_cbz(dir.path(), "plain.cbz", &[("001.png", b"a")]);
        let err = write_cover_pages(&archive, &[1]).unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidInput);
        let err = write_cover_pages(Path::new("a.png"), &[0]).unwrap_err();
        assert_eq!(err.code, ErrorCode::Unsupported);
    }
}
