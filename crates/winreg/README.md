# windowshit-winreg

Windowshit 公共注册表读取层（库 crate）。

## 职责

统一封装 Windows 注册表样板代码（UTF-16 编码、RegOpenKeyExW、RegQueryValueExW、RegEnumKeyExW），供多个组件复用。

非 Windows 平台，把注册表映射到 Linux 配置文件夹

| 根键 | 映射目录 | 说明 |
|---|---|---|
| `HKEY_LOCAL_MACHINE` | `/etc` | 系统级配置 |
| `HKEY_CURRENT_USER` | `~/.config` | 用户级配置 |
| `HKEY_SYSTEM_BOOT` | `/boot` | 引导目录 |

- 子键路径 → 对应目录（目录不存在时返回空/`None`）
- 值 → 目录下配置文件里的一行 `key = value`（支持 `=`、`:`、空白分隔，键名大小写不敏感，自动去注释与包裹引号）
- `reg_query_dword` 额外支持 `0x` 十六进制解析
- 无根键前缀时默认基于 `HKLM`（`/etc`）

## API

| 函数 | 说明 |
|---|---|
| `reg_query_string(key_path, name) -> Option<String>` | 读取 REG_SZ（UTF-16LE 自动转 String） |
| `reg_query_dword(key_path, name) -> Option<u32>` | 读取 REG_DWORD |
| `reg_enum_child_names(key_path) -> Vec<String>` | 枚举直接子键名 |

所有键路径基于 `HKLM`（HKEY_LOCAL_MACHINE）。

## 使用方

- `ver`：读 UBR（REG_DWORD）补齐 build 号
- `systeminfo`：读 ProductName、RegisteredOwner、BIOS 字段等（REG_SZ）
- `getmac`：枚举网络适配器 GUID 子键 + 读 Connection\Name 反查 transport name
