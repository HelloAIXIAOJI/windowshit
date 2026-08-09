# windowshit-i18n

Windowshit 项目公共国际化层（库 crate），供各组件复用语言检测与翻译机制。

## 功能

- **语言检测**：
  - Windows：跟随**控制台输出代码页**（`chcp 936`/`950` → 中文，其它 → 英文）——这是实测得出的原版命令行为
  - Linux/macOS：跟随系统 locale（`LANG` 环境变量）
  - 可用环境变量 `WINDOWSHIT_LANG` 强制指定（测试用）
- **翻译管理**：基于 [fluent](https://crates.io/crates/fluent)（FTL 格式），关闭了 bidi 隔离字符（U+2068/U+2069），与 Windows 原版输出一致

## 用法

```rust
// 1. 检测语言（必须在改控制台代码页之前调用）
let mut i18n = L10n::detect();

// 2. 注入组件自己的翻译文件
match i18n.lang() {
    "zh-CN" => i18n.add_ftl(include_str!("../locales/zh-CN.ftl")),
    _ => i18n.add_ftl(include_str!("../locales/en-US.ftl")),
}

// 3. 注入组件自己的帮助文本
i18n.set_help(
    include_str!("../locales/help.zh.txt"),
    include_str!("../locales/help.en.txt"),
);

// 4. 取翻译（带参数 / 不带参数）
let msg = i18n.tr("reply-v4", Some(&args));
let msg = i18n.tr("timeout", None);
```

## 组件 FTL 文件结构

每个组件在自己的 `locales/` 目录维护：

```
locales/
├── zh-CN.ftl       # 简体中文消息
├── en-US.ftl       # 英文消息
├── help.zh.txt     # 中文帮助文本（静态大文本）
└── help.en.txt     # 英文帮助文本
```

## 依赖

- `fluent`：翻译消息格式化
- `sys-locale`：Linux/macOS 系统 locale
- `unic-langid`：语言标识符
- `windows-sys`：Windows 控制台代码页查询
