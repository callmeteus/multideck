//! Icon path data used when drawing keys.
//!
//! Every icon uses a 24x24 view box and a single filled path, so it can be
//! embedded in the generated SVG without external files.

/// Returns the SVG path data for an icon name.
pub fn icon_path(name: &str) -> Option<&'static str> {
    let path = match name {
        "hotkey" => "M13 2 4 14h6l-1 8 9-12h-6l1-8z",
        "open" => {
            "M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20zm6.9 6h-2.9a15 15 0 0 0-1.4-3.6A8 8 0 0 1 18.9 8zM12 4c.8 1.2 1.4 2.5 1.8 4h-3.6A13 13 0 0 1 12 4zM4.3 14a8 8 0 0 1 0-4h3.3a17 17 0 0 0 0 4H4.3zm.8 2h2.9c.3 1.3.8 2.5 1.4 3.6A8 8 0 0 1 5.1 16zm2.9-8H5.1a8 8 0 0 1 4.3-3.6C8.8 5.5 8.3 6.7 8 8zM12 20c-.8-1.2-1.4-2.5-1.8-4h3.6A13 13 0 0 1 12 20zm2.2-6H9.8a15 15 0 0 1 0-4h4.4a15 15 0 0 1 0 4zm.4 5.6c.6-1.1 1.1-2.3 1.4-3.6h2.9a8 8 0 0 1-4.3 3.6zm1.8-5.6a17 17 0 0 0 0-4h3.3a8 8 0 0 1 0 4h-3.3z"
        }
        "text" => "M4 5h16v3h-2V7h-5v10h2v2H9v-2h2V7H6v1H4V5z",
        "media" => "M8 5v14l11-7z",
        "command" => {
            "M4 5h16v14H4V5zm2 2v10h12V7H6zm2.3 1.8L11.5 12l-3.2 3.2 1.4 1.4L14.3 12l-4.6-4.6-1.4 1.4z"
        }
        "delay" => "M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20zm1 5h-2v5.4l4.7 2.8.9-1.6-3.6-2.1V7z",
        "multi" => {
            "M12 2 2 7l10 5 10-5-10-5zm0 7.5L4.6 6 12 2.5 19.4 6 12 9.5zM2 12l10 5 10-5-1.7-.9L12 14.7 3.7 11.1 2 12zm0 5 10 5 10-5-1.7-.9L12 19.7 3.7 16.1 2 17z"
        }
        "folder" => "M4 4h7l2 2h7v14H4V4zm2 4v10h12V8H6z",
        "profile" => {
            "M12 4a4 4 0 1 1 0 8 4 4 0 0 1 0-8zm0 10c4.4 0 8 2 8 4.5V20H4v-1.5C4 16 7.6 14 12 14z"
        }
        "brightness" => {
            "M12 6a6 6 0 1 1 0 12 6 6 0 0 1 0-12zm0-6h1v3h-2V0h1zm0 21h1v3h-2v-3h1zM0 11h3v2H0v-2zm21 0h3v2h-3v-2zM3.5 2.1l2.1 2.1-1.4 1.4L2.1 3.5l1.4-1.4zm14.9 14.9 2.1 2.1-1.4 1.4-2.1-2.1 1.4-1.4zM18.5 2.1l1.4 1.4-2.1 2.1-1.4-1.4 2.1-2.1zM3.5 18.5l1.4 1.4-2.1 2.1-1.4-1.4 2.1-2.1z"
        }
        "plus" => "M11 5h2v6h6v2h-6v6h-2v-6H5v-2h6V5z",
        "grid" => "M4 4h7v7H4V4zm9 0h7v7h-7V4zM4 13h7v7H4v-7zm9 0h7v7h-7v-7z",
        _ => return None,
    };
    Some(path)
}
