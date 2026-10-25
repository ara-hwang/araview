//! 동영상 프로브 커맨드.

use std::path::Path;

use crate::app_error::AppError;
use crate::video::VideoProbe;

/// 컨테이너 헤더와 첫 비디오 스트림만 읽는다. 호출자가 blocking 풀에서 실행한다.
pub fn get_video_info_impl(file_path: &str) -> Result<VideoProbe, AppError> {
    crate::video::probe(Path::new(file_path))
}
