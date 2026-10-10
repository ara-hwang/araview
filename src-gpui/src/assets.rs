//! 앱 자산 소스: 컴포넌트 기본 아이콘(Lucide)과 뷰어 UI가 쓰는 Phosphor 아이콘.

use std::borrow::Cow;

use gpui_kit::assets::{Assets as ComponentAssets, IconNamed, icon_assets};
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

// 뷰어 UI 아이콘은 Tauri 때 쓰던 Phosphor 세트다. `resources/phosphor/`의 SVG를 그대로 넣는다.
macro_rules! phosphor_icons {
    ($($variant:ident => $file:literal,)*) => {
        #[derive(Clone, Copy, Debug)]
        pub enum Ph {
            $($variant,)*
        }

        impl Ph {
            const ALL: &[Ph] = &[$(Ph::$variant,)*];

            fn path_str(self) -> &'static str {
                match self { $(Ph::$variant => concat!("icons/phosphor/", $file, ".svg"),)* }
            }

            fn svg(self) -> &'static [u8] {
                match self {
                    $(Ph::$variant => include_bytes!(concat!("../resources/phosphor/", $file, ".svg")),)*
                }
            }
        }
    };
}

phosphor_icons! {
    ArrowClockwise => "arrow-clockwise",
    ArrowCounterClockwise => "arrow-counter-clockwise",
    ArrowsDownUp => "arrows-down-up",
    ArrowsHorizontal => "arrows-horizontal",
    ArrowsOut => "arrows-out",
    ArrowsVertical => "arrows-vertical",
    BookOpen => "book-open",
    BookOpenText => "book-open-text",
    BookOpenTextMirrored => "book-open-text-mirrored",
    CaretDown => "caret-down",
    CaretLeft => "caret-left",
    CaretRight => "caret-right",
    CaretUp => "caret-up",
    Cloud => "cloud",
    DotsThree => "dots-three",
    FileArchive => "file-archive",
    FileImage => "file-image",
    FlipHorizontal => "flip-horizontal",
    FlipVertical => "flip-vertical",
    FolderOpen => "folder-open",
    Gear => "gear",
    House => "house",
    Image => "image",
    Info => "info",
    MagnifyingGlass => "magnifying-glass",
    MagnifyingGlassMinus => "magnifying-glass-minus",
    MagnifyingGlassPlus => "magnifying-glass-plus",
    Pause => "pause",
    Play => "play",
    PushPin => "push-pin",
    PushPinFill => "push-pin-fill",
    SquaresFour => "squares-four",
    WarningCircle => "warning-circle",
    X => "x",
}

impl IconNamed for Ph {
    fn path(self) -> SharedString {
        self.path_str().into()
    }
}

pub struct AppAssets;

impl AssetSource for AppAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        // 기본 소스는 모르는 경로에 에러를 내므로 추가 아이콘을 먼저 본다.
        if let Some(icon) = Ph::ALL.iter().find(|icon| icon.path_str() == path) {
            return Ok(Some(Cow::Borrowed(icon.svg())));
        }
        if let Some(bytes) = ExtraIcons.load(path)? {
            return Ok(Some(bytes));
        }
        ComponentAssets.load(path)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut paths = ComponentAssets.list(path)?;
        paths.extend(ExtraIcons.list(path)?);
        paths.extend(
            Ph::ALL
                .iter()
                .map(|icon| icon.path_str())
                .filter(|icon| icon.starts_with(path))
                .map(SharedString::from),
        );
        paths.sort();
        paths.dedup();
        Ok(paths)
    }
}
