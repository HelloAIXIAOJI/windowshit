//! makecab —— 将文件打包为 CAB（复刻 Windows makecab.exe）。
//!
//! 实测对齐的行为（Windows 11 原版）：
//! - 无参数：显示帮助，退出码 0
//! - 单文件：`makecab src [dest]`，默认目标 = 源文件名末字符换 `_`
//!   （如 hello.txt → hello.tx_，实际是 CAB 格式，头 MSCF）
//! - `/F list.txt` 多文件：默认生成 `disk1\1.cab` + `setup.inf` + `setup.rpt`，
//!   输出统计块（Total files / Bytes before / Bytes after / After/Before / Time / Throughput）
//! - 进度条：`\r{:6.2}% - 文件名 (i of n)` 与 `\r{:6.2}% [flushing current folder]`
//!   用回车刷新同一行
//! - 错误：源不存在 `ERROR: Could not find file: X`（退出码 1）、
//!   无效压缩类型 `ERROR: Invalid Compression Type: X`、
//!   maxdisksize 非数字 / 非 512 倍数各有专门报错
//!
//! 已知差异（README 有述）：
//! - LZX 压缩不支持（cab crate 只能解 LZX 不能写），请求时报错
//! - 分卷（MaxDiskSize 超限）不支持
//! - 指令文件里的 `.set/.option` 指令忽略

use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;

use windowshit_args::{parse, Flag, Kind, Parsed, Unknown};

const BANNER: &str = "Cabinet Maker - Lossless Data Compression Tool";

const HELP: &str = "\
MAKECAB [/V[n]] [/D var=value ...] [/L dir] source [destination]
MAKECAB [/V[n]] [/D var=value ...] /F directive_file [...]

  source         File to compress.
  destination    File name to give compressed file.  If omitted, the
                 last character of the source file name is replaced
                 with an underscore (_) and used as the destination.
  /F directives  A file with MakeCAB directives (may be repeated). Refer to
                 Microsoft Cabinet SDK for information on directive_file.
  /D var=value   Defines variable with specified value.
  /L dir         Location to place destination (default is current directory).
  /V[n]          Verbosity level (1..3).";

// setup.inf 行宽对齐原版：每行 73 字节（`;**`/`;***` 前缀 + 内容区 + `**`）
// 前缀 `;*** BEGIN ` 11 字节 + 62 星 = 73；`;*** END ` 9 字节 + 64 星 = 73；
// `;**` 3 字节 + 68 空格 + 2 = 73。用代码生成避免手工数错。
fn inf_top() -> String {
    format!(";*** BEGIN {}", "*".repeat(58))
}
fn inf_end() -> String {
    format!(";*** END {}", "*".repeat(60))
}
fn inf_blank() -> String {
    format!(";**{}**", " ".repeat(64))
}

/// ctime 风格的日期字符串（本地时间），如 `Tue Aug 11 05:24:25 2026`。
fn ctime_now() -> String {
    let now = time::OffsetDateTime::now_local()
        .unwrap_or_else(|_| time::OffsetDateTime::now_utc());
    const WD: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
    const MO: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    format!(
        "{} {} {:2} {:02}:{:02}:{:02} {}",
        WD[now.weekday().number_days_from_sunday() as usize],
        MO[now.month() as usize - 1],
        now.day(),
        now.hour(),
        now.minute(),
        now.second(),
        now.year()
    )
}

fn join_path(dir: &str, name: &str) -> String {
    let d = if dir.is_empty() || dir == "." { "" } else { dir };
    if d.is_empty() {
        name.to_string()
    } else {
        Path::new(d).join(name).to_string_lossy().to_string()
    }
}

fn basename(p: &str) -> String {
    Path::new(p)
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| p.to_string())
}

/// 默认目标名：源文件名末字符换 `_`（保留路径）。
fn default_dest(source: &str) -> String {
    let name = basename(source);
    let mut n = name;
    if !n.is_empty() {
        n.pop();
        n.push('_');
    }
    let dir = Path::new(source)
        .parent()
        .map(|d| d.to_string_lossy().to_string())
        .unwrap_or_default();
    join_path(&dir, &n)
}

