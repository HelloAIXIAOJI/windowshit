/// 输出系统版本信息。
/// Windows 上还原原版 ver 的 `Microsoft Windows [Version x]` 格式
/// （原版任何语言版本都是英文文本，故不随代码页翻译）。
/// 跨平台用 `os_info` crate，无平台分支。

use os_info::Type;
use windowshit_i18n::L10n;

fn main() {
    L10n::setup_console_utf8();

    let info = os_info::get();
    let os = match info.os_type() {
        Type::Windows => "Microsoft Windows".to_string(),
        Type::Macos => "Apple macOS".to_string(),
        Type::Linux => "GNU Linux".to_string(),
        other => other.to_string(),
    };

    // UBR（Update Build Revision）补齐 os_info 缺失的 build 号，
    // 如 10.0.22621.**2428**。仅 Windows 存在此字段，非 Windows 返回 None。
    let mut version = info.version().to_string();
    if let Some(ubr) =
        windowshit_winreg::reg_query_dword("SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion", "UBR")
    {
        if !version.is_empty() {
            version.push('.');
            version.push_str(&ubr.to_string());
        }
    }

    println!("{os} [Version {version}]");
}
