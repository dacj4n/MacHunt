/// Maps file extensions to the default macOS application names.
/// This is a comprehensive mapping covering design/creative tools and common formats.
use once_cell::sync::Lazy;
use std::collections::HashMap;

/// Build a static extension → application name mapping.
static EXT_APP_MAP: Lazy<HashMap<&'static str, &'static str>> = Lazy::new(|| {
    let mut m = HashMap::new();

    // ── Adobe Creative Cloud ──
    m.insert("ai", "Adobe Illustrator");
    m.insert("ait", "Adobe Illustrator");
    m.insert("psd", "Adobe Photoshop");
    m.insert("psb", "Adobe Photoshop");
    m.insert("psp", "Adobe Photoshop");
    m.insert("indd", "Adobe InDesign");
    m.insert("indt", "Adobe InDesign");
    m.insert("idml", "Adobe InDesign");
    m.insert("aep", "Adobe After Effects");
    m.insert("aet", "Adobe After Effects");
    m.insert("aepx", "Adobe After Effects");
    m.insert("prproj", "Adobe Premiere Pro");
    m.insert("ppj", "Adobe Premiere Pro");
    m.insert("prel", "Adobe Premiere Elements");
    m.insert("lrcat", "Adobe Lightroom");
    m.insert("lrtemplate", "Adobe Lightroom");
    m.insert("xmp", "Adobe Lightroom");
    m.insert("dng", "Adobe Lightroom");
    m.insert("fla", "Adobe Animate");
    m.insert("xfl", "Adobe Animate");
    m.insert("swf", "Adobe Flash Player");
    m.insert("ase", "Adobe Swatch Exchange");
    m.insert("aco", "Adobe Photoshop");
    m.insert("abr", "Adobe Photoshop");
    m.insert("pat", "Adobe Photoshop");
    m.insert("csh", "Adobe Photoshop");
    m.insert("grd", "Adobe Photoshop");

    // ── Sketch / Figma / XD ──
    m.insert("sketch", "Sketch");
    m.insert("fig", "Figma");
    m.insert("jam", "Figma");
    m.insert("xd", "Adobe XD");

    // ── Affinity Suite ──
    m.insert("afdesign", "Affinity Designer");
    m.insert("afphoto", "Affinity Photo");
    m.insert("afpub", "Affinity Publisher");

    // ── Corel ──
    m.insert("cdr", "CorelDRAW");
    m.insert("cdt", "CorelDRAW");
    m.insert("cmx", "CorelDRAW");

    // ── Pixelmator / Acorn ──
    m.insert("pxm", "Pixelmator Pro");
    m.insert("acorn", "Acorn");

    // ── 3D / CAD ──
    m.insert("blend", "Blender");
    m.insert("blend1", "Blender");
    m.insert("max", "3ds Max");
    m.insert("3ds", "3ds Max");
    m.insert("ma", "Maya");
    m.insert("mb", "Maya");
    m.insert("c4d", "Cinema 4D");
    m.insert("skp", "SketchUp");
    m.insert("obj", "预览");
    m.insert("fbx", "预览");
    m.insert("stl", "预览");
    m.insert("glb", "预览");
    m.insert("gltf", "预览");
    m.insert("usdz", "预览");
    m.insert("usd", "预览");
    m.insert("usda", "预览");
    m.insert("usdc", "预览");

    // ── Font ──
    m.insert("ttf", "字体册");
    m.insert("otf", "字体册");
    m.insert("woff", "字体册");
    m.insert("woff2", "字体册");

    // ── 通用图片格式 ──
    m.insert("jpg", "预览");
    m.insert("jpeg", "预览");
    m.insert("jpe", "预览");
    m.insert("jfif", "预览");
    m.insert("png", "预览");
    m.insert("gif", "预览");
    m.insert("webp", "预览");
    m.insert("bmp", "预览");
    m.insert("heic", "预览");
    m.insert("heif", "预览");
    m.insert("heics", "预览");
    m.insert("tif", "预览");
    m.insert("tiff", "预览");
    m.insert("ico", "预览");
    m.insert("icns", "预览");
    m.insert("eps", "预览");
    m.insert("svg", "预览");
    m.insert("svgz", "预览");
    m.insert("raw", "预览");
    m.insert("cr2", "预览");
    m.insert("cr3", "预览");
    m.insert("crw", "预览");
    m.insert("nef", "预览");
    m.insert("nrw", "预览");
    m.insert("arw", "预览");
    m.insert("srf", "预览");
    m.insert("sr2", "预览");
    m.insert("orf", "预览");
    m.insert("rw2", "预览");
    m.insert("pef", "预览");
    m.insert("raf", "预览");
    m.insert("dcr", "预览");
    m.insert("kdc", "预览");
    m.insert("mrw", "预览");
    m.insert("3fr", "预览");
    m.insert("fff", "预览");
    m.insert("exr", "预览");
    m.insert("hdr", "预览");
    m.insert("avif", "预览");

    // ── 音视频 ──
    m.insert("mp4", "QuickTime Player");
    m.insert("m4v", "QuickTime Player");
    m.insert("mov", "QuickTime Player");
    m.insert("avi", "QuickTime Player");
    m.insert("mkv", "QuickTime Player");
    m.insert("webm", "QuickTime Player");
    m.insert("wmv", "QuickTime Player");
    m.insert("flv", "QuickTime Player");
    m.insert("mp3", "音乐");
    m.insert("m4a", "音乐");
    m.insert("m4p", "音乐");
    m.insert("wav", "音乐");
    m.insert("aiff", "音乐");
    m.insert("aif", "音乐");
    m.insert("flac", "音乐");
    m.insert("aac", "音乐");
    m.insert("ogg", "音乐");
    m.insert("wma", "音乐");
    m.insert("m4r", "音乐");
    m.insert("caf", "音乐");

    // ── 文档 / 办公 ──
    m.insert("pdf", "预览");
    m.insert("doc", "Microsoft Word");
    m.insert("docx", "Microsoft Word");
    m.insert("dot", "Microsoft Word");
    m.insert("dotx", "Microsoft Word");
    m.insert("xls", "Microsoft Excel");
    m.insert("xlsx", "Microsoft Excel");
    m.insert("xlt", "Microsoft Excel");
    m.insert("xltx", "Microsoft Excel");
    m.insert("csv", "Microsoft Excel");
    m.insert("ppt", "Microsoft PowerPoint");
    m.insert("pptx", "Microsoft PowerPoint");
    m.insert("pot", "Microsoft PowerPoint");
    m.insert("potx", "Microsoft PowerPoint");
    m.insert("txt", "文本编辑");
    m.insert("md", "文本编辑");
    m.insert("rtf", "文本编辑");
    m.insert("rtfd", "文本编辑");
    m.insert("pages", "Pages");
    m.insert("numbers", "Numbers");
    m.insert("key", "Keynote");
    m.insert("kth", "Keynote");
    m.insert("odt", "LibreOffice");
    m.insert("ods", "LibreOffice");
    m.insert("odp", "LibreOffice");

    // ── 代码 ──
    m.insert("rs", "Xcode");
    m.insert("swift", "Xcode");
    m.insert("c", "Xcode");
    m.insert("cpp", "Xcode");
    m.insert("h", "Xcode");
    m.insert("hpp", "Xcode");
    m.insert("m", "Xcode");
    m.insert("mm", "Xcode");
    m.insert("playground", "Xcode");
    m.insert("xcodeproj", "Xcode");
    m.insert("xcworkspace", "Xcode");
    m.insert("storyboard", "Xcode");
    m.insert("xib", "Xcode");
    m.insert("ts", "Visual Studio Code");
    m.insert("tsx", "Visual Studio Code");
    m.insert("js", "Visual Studio Code");
    m.insert("jsx", "Visual Studio Code");
    m.insert("json", "Visual Studio Code");
    m.insert("toml", "Visual Studio Code");
    m.insert("yaml", "Visual Studio Code");
    m.insert("yml", "Visual Studio Code");
    m.insert("py", "Visual Studio Code");
    m.insert("go", "Visual Studio Code");
    m.insert("java", "Visual Studio Code");
    m.insert("html", "Safari 浏览器");
    m.insert("htm", "Safari 浏览器");
    m.insert("css", "Safari 浏览器");
    m.insert("scss", "Visual Studio Code");
    m.insert("less", "Visual Studio Code");
    m.insert("sass", "Visual Studio Code");
    m.insert("rb", "Visual Studio Code");
    m.insert("php", "Visual Studio Code");
    m.insert("sql", "Visual Studio Code");
    m.insert("sh", "终端");
    m.insert("bash", "终端");
    m.insert("zsh", "终端");
    m.insert("fish", "终端");
    m.insert("xml", "Safari 浏览器");
    m.insert("plist", "Xcode");

    // ── 压缩包 ──
    m.insert("zip", "归档实用工具");
    m.insert("rar", "归档实用工具");
    m.insert("7z", "归档实用工具");
    m.insert("tar", "归档实用工具");
    m.insert("gz", "归档实用工具");
    m.insert("bz2", "归档实用工具");
    m.insert("xz", "归档实用工具");
    m.insert("tgz", "归档实用工具");
    m.insert("tbz2", "归档实用工具");
    m.insert("dmg", "磁盘工具");

    // ── 磁盘映像 ──
    m.insert("iso", "磁盘工具");
    m.insert("sparseimage", "磁盘工具");
    m.insert("sparsebundle", "磁盘工具");

    m
});