/// 模板展开：`disk*` → `disk1`，`*` 替换为卷号。
fn expand_template(tpl: &str, vol: u32) -> String {
    tpl.replace('*', &vol.to_string())
}

/// /D 指令集合。
#[derive(Default)]
struct Directives {
    compression_type: Option<String>,
    compression_memory: Option<String>,
    max_disk_size: Option<String>,
    cabinet_name: Option<String>,
    disk_dir_template: Option<String>,
}

fn parse_directives(values: &[&str]) -> Result<Directives, String> {
    let mut d = Directives::default();
    for v in values {
        let Some((key, val)) = v.split_once('=') else {
            return Err(format!("ERROR: Invalid directive: {v}"));
        };
        match key.to_ascii_lowercase().as_str() {
            "compressiontype" => d.compression_type = Some(val.to_string()),
            "compressionmemory" => d.compression_memory = Some(val.to_string()),
            "maxdisksize" => d.max_disk_size = Some(val.to_string()),
            "cabinetname1" => d.cabinet_name = Some(val.to_string()),
            "diskdirectorytemplate" => d.disk_dir_template = Some(val.to_string()),
            other => return Err(format!("ERROR: Unknown directive: {other}")),
        }
    }
    Ok(d)
}

/// 解析压缩类型，返回 cab crate 用的 CompressionType。
fn resolve_compression(d: &Directives) -> Result<cab::CompressionType, String> {
    match d.compression_type.as_deref() {
        None => Ok(cab::CompressionType::MsZip),
        Some(v) if v.eq_ignore_ascii_case("mszip") => Ok(cab::CompressionType::MsZip),
        Some(v) if v.eq_ignore_ascii_case("lzx") => {
            Err("ERROR: LZX compression is not supported by this implementation".to_string())
        }
        Some(v) => Err(format!("ERROR: Invalid Compression Type: {v}")),
    }
}

/// 校验 MaxDiskSize（需为数字且是 512 的倍数）。
fn check_max_disk_size(d: &Directives) -> Result<u64, String> {
    let Some(v) = &d.max_disk_size else {
        return Ok(u64::MAX);
    };
    let n: u64 = match v.parse() {
        Ok(n) => n,
        Err(_) => {
            return Err(format!(
                "ERROR: Value of variable 'MaxDiskSize' must be a number: {v}"
            ));
        }
    };
    if n % 512 != 0 {
        return Err(format!(
            "ERROR: MaxDiskSize({v}) is not a multiple of ClusterSize(512)"
        ));
    }
    Ok(n)
}

fn join_l(d: &Directives) -> String {
    let tpl = d
        .disk_dir_template
        .clone()
        .unwrap_or_else(|| "disk*".to_string());
    let dir = expand_template(&tpl, 1);
    if dir == "." {
        String::new()
    } else {
        dir
    }
}

fn cab_name(d: &Directives) -> String {
    d.cabinet_name
        .clone()
        .unwrap_or_else(|| "1.cab".to_string())
}

fn inf_mid(content: &str) -> String {
    format!(";**{content:<64}**")
}

/// 原版 `;** MakeCAB Version: 5.00` 行（60 字节，非 64 填充）。
fn inf_version() -> String {
    format!(";** MakeCAB Version: 5.00{}**", " ".repeat(33))
}

