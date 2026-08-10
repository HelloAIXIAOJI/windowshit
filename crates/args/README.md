# windowshit-args

Windowshit 项目公共命令行参数解析层（库 crate）。当前被 `sort`、`more`、`tree`、`where`、`findstr`、`tasklist`、`getmac` 使用。

## 核心规则

**只把精确命中已知开关表的参数当开关，其余一律按策略处理（路径 / 报错 / 忽略）。**

原因：Linux 绝对路径（`/tmp/x.txt`）以 `/` 开头，与 Windows 的 `/R`、`/O` 这类开关前缀天然冲突。开关集合是有限的、已知的，路径是无限的，因此"宁可误伤开关，不可误吞路径"。

## 开关形式

- 前缀：`/` 或 `-`，大小写不敏感
- 无值开关：`/R`
- 取值开关：`/O file` 或 `/O:file`（空格与冒号两种写法都支持）
- 已知但忽略：`/M`（原版存在但本实现不处理）

## 用法

```rust
use windowshit_args::{parse, Flag, Kind, Parsed, Unknown};

const FLAGS: &[Flag] = &[
    Flag::new("R", Kind::Flag),
    Flag::new("O", Kind::Value),
    Flag::new("M", Kind::Ignore),
];

let parsed = match parse(&raw, FLAGS, Unknown::Path) {
    Ok(p) => p,
    Err(_) => Parsed::default(),
};

if parsed.flags.contains_key("R") {
    // /R 出现
}
if let Some(v) = parsed.flags.get("O").and_then(|v| *v) {
    // /O file 或 /O:file 的值
}
for p in &parsed.paths {
    // 普通参数（路径）
}
```

`Unknown` 策略：

| 策略 | 含义 |
| --- | --- |
| `Path` | 未知开关当作普通参数（路径）收集到 `paths` |
| `Error` | 未知开关返回 `Error::Unknown`（如 `where` 还原原版 Invalid switch） |
| `Ignore` | 静默丢弃（如 `tasklist` / `getmac`） |

## 特殊形态

sort 的 `/+n`、more 的 `/Tn` 与 `+n` 这类"开关后连写数字"的形态不属于本库范围，各组件在调用 `parse` 前自行预提取。

## 关键经验

- **不要用前缀匹配判断开关**：`/tmp/x.txt` 会被 `/T` 前缀匹配误判为 tab 宽度，`/FOO` 会被 `/F` 误判——历史上的两个 bug 根源。
- 取值开关消费下一个参数时无条件（`/O /R` 会把 `/R` 当值），与原版行为一致。