/// Return the default macOS application name for a file, based on its extension.
/// Returns "其他" if the extension is unknown.
pub fn app_for_extension(ext: &str) -> String {
    if ext.is_empty() {
        return "其他".to_string();
    }
    let lower = ext.to_lowercase();
    EXT_APP_MAP
        .get(lower.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "其他".to_string())
}

/// Return the application name in English (for non-Chinese settings).
pub fn app_for_extension_en(ext: &str) -> String {
    if ext.is_empty() {
        return "Other".to_string();
    }
    let lower = ext.to_lowercase();

    // English names for common apps
    let en_override: HashMap<&str, &str> = [
        ("预览", "Preview"),
        ("文本编辑", "TextEdit"),
        ("字体册", "Font Book"),
        ("音乐", "Music"),
        ("终端", "Terminal"),
        ("归档实用工具", "Archive Utility"),
        ("磁盘工具", "Disk Utility"),
        ("其他", "Other"),
        ("Safari 浏览器", "Safari"),
    ]
    .iter()
    .cloned()
    .collect();

    let cn_name = EXT_APP_MAP
        .get(lower.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "Other".to_string());

    en_override
        .get(cn_name.as_str())
        .map(|s| s.to_string())
        .unwrap_or(cn_name)
}

/// Extract file extension from a filename (without the dot), lowercased.
pub fn extension_of(filename: &str) -> String {
    filename
        .rfind('.')
        .map(|i| filename[i + 1..].to_lowercase())
        .unwrap_or_default()
}