/// 生成 setup.inf。
fn write_setup_inf(files: &[String], sizes: &[u64], cab_name: &str) -> Result<(), String> {
    let date = ctime_now();
    let top = inf_top();
    let end = inf_end();
    let blank = inf_blank();
    let mut s = String::new();
    s.push_str(&top);
    s.push_str("\r\n");
    s.push_str(&blank);
    s.push_str("\r\n");
    s.push_str(&inf_mid(&format!(
        " Automatically generated on: {date}"
    )));
    s.push_str("\r\n");
    s.push_str(&blank);
    s.push_str("\r\n");
    s.push_str(&inf_version());
    s.push_str("\r\n");
    s.push_str(&blank);
    s.push_str("\r\n");
    s.push_str(&top);
    s.push_str("\r\n");
    s.push_str("[disk list]\r\n1,Disk 1\r\n[cabinet list]\r\n1,1,");
    s.push_str(cab_name);
    s.push_str("\r\n[file list]\r\n");
    for (i, f) in files.iter().enumerate() {
        s.push_str(&format!("1,1,{},{}\r\n", basename(f), sizes[i]));
    }
    s.push_str(&end);
    s.push_str("\r\n");
    s.push_str(&blank);
    s.push_str("\r\n");
    s.push_str(&inf_mid(&format!(" Automatically generated on: {date}")));
    s.push_str("\r\n");
    s.push_str(&blank);
    s.push_str("\r\n");
    s.push_str(&end);
    s.push_str("\r\n");
    fs::write("setup.inf", s).map_err(|e| e.to_string())
}

/// 进度输出：百分比格式 `{:6.2}`（如 `  0.00`、`100.00`）。
fn progress(pct: f64, suffix: &str) {
    print!("\r{pct:6.2}% {suffix}");
    let _ = io::stdout().flush();
}

/// 打包一组文件为一个 CAB，输出原版风格的进度条。
/// 先写入临时文件（cab crate 的 writer 要求 `Write + Seek`），
/// 再读回内存用于 flush 进度展示与统计（Bytes after）。
fn build_cab(
    files: &[String],
    dest: &str,
    compression: cab::CompressionType,
) -> Result<Vec<u8>, String> {
    let mut builder = cab::CabinetBuilder::new();
    let folder = builder.add_folder(compression);
    for f in files {
        folder.add_file(&basename(f));
    }
    let tmp = format!("{dest}.~tmp");
    {
        let mut tmpf = fs::File::create(&tmp).map_err(|e| e.to_string())?;
        {
            let mut writer = builder.build(&mut tmpf).map_err(|e| e.to_string())?;
            let n = files.len();
            let mut idx = 0usize;
            while let Some(mut fw) = writer.next_file().map_err(|e| e.to_string())? {
                idx += 1;
                progress(
                    0.0,
                    &format!("- {} ({} of {})", basename(&files[idx - 1]), idx, n),
                );
                let mut reader = fs::File::open(&files[idx - 1]).map_err(|e| e.to_string())?;
                io::copy(&mut reader, &mut fw).map_err(|e| e.to_string())?;
                progress(
                    100.0,
                    &format!("- {} ({} of {})", basename(&files[idx - 1]), idx, n),
                );
            }
            writer.finish().map_err(|e| e.to_string())?;
        }
    }
    let buf = fs::read(&tmp).map_err(|e| e.to_string())?;
    let _ = fs::remove_file(&tmp);

    // flush 进度：按写出字节分段的真实进度
    progress(0.0, "[flushing current folder]");
    let total = buf.len();
    let third = (total / 3).max(1);
    let mut f = fs::File::create(dest).map_err(|e| e.to_string())?;
    let mut written = 0usize;
    while written < total {
        let chunk = third.min(total - written);
        f.write_all(&buf[written..written + chunk])
            .map_err(|e| e.to_string())?;
        written += chunk;
        progress(
            written as f64 / total as f64 * 100.0,
            "[flushing current folder]",
        );
    }
    println!();
    Ok(buf)
}

/// 统计块文本（终端输出与 setup.rpt 共用，与原版一致）。
fn stats_text(total_files: usize, before: u64, after: u64, elapsed_secs: f64) -> String {
    let ratio = if before > 0 {
        after as f64 / before as f64 * 100.0
    } else {
        0.0
    };
    let hr = (elapsed_secs / 3600.0) as u64;
    let min = ((elapsed_secs % 3600.0) / 60.0) as u64;
    let sec = elapsed_secs % 60.0;
    let tp = if elapsed_secs > 0.0 {
        before as f64 / 1024.0 / elapsed_secs
    } else {
        0.0
    };
    // 空格数逐字节对齐原版：14/12/13/12/21/15
    format!(
        "Total files:              {total_files}\r\n\
Bytes before:            {before}\r\n\
Bytes after:             {after}\r\n\
After/Before:            {ratio:.2}% compression\r\n\
Time:                     {elapsed_secs:.2} seconds ( {hr} hr  {min} min  {sec:.2} sec)\r\n\
Throughput:               {tp:.2} Kb/second"
    )
}

