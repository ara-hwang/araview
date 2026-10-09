//! 앱 자산 소스: 컴포넌트 기본 아이콘과 뷰어가 쓰는 Lucide 아이콘.

use std::borrow::Cow;

use gpui_kit::assets::{Assets as ComponentAssets, icon_assets};
use gpui_kit::{AssetSource, Result, SharedString};

// 기본 컴포넌트 아이콘(101개) 밖의 Lucide 아이콘은 여기 적은 것만 바이너리에 들어간다.
icon_assets!(
    ExtraIcons,
    [
        BookOpen,
        BookOpenText,
        ChevronLeft,
        ChevronRight,
        Cloud,
        Columns2,
        Copy,
        Ellipsis,
        ExternalLink,
        Eye,
        EyeOff,
        FlipHorizontal2,
        FlipVertical2,
        FolderOpen,
        GalleryVertical,
        House,
        Image,
        ImageOff,
        Info,
        Keyboard,
        LayoutGrid,
        Maximize,
        MoveHorizontal,
        MoveVertical,
        Pause,
        Pencil,
        Pin,
        PinOff,
        Play,
        RotateCcw,
        RotateCw,
        Search,
        Settings,
        SkipBack,
        SkipForward,
        Square,
        Trash,
        TriangleAlert,
        X,
        ZoomIn,
        ZoomOut,
    ]
);

pub struct AppAssets;

impl AssetSource for AppAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        // 기본 소스는 모르는 경로에 에러를 내므로 추가 아이콘을 먼저 본다.
        if let Some(bytes) = ExtraIcons.load(path)? {
            return Ok(Some(bytes));
        }
        ComponentAssets.load(path)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut paths = ComponentAssets.list(path)?;
        paths.extend(ExtraIcons.list(path)?);
        paths.sort();
        paths.dedup();
        Ok(paths)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extra_and_default_icons_both_load() {
        assert!(AppAssets.load("icons/zoom-in.svg").unwrap().is_some());
        assert!(AppAssets.load("icons/check.svg").unwrap().is_some());
    }
}