/// Group a list of file paths by their default application.
/// Returns a vec of (app_name, file_paths) sorted by file count descending.
pub fn group_by_app(paths: &[String]) -> Vec<(String, Vec<String>)> {
    let mut groups: HashMap<String, Vec<String>> = HashMap::new();
    for path in paths {
        let name = std::path::Path::new(path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");
        let ext = extension_of(name);
        let app = app_for_extension(&ext);
        groups.entry(app).or_default().push(path.clone());
    }

    // Sort groups by file count (largest first), ties by app name
    let mut sorted: Vec<(String, Vec<String>)> = groups.into_iter().collect();
    sorted.sort_by(|a, b| {
        b.1.len()
            .cmp(&a.1.len())
            .then_with(|| a.0.cmp(&b.0))
    });
    sorted
}

/// Same as group_by_app but with English app names.
pub fn group_by_app_en(paths: &[String]) -> Vec<(String, Vec<String>)> {
    let mut groups: HashMap<String, Vec<String>> = HashMap::new();
    for path in paths {
        let name = std::path::Path::new(path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");
        let ext = extension_of(name);
        let app = app_for_extension_en(&ext);
        groups.entry(app).or_default().push(path.clone());
    }

    let mut sorted: Vec<(String, Vec<String>)> = groups.into_iter().collect();
    sorted.sort_by(|a, b| {
        b.1.len()
            .cmp(&a.1.len())
            .then_with(|| a.0.cmp(&b.0))
    });
    sorted
}
