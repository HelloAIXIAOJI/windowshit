//! Windowshit 公共国际化层。
//!
//! 语言检测规则（还原 Windows 原版命令的行为）：
//! Windows 上由"控制台输出代码页"决定（chcp 936 → 中文，其它 → 英文），
//! 必须在改代码页之前调用 `detect()`。macOS/Linux 回退到系统 locale。
//!
//! 各组件通过 [`L10n::add_ftl`] 注入自己的翻译文件，通过 [`L10n::set_help`]
//! 注入自己的帮助文本，避免重复实现语言检测与翻译机制。

pub use fluent::FluentArgs;
use fluent::{FluentBundle, FluentResource};
use unic_langid::LanguageIdentifier;

pub struct L10n {
    bundle: FluentBundle<FluentResource>,
    lang: String,
    zh_help: Option<&'static str>,
    en_help: Option<&'static str>,
}

impl L10n {
    /// 根据系统环境自动选择界面语言。
    ///
    /// 必须在 Windows 上修改控制台代码页**之前**调用。
    pub fn detect() -> Self {
        if let Ok(lang) = std::env::var("WINDOWSHIT_LANG") {
            if !lang.is_empty() {
                return Self::for_locale(&lang);
            }
        }
        #[cfg(windows)]
        {
            // SAFETY: 标准 API，无指针参数
            let cp = unsafe { windows_sys::Win32::System::Console::GetConsoleOutputCP() };
            let lang = match cp {
                936 | 950 => "zh-CN",
                _ => "en-US",
            };
            return Self::for_lang(lang);
        }
        #[cfg(not(windows))]
        {
            let locale = sys_locale::get_locale().unwrap_or_else(|| "en-US".to_string());
            Self::for_locale(&locale)
        }
    }

    /// 根据 BCP-47 locale 字符串（如 "zh-CN"、"en-US"）选择语言。
    pub fn for_locale(locale: &str) -> Self {
        let lower = locale.to_lowercase();
        let lang = if lower.starts_with("zh") {
            "zh-CN"
        } else {
            "en-US"
        };
        Self::for_lang(lang)
    }

    /// 直接指定语言标识符（用于测试或强制指定）。
    pub fn for_lang(lang: &str) -> Self {
        let lid: LanguageIdentifier = lang.parse().expect("invalid langid");
        let mut bundle = FluentBundle::new(vec![lid]);
        // 关闭双向文本隔离（U+2068/U+2069），Windows 原版输出不含这些字符
        bundle.set_use_isolating(false);
        L10n {
            bundle,
            lang: lang.to_string(),
            zh_help: None,
            en_help: None,
        }
    }

    /// 注入组件自己的 FTL 翻译文本。可在构建后追加多个资源。
    pub fn add_ftl(&mut self, ftl: &'static str) {
        let res = FluentResource::try_new(ftl.to_owned()).expect("invalid FTL resource");
        self.bundle
            .add_resource(res)
            .expect("failed to add FTL resource");
    }

    /// 注入组件自己的帮助文本（中文/英文）。
    pub fn set_help(&mut self, zh: &'static str, en: &'static str) {
        self.zh_help = Some(zh);
        self.en_help = Some(en);
    }

    /// 取翻译后的动态消息。args 为 None 表示无参数。
    pub fn tr(&self, key: &str, args: Option<&FluentArgs>) -> String {
        let msg = match self.bundle.get_message(key) {
            Some(m) => m,
            None => return format!("[missing message: {key}]"),
        };
        let pattern = match msg.value() {
            Some(p) => p,
            None => return format!("[missing value: {key}]"),
        };
        let mut errors = vec![];
        self.bundle
            .format_pattern(pattern, args, &mut errors)
            .into_owned()
    }

    /// 当前语言标识符（如 "zh-CN"）。
    pub fn lang(&self) -> &str {
        &self.lang
    }

    /// 完整帮助文本（含用法）。
    pub fn help(&self) -> &'static str {
        match self.lang.as_str() {
            "zh-CN" => self.zh_help.unwrap_or("[no help]"),
            _ => self.en_help.unwrap_or("[no help]"),
        }
    }

    /// Windows 上把控制台输出代码页设为 UTF-8，避免中文乱码。
    ///
    /// 必须在 `detect()` 之后调用（语言检测读的是改之前的代码页）。
    /// 非 Windows 平台为空操作，无需调用方写平台分支。
    pub fn setup_console_utf8() {
        #[cfg(windows)]
        // SAFETY: 只调用标准 Win32 API，无其他副作用
        unsafe {
            windows_sys::Win32::System::Console::SetConsoleOutputCP(65001);
        }
        #[cfg(not(windows))]
        {}
    }
}
