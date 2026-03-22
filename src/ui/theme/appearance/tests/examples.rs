use super::*;

#[test]
fn blush_light_example_theme_parses_as_user_theme_and_applies_custom_icon_and_code_colors() {
    let theme = load_alternate_example_theme("blush-light");

    assert_eq!(theme.preview.code.keyword, rgb(0xd8, 0x63, 0x92));
    assert_eq!(theme.preview.code.function, rgb(0x8f, 0x71, 0xbf));

    let directory = theme.resolve(Path::new("projects"), EntryKind::Directory);
    assert_eq!(directory.class, FileClass::Directory);
    assert_eq!(directory.icon, "󰉋");
    assert_eq!(directory.color, rgb(0xd4, 0x6b, 0x93));

    let rust = theme.resolve(Path::new("main.rs"), EntryKind::File);
    assert_eq!(rust.class, FileClass::Code);
    assert_eq!(rust.icon, "");
    assert_eq!(rust.color, rgb(0xca, 0x81, 0x68));

    let readme = theme.resolve(Path::new("README.md"), EntryKind::File);
    assert_eq!(readme.class, FileClass::Document);
    assert_eq!(readme.icon, "");
    assert_eq!(readme.color, rgb(0xbb, 0x90, 0x7b));
}

#[test]
fn default_light_example_theme_parses_as_user_theme_and_preserves_default_icon_mappings() {
    let theme = load_alternate_example_theme("default-light");

    assert_eq!(theme.palette.bg, rgb(0xef, 0xf2, 0xf5));
    assert_eq!(theme.preview.code.keyword, rgb(0x7a, 0xae, 0xff));
    assert_eq!(theme.preview.code.function, rgb(0x46, 0x9f, 0xc3));
    assert_eq!(theme.preview.code.string, rgb(0x4d, 0x92, 0x79));
    assert_eq!(theme.preview.code.r#type, rgb(0x8a, 0x74, 0xc8));

    let directory = theme.resolve(Path::new("projects"), EntryKind::Directory);
    assert_eq!(directory.class, FileClass::Directory);
    assert_eq!(directory.icon, "󰉋");
    assert_eq!(directory.color, rgb(0x5b, 0xa8, 0xff));

    let downloads = theme.resolve(Path::new("Downloads"), EntryKind::Directory);
    assert_eq!(downloads.class, FileClass::Directory);
    assert_eq!(downloads.icon, "󰉍");
    assert_eq!(downloads.color, rgb(0xb9, 0x97, 0x3e));

    let pictures = theme.resolve(Path::new("Pictures"), EntryKind::Directory);
    assert_eq!(pictures.class, FileClass::Directory);
    assert_eq!(pictures.icon, "󰉏");
    assert_eq!(pictures.color, rgb(0x55, 0xa7, 0x9e));

    let music = theme.resolve(Path::new("Music"), EntryKind::Directory);
    assert_eq!(music.class, FileClass::Directory);
    assert_eq!(music.icon, "󱍙");
    assert_eq!(music.color, rgb(0x9a, 0x81, 0xcf));

    let src = theme.resolve(Path::new("src"), EntryKind::Directory);
    assert_eq!(src.class, FileClass::Directory);
    assert_eq!(src.icon, "󰉋");
    assert_eq!(src.color, rgb(0x5b, 0xa8, 0xff));

    let shell = theme.resolve(Path::new("deploy.sh"), EntryKind::File);
    assert_eq!(shell.class, FileClass::Code);
    assert_eq!(shell.icon, "");
    assert_eq!(shell.color, rgb(0x69, 0x78, 0x8b));

    let rust = theme.resolve(Path::new("main.rs"), EntryKind::File);
    assert_eq!(rust.class, FileClass::Code);
    assert_eq!(rust.icon, "");
    assert_eq!(rust.color, rgb(0xb8, 0x74, 0x45));

    let package = theme.resolve(Path::new("package.json"), EntryKind::File);
    assert_eq!(package.class, FileClass::Config);
    assert_eq!(package.icon, "󰏗");
    assert_eq!(package.color, rgb(0x7d, 0xb0, 0xff));

    let readme = theme.resolve(Path::new("README.md"), EntryKind::File);
    assert_eq!(readme.class, FileClass::Document);
    assert_eq!(readme.icon, "");
    assert_eq!(readme.color, rgb(0xab, 0x97, 0x7a));

    let turbo = theme.resolve(Path::new("turbo.json"), EntryKind::File);
    assert_eq!(turbo.class, FileClass::Config);
    assert_eq!(turbo.icon, "󰐷");
    assert_eq!(turbo.color, rgb(0x72, 0x81, 0x95));
}

#[test]
fn amber_dusk_example_theme_parses_as_user_theme_and_applies_warm_dark_palette() {
    let theme = load_alternate_example_theme("amber-dusk");

    assert_eq!(theme.palette.bg, rgb(0x12, 0x0f, 0x0d));
    assert_eq!(theme.preview.code.keyword, rgb(0xcf, 0x98, 0x51));
    assert_eq!(theme.preview.code.function, rgb(0x7f, 0xa7, 0xa5));

    let directory = theme.resolve(Path::new("projects"), EntryKind::Directory);
    assert_eq!(directory.class, FileClass::Directory);
    assert_eq!(directory.icon, "󰉋");
    assert_eq!(directory.color, rgb(0xcf, 0x9c, 0x67));

    let downloads = theme.resolve(Path::new("Downloads"), EntryKind::Directory);
    assert_eq!(downloads.class, FileClass::Directory);
    assert_eq!(downloads.icon, "󰉍");
    assert_eq!(downloads.color, rgb(0xd4, 0xa4, 0x66));

    let src = theme.resolve(Path::new("src"), EntryKind::Directory);
    assert_eq!(src.class, FileClass::Directory);
    assert_eq!(src.icon, "󰉋");
    assert_eq!(src.color, rgb(0xcf, 0x9c, 0x67));

    let vendor = theme.resolve(Path::new("vendor"), EntryKind::Directory);
    assert_eq!(vendor.class, FileClass::Directory);
    assert_eq!(vendor.icon, "󰉋");
    assert_eq!(vendor.color, rgb(0xcf, 0x9c, 0x67));

    let rust = theme.resolve(Path::new("main.rs"), EntryKind::File);
    assert_eq!(rust.class, FileClass::Code);
    assert_eq!(rust.icon, "");
    assert_eq!(rust.color, rgb(0xc5, 0x8a, 0x5e));
}

#[test]
fn catppuccin_mocha_example_theme_parses_as_user_theme_and_applies_palette_consistently() {
    let theme = load_alternate_example_theme("catppuccin-mocha");

    assert_eq!(theme.palette.bg, rgb(0x1e, 0x1e, 0x2e));
    assert_eq!(theme.palette.selected_bg, rgb(0x45, 0x47, 0x5a));
    assert_ne!(theme.palette.selected_bg, theme.palette.surface);
    assert_eq!(theme.preview.code.keyword, rgb(0xcb, 0xa6, 0xf7));
    assert_eq!(theme.preview.code.function, rgb(0x89, 0xb4, 0xfa));
    assert_eq!(theme.preview.code.string, rgb(0xa6, 0xe3, 0xa1));
    assert_eq!(theme.preview.code.r#type, rgb(0xf9, 0xe2, 0xaf));

    let directory = theme.resolve(Path::new("projects"), EntryKind::Directory);
    assert_eq!(directory.class, FileClass::Directory);
    assert_eq!(directory.icon, "󰉋");
    assert_eq!(directory.color, rgb(0x89, 0xb4, 0xfa));

    let downloads = theme.resolve(Path::new("Downloads"), EntryKind::Directory);
    assert_eq!(downloads.class, FileClass::Directory);
    assert_eq!(downloads.icon, "󰉍");
    assert_eq!(downloads.color, rgb(0xf9, 0xe2, 0xaf));

    let pictures = theme.resolve(Path::new("Pictures"), EntryKind::Directory);
    assert_eq!(pictures.class, FileClass::Directory);
    assert_eq!(pictures.icon, "󰉏");
    assert_eq!(pictures.color, rgb(0x94, 0xe2, 0xd5));

    let music = theme.resolve(Path::new("Music"), EntryKind::Directory);
    assert_eq!(music.class, FileClass::Directory);
    assert_eq!(music.icon, "󱍙");
    assert_eq!(music.color, rgb(0xcb, 0xa6, 0xf7));

    let src = theme.resolve(Path::new("src"), EntryKind::Directory);
    assert_eq!(src.class, FileClass::Directory);
    assert_eq!(src.icon, "󰉋");
    assert_eq!(src.color, rgb(0x89, 0xb4, 0xfa));

    let rust = theme.resolve(Path::new("main.rs"), EntryKind::File);
    assert_eq!(rust.class, FileClass::Code);
    assert_eq!(rust.icon, "");
    assert_eq!(rust.color, rgb(0xfa, 0xb3, 0x87));

    let package = theme.resolve(Path::new("package.json"), EntryKind::File);
    assert_eq!(package.class, FileClass::Config);
    assert_eq!(package.icon, "󰏗");
    assert_eq!(package.color, rgb(0x89, 0xb4, 0xfa));

    let readme = theme.resolve(Path::new("README.md"), EntryKind::File);
    assert_eq!(readme.class, FileClass::Document);
    assert_eq!(readme.icon, "");
    assert_eq!(readme.color, rgb(0xf9, 0xe2, 0xaf));
}

#[test]
fn tokyo_night_example_theme_parses_as_user_theme_and_applies_palette_consistently() {
    let theme = load_alternate_example_theme("tokyo-night");

    assert_eq!(theme.palette.bg, rgb(0x1a, 0x1b, 0x26));
    assert_eq!(theme.preview.code.keyword, rgb(0xbb, 0x9a, 0xf7));
    assert_eq!(theme.preview.code.function, rgb(0x7d, 0xcf, 0xff));
    assert_eq!(theme.preview.code.string, rgb(0x9e, 0xce, 0x6a));
    assert_eq!(theme.preview.code.r#type, rgb(0xe0, 0xaf, 0x68));

    let directory = theme.resolve(Path::new("projects"), EntryKind::Directory);
    assert_eq!(directory.class, FileClass::Directory);
    assert_eq!(directory.icon, "󰉋");
    assert_eq!(directory.color, rgb(0x7a, 0xa2, 0xf7));

    let downloads = theme.resolve(Path::new("Downloads"), EntryKind::Directory);
    assert_eq!(downloads.class, FileClass::Directory);
    assert_eq!(downloads.icon, "󰉍");
    assert_eq!(downloads.color, rgb(0xe0, 0xaf, 0x68));

    let pictures = theme.resolve(Path::new("Pictures"), EntryKind::Directory);
    assert_eq!(pictures.class, FileClass::Directory);
    assert_eq!(pictures.icon, "󰉏");
    assert_eq!(pictures.color, rgb(0x73, 0xda, 0xca));

    let music = theme.resolve(Path::new("Music"), EntryKind::Directory);
    assert_eq!(music.class, FileClass::Directory);
    assert_eq!(music.icon, "󱍙");
    assert_eq!(music.color, rgb(0xbb, 0x9a, 0xf7));

    let src = theme.resolve(Path::new("src"), EntryKind::Directory);
    assert_eq!(src.class, FileClass::Directory);
    assert_eq!(src.icon, "󰉋");
    assert_eq!(src.color, rgb(0x7a, 0xa2, 0xf7));

    let rust = theme.resolve(Path::new("main.rs"), EntryKind::File);
    assert_eq!(rust.class, FileClass::Code);
    assert_eq!(rust.icon, "");
    assert_eq!(rust.color, rgb(0xff, 0x9e, 0x64));

    let package = theme.resolve(Path::new("package.json"), EntryKind::File);
    assert_eq!(package.class, FileClass::Config);
    assert_eq!(package.icon, "󰏗");
    assert_eq!(package.color, rgb(0x7a, 0xa2, 0xf7));

    let readme = theme.resolve(Path::new("README.md"), EntryKind::File);
    assert_eq!(readme.class, FileClass::Document);
    assert_eq!(readme.icon, "");
    assert_eq!(readme.color, rgb(0xe0, 0xaf, 0x68));
}

#[test]
fn alternate_example_themes_use_normal_folder_color_for_generic_dev_directories() {
    for label in ALTERNATE_EXAMPLE_THEME_NAMES {
        let theme = load_alternate_example_theme(label);
        assert_uses_normal_folder_color_for_generic_dev_directories(&theme, label);
    }
}