/// 单文件模式打包。
fn do_single(
    source: &str,
    dest: Option<&str>,
    l_dir: Option<&str>,
    d: &Directives,
) -> Result<Vec<u8>, String> {
    if !Path::new(source).exists() {
        return Err(format!("ERROR: Could not find file: {source}"));
    }
    let compression = resolve_compression(d)?;
    check_max_disk_size(d)?;
    let out = match (dest, l_dir) {
        (Some(dest), Some(dir)) => join_path(dir, dest),
        (Some(dest), None) => dest.to_string(),
        (None, Some(dir)) => join_path(dir, &default_dest(source)),
        (None, None) => default_dest(source),
    };
    let files = vec![source.to_string()];
    build_cab(&files, &out, compression)
}

/// /F 多文件模式打包。
fn do_multi(list_file: &str, d: &Directives) -> Result<Vec<u8>, String> {
    let start = std::time::Instant::now();
    let compression = resolve_compression(d)?;
    let max_size = check_max_disk_size(d)?;

    // 读指令文件：每行一个文件（支持通配符）
    let content = fs::read_to_string(list_file)
        .map_err(|_| format!("ERROR: Could not find file: {list_file}"))?;
    let mut raw: Vec<String> = Vec::new();
    let mut line_count = 0usize;
    for line in content.lines() {
        let t = line.trim();
        if t.is_empty() || t.starts_with(';') {
            continue;
        }
        if t.starts_with('.') {
            continue; // .set/.option 指令忽略
        }
        line_count += 1;
        raw.push(t.to_string());
    }
    println!("Parsing directives");
    println!("Parsing directives ({list_file}: {line_count} lines)");

    // 展开通配符
    let mut files = Vec::new();
    let mut total_before = 0u64;
    for p in &raw {
        let expanded = glob_files(p);
        if expanded.is_empty() {
            return Err(format!("ERROR: Could not find file: {p}"));
        }
        for f in expanded {
            match fs::metadata(&f) {
                Ok(m) if m.is_file() => {
                    files.push(f);
                    total_before += m.len();
                }
                _ => return Err(format!("ERROR: Could not find file: {f}")),
            }
        }
    }
    println!("{total_before} bytes in {} files", files.len());
    println!("Executing directives");

    if total_before > max_size {
        return Err(
            "ERROR: Cabinet would exceed MaxDiskSize; multi-volume cabinets are not supported"
                .to_string(),
        );
    }

    let dir = join_l(d);
    let name = cab_name(d);
    let cab_path = join_path(&dir, &name);
    if !dir.is_empty() {
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    }

    let buf = build_cab(&files, &cab_path, compression)?;

    // setup.inf / setup.rpt
    let sizes: Vec<u64> = files
        .iter()
        .map(|f| fs::metadata(f).map(|m| m.len()).unwrap_or(0))
        .collect();
    write_setup_inf(&files, &sizes, &name)?;
    let elapsed = start.elapsed().as_secs_f64();
    let stats = stats_text(files.len(), total_before, buf.len() as u64, elapsed);
    // 原版 setup.rpt = MakeCAB Report 头 + 空行 + 统计块
    let rpt = format!("MakeCAB Report: {}\r\n\r\n{stats}\r\n", ctime_now());
    fs::write("setup.rpt", rpt).map_err(|e| e.to_string())?;

    println!("{stats}");
    Ok(buf)
}

/// 通配符展开（Windows 风格 `*` `?`，大小写不敏感）。
fn glob_files(pattern: &str) -> Vec<String> {
    if !pattern.contains('*') && !pattern.contains('?') {
        if Path::new(pattern).exists() {
            return vec![pattern.to_string()];
        }
        return Vec::new();
    }
    let (dir, file_pat) = split_pattern(pattern);
    let mut out = Vec::new();
    if let Ok(entries) = fs::read_dir(&dir) {
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if glob_match(&name, &file_pat) {
                out.push(join_path(&dir, &name));
            }
        }
    }
    out.sort();
    out
}

