//! 国际化：系统语言检测 + fluent 翻译。

use fluent::{FluentArgs, FluentBundle, FluentResource};
use unic_langid::LanguageIdentifier;

pub struct L10n {
    bundle: FluentBundle<FluentResource>,
    lang: String,
}

impl L10n {
    /// 根据当前系统语言自动选择界面语言。
    /// 可通过环境变量 WINDOWSHIT_LANG 强制指定（测试/彩蛋用途）。
    pub fn detect() -> Self {
        if let Ok(lang) = std::env::var("WINDOWSHIT_LANG") {
            if !lang.is_empty() {
                return Self::for_locale(&lang);
            }
        }
        let locale = sys_locale::get_locale().unwrap_or_else(|| "en-US".to_string());
        Self::for_locale(&locale)
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
        let ftl: &str = match lang {
            "zh-CN" => include_str!("../locales/zh-CN.ftl"),
            _ => include_str!("../locales/en-US.ftl"),
        };
        let res = FluentResource::try_new(ftl.to_owned()).expect("invalid FTL resource");
        let lid: LanguageIdentifier = lang.parse().expect("invalid langid");
        let mut bundle = FluentBundle::new(vec![lid]);
        // 关闭双向文本隔离（U+2068/U+2069），Windows 原版输出不含这些字符
        bundle.set_use_isolating(false);
        bundle.add_resource(res).expect("failed to add FTL resource");
        L10n {
            bundle,
            lang: lang.to_string(),
        }
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

    /// 完整帮助文本（含用法）。
    pub fn help(&self) -> &'static str {
        match self.lang.as_str() {
            "zh-CN" => include_str!("../locales/help.zh.txt"),
            _ => include_str!("../locales/help.en.txt"),
        }
    }

    /// 单行用法（出错时提示）。
    pub fn usage(&self) -> String {
        // 从帮助文本中截取第一段（用法行块）
        self.help()
            .lines()
            .take(4)
            .collect::<Vec<_>>()
            .join("\n")
    }
}