fn split_pattern(p: &str) -> (String, String) {
    let path = Path::new(p);
    match (path.parent(), path.file_name()) {
        (Some(d), Some(f)) if !f.is_empty() => {
            let dir = d.to_string_lossy().to_string();
            (if dir.is_empty() { ".".to_string() } else { dir }, f.to_string_lossy().to_string())
        }
        _ => (String::from("."), p.to_string()),
    }
}

fn glob_match(name: &str, pat: &str) -> bool {
    fn m(n: &[u8], p: &[u8]) -> bool {
        if p.is_empty() {
            return n.is_empty();
        }
        match p[0] {
            b'*' => (0..=n.len()).any(|i| m(&n[i..], &p[1..])),
            b'?' => !n.is_empty() && m(&n[1..], &p[1..]),
            c => !n.is_empty() && n[0].eq_ignore_ascii_case(&c) && m(&n[1..], &p[1..]),
        }
    }
    m(name.as_bytes(), pat.as_bytes())
}

fn main() -> ExitCode {
    let raw: Vec<String> = env::args().skip(1).collect();

    println!("{BANNER}");
    println!();

    if raw.is_empty() {
        println!("{HELP}");
        return ExitCode::SUCCESS;
    }
    if raw.iter().any(|a| a == "/?" || a == "-?") {
        println!("{HELP}");
        return ExitCode::SUCCESS;
    }

    // 预提取 /V[n]（n 为数字）与全部 /D var=value / /D:var=value
    let mut verbosity = 1u32;
    let mut d_values: Vec<String> = Vec::new();
    let mut clean: Vec<String> = Vec::new();
    let mut i = 0usize;
    while i < raw.len() {
        let a = &raw[i];
        let up = a.to_ascii_uppercase();
        let prefixed = up.strip_prefix('/').or_else(|| up.strip_prefix('-'));
        if let Some(body) = prefixed {
            if let Some(v) = body.strip_prefix("V") {
                if !v.is_empty() && v.chars().all(|c| c.is_ascii_digit()) {
                    verbosity = v.parse().unwrap_or(3);
                    i += 1;
                    continue;
                }
            }
            if body == "D" {
                // 空格形式：/D var=value，消费下一参数
                if i + 1 >= raw.len() {
                    println!("ERROR: Missing value for /D");
                    return ExitCode::from(1);
                }
                d_values.push(raw[i + 1].clone());
                i += 2;
                continue;
            }
            if let Some(v) = body.strip_prefix("D:") {
                d_values.push(v.to_string());
                i += 1;
                continue;
            }
        }
        clean.push(a.clone());
        i += 1;
    }
    let _ = verbosity;

    // 解析 /L /F
    const FLAGS: &[Flag] = &[
        Flag::new("L", Kind::Value),
        Flag::new("F", Kind::Value),
    ];
    let parsed: Parsed = match parse(&clean, FLAGS, Unknown::Path) {
        Ok(p) => p,
        Err(_) => Parsed::default(),
    };

    let directives = match parse_directives(&d_values.iter().map(|s| s.as_str()).collect::<Vec<_>>()) {
        Ok(d) => d,
        Err(e) => {
            println!("{e}");
            return ExitCode::from(1);
        }
    };
    let l_dir = parsed.flags.get("L").and_then(|v| *v);

    if let Some(list_file) = parsed.flags.get("F").and_then(|v| *v) {
        match do_multi(list_file, &directives) {
            Ok(_) => ExitCode::SUCCESS,
            Err(e) => {
                println!("{e}");
                ExitCode::from(1)
            }
        }
    } else {
        let paths: Vec<&str> = parsed.paths;
        if paths.is_empty() {
            println!("{HELP}");
            return ExitCode::SUCCESS;
        }
        let source = paths[0];
        let dest = paths.get(1).copied();
        match do_single(source, dest, l_dir, &directives) {
            Ok(_) => ExitCode::SUCCESS,
            Err(e) => {
                println!("{e}");
                ExitCode::from(1)
            }
        }
    }
}
